use axum::{
    extract::{ConnectInfo, Host, Path},
    http::{
        uri::{Authority, Scheme},
        Request, Response,
    },
    response::IntoResponse,
    Extension, Json,
};
use base64ct::Encoding;
use headers::HeaderValue;
use http::{header::AUTHORIZATION, Version};
use hyper::{
    header::{HOST, LOCATION},
    Body, StatusCode, Uri,
};
use hyper_reverse_proxy::ReverseProxy;
use hyper_trust_dns::{RustlsHttpsConnector, TrustDnsResolver};
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, sync::Arc};
use tracing::{debug, error};

use crate::{
    configuration::{config_or_error, Config, ConfigFile, HostType},
    users::{check_authorization, AdminToken, UserTokenWithoutXSRFCheck},
    utils::{is_default, option_vec_trim_remove_empties, string_trim, vec_trim_remove_empties},
};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct App {
    pub id: usize,
    #[serde(deserialize_with = "string_trim")]
    pub name: String,
    pub icon: usize,
    pub color: usize,
    #[serde(default, skip_serializing_if = "is_default")]
    pub is_proxy: bool,
    #[serde(deserialize_with = "string_trim")]
    pub host: String,
    #[serde(deserialize_with = "string_trim")]
    pub target: String,
    #[serde(default, skip_serializing_if = "is_default")]
    pub secured: bool,
    #[serde(
        default,
        skip_serializing_if = "is_default",
        deserialize_with = "string_trim"
    )]
    pub login: String,
    #[serde(
        default,
        skip_serializing_if = "is_default",
        deserialize_with = "string_trim"
    )]
    pub password: String,
    #[serde(
        default,
        skip_serializing_if = "is_default",
        deserialize_with = "string_trim"
    )]
    pub openpath: String,
    #[serde(
        default,
        skip_serializing_if = "is_default",
        deserialize_with = "vec_trim_remove_empties"
    )]
    pub roles: Vec<String>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub inject_security_headers: bool,
    #[serde(
        default,
        skip_serializing_if = "is_default",
        deserialize_with = "option_vec_trim_remove_empties"
    )]
    pub subdomains: Option<Vec<String>>,
}

#[derive(PartialEq, Debug, Clone)]
pub struct AppWithUri {
    pub inner: App,
    pub app_scheme: Scheme,
    pub app_authority: Authority,
    pub forward_uri: Uri,
    pub forward_scheme: Scheme,
    pub forward_authority: Authority,
    pub forward_host: String,
}

impl AppWithUri {
    pub fn from_app_domain_and_http_port(inner: App, domain: &str, port: Option<u16>) -> Self {
        let app_scheme = if port.is_some() {
            Scheme::HTTP
        } else {
            Scheme::HTTPS
        };
        let mut app_authority = if inner.host.contains(domain) {
            inner.host.clone()
        } else {
            format!("{}.{}", inner.host, domain)
        };
        if let Some(port) = port {
            app_authority.push_str(&format!(":{}", port));
        }
        let app_authority = app_authority
            .parse()
            .expect("could not work out authority from app configuration");
        let forward_scheme = if inner.target.starts_with("https://") {
            Scheme::HTTPS
        } else {
            Scheme::HTTP
        };
        let forward_uri: Uri = inner
            .target
            .parse()
            .expect("could not parse app target service");
        let mut forward_parts = forward_uri.into_parts();
        let forward_authority = forward_parts
            .authority
            .clone()
            .expect("could not parse app target service host");

        let forward_host = forward_authority.host().to_owned();
        forward_parts.scheme = Some(forward_scheme.clone());
        forward_parts.path_and_query = Some("/".parse().unwrap());
        let forward_uri = Uri::from_parts(forward_parts).unwrap();
        Self {
            inner,
            app_scheme,
            app_authority,
            forward_uri,
            forward_scheme,
            forward_authority,
            forward_host,
        }
    }
}

lazy_static::lazy_static! {
    static ref  PROXY_CLIENT: ReverseProxy<RustlsHttpsConnector> = {
        ReverseProxy::new(
            hyper::Client::builder().build::<_, hyper::Body>(TrustDnsResolver::default().into_rustls_webpki_https_connector()),
        )
    };
}

pub async fn proxy_handler(
    user: Option<UserTokenWithoutXSRFCheck>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    app: HostType,
    Host(hostname): Host,
    mut req: Request<Body>,
) -> Response<Body> {
    // Downgrade to HTTP/1.1 to be compatible with any website
    *req.version_mut() = Version::HTTP_11;
    let domain = hostname.split(":").next().unwrap_or_default();
    if let Some(value) = check_authorization(&app, &user.map(|u| u.0), domain, req.uri().path()) {
        return value;
    }

    let app = match app {
        HostType::ReverseApp(app) => app,
        _ => panic!("Service is not an app !"),
    };

    // If the target service contains no port, is to an external service and we need to rewrite the request to fool the target site
    if app.forward_authority.port().is_none() {
        let uri = req.uri_mut();
        let mut parts = uri.clone().into_parts();
        parts.scheme = Some(app.forward_scheme);
        if let Some(port) = &app.forward_authority.port() {
            parts.authority = Some(format!("{}:{}", app.forward_host, port).parse().unwrap());
        } else {
            parts.authority = Some(app.forward_host.parse().unwrap());
        }
        *uri = Uri::from_parts(parts).unwrap();
        req.headers_mut().insert(
            HOST,
            HeaderValue::from_str(&app.forward_authority.to_string()).unwrap(),
        );
    } else {
        // else we inform the app that we are proxying to it
        req.headers_mut().insert(
            "X-Forwarded-Host",
            HeaderValue::from_str(&app.app_authority.to_string()).unwrap(),
        );
        req.headers_mut().insert(
            "X-Forwarded-Proto",
            HeaderValue::from_str(&app.app_scheme.to_string()).unwrap(),
        );
    }

    // If the app contains basic auth information, forge a basic auth header
    if app.inner.login != "" && app.inner.password != "" {
        let bauth = format!("{}:{}", app.inner.login, app.inner.password);
        req.headers_mut().insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!(
                "Basic {}",
                base64ct::Base64::encode_string(bauth.as_bytes())
            ))
            .unwrap(),
        );
    }

    match PROXY_CLIENT
        .call(addr.ip(), &app.forward_uri.to_string(), req)
        .await
    {
        Ok(mut response) => {
            // If the response contains a location, alter the redirect location if the redirection is relative to the proxied host

            if let Some(location) = response.headers().get("location") {
                // parse location as an url
                let location_uri: Uri =
                    match location.to_str().unwrap().trim_start_matches(".").parse() {
                        Ok(uri) => uri,
                        Err(e) => {
                            error!("Proxy uri parse error : {:?}", e);
                            return Response::builder()
                                .status(StatusCode::INTERNAL_SERVER_ERROR)
                                .body(Body::empty())
                                .unwrap();
                        }
                    };
                // test if the host of this url contains the target service host
                if location_uri.host().is_some()
                    && location_uri.host().unwrap().contains(&app.forward_host)
                {
                    // if so, replace the target service host with the front service host
                    let mut parts = location_uri.into_parts();
                    parts.scheme = Some(app.app_scheme);
                    parts.authority = Some(app.app_authority);
                    let uri = Uri::from_parts(parts).unwrap();

                    response
                        .headers_mut()
                        .insert(LOCATION, HeaderValue::from_str(&uri.to_string()).unwrap());
                }
            }
            response
        }
        Err(e) => {
            debug!("Proxy error: {:?}", e);
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .unwrap()
        }
    }
}

pub async fn get_apps(
    Extension(config_file): Extension<ConfigFile>,
    _admin: AdminToken,
) -> Result<Json<Vec<App>>, (StatusCode, &'static str)> {
    let config = config_or_error(&config_file).await?;
    // Return all the apps as Json
    Ok(Json(config.apps))
}

pub async fn delete_app(
    Extension(config_file): Extension<ConfigFile>,
    _admin: AdminToken,
    Path(app_id): Path<(String, usize)>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let mut config = config_or_error(&config_file).await?;
    // Find the app
    if let Some(pos) = config.apps.iter().position(|a| a.id == app_id.1) {
        // It is an existing app, delete it
        config.apps.remove(pos);
    } else {
        // If the app doesn't exist, respond with an error
        return Err((StatusCode::BAD_REQUEST, "app doesn't exist"));
    }

    config
        .to_file_or_internal_server_error(&config_file)
        .await?;

    Ok((StatusCode::OK, "app deleted successfully"))
}

pub async fn add_app(
    config_file: Extension<ConfigFile>,
    Extension(config): Extension<Arc<Config>>,
    _admin: AdminToken,
    Json(payload): Json<App>,
) -> Result<(StatusCode, &'static str), (StatusCode, &'static str)> {
    // Clone the config
    let mut config = (*config).clone();
    // Find the app
    if let Some(app) = config.apps.iter_mut().find(|a| a.id == payload.id) {
        *app = payload;
    } else {
        config.apps.push(payload);
    }

    config
        .to_file_or_internal_server_error(&config_file)
        .await?;

    Ok((StatusCode::CREATED, "app created or updated successfully"))
}
