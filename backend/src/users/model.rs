use crate::{
    apps::App,
    appstate::{ConfigFile, ConfigState},
    configuration::config_or_error,
    davs::model::Dav,
    users::share::Share,
    utils::{is_default, string_trim, vec_trim_remove_empties},
};
use argon2::{Argon2, PasswordHasher, password_hash::SaltString};
use axum::{
    Json,
    extract::{Path, State},
    response::IntoResponse,
};
use chacha20poly1305::aead::OsRng;
use http::StatusCode;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::auth::{REDACTED, check_user_has_role};

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserInfo {
    #[serde(
        skip_serializing_if = "is_default",
        default,
        deserialize_with = "string_trim"
    )]
    pub given_name: String,
    #[serde(
        skip_serializing_if = "is_default",
        default,
        deserialize_with = "string_trim"
    )]
    pub family_name: String,
    #[serde(
        skip_serializing_if = "is_default",
        default,
        deserialize_with = "string_trim"
    )]
    pub email: String,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    #[serde(deserialize_with = "string_trim")]
    pub login: String,
    #[serde(
        skip_serializing_if = "is_default",
        default,
        deserialize_with = "string_trim"
    )]
    pub password: String,
    #[serde(
        default,
        skip_serializing_if = "is_default",
        deserialize_with = "vec_trim_remove_empties"
    )]
    pub roles: Vec<String>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub info: Option<UserInfo>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserToken {
    pub login: String,
    pub roles: Vec<String>,
    pub xsrf_token: Option<String>,
    pub share: Option<Share>,
    pub expires: i64,
    pub info: Option<UserInfo>,
}

impl UserToken {
    pub(crate) fn from_json(
        serialized_user_token: &str,
    ) -> Result<Self, (StatusCode, &'static str)> {
        let user_token = serde_json::from_str::<Self>(serialized_user_token).map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "could not deserialize user token",
            )
        })?;
        user_token.check_expires()
    }

    pub(crate) fn check_expires(self) -> Result<Self, (StatusCode, &'static str)> {
        let now = OffsetDateTime::now_utc().unix_timestamp();
        if now > self.expires {
            Err((StatusCode::FORBIDDEN, "user token is expired"))
        } else {
            Ok(self)
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct AdminToken(pub(crate) UserToken);

#[derive(Deserialize)]
pub struct LocalAuth {
    pub(crate) login: String,
    pub(crate) password: String,
}

#[derive(Deserialize, Serialize)]
pub struct AuthResponse {
    pub is_admin: bool,
    pub xsrf_token: Option<String>,
}

pub async fn get_users(
    State(config_file): State<ConfigFile>,
    _admin: AdminToken,
) -> Result<Json<Vec<User>>, (StatusCode, &'static str)> {
    let config = config_or_error(&config_file).await?;
    // Return all the users as Json
    Ok(Json(config.users))
}

pub async fn delete_user(
    State(config_file): State<ConfigFile>,
    _admin: AdminToken,
    Path(user_login): Path<String>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let mut config = config_or_error(&config_file).await?;
    // Find the user
    if let Some(pos) = config.users.iter().position(|u| u.login == user_login) {
        // It is an existing user, delete it
        config.users.remove(pos);
    } else {
        // If the user does not exist, respond with an error
        return Err((StatusCode::BAD_REQUEST, "user does not exist"));
    }

    config
        .to_file_or_internal_server_error(&config_file)
        .await?;

    Ok((StatusCode::OK, "user deleted successfully"))
}

pub async fn add_user(
    State(config_file): State<ConfigFile>,
    State(config): State<ConfigState>,
    _admin: AdminToken,
    Json(mut payload): Json<User>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    // Clone the config
    let mut config = (*config).clone();
    // Find the user
    if let Some(user) = config.users.iter_mut().find(|u| u.login == payload.login) {
        // It is an existing user, we only hash the password if it is not empty
        if !payload.password.is_empty() {
            hash_password(&mut payload)
                .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "password hash failed"))?;
        } else {
            payload.password = user.password.clone();
        }
        *user = payload;
    } else {
        // It is a new user, we need to hash the password
        if payload.password.is_empty() {
            return Err((StatusCode::NOT_ACCEPTABLE, "password is required"));
        }
        hash_password(&mut payload)
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "password hash failed"))?;
        config.users.push(payload);
    }

    config
        .to_file_or_internal_server_error(&config_file)
        .await?;

    Ok((StatusCode::CREATED, "user created or updated successfully"))
}

pub(crate) fn hash_password(payload: &mut User) -> Result<(), argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    payload.password = argon2
        .hash_password(payload.password.trim().as_bytes(), &salt)?
        .to_string();
    Ok(())
}

pub async fn list_services(
    State(config): State<ConfigState>,
    user: UserToken,
) -> Result<Json<(Vec<App>, Vec<Dav>)>, StatusCode> {
    if user.share.is_some() {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(Json((
        config
            .apps
            .iter()
            .filter(|app| !app.secured || check_user_has_role(&user, &app.roles))
            .cloned()
            .map(|mut app| {
                app.login = REDACTED.to_owned();
                app.password = REDACTED.to_owned();
                app
            })
            .collect(),
        config
            .davs
            .iter()
            .filter(|dav| !dav.secured || check_user_has_role(&user, &dav.roles))
            .cloned()
            .map(|mut dav| {
                dav.passphrase = None;
                dav
            })
            .collect(),
    )))
}

pub async fn whoami(token: UserToken) -> Json<User> {
    let user = User {
        login: token.login,
        password: REDACTED.to_owned(),
        roles: token.roles,
        info: token.info,
    };
    Json(user)
}

#[cfg(test)]
mod check_expires_test {
    use super::UserToken;
    use time::{Duration, OffsetDateTime};

    #[test]
    fn test_expires_ok() {
        let user = UserToken {
            expires: (OffsetDateTime::now_utc() + Duration::seconds(1)).unix_timestamp(),
            ..Default::default()
        };
        assert!(user.check_expires().is_ok());
    }

    #[test]
    fn test_expires_error() {
        let user = UserToken {
            expires: (OffsetDateTime::now_utc() - Duration::seconds(1)).unix_timestamp(),
            ..Default::default()
        };
        assert!(user.check_expires().is_err());
    }
}
