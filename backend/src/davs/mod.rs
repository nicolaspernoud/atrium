pub(crate) mod dav_file;
pub(crate) mod headers;
pub mod model;
pub(crate) mod webdav_server;
use once_cell::sync::Lazy;

use crate::{
    appstate::MAXMIND_READER,
    configuration::HostType,
    logger::city_from_ip,
    users::{check_authorization, UserToken},
};
use axum::{
    extract::{ConnectInfo, Host},
    http::{Request, Response},
};
use http::Method;
use hyper::{Body, StatusCode};

use std::{net::SocketAddr, sync::Arc};
use tracing::info;

static WEBDAV_SERVER: Lazy<Arc<webdav_server::WebdavServer>> =
    Lazy::new(|| Arc::new(webdav_server::WebdavServer::new()));

static UNLOGGED_METHODS: Lazy<[Method; 5]> = Lazy::new(|| {
    [
        Method::OPTIONS,
        Method::HEAD,
        Method::from_bytes(b"LOCK").unwrap(),
        Method::from_bytes(b"UNLOCK").unwrap(),
        Method::from_bytes(b"PROPFIND").unwrap(),
    ]
});

pub async fn webdav_handler(
    user: Option<UserToken>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    dav: HostType,
    Host(hostname): Host,
    req: Request<Body>,
) -> Response<Body> {
    // Strings for logging
    let method = req.method().to_owned();
    let uri_str = req.uri().path().to_owned();
    let query_str = req.uri().query().unwrap_or_default().to_owned();
    let dav_host_str = dav.host().to_owned();
    let user_str = user
        .as_ref()
        .map(|u| u.login.to_owned())
        .unwrap_or_else(|| "unknown user".to_owned());

    let domain = hostname.split(':').next().unwrap_or_default();

    if method != Method::OPTIONS {
        if let Some(access_denied_resp) =
            check_authorization(&dav, &user.as_ref(), domain, req.uri().path())
        {
            tokio::spawn(async move {
                info!(
                    "FILE ACCESS DENIED: {} \"{}{}\" by {} from {}",
                    method,
                    dav_host_str,
                    uri_str,
                    user_str,
                    city_from_ip(addr, Arc::clone(&MAXMIND_READER))
                );
            });
            return access_denied_resp;
        }
    }

    let dav = match dav {
        HostType::Dav(app) => app,
        _ => panic!("Service is not a dav !"),
    };

    match WEBDAV_SERVER.clone().call(req, addr, &dav).await {
        Ok(response) => {
            if !UNLOGGED_METHODS.contains(&method) && query_str != "diskusage" {
                tokio::spawn(async move {
                    info!(
                        "FILE ACCESS: {} \"{}{}\" by {} from {}",
                        method,
                        dav_host_str,
                        uri_str,
                        user_str,
                        city_from_ip(addr, Arc::clone(&MAXMIND_READER))
                    );
                });
            }
            response
        }
        Err(_) => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::empty())
            .unwrap(),
    }
}
