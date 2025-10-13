pub mod dav_file;
pub mod error;
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
use std::{net::SocketAddr, sync::LazyLock};
use tracing::info;

static WEBDAV_SERVER: LazyLock<webdav_server::WebdavServer> =
    LazyLock::new(webdav_server::WebdavServer::new);

static UNLOGGED_METHODS: LazyLock<[Method; 5]> = LazyLock::new(|| {
    [
        Method::OPTIONS,
        Method::HEAD,
        Method::from_bytes(b"LOCK").expect("infallible"),
        Method::from_bytes(b"UNLOCK").expect("infallible"),
        Method::from_bytes(b"PROPFIND").expect("infallible"),
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

    if method != Method::OPTIONS
        && let Err(access_denied_resp) =
            check_authorization(&dav, user.as_ref(), domain, req.uri().path())
    {
        tokio::spawn(async move {
            info!("FILE ACCESS DENIED: {log_str}");
        });
        return *access_denied_resp;
    }

    let dav = match dav {
        HostType::Dav(app) => app,
        _ => panic!("Service is not a dav !"),
    };

    if !UNLOGGED_METHODS.contains(&method) && req.uri().query().is_none_or(|q| q != "diskusage") {
        tokio::spawn(async move {
            info!("FILE ACCESS: {log_str}");
        });
    };
    WEBDAV_SERVER.call(req, addr, &dav).await
}
