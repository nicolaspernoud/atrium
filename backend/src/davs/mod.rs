pub mod dav_file;
pub mod error;
pub(crate) mod headers;
pub mod model;
pub(crate) mod webdav_server;

use crate::configuration::HostType;
use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Request, Response},
};
use std::{net::SocketAddr, sync::LazyLock};

static WEBDAV_SERVER: LazyLock<webdav_server::WebdavServer> =
    LazyLock::new(webdav_server::WebdavServer::new);

pub async fn webdav_handler(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    host_type: HostType,
    mut req: Request<Body>,
) -> Response<Body> {
    // If the middleware modified the HostType, it's in the extensions
    let dav = if let Some(HostType::Dav(dav)) = req.extensions_mut().remove::<HostType>() {
        dav
    } else {
        match host_type {
            HostType::Dav(app) => app,
            _ => panic!("Service is not a dav !"),
        }
    };

    WEBDAV_SERVER.call(req, addr, &dav).await
}
