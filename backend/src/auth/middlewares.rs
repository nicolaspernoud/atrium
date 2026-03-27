use super::user::UserToken;
use crate::{
    appstate::{ConfigState, MAXMIND_READER},
    auth::{AUTH_COOKIE, cookie_user::CookieUserToken},
    configuration::HostType,
    extract::Host,
    headers::XSRFToken,
    logger::city_from_ip,
    utils::is_path_within_base,
};
use axum::{
    body::Body,
    extract::{ConnectInfo, Request, State},
    middleware::Next,
    response::{IntoResponse, Response},
};
use axum_extra::{
    TypedHeader,
    extract::{
        PrivateCookieJar,
        cookie::{Cookie, SameSite},
    },
};
use http::{
    HeaderValue, Method, StatusCode,
    header::{LOCATION, SET_COOKIE},
};
use std::{net::SocketAddr, path::PathBuf};
use tracing::info;

#[derive(Debug, Clone, Copy)]
pub enum AuthError {
    Unauthorized,
    Forbidden,
}

pub async fn auth_middleware(
    State(config): State<ConfigState>,
    host_type: HostType,
    host: Host,
    user: Option<CookieUserToken>,
    req: Request,
    next: Next,
) -> Response {
    if host_type.secured() {
        let hostname = host.as_str();
        let domain = hostname.split(':').next().unwrap_or_default();
        match check_user_role_and_share(
            user.map(|u| u.into()).as_ref(),
            &host_type,
            domain,
            req.uri().path(),
        ) {
            Ok(_) => {}
            Err(AuthError::Forbidden) => return StatusCode::FORBIDDEN.into_response(),
            Err(AuthError::Unauthorized) => {
                let mut res = StatusCode::FOUND.into_response();

                let mut login_url = config.full_domain();
                if config.single_proxy {
                    login_url = format!("{}/auth/oauth2login", config.full_domain());
                }

                if let Ok(hn) = HeaderValue::from_str(&login_url) {
                    res.headers_mut().append(LOCATION, hn);
                }

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
                    res.headers_mut().append(SET_COOKIE, header_value);
                }
                return res;
            }
        }
    }
    next.run(req).await
}

pub async fn dav_auth_middleware(
    #[cfg(target_os = "linux")] State(jail): State<crate::OptionalJail>,
    mut app: HostType,
    host: Host,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    user: Option<UserToken>,
    mut req: Request,
    next: Next,
) -> Response {
    let method = req.method();
    let path = req.uri().path();
    let query = req.uri().query();

    let log_str = format!(
        "{} \"{}{}\" by {} from {}",
        method,
        app.host(),
        path,
        user.as_ref().map_or_else(|| "unknown user", |u| &u.login),
        city_from_ip(addr, MAXMIND_READER.get())
    );

    if method != Method::OPTIONS
        && app.secured()
        && let Err(err) = check_user_role_and_share(user.as_ref(), &app, host.hostname(), path)
    {
        #[cfg(target_os = "linux")]
        if let Some(jail) = jail {
            jail.report_failure(addr.ip());
        }
        info!("FILE ACCESS DENIED: {log_str}");
        return match err {
            AuthError::Forbidden => StatusCode::FORBIDDEN.into_response(),
            AuthError::Unauthorized => Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .header("www-authenticate", r#"Basic realm="server""#)
                .body(Body::empty())
                .expect("cannot fail")
                .into_response(),
        };
    }

    let unlogged_methods = [
        Method::OPTIONS,
        Method::HEAD,
        Method::from_bytes(b"LOCK").expect("infallible"),
        Method::from_bytes(b"UNLOCK").expect("infallible"),
        Method::from_bytes(b"PROPFIND").expect("infallible"),
    ];

    if !unlogged_methods.contains(method) && query.is_none_or(|q| q != "diskusage") {
        info!("FILE ACCESS: {log_str}");
    }

    // If we have a non writable share, alter the host so that is not writable
    if let Some(user) = &user
        && let Some(share) = &user.share
        && !share.writable
        && let HostType::Dav(dav) = &mut app
    {
        dav.writable = false;
        req.extensions_mut().insert(app);
    }

    next.run(req).await
}

pub async fn xsrf_middleware(
    xsrf_token: Option<TypedHeader<XSRFToken>>,
    user: Option<UserToken>,
    State(config): State<ConfigState>,
    jar: PrivateCookieJar,
    req: Request,
    next: Next,
) -> Response {
    if let Some(user) = user
        && let Some(user_xsrf) = user.xsrf_token
        && xsrf_token.as_ref().map(|v| &v.0.0) != Some(&user_xsrf)
    {
        (
            jar.remove(
                Cookie::build(AUTH_COOKIE)
                    .domain(config.domain.clone())
                    .path("/")
                    .same_site(axum_extra::extract::cookie::SameSite::Lax)
                    .secure(config.tls_mode.is_secure())
                    .http_only(true)
                    .build(),
            ),
            (
                StatusCode::FORBIDDEN,
                "xsrf token not provided or not matching",
            ),
        )
            .into_response()
    } else {
        next.run(req).await
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

pub fn check_user_role_and_share(
    user: Option<&UserToken>,
    target: &HostType,
    hostname: &str,
    path: &str,
) -> Result<(), AuthError> {
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
                            return Ok(());
                        }
                    }
                }
            }
        }
        return Err(AuthError::Forbidden);
    }
    Err(AuthError::Unauthorized)
}

#[cfg(test)]
mod check_user_has_role_or_forbid_tests {
    use crate::{
        apps::{App, AppWithUri},
        auth::middlewares::check_user_role_and_share,
        auth::user::UserToken,
        configuration::HostType,
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
        let res = check_user_role_and_share(user, &target, "", "");
        assert!(matches!(res, Err(super::AuthError::Unauthorized)));
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
        assert!(check_user_role_and_share(Some(&user), &target, "", "").is_ok());
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
        assert!(check_user_role_and_share(Some(&user), &target, "", "").is_ok());
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
        let res = check_user_role_and_share(Some(&user), &target, "", "");
        assert!(matches!(res, Err(super::AuthError::Forbidden)));
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
        let res = check_user_role_and_share(Some(&user), &target, "", "");
        assert!(matches!(res, Err(super::AuthError::Forbidden)));
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
        let res = check_user_role_and_share(Some(&user), &target, "", "");
        assert!(matches!(res, Err(super::AuthError::Forbidden)));
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
        let res = check_user_role_and_share(Some(&user), &target, "", "");
        assert!(matches!(res, Err(super::AuthError::Forbidden)));
    }
}
