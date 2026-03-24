use crate::{
    appstate::{ConfigState, MAXMIND_READER},
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
    response::Response,
};
use axum_extra::{
    TypedHeader,
    extract::cookie::{Cookie, SameSite},
};
use http::{
    HeaderValue, Method, StatusCode,
    header::{LOCATION, SET_COOKIE},
};
use std::{net::SocketAddr, path::PathBuf};
use tracing::info;

use super::user::{AdminToken, UserToken};

pub async fn auth_middleware(
    State(config): State<ConfigState>,
    host_type: HostType,
    host: Host,
    user: Option<UserToken>,
    req: Request,
    next: Next,
) -> Response {
    let hostname = host.as_str().to_owned();
    if let Err(res) = authorized_or_redirect_to_login(&host_type, &user, &hostname, &req, &config) {
        return *res;
    }
    next.run(req).await
}

pub async fn admin_auth_middleware(_admin: AdminToken, req: Request, next: Next) -> Response {
    next.run(req).await
}

pub async fn dav_auth_middleware(
    #[cfg(target_os = "linux")] State(jail): State<crate::OptionalJail>,
    mut host_type: HostType,
    host: Host,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    user: Option<UserToken>,
    mut req: Request,
    next: Next,
) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_owned();
    let hostname = host.hostname().to_owned();
    let query = req.uri().query().map(|q| q.to_owned());

    let log_str = format!(
        "{} \"{}{}\" by {} from {}",
        method,
        host_type.host(),
        path,
        user.as_ref().map_or_else(|| "unknown user", |u| &u.login),
        city_from_ip(addr, MAXMIND_READER.get())
    );

    if method != Method::OPTIONS
        && let Err(access_denied_resp) =
            check_authorization(&host_type, user.as_ref(), &hostname, &path)
    {
        #[cfg(target_os = "linux")]
        if let Some(jail) = jail {
            jail.report_failure(addr.ip());
        }
        info!("FILE ACCESS DENIED: {log_str}");
        return *access_denied_resp;
    }

    let unlogged_methods = [
        Method::OPTIONS,
        Method::HEAD,
        Method::from_bytes(b"LOCK").expect("infallible"),
        Method::from_bytes(b"UNLOCK").expect("infallible"),
        Method::from_bytes(b"PROPFIND").expect("infallible"),
    ];

    if !unlogged_methods.contains(&method) && query.as_deref().is_none_or(|q| q != "diskusage") {
        info!("FILE ACCESS: {log_str}");
    }

    // If we have a non writable share, alter the host so that is not writable
    if let Some(user) = &user
        && let Some(share) = &user.share
        && !share.writable
        && let HostType::Dav(dav) = &mut host_type
    {
        dav.writable = false;
        req.extensions_mut().insert(host_type);
    }

    next.run(req).await
}

pub async fn xsrf_middleware(
    xsrf_token: Option<TypedHeader<XSRFToken>>,
    user: Option<UserToken>,
    req: Request,
    next: Next,
) -> Result<Response, (StatusCode, &'static str)> {
    if let Some(user) = user
        && let Some(user_xsrf) = user.xsrf_token
        && xsrf_token.as_ref().map(|v| &v.0.0) != Some(&user_xsrf)
    {
        Err((
            StatusCode::FORBIDDEN,
            "xsrf token not provided or not matching",
        ))
    } else {
        Ok(next.run(req).await)
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
            .header("www-authenticate", r#"Basic realm="server""#)
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
) -> Result<(), Box<Response<Body>>> {
    if app.secured() {
        check_user_has_role_or_forbid(user, app, hostname, path)?;
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
    if let Err(mut value) = check_authorization(app, user.as_ref(), domain, req.uri().path()) {
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
        auth::middlewares::check_user_has_role_or_forbid,
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
        assert!(check_user_has_role_or_forbid(Some(&user), &target, "", "").is_ok());
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
        assert!(check_user_has_role_or_forbid(Some(&user), &target, "", "").is_ok());
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
        assert!(check_user_has_role_or_forbid(Some(&user), &target, "", "").is_err());
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
        assert!(check_user_has_role_or_forbid(Some(&user), &target, "", "").is_err());
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
        assert!(check_user_has_role_or_forbid(Some(&user), &target, "", "").is_err());
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
        assert!(check_user_has_role_or_forbid(Some(&user), &target, "", "").is_err());
    }
}
