pub mod dav_file;
pub mod error;
pub(crate) mod headers;
pub mod model;
pub(crate) mod webdav_server;

use crate::{configuration::HostType, users::UserToken};
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
    dav: HostType,
    user: Option<UserToken>,
    req: Request<Body>,
) -> Response<Body> {
    let mut dav = match dav {
        HostType::Dav(app) => app,
        _ => panic!("Service is not a dav !"),
    };

    // If we have a non writable share, alter the host so that is not writable
    if let Some(user) = user
        && let Some(share) = user.share
        && !share.writable
    {
        dav.writable = false;
    }

    WEBDAV_SERVER.call(req, addr, &dav).await
}
