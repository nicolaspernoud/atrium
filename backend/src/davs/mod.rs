pub(crate) mod dav_file;
pub(crate) mod headers;
pub mod model;
pub(crate) mod webdav_server;

use crate::{
    appstate::MAXMIND_READER,
    configuration::HostType,
    logger::city_from_ip,
    users::{UserToken, check_authorization},
};
use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Request, Response},
};
use axum_extra::extract::Host;
use http::Method;
use hyper::StatusCode;
use std::{net::SocketAddr, sync::LazyLock};
use tracing::info;

static WEBDAV_SERVER: LazyLock<webdav_server::WebdavServer> =
    LazyLock::new(webdav_server::WebdavServer::new);

static UNLOGGED_METHODS: LazyLock<[Method; 5]> = LazyLock::new(|| {
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
    let method = req.method().to_owned();
    let log_str = format!(
        "{} \"{}{}\" by {} from {}",
        req.method(),
        dav.host(),
        req.uri().path(),
        user.as_ref().map_or_else(|| "unknown user", |u| &u.login),
        city_from_ip(addr, MAXMIND_READER.get())
    );
    let domain = hostname.split(':').next().unwrap_or_default();

    if method != Method::OPTIONS {
        if let Err(access_denied_resp) =
            check_authorization(&dav, user.as_ref(), domain, req.uri().path())
        {
            tokio::spawn(async move {
                info!("FILE ACCESS DENIED: {log_str}");
            });
            return access_denied_resp;
        }
    }

    let dav = match dav {
        HostType::Dav(app) => app,
        _ => panic!("Service is not a dav !"),
    };

    let query_str = req.uri().query().unwrap_or_default().to_owned();
    match WEBDAV_SERVER.call(req, addr, &dav).await {
        Ok(response) => {
            if !UNLOGGED_METHODS.contains(&method) && query_str != "diskusage" {
                tokio::spawn(async move {
                    info!("FILE ACCESS: {log_str}");
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
