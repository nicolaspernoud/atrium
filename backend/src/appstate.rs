use axum::extract::FromRef;
use axum_extra::extract::cookie::Key;
use maxminddb::Reader;

use std::{collections::HashMap, sync::Arc};

use crate::configuration::{Config, HostType};

pub type OptionalMaxMindReader = Arc<Option<Reader<Vec<u8>>>>;
pub type ConfigMap = Arc<HashMap<String, HostType>>;
pub type ConfigFile = Arc<String>;
pub type ConfigState = Arc<Config>;

#[derive(Clone)]
pub struct AppState {
    key: Key,
    config: ConfigState,
    config_map: ConfigMap,
    config_file: ConfigFile,
    maxmind_reader: OptionalMaxMindReader,
}

impl AppState {
    pub(crate) fn new(
        key: Key,
        config: ConfigState,
        config_map: ConfigMap,
        config_file: String,
        maxmind_reader: Option<Reader<Vec<u8>>>,
    ) -> Self {
        AppState {
            key,
            config,
            config_map,
            config_file: Arc::new(config_file),
            maxmind_reader: Arc::new(maxmind_reader),
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

impl FromRef<AppState> for OptionalMaxMindReader {
    fn from_ref(state: &AppState) -> Self {
        Arc::clone(&state.maxmind_reader)
    }
}
