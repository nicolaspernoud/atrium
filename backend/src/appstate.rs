use crate::configuration::{Config, HostType};
#[cfg(target_os = "linux")]
use crate::jail::Jail;
use axum::{body::Body, extract::FromRef};
use axum_extra::extract::cookie::Key;
use http::Request;
use hyper::Response;
use hyper::body::Incoming;
use hyper_hickory::{TokioHickoryHttpConnector, TokioHickoryResolver};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use hyper_util::rt::TokioExecutor;
use maxminddb::Reader;
use rustls::ClientConfig;
use std::{
    collections::HashMap,
    sync::{Arc, OnceLock, LazyLock},
};
use tracing::warn;

pub(crate) type OptionalMaxMindReader = Option<&'static Reader<Vec<u8>>>;
pub(crate) type ConfigMap = Arc<HashMap<String, HostType>>;
pub(crate) type ConfigFile = Arc<String>;
pub(crate) type ConfigState = Arc<Config>;
pub(crate) type ConfigLock = Arc<tokio::sync::Mutex<()>>;
pub(crate) type UpgradedConnectionsSemaphore = Arc<tokio::sync::Semaphore>;

pub(crate) static CONFIG_FILE_LOCK: LazyLock<ConfigLock> =
    LazyLock::new(|| Arc::new(tokio::sync::Mutex::new(())));
    
pub struct Client(
    pub hyper_util::client::legacy::Client<HttpsConnector<TokioHickoryHttpConnector>, Body>,
);
pub struct InsecureSkipVerifyClient(
    pub hyper_util::client::legacy::Client<HttpsConnector<TokioHickoryHttpConnector>, Body>,
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
    client: Client,
    insecure_skip_verify_client: InsecureSkipVerifyClient,
    upgraded_connections_semaphore: UpgradedConnectionsSemaphore,
    config_file: ConfigFile,
    #[cfg(target_os = "linux")]
    jail: Option<Arc<Jail>>,
}

impl AppState {
    pub(crate) fn new(
        key: Key,
        config: ConfigState,
        config_map: ConfigMap,
        config_file: String,
        #[cfg(target_os = "linux")] jail: Option<Arc<Jail>>,
    ) -> Self {
        match maxminddb::Reader::open_readfile("GeoLite2-City.mmdb") {
            Ok(r) => {
                MAXMIND_READER.get_or_init(|| r);
            }
            Err(e) => {
                warn!("Could not open GeoLite2-City.mmdb: {}, GeoIP lookups will be disabled", e);
            }
        }

        // Create a secure HTTPS Client that use Hickory as DNS resolver, and get the configuration from system conf
        let mut dns_resolver = TokioHickoryResolver::from_system_conf()
            .expect("could not create DNS resolver from system configuration")
            .into_http_connector();
        dns_resolver.enforce_http(false);

        let mut client_builder = hyper_util::client::legacy::Client::builder(TokioExecutor::new());
        client_builder.http1_title_case_headers(true);

        let client = client_builder.build(
            HttpsConnectorBuilder::new()
                .with_webpki_roots()
                .https_or_http()
                .enable_http1()
                .wrap_connector(dns_resolver.clone()),
        );

        let unsecure_client = client_builder.build(
            HttpsConnectorBuilder::new()
                .with_tls_config(get_rustls_config_dangerous())
                .https_or_http()
                .enable_http1()
                .wrap_connector(dns_resolver),
        );

        AppState {
            key,
            config,
            config_map,
            config_file: Arc::new(config_file),
            client: Client(client),
            insecure_skip_verify_client: InsecureSkipVerifyClient(unsecure_client),
            upgraded_connections_semaphore: Arc::new(tokio::sync::Semaphore::new(100)),
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

impl FromRef<AppState> for ConfigLock {
    fn from_ref(_state: &AppState) -> Self {
        Arc::clone(&CONFIG_FILE_LOCK)
    }
}

impl FromRef<AppState> for UpgradedConnectionsSemaphore {
    fn from_ref(state: &AppState) -> Self {
        Arc::clone(&state.upgraded_connections_semaphore)
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

    config.dangerous().set_certificate_verifier(Arc::new(
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
