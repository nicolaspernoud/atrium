// inspired by https://github.com/felipenoris/hyper-reverse-proxy

use axum::response::IntoResponse;
use http::uri::{Authority, Scheme};
use http::Uri;
use hyper::header::{HeaderMap, HeaderName, HeaderValue};
use hyper::http::header::{InvalidHeaderValue, ToStrError};
use hyper::http::uri::InvalidUri;
use hyper::upgrade::OnUpgrade;
use hyper::{Body, Error, Request, Response, StatusCode};
use std::net::IpAddr;
use tokio::io::copy_bidirectional;
use tracing::debug;

use crate::appstate::{Client, InsecureSkipVerifyClient};

static CONNECTION_HEADER: HeaderName = HeaderName::from_static("connection");
static TE_HEADER: HeaderName = HeaderName::from_static("te");
static UPGRADE_HEADER: HeaderName = HeaderName::from_static("upgrade");
static TRAILERS_HEADER: HeaderName = HeaderName::from_static("trailers");
static HOP_HEADERS: [HeaderName; 9] = [
    HeaderName::from_static("connection"),
    HeaderName::from_static("te"),
    HeaderName::from_static("trailer"),
    HeaderName::from_static("keep-alive"),
    HeaderName::from_static("proxy-connection"),
    HeaderName::from_static("proxy-authenticate"),
    HeaderName::from_static("proxy-authorization"),
    HeaderName::from_static("transfer-encoding"),
    HeaderName::from_static("upgrade"),
];

static X_FORWARDED_FOR: HeaderName = HeaderName::from_static("x-forwarded-for");

#[derive(Debug)]
pub enum ProxyError {
    InvalidUri(InvalidUri),
    HyperError(Error),
    ForwardHeaderError,
    UpgradeError(String),
    BadRedirectResponseError,
}

impl From<Error> for ProxyError {
    fn from(err: Error) -> ProxyError {
        ProxyError::HyperError(err)
    }
}

impl From<InvalidUri> for ProxyError {
    fn from(err: InvalidUri) -> ProxyError {
        ProxyError::InvalidUri(err)
    }
}

impl From<ToStrError> for ProxyError {
    fn from(_err: ToStrError) -> ProxyError {
        ProxyError::ForwardHeaderError
    }
}

impl From<InvalidHeaderValue> for ProxyError {
    fn from(_err: InvalidHeaderValue) -> ProxyError {
        ProxyError::ForwardHeaderError
    }
}

impl IntoResponse for ProxyError {
    fn into_response(self) -> axum::response::Response {
        match self {
            ProxyError::ForwardHeaderError => StatusCode::BAD_GATEWAY.into_response(),
            ProxyError::UpgradeError(s) => (StatusCode::BAD_GATEWAY, s).into_response(),
            _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    }
}

fn remove_hop_headers(headers: &mut HeaderMap) {
    debug!("Removing hop headers");
    for header in &HOP_HEADERS {
        headers.remove(header);
    }
}

fn get_upgrade_type(headers: &HeaderMap) -> Option<String> {
    #[allow(clippy::blocks_in_if_conditions)]
    if headers
        .get(&CONNECTION_HEADER)
        .map(|value| {
            value
                .to_str()
                .unwrap()
                .split(',')
                .any(|e| e.trim() == UPGRADE_HEADER)
        })
        .unwrap_or(false)
    {
        if let Some(upgrade_value) = headers.get(&UPGRADE_HEADER) {
            debug!(
                "Found upgrade header with value: {}",
                upgrade_value.to_str().unwrap().to_owned()
            );
            return Some(upgrade_value.to_str().unwrap().to_lowercase());
        }
    }

    None
}

fn remove_connection_headers(headers: &mut HeaderMap) {
    if headers.get(&CONNECTION_HEADER).is_some() {
        debug!("Removing connection headers");

        let value = headers.get(&CONNECTION_HEADER).cloned().unwrap();

        for name in value.to_str().unwrap().split(',') {
            if !name.trim().is_empty() {
                headers.remove(name.trim());
            }
        }
    }
}

fn create_proxied_response<B>(mut response: Response<B>) -> Response<B> {
    debug!("Creating proxied response");
    remove_hop_headers(response.headers_mut());
    remove_connection_headers(response.headers_mut());
    response
}

fn create_proxied_request<B>(
    client_ip: IpAddr,
    forward_scheme: Scheme,
    forward_authority: &Authority,
    mut request: Request<B>,
    upgrade_type: Option<&String>,
) -> Result<Request<B>, ProxyError> {
    debug!("Creating proxied request");

    let contains_te_trailers_value = request
        .headers()
        .get(&TE_HEADER)
        .map(|value| {
            value
                .to_str()
                .unwrap()
                .split(',')
                .any(|e| e.trim() == TRAILERS_HEADER)
        })
        .unwrap_or(false);

    debug!("Setting headers of proxied request");

    let mut request_parts = request.uri().clone().into_parts();
    request_parts.scheme = Some(forward_scheme);
    request_parts.authority = Some(forward_authority.clone());
    *request.uri_mut() = Uri::from_parts(request_parts).unwrap();

    // Downgrade to HTTP/1.1 to be compatible with any website
    *request.version_mut() = http::Version::HTTP_11;

    remove_hop_headers(request.headers_mut());
    remove_connection_headers(request.headers_mut());

    if contains_te_trailers_value {
        debug!("Setting up trailer headers");

        request
            .headers_mut()
            .insert(&TE_HEADER, HeaderValue::from_static("trailers"));
    }

    if let Some(value) = upgrade_type {
        debug!("Repopulate upgrade headers");
        request
            .headers_mut()
            .insert(&UPGRADE_HEADER, value.parse().unwrap());
        request
            .headers_mut()
            .insert(&CONNECTION_HEADER, HeaderValue::from_static("UPGRADE"));
    }

    // Add forwarding information in the headers
    match request.headers_mut().entry(&X_FORWARDED_FOR) {
        hyper::header::Entry::Vacant(entry) => {
            debug!("X-Forwarded-For header was vacant");
            entry.insert(client_ip.to_string().parse()?);
        }

        hyper::header::Entry::Occupied(mut entry) => {
            debug!("X-Forwarded-For header was occupied");
            let client_ip_str = client_ip.to_string();
            let mut addr =
                String::with_capacity(entry.get().as_bytes().len() + 2 + client_ip_str.len());
            addr.push_str(std::str::from_utf8(entry.get().as_bytes()).unwrap());
            addr.push(',');
            addr.push(' ');
            addr.push_str(&client_ip_str);
            entry.insert(addr.parse()?);
        }
    }

    Ok(request)
}

pub async fn call<S>(
    client_ip: IpAddr,
    forward_scheme: Scheme,
    forward_authority: &Authority,
    mut request: Request<Body>,
    client: S,
) -> Result<Response<Body>, ProxyError>
where
    S: HyperClient,
{
    debug!(
        "Received proxy call from {} to {}, client: {}",
        request.uri().to_string(),
        forward_authority,
        client_ip
    );

    let request_upgrade_type = get_upgrade_type(request.headers());
    let request_upgraded = request.extensions_mut().remove::<OnUpgrade>();

    let proxied_request = create_proxied_request(
        client_ip,
        forward_scheme,
        forward_authority,
        request,
        request_upgrade_type.as_ref(),
    )?;

    //////////////////////////////////////////////
    // UNCOMMENT THIS FOR FULL REQUEST LOGGING //
    ////////////////////////////////////////////
    /*
        let (parts, body) = proxied_request.into_parts();
        debug!(
            "proxied request = {} {} {:?}",
            parts.method, parts.uri, parts.headers
        );
        let bytes = hyper::body::to_bytes(body)
            .await
            .expect("could not get body data");
        if let Ok(body) = std::str::from_utf8(&bytes) {
            debug!("proxied request body = {:?}", body);
            //std::fs::write("./request_body.xml", body).expect("Unable to write file");
        }
        let proxied_request = Request::from_parts(parts, Body::from(bytes));
    */
    let mut response = client.request(proxied_request).await?;

    if response.status() == StatusCode::SWITCHING_PROTOCOLS {
        let response_upgrade_type = get_upgrade_type(response.headers());

        if request_upgrade_type == response_upgrade_type {
            if let Some(request_upgraded) = request_upgraded {
                let mut response_upgraded = response
                    .extensions_mut()
                    .remove::<OnUpgrade>()
                    .expect("response does not have an upgrade extension")
                    .await?;

                debug!("Responding to a connection upgrade response");

                tokio::spawn(async move {
                    let mut request_upgraded =
                        request_upgraded.await.expect("failed to upgrade request");

                    match copy_bidirectional(&mut response_upgraded, &mut request_upgraded).await {
                        Ok(_) => debug!("successfull copy between upgraded connections"),
                        Err(_) => debug!(
                            "failed copy between upgraded connections (EOF), for client IP: {}",
                            client_ip
                        ),
                    }
                });
                Ok(response)
            } else {
                Err(ProxyError::UpgradeError(
                    "request does not have an upgrade extension".to_string(),
                ))
            }
        } else {
            Err(ProxyError::UpgradeError(format!(
                "backend tried to switch to protocol {:?} when {:?} was requested",
                response_upgrade_type, request_upgrade_type
            )))
        }
    } else {
        let proxied_response = create_proxied_response(response);
        debug!("Responding to call with response");
        Ok(proxied_response)
    }
}

pub trait HyperClient {
    fn request(&self, req: Request<Body>) -> hyper::client::ResponseFuture;
}

impl HyperClient for Client {
    fn request(&self, req: Request<Body>) -> hyper::client::ResponseFuture {
        self.request(req)
    }
}

impl HyperClient for InsecureSkipVerifyClient {
    fn request(&self, req: Request<Body>) -> hyper::client::ResponseFuture {
        self.request(req)
    }
}
