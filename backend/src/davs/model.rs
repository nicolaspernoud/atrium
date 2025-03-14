use crate::{
    appstate::{ConfigFile, ConfigState},
    configuration::config_or_error,
    users::AdminToken,
    utils::{is_default, option_string_trim, string_trim, vec_trim_remove_empties},
};
use axum::{
    Json,
    extract::{Path, State},
    response::IntoResponse,
};
use hyper::StatusCode;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dav {
    pub id: usize,
    #[serde(deserialize_with = "string_trim")]
    pub host: String,
    #[serde(deserialize_with = "string_trim")]
    pub directory: String,
    #[serde(default, skip_serializing_if = "is_default")]
    pub writable: bool,
    #[serde(deserialize_with = "string_trim")]
    pub name: String,
    #[serde(default, skip_serializing_if = "is_default")]
    pub icon: String,
    pub color: usize,
    #[serde(default, skip_serializing_if = "is_default")]
    pub secured: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    pub allow_symlinks: bool,
    #[serde(
        default,
        skip_serializing_if = "is_default",
        deserialize_with = "vec_trim_remove_empties"
    )]
    pub roles: Vec<String>,
    #[serde(
        default,
        deserialize_with = "option_string_trim",
        skip_serializing_if = "is_default"
    )]
    pub passphrase: Option<String>,
    #[serde(skip)]
    pub key: Option<[u8; 32]>,
}

impl Dav {
    pub fn compute_key(&mut self) {
        if let Some(passphrase) = &self.passphrase {
            let mut hasher = Sha256::new();
            hasher.update(passphrase);
            let result: [u8; 32] = hasher.finalize().into();
            self.key = Some(result);
        }
    }
}

pub async fn get_davs(
    State(config_file): State<ConfigFile>,
    _admin: AdminToken,
) -> Result<Json<Vec<Dav>>, (StatusCode, &'static str)> {
    let config = config_or_error(&config_file).await?;
    // Return all the davs as Json
    Ok(Json(config.davs))
}

pub async fn delete_dav(
    State(config_file): State<ConfigFile>,
    _admin: AdminToken,
    Path(dav_id): Path<usize>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let mut config = config_or_error(&config_file).await?;
    // Find the dav
    if let Some(pos) = config.davs.iter().position(|d| d.id == dav_id) {
        // It is an existing dav, delete it
        config.davs.remove(pos);
    } else {
        // If the dav doesn't exist, respond with an error
        return Err((StatusCode::BAD_REQUEST, "dav doesn't exist"));
    }

    config
        .to_file_or_internal_server_error(&config_file)
        .await?;

    Ok((StatusCode::OK, "dav deleted successfully"))
}

pub async fn add_dav(
    State(config_file): State<ConfigFile>,
    State(config): State<ConfigState>,
    _admin: AdminToken,
    Json(payload): Json<Dav>,
) -> Result<(StatusCode, &'static str), (StatusCode, &'static str)> {
    // Clone the config
    let mut config = (*config).clone();
    // Find the dav
    if let Some(dav) = config.davs.iter_mut().find(|d| d.id == payload.id) {
        *dav = payload;
    } else {
        config.davs.push(payload);
    }

    config
        .to_file_or_internal_server_error(&config_file)
        .await?;

    Ok((StatusCode::CREATED, "dav created or updated successfully"))
}
