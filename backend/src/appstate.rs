use axum::extract::FromRef;
use axum_extra::extract::cookie::Key;
use hyper_trust_dns::{RustlsHttpsConnector, TrustDnsResolver};
use maxminddb::Reader;
use once_cell::sync::Lazy;
use std::{collections::HashMap, sync::Arc};

use crate::configuration::{Config, HostType};

pub type OptionalMaxMindReader = Arc<Option<Reader<Vec<u8>>>>;
pub type ConfigMap = Arc<HashMap<String, HostType>>;
pub type ConfigFile = Arc<String>;
pub type ConfigState = Arc<Config>;
pub type Client = hyper::client::Client<RustlsHttpsConnector>;

pub static MAXMIND_READER: Lazy<OptionalMaxMindReader> = Lazy::new(|| {
    let maxmind_reader = maxminddb::Reader::open_readfile("GeoLite2-City.mmdb").ok();
    Arc::new(maxmind_reader)
});

#[derive(Clone)]
pub struct AppState {
    key: Key,
    config: ConfigState,
    config_map: ConfigMap,
    config_file: ConfigFile,
    client: Client,
}

impl AppState {
    pub(crate) fn new(
        key: Key,
        config: ConfigState,
        config_map: ConfigMap,
        config_file: String,
    ) -> Self {
        AppState {
            key,
            config,
            config_map,
            config_file: Arc::new(config_file),
            client: hyper::Client::builder()
                .http1_title_case_headers(true)
                .build::<_, hyper::Body>(
                    TrustDnsResolver::default().into_rustls_webpki_https_connector(),
                ),
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
