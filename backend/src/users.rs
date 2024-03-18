use crate::{
    apps::App,
    appstate::{ConfigFile, ConfigState, OptionalMaxMindReader, MAXMIND_READER},
    configuration::{config_or_error, trim_host, Config, HostType},
    davs::model::Dav,
    errors::ErrResponse,
    headers::XSRFToken,
    logger::city_from_ip,
    utils::{
        is_default, query_pairs_or_error, random_string, string_trim, vec_trim_remove_empties,
    },
};
use argon2::{password_hash::SaltString, Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use axum::{
    async_trait,
    body::Body,
    extract::{ConnectInfo, FromRef, FromRequestParts, Host, Path, RawQuery, Request, State},
    middleware::Next,
    response::{IntoResponse, Response},
    Extension, Json, RequestPartsExt,
};
use axum_extra::{
    extract::cookie::{Cookie, Key, PrivateCookieJar, SameSite},
    TypedHeader,
};
use headers::{authorization::Basic, Authorization, HeaderName};
use http::{
    header::{CONTENT_LENGTH, LOCATION, SET_COOKIE},
    request::Parts,
    HeaderValue, StatusCode,
};

use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use time::{Duration, OffsetDateTime};
use tracing::info;

pub static AUTH_COOKIE: &str = "ATRIUM_AUTH";
static SHARE_TOKEN: &str = "SHARE_TOKEN";
static WWWAUTHENTICATE: HeaderName = HeaderName::from_static("www-authenticate");
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
pub struct Share {
    pub hostname: String,
    pub path: String,
    pub share_with: Option<String>,
    pub share_for_days: Option<i64>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserToken {
    pub login: String,
    pub roles: Vec<String>,
    pub xsrf_token: String,
    pub share: Option<Share>,
    pub expires: i64,
    pub info: Option<UserInfo>,
}

impl UserToken {
    fn from_json(serialized_user_token: &str) -> Result<Self, (StatusCode, &'static str)> {
        let user_token = serde_json::from_str::<Self>(serialized_user_token).map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "could not deserialize user token",
            )
        })?;
        user_token.check_expires()
    }

    fn check_expires(self) -> Result<Self, (StatusCode, &'static str)> {
        let now = OffsetDateTime::now_utc().unix_timestamp();
        if now > self.expires {
            Err((StatusCode::FORBIDDEN, "user token is expired"))
        } else {
            Ok(self)
        }
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for UserToken
where
    S: Send + Sync,
    Key: FromRef<S>,
    ConfigState: FromRef<S>,
{
    type Rejection = (StatusCode, &'static str);
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let jar = PrivateCookieJar::from_request_parts(parts, state)
            .await
            .expect("Could not find cookie jar");

        // Get the serialized user_token from the cookie jar, and check the xsrf token
        if let Some(cookie) = jar.get(AUTH_COOKIE) {
            if let Ok(TypedHeader(XSRFToken(xsrf_token))) =
                TypedHeader::<XSRFToken>::from_request_parts(parts, state).await
            {
                // Deserialize the user_token and return him/her
                let serialized_user_token = cookie.value();
                let user_token = UserToken::from_json(serialized_user_token)?;

                if user_token.xsrf_token != xsrf_token {
                    return Err((StatusCode::FORBIDDEN, "xsrf token doesn't match"));
                }
                return Ok(user_token);
            }
        }

        // OR Try to get user_token from the query
        if let Ok(query) = RawQuery::from_request_parts(parts, state).await {
            if let Some(Some(password)) = query_pairs_or_error(query.0.as_deref())
                .ok()
                .map(|hm| hm.get("token").map(|v| v.to_owned()))
            {
                let res = cookie_from_password(AUTH_COOKIE, &jar, password);
                if res.is_ok() {
                    return res;
                } else {
                    return cookie_from_password(SHARE_TOKEN, &jar, password);
                }
            }
        }

        // OR Try to get user_token from basic auth headers

        if let Ok(TypedHeader(Authorization(basic))) =
            TypedHeader::<Authorization<Basic>>::from_request_parts(parts, state).await
        {
            match cookie_from_password(AUTH_COOKIE, &jar, basic.password()) {
                Ok(token) => return Ok(token),
                Err(_) => {
                    let config = ConfigState::from_ref(state);

                    let Extension(addr) = parts
                        .extract::<Extension<ConnectInfo<SocketAddr>>>()
                        .await
                        .expect("Could not find socket address");
                    return match authenticate_local_user(
                        &config,
                        LocalAuth {
                            login: basic.username().to_string(),
                            password: basic.password().to_string(),
                        },
                        MAXMIND_READER.get(),
                        addr.0,
                    ) {
                        Ok(user) => Ok(user.1),
                        Err(e) => Err((e.0, "no user found in basic auth")),
                    };
                }
            }
        }

        Err((
            StatusCode::UNAUTHORIZED,
            "no user found or xsrf token not provided",
        ))
    }
}

fn cookie_from_password(
    cookie_name: &str,
    jar: &PrivateCookieJar,
    password: &str,
) -> Result<UserToken, (StatusCode, &'static str)> {
    let cookie = Cookie::parse_encoded(format!("{}={}", cookie_name, password)).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "could not find user token",
        )
    })?;
    let decrypted_cookie = jar.decrypt(cookie).ok_or(()).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "could not decrypt user token",
        )
    })?;
    let serialized_user_token = decrypted_cookie.value();
    UserToken::from_json(serialized_user_token)
}

#[derive(Serialize, Deserialize)]
pub struct AdminToken(UserToken);

#[async_trait]
impl<S> FromRequestParts<S> for AdminToken
where
    S: Send + Sync,
    Key: FromRef<S>,
    ConfigState: FromRef<S>,
{
    type Rejection = (StatusCode, &'static str);
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let user = UserToken::from_request_parts(parts, state).await?;
        if !user.roles.contains(&ADMINS_ROLE.to_owned()) {
            return Err((StatusCode::UNAUTHORIZED, "user is not in admin group"));
        }
        Ok(AdminToken(user))
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserTokenWithoutXSRFCheck(pub UserToken);

#[async_trait]
impl<S> FromRequestParts<S> for UserTokenWithoutXSRFCheck
where
    S: Send + Sync,
    Key: FromRef<S>,
{
    type Rejection = (StatusCode, &'static str);
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let jar: PrivateCookieJar = PrivateCookieJar::from_request_parts(parts, state)
            .await
            .expect("Could not find cookie jar");

        // Get the serialized user_token from the cookie jar, and check the xsrf token
        if let Some(cookie) = jar.get(AUTH_COOKIE) {
            // Deserialize the user_token and return him/her
            let serialized_user_token = cookie.value();
            let user_token = UserToken::from_json(serialized_user_token)?;
            return Ok(UserTokenWithoutXSRFCheck(user_token));
        }
        Err((StatusCode::UNAUTHORIZED, "no user found"))
    }
}

#[derive(Deserialize)]
pub struct LocalAuth {
    login: String,
    password: String,
}

#[derive(Deserialize, Serialize)]
pub struct AuthResponse {
    pub is_admin: bool,
    pub xsrf_token: String,
}

pub async fn local_auth(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    jar: PrivateCookieJar,
    State(config): State<ConfigState>,
    Host(hostname): Host,
    Json(payload): Json<LocalAuth>,
) -> Result<(PrivateCookieJar, Json<AuthResponse>), (StatusCode, &'static str)> {
    // Find the user in configuration
    let (user, user_token) = authenticate_local_user(&config, payload, MAXMIND_READER.get(), addr)?;
    let cookie = create_user_cookie(
        &user_token,
        hostname,
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

pub async fn logout(
    jar: PrivateCookieJar,
    Host(hostname): Host,
) -> Result<PrivateCookieJar, ErrResponse> {
    let domain = domain_from_hostname(hostname)?;
    let cookie = Cookie::build((AUTH_COOKIE, "")).path("/").domain(domain);
    Ok(jar.remove(cookie))
}

pub(crate) fn create_user_cookie(
    user_token: &UserToken,
    hostname: String,
    config: &Config,
    addr: SocketAddr,
    reader: OptionalMaxMindReader,
    user: &User,
) -> Result<Cookie<'static>, ErrResponse> {
    let encoded = serde_json::to_string(user_token)
        .map_err(|_| ErrResponse::S500("could not encode user"))?;
    let domain = domain_from_hostname(hostname)?;
    let cookie = Cookie::build((AUTH_COOKIE, encoded))
        .domain(domain)
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

fn domain_from_hostname(hostname: String) -> Result<String, ErrResponse> {
    let domain = hostname
        .split(':')
        .next()
        .ok_or(ErrResponse::S500("could not find domain"))?
        .to_owned();
    Ok(domain)
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
        login: user.login.to_owned(),
        roles: user.roles.to_owned(),
        xsrf_token: random_string(16),
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

pub async fn list_services(
    State(config): State<ConfigState>,
    user: UserToken,
) -> Json<(Vec<App>, Vec<Dav>)> {
    Json((
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
    ))
}

pub async fn whoami(token: UserTokenWithoutXSRFCheck) -> Json<User> {
    let user = User {
        login: token.0.login,
        password: REDACTED.to_owned(),
        roles: token.0.roles,
        info: token.0.info,
    };
    Json(user)
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
                || format!("{}.{}", trim_host(&d.host), config.hostname) == share.hostname
        })
        .ok_or(StatusCode::FORBIDDEN)?;
    // Check that the user is allowed to access the wanted share
    if !&to_share.secured || check_user_has_role(&user, &to_share.roles) {
        // Create a token with the required information
        let share_login = share
            .share_with
            .as_ref()
            .map(|share_with| format!("{} (shared by {})", share_with, user.login))
            .unwrap_or(format!("{} (downloading)", user.login));
        let expires = share
            .share_for_days
            .as_ref()
            .map(|d| Duration::days(*d))
            .unwrap_or(Duration::seconds(2));
        let user_token = UserToken {
            login: share_login,
            roles: user.roles,
            xsrf_token: random_string(16),
            share: Some(share),
            expires: (OffsetDateTime::now_utc() + expires).unix_timestamp(),
            info: None,
        };
        let encoded =
            serde_json::to_string(&user_token).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        // Store the user into the cookie
        let cookie = Cookie::new(SHARE_TOKEN, encoded);
        Ok(jar.add(cookie))
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

pub async fn cookie_to_body(req: Request, next: Next) -> Result<impl IntoResponse, StatusCode> {
    let res = next.run(req).await;
    let (mut parts, _) = res.into_parts();
    if parts.status == StatusCode::OK {
        let cookie = parts
            .headers
            .get("set-cookie")
            .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?
            .as_bytes()
            .to_owned();
        parts
            .headers
            .insert(CONTENT_LENGTH, HeaderValue::from(cookie.len()));
        let res = Response::from_parts(parts, Body::from(cookie));
        Ok(res)
    } else {
        Ok(Response::from_parts(parts, Body::empty()))
    }
}

fn hash_password(payload: &mut User) -> Result<(), argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    payload.password = argon2
        .hash_password(payload.password.trim().as_bytes(), &salt)?
        .to_string();
    Ok(())
}

pub fn check_user_has_role(user: &UserToken, roles: &[String]) -> bool {
    for user_role in user.roles.iter() {
        for role in roles.iter() {
            if user_role == role {
                return true;
            }
        }
    }
    false
}

pub fn check_user_has_role_or_forbid(
    user: &Option<&UserToken>,
    target: &HostType,
    hostname: &str,
    path: &str,
) -> Result<(), Response<Body>> {
    if let Some(user) = user {
        if !check_user_has_role(user, target.roles())
            || (user.share.is_some()
                && (user.share.as_ref().unwrap().path
                    != urlencoding::decode(path)
                        .to_owned()
                        .map_err(|_| forbidden())?
                    || user.share.as_ref().unwrap().hostname != hostname))
        {
            return Err(forbidden());
        }
        return Ok(());
    }
    Err(Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .header(&WWWAUTHENTICATE, r#"Basic realm="server""#)
        .body(Body::empty())
        .unwrap())
}

fn forbidden() -> http::Response<Body> {
    Response::builder()
        .status(StatusCode::FORBIDDEN)
        .body(Body::empty())
        .unwrap()
}

pub fn check_authorization(
    app: &HostType,
    user: &Option<&UserToken>,
    hostname: &str,
    path: &str,
) -> Result<(), Response<Body>> {
    if app.secured() {
        check_user_has_role_or_forbid(user, app, hostname, path)?
    }
    Ok(())
}

pub fn authorized_or_redirect_to_login(
    app: &HostType,
    user: &Option<UserTokenWithoutXSRFCheck>,
    hostname: &str,
    req: &Request<Body>,
    config: &std::sync::Arc<crate::configuration::Config>,
) -> Result<(), Response<Body>> {
    let domain = hostname.split(':').next().unwrap_or_default();
    if let Err(mut value) =
        check_authorization(app, &user.as_ref().map(|u| &u.0), domain, req.uri().path())
    {
        // Redirect to login page if user is not logged, write where to get back after login in a cookie
        if value.status() == StatusCode::UNAUTHORIZED {
            if let Ok(mut hn) = HeaderValue::from_str(&config.full_domain()) {
                *value.status_mut() = StatusCode::FOUND;
                // If single proxy mode, redirect directly to IdP without passing through atrium main app
                if config.single_proxy {
                    hn = HeaderValue::from_str(&(config.full_domain() + "/auth/oauth2login"))
                        .unwrap();
                }
                value.headers_mut().append(LOCATION, hn);
                let cookie = Cookie::build((
                    "ATRIUM_REDIRECT",
                    format!("{}://{hostname}", config.scheme()),
                ))
                .domain(config.domain.clone())
                .path("/")
                .same_site(SameSite::Lax)
                .secure(false)
                .max_age(time::Duration::seconds(60))
                .http_only(false);
                value.headers_mut().append(
                    SET_COOKIE,
                    HeaderValue::from_str(&format!("{cookie}")).unwrap(),
                );
            }
        }
        return Err(value);
    }
    Ok(())
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

#[cfg(test)]
mod check_user_has_role_or_forbid_tests {
    use crate::{
        apps::{App, AppWithUri},
        configuration::HostType,
        users::{check_user_has_role_or_forbid, UserToken},
    };

    #[test]
    fn test_no_user() {
        let user = &None;
        let app: App = App {
            target: "www.example.com".to_string(), // to prevent failing when parsing url
            roles: vec!["role1".to_string(), "role2".to_string()],
            ..Default::default()
        };
        let app = AppWithUri::from_app(app, None);
        let target = HostType::ReverseApp(Box::new(app));
        assert!(check_user_has_role_or_forbid(user, &target, "", "").is_err());
    }

    #[test]
    fn test_user_has_all_roles() {
        let user = UserToken {
            roles: vec!["role1".to_string(), "role2".to_string()],
            ..Default::default()
        };
        let app: App = App {
            target: "www.example.com".to_string(), // to prevent failing when parsing url
            roles: vec!["role1".to_string(), "role2".to_string()],
            ..Default::default()
        };
        let app = AppWithUri::from_app(app, None);
        let target = HostType::ReverseApp(Box::new(app));
        assert!(check_user_has_role_or_forbid(&Some(&user), &target, "", "").is_ok());
    }

    #[test]
    fn test_user_has_one_role() {
        let user = UserToken {
            roles: vec!["role1".to_string()],
            ..Default::default()
        };
        let app: App = App {
            target: "www.example.com".to_string(), // to prevent failing when parsing url
            roles: vec!["role1".to_string(), "role2".to_string()],
            ..Default::default()
        };
        let app = AppWithUri::from_app(app, None);
        let target = HostType::ReverseApp(Box::new(app));
        assert!(check_user_has_role_or_forbid(&Some(&user), &target, "", "").is_ok());
    }

    #[test]
    fn test_user_has_no_role() {
        let user = UserToken {
            roles: vec!["role3".to_string(), "role4".to_string()],
            ..Default::default()
        };
        let app: App = App {
            target: "www.example.com".to_string(), // to prevent failing when parsing url
            roles: vec!["role1".to_string(), "role2".to_string()],
            ..Default::default()
        };
        let app = AppWithUri::from_app(app, None);
        let target = HostType::ReverseApp(Box::new(app));
        assert!(check_user_has_role_or_forbid(&Some(&user), &target, "", "").is_err());
    }

    #[test]
    fn test_user_roles_are_empty() {
        let user = UserToken::default();
        let app = App {
            target: "www.example.com".to_string(), // to prevent failing when parsing url
            roles: vec!["role1".to_string(), "role2".to_string()],
            ..Default::default()
        };
        let app = AppWithUri::from_app(app, None);
        let target = HostType::ReverseApp(Box::new(app));
        assert!(check_user_has_role_or_forbid(&Some(&user), &target, "", "").is_err());
    }

    #[test]
    fn test_allowed_roles_are_empty() {
        let user = UserToken {
            roles: vec!["role1".to_string(), "role2".to_string()],
            ..Default::default()
        };
        let app = App {
            target: "www.example.com".to_string(), // to prevent failing when parsing url
            ..Default::default()
        };
        let app = AppWithUri::from_app(app, None);
        let target = HostType::ReverseApp(Box::new(app));
        assert!(check_user_has_role_or_forbid(&Some(&user), &target, "", "").is_err());
    }

    #[test]
    fn test_all_roles_are_empty() {
        let user = UserToken::default();
        let app = App {
            target: "www.example.com".to_string(), // to prevent failing when parsing url
            ..Default::default()
        };
        let app = AppWithUri::from_app(app, None);
        let target = HostType::ReverseApp(Box::new(app));
        assert!(check_user_has_role_or_forbid(&Some(&user), &target, "", "").is_err());
    }
}
