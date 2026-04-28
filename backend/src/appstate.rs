use crate::configuration::{Config, HostType};
#[cfg(target_os = "linux")]
use crate::jail::Jail;
use axum::{body::Body, extract::FromRef};
use axum_extra::extract::cookie::Key;
use http::Request;
use hyper::Response;
use hyper::body::Incoming;
use hyper_hickory::{HickoryResolver, TokioHickoryResolver};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use hyper_util::{client::legacy::connect::HttpConnector, rt::TokioExecutor};
use maxminddb::Reader;
use rustls::ClientConfig;
use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
};

pub type OptionalMaxMindReader = Option<&'static Reader<Vec<u8>>>;
pub type ConfigMap = Arc<HashMap<String, HostType>>;
pub type ConfigFile = Arc<String>;
pub type ConfigState = Arc<Config>;
pub struct Client(
    pub hyper_util::client::legacy::Client<HttpsConnector<HttpConnector<TokioHickoryResolver>>, Body>,
);
pub struct InsecureSkipVerifyClient(
    pub hyper_util::client::legacy::Client<HttpsConnector<HttpConnector<TokioHickoryResolver>>, Body>,
);

impl Clone for Client {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl Clone for InsecureSkipVerifyClient {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

pub static MAXMIND_READER: OnceLock<Reader<Vec<u8>>> = OnceLock::new();

#[derive(Clone)]
pub struct AppState {
    key: Key,
    config: ConfigState,
    config_map: ConfigMap,
    config_file: ConfigFile,
    client: Client,
    insecure_skip_verify_client: InsecureSkipVerifyClient,
    #[cfg(target_os = "linux")]
    pub jail: Option<Arc<Jail>>,
}

impl AppState {
    pub(crate) fn new(
        key: Key,
        config: ConfigState,
        config_map: ConfigMap,
        config_file: String,
        #[cfg(target_os = "linux")] jail: Option<Arc<Jail>>,
    ) -> Self {
        if let Ok(r) = maxminddb::Reader::open_readfile("GeoLite2-City.mmdb") {
            MAXMIND_READER.get_or_init(|| r);
        }

        // Create a secure HTTPS Client that use Hickory as DNS resolver, and get the configuration from system conf
        let mut dns_resolver = HickoryResolver::from_system_conf()
            .expect("could not create DNS resolver from system configuration")
            .into_http_connector();
        dns_resolver.enforce_http(false);

        let rustls_connector = HttpsConnectorBuilder::new()
            .with_webpki_roots()
            .https_or_http()
            .enable_http1()
            .wrap_connector(dns_resolver.clone());

        let client: hyper_util::client::legacy::Client<_, Body> =
            hyper_util::client::legacy::Client::builder(TokioExecutor::new())
                .http1_title_case_headers(true)
                .build(rustls_connector);

        let unsecure_connector = HttpsConnectorBuilder::new()
            .with_tls_config(get_rustls_config_dangerous())
            .https_or_http()
            .enable_http1()
            .wrap_connector(dns_resolver);

        let unsecure_client = hyper_util::client::legacy::Client::builder(TokioExecutor::new())
            .http1_title_case_headers(true)
            .build::<_, Body>(unsecure_connector);

        AppState {
            key,
            config,
            config_map,
            config_file: Arc::new(config_file),
            client: Client(client),
            insecure_skip_verify_client: InsecureSkipVerifyClient(unsecure_client),
            #[cfg(target_os = "linux")]
            jail,
        }
    }
}

impl FromRef<AppState> for Key {
    fn from_ref(state: &AppState) -> Self {
        state.key.clone()
    }
}

impl FromRef<AppState> for ConfigState {
    fn from_ref(state: &AppState) -> Self {
        Arc::clone(&state.config)
    }
}

impl FromRef<AppState> for ConfigMap {
    fn from_ref(state: &AppState) -> Self {
        Arc::clone(&state.config_map)
    }
}

impl FromRef<AppState> for ConfigFile {
    fn from_ref(state: &AppState) -> Self {
        Arc::clone(&state.config_file)
    }
}

impl FromRef<AppState> for Client {
    fn from_ref(state: &AppState) -> Self {
        state.client.clone()
    }
}

impl FromRef<AppState> for InsecureSkipVerifyClient {
    fn from_ref(state: &AppState) -> Self {
        state.insecure_skip_verify_client.clone()
    }
}

impl tower_service::Service<Request<Body>> for Client {
    type Response = Response<Incoming>;
    type Error = hyper_util::client::legacy::Error;
    type Future = hyper_util::client::legacy::ResponseFuture;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.0.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        self.0.call(req)
    }
}

impl tower_service::Service<Request<Body>> for InsecureSkipVerifyClient {
    type Response = Response<Incoming>;
    type Error = hyper_util::client::legacy::Error;
    type Future = hyper_util::client::legacy::ResponseFuture;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.0.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        self.0.call(req)
    }
}

impl FromRef<AppState> for crate::OptionalJail {
    fn from_ref(state: &AppState) -> Self {
        #[cfg(target_os = "linux")]
        {
            state.jail.clone()
        }
        #[cfg(not(target_os = "linux"))]
        {
            ()
        }
    }
}

pub fn get_rustls_config_dangerous() -> ClientConfig {
    let empty_store = rustls::RootCertStore::empty();

    let mut config = ClientConfig::builder()
        .with_root_certificates(empty_store)
        .with_no_client_auth();

    let mut dangerous_config = ClientConfig::dangerous(&mut config);
    dangerous_config.set_certificate_verifier(Arc::new(
        insecure_certificate_verifier::NoCertificateVerification {},
    ));

    config
}

mod insecure_certificate_verifier {

    use rustls::{
        DigitallySignedStruct,
        client::danger::HandshakeSignatureValid,
        crypto::{verify_tls12_signature, verify_tls13_signature},
    };
    use rustls_pki_types::{CertificateDer, ServerName, UnixTime};

    #[derive(Debug)]
    pub struct NoCertificateVerification {}

    impl rustls::client::danger::ServerCertVerifier for NoCertificateVerification {
        fn verify_server_cert(
            &self,
            _end_entity: &CertificateDer<'_>,
            _intermediates: &[CertificateDer<'_>],
            _server_name: &ServerName<'_>,
            _ocsp: &[u8],
            _now: UnixTime,
        ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
            Ok(rustls::client::danger::ServerCertVerified::assertion())
        }

        fn verify_tls12_signature(
            &self,
            message: &[u8],
            cert: &CertificateDer<'_>,
            dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, rustls::Error> {
            verify_tls12_signature(
                message,
                cert,
                dss,
                &rustls::crypto::aws_lc_rs::default_provider().signature_verification_algorithms,
            )
        }

        fn verify_tls13_signature(
            &self,
            message: &[u8],
            cert: &CertificateDer<'_>,
            dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, rustls::Error> {
            verify_tls13_signature(
                message,
                cert,
                dss,
                &rustls::crypto::aws_lc_rs::default_provider().signature_verification_algorithms,
            )
        }

        fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
            rustls::crypto::aws_lc_rs::default_provider()
                .signature_verification_algorithms
                .supported_schemes()
        }
    }
}
