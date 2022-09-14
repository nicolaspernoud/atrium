use std::{net::SocketAddr, sync::Arc};

use axum::{
    body::{Body, Bytes},
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use maxminddb::{geoip2, Reader};

const UNKNOWN_CITY: &'static str = "unknown city";
const UNKNOWN_COUNTRY: &'static str = "unknown country";

pub fn city_from_ip(addr: SocketAddr, reader: Arc<Option<Reader<Vec<u8>>>>) -> String {
    let location = if addr.ip().is_loopback() {
        "localhost".to_owned()
    } else if addr.is_ipv4() && addr.ip().to_string().starts_with("192.168.") {
        "local network".to_owned()
    } else if reader.is_none() {
        "no geo ip database".to_owned()
    } else {
        match (*reader)
            .as_ref()
            .unwrap()
            .lookup::<geoip2::City>(addr.ip())
        {
            Ok(city) => format!(
                "{}, {}",
                city.city.map_or(UNKNOWN_CITY, |c| c
                    .names
                    .map_or(UNKNOWN_CITY, |n| n.get("en").unwrap_or(&UNKNOWN_CITY))),
                city.country.map_or(UNKNOWN_COUNTRY, |c| c
                    .names
                    .map_or(UNKNOWN_COUNTRY, |n| n.get("en").unwrap_or(&UNKNOWN_COUNTRY)))
            ),
            Err(_) => "unknown location".to_owned(),
        }
    };
    format!("{location} ({})", addr.ip())
}

pub async fn print_request_response(
    req: Request<Body>,
    next: Next<Body>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let (parts, body) = req.into_parts();
    tracing::debug!(
        "request = {} {} {:?}",
        parts.method,
        parts.uri,
        parts.headers
    );
    let bytes = buffer_and_print("request", body).await?;
    let req = Request::from_parts(parts, Body::from(bytes));

    let res = next.run(req).await;

    let (parts, body) = res.into_parts();
    tracing::debug!("response headers = {} {:?}", parts.status, parts.headers);
    let bytes = buffer_and_print("response", body).await?;
    let res = Response::from_parts(parts, Body::from(bytes));

    Ok(res)
}

async fn buffer_and_print<B>(direction: &str, body: B) -> Result<Bytes, (StatusCode, String)>
where
    B: axum::body::HttpBody<Data = Bytes>,
    B::Error: std::fmt::Display,
{
    let bytes = match hyper::body::to_bytes(body).await {
        Ok(bytes) => bytes,
        Err(err) => {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("failed to read {} body: {}", direction, err),
            ));
        }
    };

    if let Ok(body) = std::str::from_utf8(&bytes) {
        tracing::debug!("{} body = {:?}", direction, body);
    }

    Ok(bytes)
}
