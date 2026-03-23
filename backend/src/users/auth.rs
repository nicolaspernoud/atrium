use crate::{
    appstate::{ConfigState, MAXMIND_READER, OptionalMaxMindReader},
    configuration::{Config, HostType},
    errors::ErrResponse,
    extract::Host,
    logger::city_from_ip,
    utils::{is_path_within_base, query_pairs_or_error, random_string},
};
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::{
    Extension, Json, RequestPartsExt,
    body::Body,
    extract::{
        ConnectInfo, FromRef, FromRequestParts, OptionalFromRequestParts, RawQuery, Request, State,
    },
    response::Response,
};
use axum_extra::{
    TypedHeader,
    extract::cookie::{Cookie, Key, PrivateCookieJar, SameSite},
};
use headers::{Authorization, HeaderName, authorization::Basic};
use http::{
    HeaderValue, StatusCode,
    header::{LOCATION, SET_COOKIE},
    request::Parts,
};

use std::{convert::Infallible, net::SocketAddr, path::PathBuf};
use time::{Duration, OffsetDateTime};
use tracing::info;

use super::model::{AdminToken, AuthResponse, LocalAuth, User, UserToken};

pub static AUTH_COOKIE: &str = "ATRIUM_AUTH";
static WWWAUTHENTICATE: HeaderName = HeaderName::from_static("www-authenticate");
pub static ADMINS_ROLE: &str = "ADMINS";
pub static REDACTED: &str = "REDACTED";

impl<S> FromRequestParts<S> for UserToken
where
    S: Send + Sync,
    Key: FromRef<S>,
    ConfigState: FromRef<S>,
    crate::OptionalJail: FromRef<S>,
{
    type Rejection = (StatusCode, &'static str);
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        #[cfg(target_os = "linux")]
        let jail = crate::OptionalJail::from_ref(state);
        let jar = PrivateCookieJar::from_request_parts(parts, state)
            .await
            .expect("Cookie jar retrieval is Infallible");

        // Get the serialized user_token from the cookie jar, and check the xsrf token
        if let Some(cookie) = jar.get(AUTH_COOKIE) {
            // Deserialize the user_token and return him/her
            let serialized_user_token = cookie.value();
            let user_token = UserToken::from_json(serialized_user_token)?;
            return Ok(user_token);
        }

        // OR Try to get user_token from the query
        let Ok(query) = RawQuery::from_request_parts(parts, state).await;
        if let Some(Some(password)) = query_pairs_or_error(query.0.as_deref())
            .ok()
            .map(|hm| hm.get("token").map(|v| v.to_owned()))
        {
            return decrypt_user_token(AUTH_COOKIE, &jar, password).map(|mut t| {
                t.xsrf_token = None;
                t
            });
        }

        // OR Try to get user_token from basic auth headers
        if let Ok(TypedHeader(Authorization(basic))) =
            <TypedHeader<Authorization<Basic>> as FromRequestParts<S>>::from_request_parts(
                parts, state,
            )
            .await
        {
            if let Ok(token) =
                decrypt_user_token(AUTH_COOKIE, &jar, basic.password()).map(|mut t| {
                    t.xsrf_token = None;
                    t
                })
            {
                return Ok(token);
            } else {
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
                    Ok(user) => Ok(user.1).map(|mut t| {
                        t.xsrf_token = None;
                        t
                    }),
                    Err(e) => {
                        #[cfg(target_os = "linux")]
                        if let Some(jail) = jail {
                            jail.report_failure(addr.0.ip());
                        }
                        Err((e.0, "no user found in basic auth"))
                    }
                };
            }
        }

        Err((
            StatusCode::UNAUTHORIZED,
            "xsrf token not provided or not matching",
        ))
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
    type Rejection = (StatusCode, &'static str);
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let user = <UserToken as FromRequestParts<S>>::from_request_parts(parts, state).await?;
        if !user.roles.contains(&ADMINS_ROLE.to_owned()) {
            return Err((StatusCode::UNAUTHORIZED, "user is not in admin group"));
        }
        Ok(AdminToken(user))
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
    user: Option<&UserToken>,
    target: &HostType,
    hostname: &str,
    path: &str,
    method: &http::Method,
) -> Result<(), Box<Response<Body>>> {
    if let Some(user) = user {
        if check_user_has_role(user, target.roles()) {
            match &user.share {
                None => return Ok(()),
                Some(share) => {
                    if share.hostname == hostname
                        && let Ok(decoded_path) = urlencoding::decode(path)
                    {
                        let decoded_path = PathBuf::from(decoded_path.to_string());
                        if share.path == decoded_path
                            || is_path_within_base(&decoded_path, &share.path)
                        {
                            if !share.writable
                                && method != http::Method::GET
                                && method != http::Method::HEAD
                                && method != http::Method::OPTIONS
                                && method.as_str() != "PROPFIND"
                            {
                                return Err(Box::new(forbidden()));
                            }
                            return Ok(());
                        }
                    }
                }
            }
        }
        return Err(Box::new(forbidden()));
    }
    Err(Box::new(
        Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header(&WWWAUTHENTICATE, r#"Basic realm="server""#)
            .body(Body::empty())
            .expect("cannot vary"),
    ))
}

fn forbidden() -> http::Response<Body> {
    Response::builder()
        .status(StatusCode::FORBIDDEN)
        .body(Body::empty())
        .expect("constant method")
}

pub fn check_authorization(
    app: &HostType,
    user: Option<&UserToken>,
    hostname: &str,
    path: &str,
    method: &http::Method,
) -> Result<(), Box<Response<Body>>> {
    if app.secured() {
        check_user_has_role_or_forbid(user, app, hostname, path, method)?;
    }
    Ok(())
}

pub fn authorized_or_redirect_to_login(
    app: &HostType,
    user: &Option<UserToken>,
    hostname: &str,
    req: &Request<Body>,
    config: &std::sync::Arc<crate::configuration::Config>,
) -> Result<(), Box<Response<Body>>> {
    let domain = hostname.split(':').next().unwrap_or_default();
    if let Err(mut value) =
        check_authorization(app, user.as_ref(), domain, req.uri().path(), req.method())
    {
        // Redirect to login page if user is not logged, write where to get back after login in a cookie
        if value.status() == StatusCode::UNAUTHORIZED
            && let Ok(mut hn) = HeaderValue::from_str(&config.full_domain())
        {
            *value.status_mut() = StatusCode::FOUND;
            // If single proxy mode, redirect directly to IdP without passing through atrium main app
            if config.single_proxy
                && let Ok(value) =
                    HeaderValue::from_str(&format!("{}/auth/oauth2login", config.full_domain()))
            {
                hn = value;
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
            if let Ok(header_value) = HeaderValue::from_str(&format!("{cookie}")) {
                value.headers_mut().append(SET_COOKIE, header_value);
            }
        }
        return Err(value);
    }
    Ok(())
}

#[cfg(test)]
mod check_user_has_role_or_forbid_tests {
    use crate::{
        apps::{App, AppWithUri},
        configuration::HostType,
        users::{UserToken, check_user_has_role_or_forbid},
    };

    #[test]
    fn test_no_user() {
        let user = None;
        let app: App = App {
            target: "www.example.com".to_string(), // to prevent failing when parsing url
            roles: vec!["role1".to_string(), "role2".to_string()],
            ..Default::default()
        };
        let app = AppWithUri::from_app(app, None);
        let target = HostType::ReverseApp(Box::new(app));
        assert!(check_user_has_role_or_forbid(user, &target, "", "", &http::Method::GET).is_err());
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
        assert!(
            check_user_has_role_or_forbid(Some(&user), &target, "", "", &http::Method::GET).is_ok()
        );
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
        assert!(
            check_user_has_role_or_forbid(Some(&user), &target, "", "", &http::Method::GET).is_ok()
        );
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
        assert!(
            check_user_has_role_or_forbid(Some(&user), &target, "", "", &http::Method::GET)
                .is_err()
        );
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
        assert!(
            check_user_has_role_or_forbid(Some(&user), &target, "", "", &http::Method::GET)
                .is_err()
        );
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
        assert!(
            check_user_has_role_or_forbid(Some(&user), &target, "", "", &http::Method::GET)
                .is_err()
        );
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
        assert!(
            check_user_has_role_or_forbid(Some(&user), &target, "", "", &http::Method::GET)
                .is_err()
        );
    }
}
