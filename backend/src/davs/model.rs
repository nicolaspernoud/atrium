use crate::{
    configuration::{Config, ConfigFile},
    users::AdminToken,
    utils::{option_string_trim, string_trim, vec_trim_remove_empties},
};
use axum::{extract::Path, response::IntoResponse, Extension, Json};
use hyper::StatusCode;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dav {
    pub id: usize,
    #[serde(deserialize_with = "string_trim")]
    pub host: String,
    #[serde(deserialize_with = "string_trim")]
    pub directory: String,
    pub writable: bool,
    #[serde(deserialize_with = "string_trim")]
    pub name: String,
    pub icon: usize,
    pub color: usize,
    pub secured: bool,
    #[serde(default)]
    pub allow_symlinks: bool,
    #[serde(deserialize_with = "vec_trim_remove_empties")]
    pub roles: Vec<String>,
    #[serde(
        default,
        deserialize_with = "option_string_trim",
        skip_serializing_if = "Option::is_none"
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

pub async fn get_davs(config: Config, _admin: AdminToken) -> Json<Vec<Dav>> {
    Json(config.davs)
}

pub async fn delete_dav(
    config_file: Extension<ConfigFile>,
    mut config: Config,
    _admin: AdminToken,
    Path(dav_id): Path<(String, usize)>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    // Find the dav
    if let Some(pos) = config.davs.iter().position(|d| d.id == dav_id.1) {
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
    config_file: Extension<ConfigFile>,
    mut config: Config,
    _admin: AdminToken,
    Json(payload): Json<Dav>,
) -> Result<(StatusCode, &'static str), (StatusCode, &'static str)> {
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
