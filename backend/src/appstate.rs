use crate::configuration::{Config, HostType};
use axum::extract::FromRef;
use axum_extra::extract::cookie::Key;
use hyper::client::HttpConnector;
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use hyper_trust_dns::{RustlsHttpsConnector, TrustDnsResolver};
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
pub type Client = hyper::client::Client<RustlsHttpsConnector>;
pub type InsecureSkipVerifyClient = hyper::client::Client<HttpsConnector<HttpConnector>>;

pub static MAXMIND_READER: OnceLock<Reader<Vec<u8>>> = OnceLock::new();

#[derive(Clone)]
pub struct AppState {
    key: Key,
    config: ConfigState,
    config_map: ConfigMap,
    config_file: ConfigFile,
    client: Client,
    insecure_skip_verify_client: InsecureSkipVerifyClient,
}

impl AppState {
    pub(crate) fn new(
        key: Key,
        config: ConfigState,
        config_map: ConfigMap,
        config_file: String,
    ) -> Self {
        if let Ok(r) = maxminddb::Reader::open_readfile("GeoLite2-City.mmdb") {
            MAXMIND_READER.get_or_init(|| r);
        }

        let client = hyper::Client::builder()
            .http1_title_case_headers(true)
            .build::<_, hyper::Body>(
                TrustDnsResolver::from_system_conf().into_rustls_webpki_https_connector(),
            );

        // Create an insecure HTTPS connector
        let https = HttpsConnectorBuilder::new()
            .with_tls_config(get_rustls_config_dangerous())
            .https_or_http()
            .enable_http1()
            .build();

        let unsecure_client = hyper::Client::builder()
            .http1_title_case_headers(true)
            .build::<_, hyper::Body>(https);

        AppState {
            key,
            config,
            config_map,
            config_file: Arc::new(config_file),
            client,
            insecure_skip_verify_client: unsecure_client,
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

pub fn get_rustls_config_dangerous() -> ClientConfig {
    let store = rustls::RootCertStore::empty();

    let mut config = ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(store)
        .with_no_client_auth();

    let mut dangerous_config = ClientConfig::dangerous(&mut config);
    dangerous_config.set_certificate_verifier(Arc::new(NoCertificateVerification {}));

    config
}
pub struct NoCertificateVerification {}
impl rustls::client::ServerCertVerifier for NoCertificateVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::Certificate,
        _intermediates: &[rustls::Certificate],
        _server_name: &rustls::ServerName,
        _scts: &mut dyn Iterator<Item = &[u8]>,
        _ocsp: &[u8],
        _now: std::time::SystemTime,
    ) -> Result<rustls::client::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::ServerCertVerified::assertion())
    }
}
