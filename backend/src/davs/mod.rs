pub(crate) mod dav_file;
pub(crate) mod headers;
pub mod model;
pub(crate) mod webdav_server;

use crate::{
    configuration::HostType,
    logger::city_from_ip,
    users::{check_authorization, UserToken},
};
use axum::{
    extract::{ConnectInfo, Host},
    http::{Request, Response},
    Extension,
};
use http::Method;
use hyper::{Body, StatusCode};
use maxminddb::Reader;
use std::{net::SocketAddr, sync::Arc};
use tracing::info;

lazy_static::lazy_static! {
    static ref  WEBDAV_SERVER: Arc<webdav_server::WebdavServer> = {
        Arc::new(webdav_server::WebdavServer::new(

        ))
    };
}

pub async fn webdav_handler(
    Extension(reader): Extension<Arc<Option<Reader<Vec<u8>>>>>,
    user: Option<UserToken>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    dav: HostType,
    Host(hostname): Host,
    req: Request<Body>,
) -> Response<Body> {
    // Strings for logging
    let method = req.method().to_owned();
    let uri_str = req.uri().to_string();
    let dav_host_str = dav.host().to_owned();
    let user_str = user
        .as_ref()
        .map(|u| u.login.to_owned())
        .unwrap_or("unknown user".to_owned());

    let domain = hostname.split(":").next().unwrap_or_default();

    if method != Method::OPTIONS {
        if let Some(access_denied_resp) = check_authorization(&dav, &user, domain, req.uri().path())
        {
            tokio::spawn(async move {
                info!(
                    "FILE ACCESS DENIED: {} \"{}{}\" by {} from {}",
                    method,
                    dav_host_str,
                    uri_str,
                    user_str,
                    city_from_ip(addr, reader)
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
            if method != Method::OPTIONS && method != Method::from_bytes(b"PROPFIND").unwrap() {
                tokio::spawn(async move {
                    info!(
                        "FILE ACCESS: {} \"{}{}\" by {} from {}",
                        method,
                        dav_host_str,
                        uri_str,
                        user_str,
                        city_from_ip(addr, reader)
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
