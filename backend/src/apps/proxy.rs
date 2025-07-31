// inspired by https://github.com/felipenoris/hyper-reverse-proxy

use axum::{body::Body, response::IntoResponse};
use http::{
    Uri,
    uri::{Authority, Scheme},
};
use hyper::{
    Request, Response, StatusCode,
    body::Incoming,
    header::{HeaderMap, HeaderName, HeaderValue},
    http::{
        header::{InvalidHeaderValue, ToStrError},
        uri::InvalidUri,
    },
    upgrade::OnUpgrade,
};
use hyper_util::client::legacy::Error;
use std::net::IpAddr;
use tokio::io::copy_bidirectional;
use tracing::debug;

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
    HyperError(hyper::Error),
    HyperClientError(Error),
    ForwardHeaderError,
    UpgradeError(String),
    BadRedirectResponseError,
    ClientError(&'static str),
}

impl From<Error> for ProxyError {
    fn from(err: Error) -> ProxyError {
        ProxyError::HyperClientError(err)
    }
}

impl From<hyper::Error> for ProxyError {
    fn from(err: hyper::Error) -> ProxyError {
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
            ProxyError::ClientError(s) => {
                (StatusCode::BAD_GATEWAY, format!("Proxy client error: {s}")).into_response()
            }
            _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    }
}

impl From<ProxyError> for Response<Body> {
    fn from(value: ProxyError) -> Self {
        value.into_response()
    }
}

fn remove_hop_headers(headers: &mut HeaderMap) {
    debug!("Removing hop headers");
    for header in &HOP_HEADERS {
        headers.remove(header);
    }
}

fn get_upgrade_type(headers: &HeaderMap) -> Option<String> {
    if let Some(value) = headers.get(&CONNECTION_HEADER)
        && let Ok(value) = value.to_str()
        && value.split(',').any(|e| e.trim() == UPGRADE_HEADER)
        && let Some(upgrade_value) = headers.get(&UPGRADE_HEADER)
        && let Ok(upgrade_value) = upgrade_value.to_str()
    {
        debug!("Found upgrade header with value: {}", upgrade_value);
        return Some(upgrade_value.to_lowercase());
    }
    None
}

fn remove_connection_headers(headers: &mut HeaderMap) {
    if let Some(value) = headers.get(&CONNECTION_HEADER)
        && let Ok(value) = value.to_str()
    {
        debug!("Removing connection headers");
        let names_to_remove: Vec<String> = value
            .split(',')
            .map(|name| name.trim())
            .filter(|name| !name.is_empty())
            .map(|name| name.to_string())
            .collect();
        for name in names_to_remove {
            headers.remove(&name);
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
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.split(',').any(|e| e.trim() == TRAILERS_HEADER));

    debug!("Setting headers of proxied request");

    let mut request_parts = request.uri().clone().into_parts();
    request_parts.scheme = Some(forward_scheme);
    request_parts.authority = Some(forward_authority.clone());
    *request.uri_mut() = Uri::from_parts(request_parts)
        .map_err(|_| ProxyError::ClientError("request uri is malformed"))?;

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

    if let Some(value) = upgrade_type
        && let Ok(value) = value.parse()
    {
        debug!("Repopulate upgrade headers");
        request.headers_mut().insert(&UPGRADE_HEADER, value);
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
            if let Ok(entry_str) = std::str::from_utf8(entry.get().as_bytes()) {
                addr.push_str(entry_str);
                addr.push(',');
                addr.push(' ');
                addr.push_str(&client_ip_str);
                entry.insert(addr.parse()?);
            }
        }
    }

    Ok(request)
}

pub async fn call<S>(
    client_ip: IpAddr,
    forward_scheme: Scheme,
    forward_authority: &Authority,
    mut request: Request<Body>,
    mut client: S,
) -> Result<Response<Incoming>, ProxyError>
where
    S: tower_service::Service<Request<Body>, Response = http::Response<Incoming>>,
    <S as tower_service::Service<Request<Body>>>::Error: std::fmt::Debug,
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
    let mut response = client.call(proxied_request).await.map_err(|_| {
        ProxyError::ClientError(
            "invalid TLS certificate, use option to skip verification if needed",
        )
    })?;

    if response.status() == StatusCode::SWITCHING_PROTOCOLS {
        let response_upgrade_type = get_upgrade_type(response.headers());

        if request_upgrade_type == response_upgrade_type {
            if let Some(request_upgraded) = request_upgraded {
                let response_upgraded = response
                    .extensions_mut()
                    .remove::<OnUpgrade>()
                    .expect("response does not have an upgrade extension")
                    .await?;

                debug!("Responding to a connection upgrade response");

                tokio::spawn(async move {
                    let request_upgraded =
                        request_upgraded.await.expect("failed to upgrade request");

                    let mut request_upgraded =
                        hyper_util::rt::tokio::TokioIo::new(request_upgraded);

                    let mut response_upgraded =
                        hyper_util::rt::tokio::TokioIo::new(response_upgraded);

                    if copy_bidirectional(&mut response_upgraded, &mut request_upgraded)
                        .await
                        .is_ok()
                    {
                        debug!("successfull copy between upgraded connections");
                    } else {
                        debug!(
                            "failed copy between upgraded connections (EOF), for client IP: {}",
                            client_ip
                        );
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
                "backend tried to switch to protocol {response_upgrade_type:?} when {request_upgrade_type:?} was requested"
            )))
        }
    } else {
        let proxied_response = create_proxied_response(response);
        debug!("Responding to call with response");
        Ok(proxied_response)
    }
}
