use std::path::PathBuf;

use axum::{
    Json,
    extract::{Request, State},
    middleware::Next,
    response::IntoResponse,
};
use axum_extra::extract::{PrivateCookieJar, cookie::Cookie};
use http::StatusCode;
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};

use crate::{
    appstate::ConfigState,
    users::{AUTH_COOKIE, UserToken, check_user_has_role, decrypt_user_token},
    utils::{is_path_within_base, random_string},
};

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Share {
    pub hostname: String,
    pub path: PathBuf,
    pub share_with: Option<String>,
    pub share_for_days: Option<i64>,
    #[serde(default)]
    pub writable: bool,
}

#[derive(Serialize, Deserialize)]
pub struct ShareResponse {
    pub token: String,
    pub xsrf_token: Option<String>,
}

pub async fn get_share_token(
    State(config): State<ConfigState>,
    user: UserToken,
    jar: PrivateCookieJar,
    Json(share): Json<Share>,
) -> Result<PrivateCookieJar, StatusCode> {
    // Get the dav from the config map
    let to_share = config
        .davs
        .iter()
        .find(|d| {
            d.host == share.hostname
                || format!(
                    "{}.{}",
                    crate::configuration::trim_host(&d.host),
                    config.hostname
                ) == share.hostname
        })
        .ok_or(StatusCode::FORBIDDEN)?;
    // Check that the user is allowed to access the wanted share
    if !&to_share.secured || check_user_has_role(&user, &to_share.roles) {
        // If it's already a share token, check that the new share is not more permissive
        if let Some(existing_share) = &user.share {
            if share.hostname != existing_share.hostname {
                return Err(StatusCode::FORBIDDEN);
            }
            if !is_path_within_base(&share.path, &existing_share.path) {
                return Err(StatusCode::FORBIDDEN);
            }
            if !existing_share.writable && share.writable {
                return Err(StatusCode::FORBIDDEN);
            }
        }

        // Create a token with the required information
        let share_login = share
            .share_with
            .as_ref()
            .map(|share_with| format!("{} (shared by {})", share_with, user.login))
            .unwrap_or(format!("{} (downloading)", user.login));
        let expires = share
            .share_for_days
            .as_ref()
            .map_or(Duration::seconds(2), |d| Duration::days(*d));
        let mut expires_timestamp = (OffsetDateTime::now_utc() + expires).unix_timestamp();

        // If it's already a share token, the new token cannot last longer than the original one
        if user.share.is_some() && expires_timestamp > user.expires {
            expires_timestamp = user.expires;
        }

        let share_token = UserToken {
            login: share_login,
            roles: user.roles,
            xsrf_token: Some(random_string(16)),
            share: Some(share),
            expires: expires_timestamp,
            info: None,
        };
        let encoded =
            serde_json::to_string(&share_token).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        // Store the user into the cookie
        let cookie = Cookie::new(AUTH_COOKIE, encoded);
        Ok(jar.add(cookie))
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

pub async fn cookie_to_body(
    jar: PrivateCookieJar,
    req: Request,
    next: Next,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let res = next.run(req).await;
    let (parts, _) = res.into_parts();
    if parts.status == StatusCode::OK {
        let encrypted_token = parts
            .headers
            .get("set-cookie")
            .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?
            .to_str()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .split_once("=")
            .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
        let plain_token =
            decrypt_user_token(encrypted_token.0, &jar, encrypted_token.1).map_err(|e| e.0)?;
        let res = Json(ShareResponse {
            token: encrypted_token.1.to_owned(),
            xsrf_token: plain_token.xsrf_token,
        });
        Ok(res)
    } else {
        Err(parts.status)
    }
}
