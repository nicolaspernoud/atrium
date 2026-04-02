use crate::{
    appstate::ConfigFile,
    appstate::{ConfigState, MAXMIND_READER, OptionalMaxMindReader},
    auth::check_user_has_role,
    configuration::Config,
    configuration::config_or_error,
    errors::ErrResponse,
    extract::Host,
    logger::city_from_ip,
    utils::{
        is_default, query_pairs_or_error, random_string, string_trim, vec_trim_remove_empties,
    },
};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use axum::{
    Extension, Json, RequestPartsExt,
    extract::{
        ConnectInfo, FromRef, FromRequestParts, OptionalFromRequestParts, Path, RawQuery, State,
    },
    response::{IntoResponse, Response},
};
use axum_extra::{
    TypedHeader,
    extract::cookie::{Cookie, Key, PrivateCookieJar},
};
use chacha20poly1305::aead::OsRng;
use headers::{Authorization, authorization::Basic};
use http::{StatusCode, request::Parts};
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, net::SocketAddr};
use time::{Duration, OffsetDateTime};
use tracing::info;

pub use super::share::Share;

pub static AUTH_COOKIE: &str = "ATRIUM_AUTH";
pub static ADMINS_ROLE: &str = "ADMINS";
pub static REDACTED: &str = "REDACTED";

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

#[derive(Serialize, Deserialize, Clone)]
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

impl<S> FromRequestParts<S> for UserToken
where
    S: Send + Sync,
    Key: FromRef<S>,
    ConfigState: FromRef<S>,
    crate::OptionalJail: FromRef<S>,
{
    type Rejection = Response;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        #[cfg(target_os = "linux")]
        let jail = crate::OptionalJail::from_ref(state);
        let jar = PrivateCookieJar::from_request_parts(parts, state)
            .await
            .expect("Cookie jar retrieval is Infallible");

        // Try to get user_token from the query
        let Ok(query) = RawQuery::from_request_parts(parts, state).await;
        if let Some(Some(password)) = query_pairs_or_error(query.0.as_deref())
            .ok()
            .map(|hm| hm.get("token").map(|v| v.to_owned()))
        {
            let user_token = decrypt_user_token(AUTH_COOKIE, &jar, password)
                .map(|mut t| {
                    t.xsrf_token = None;
                    t
                })
                .map_err(|e| (e.0, e.1).into_response())?;
            return Ok(user_token);
        }

        // OR Try to get the serialized user_token from the cookie jar, and check the xsrf token
        if let Some(cookie) = jar.get(AUTH_COOKIE) {
            // Deserialize the user_token and return him/her
            let serialized_user_token = cookie.value();
            let user_token = UserToken::from_json(serialized_user_token)
                .map_err(|e| (e.0, e.1).into_response())?;
            return Ok(user_token);
        }

        // OR Try to get user_token from basic auth headers
        if let Ok(TypedHeader(Authorization(basic))) =
            <TypedHeader<Authorization<Basic>> as FromRequestParts<S>>::from_request_parts(
                parts, state,
            )
            .await
        {
            let user_token = if let Ok(token) =
                decrypt_user_token(AUTH_COOKIE, &jar, basic.password()).map(|mut t| {
                    t.xsrf_token = None;
                    t
                }) {
                token
            } else {
                let config = ConfigState::from_ref(state);

                let Extension(addr) = parts
                    .extract::<Extension<ConnectInfo<SocketAddr>>>()
                    .await
                    .expect("Could not find socket address");
                match authenticate_local_user(
                    &config,
                    LocalAuth {
                        login: basic.username().to_string(),
                        password: basic.password().to_string(),
                    },
                    MAXMIND_READER.get(),
                    addr.0,
                ) {
                    Ok(user) => {
                        let mut t = user.1;
                        t.xsrf_token = None;
                        t
                    }
                    Err(e) => {
                        #[cfg(target_os = "linux")]
                        if let Some(jail) = jail {
                            jail.report_failure(addr.0.ip());
                        }
                        return Err((e.0, "no user found in basic auth").into_response());
                    }
                }
            };
            return Ok(user_token);
        }

        Err((
            StatusCode::UNAUTHORIZED,
            jar.remove(Cookie::build((AUTH_COOKIE, ""))),
            "xsrf token not provided or not matching",
        )
            .into_response())
    }
}

impl<S> OptionalFromRequestParts<S> for UserToken
where
    S: Send + Sync,
    Key: FromRef<S>,
    ConfigState: FromRef<S>,
    crate::OptionalJail: FromRef<S>,
{
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Option<Self>, Self::Rejection> {
        Ok(
            <UserToken as FromRequestParts<S>>::from_request_parts(parts, state)
                .await
                .ok(),
        )
    }
}

pub(crate) fn decrypt_user_token(
    cookie_name: &str,
    jar: &PrivateCookieJar,
    encrypted_token: &str,
) -> Result<UserToken, (StatusCode, &'static str)> {
    let cookie =
        Cookie::parse_encoded(format!("{cookie_name}={encrypted_token}")).map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "could not parse encrypted user token",
            )
        })?;
    let decrypted_cookie = jar.decrypt(cookie).ok_or({
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "could not decrypt user token",
        )
    })?;
    let serialized_user_token = decrypted_cookie.value();
    UserToken::from_json(serialized_user_token)
}

impl<S> FromRequestParts<S> for AdminToken
where
    S: Send + Sync,
    Key: FromRef<S>,
    ConfigState: FromRef<S>,
    crate::OptionalJail: FromRef<S>,
{
    type Rejection = Response;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let user = <UserToken as FromRequestParts<S>>::from_request_parts(parts, state).await?;
        if !user.roles.contains(&ADMINS_ROLE.to_owned()) {
            return Err((StatusCode::UNAUTHORIZED, "user is not in admin group").into_response());
        }
        if user.share.is_some() {
            return Err((
                StatusCode::FORBIDDEN,
                "share token cannot be used to access admin API",
            )
                .into_response());
        }
        let admin = AdminToken(user);
        Ok(admin)
    }
}

pub async fn local_auth(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    jar: PrivateCookieJar,
    State(config): State<ConfigState>,
    #[cfg(target_os = "linux")] State(jail): State<crate::OptionalJail>,
    host: Host,
    Json(payload): Json<LocalAuth>,
) -> Result<(PrivateCookieJar, Json<AuthResponse>), (StatusCode, &'static str)> {
    // Find the user in configuration
    let (user, user_token) = authenticate_local_user(&config, payload, MAXMIND_READER.get(), addr)
        .inspect_err(|_| {
            #[cfg(target_os = "linux")]
            if let Some(jail) = jail {
                jail.report_failure(addr.ip());
            }
        })?;
    let cookie = create_user_cookie(
        &user_token,
        &host,
        &config,
        addr,
        MAXMIND_READER.get(),
        user,
    )?;

    Ok((
        jar.add(cookie),
        Json(AuthResponse {
            is_admin: user.roles.contains(&ADMINS_ROLE.to_owned()),
            xsrf_token: user_token.xsrf_token,
        }),
    ))
}

pub async fn logout(jar: PrivateCookieJar, host: Host) -> Result<PrivateCookieJar, ErrResponse> {
    let cookie = Cookie::build((AUTH_COOKIE, ""))
        .path("/")
        .domain(host.hostname().to_owned());
    Ok(jar.remove(cookie))
}

pub(crate) fn create_user_cookie(
    user_token: &UserToken,
    host: &Host,
    config: &Config,
    addr: SocketAddr,
    reader: OptionalMaxMindReader,
    user: &User,
) -> Result<Cookie<'static>, ErrResponse> {
    let encoded = serde_json::to_string(user_token)
        .map_err(|_| ErrResponse::S500("could not encode user"))?;
    let cookie = Cookie::build((AUTH_COOKIE, encoded))
        .domain(host.hostname().to_owned())
        .path("/")
        .same_site(axum_extra::extract::cookie::SameSite::Lax)
        .secure(config.tls_mode.is_secure())
        .max_age(Duration::days(config.session_duration_days.unwrap_or(1)))
        .http_only(true)
        .build();
    info!(
        "AUTHENTICATION SUCCESS for {} from {}",
        user.login,
        city_from_ip(addr, reader)
    );
    Ok(cookie)
}

pub fn authenticate_local_user(
    config: &Config,
    payload: LocalAuth,
    reader: OptionalMaxMindReader,
    addr: SocketAddr,
) -> Result<(&User, UserToken), (StatusCode, &'static str)> {
    let user = config
        .users
        .iter()
        .find(|u| u.login == payload.login)
        .ok_or(StatusCode::UNAUTHORIZED)
        .map_err(|e| {
            info!(
                "AUTHENTICATION ERROR for {} from {} : user does not exist",
                payload.login,
                city_from_ip(addr, reader)
            );
            (e, "user does not exist")
        })?;

    // Check if the given password is correct
    let parsed_hash = PasswordHash::new(&user.password).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "could not compute password hash",
        )
    })?;
    Argon2::default()
        .verify_password(payload.password.as_bytes(), &parsed_hash)
        .map_err(|_| {
            info!(
                "AUTHENTICATION ERROR for {} from {} : password does not match",
                user.login,
                city_from_ip(addr, reader)
            );
            (StatusCode::UNAUTHORIZED, "user is not authorized")
        })?;

    // Create a token payload from the user
    let user_token = user_to_token(user, config);
    Ok((user, user_token))
}

pub(crate) fn user_to_token(user: &User, config: &Config) -> UserToken {
    UserToken {
        login: user.login.clone(),
        roles: user.roles.clone(),
        xsrf_token: Some(random_string(16)),
        share: None,
        expires: (OffsetDateTime::now_utc()
            + Duration::days(config.session_duration_days.unwrap_or(1)))
        .unix_timestamp(),
        info: user.info.clone(),
    }
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

pub async fn whoami(token: UserToken) -> Json<User> {
    let user = User {
        login: token.login,
        password: REDACTED.to_owned(),
        roles: token.roles,
        info: token.info,
    };
    Json(user)
}

pub async fn list_services(
    State(config): State<ConfigState>,
    user: UserToken,
) -> Result<axum::Json<(Vec<crate::apps::App>, Vec<crate::davs::model::Dav>)>, StatusCode> {
    if user.share.is_some() {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(axum::Json((
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

#[cfg(test)]
mod user_tests {
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
