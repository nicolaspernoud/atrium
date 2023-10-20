use crate::appstate::OptionalMaxMindReader;
use axum::{
    body::{Body, Bytes},
    extract::ConnectInfo,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use maxminddb::geoip2;
use std::net::SocketAddr;
// use tokio::io::AsyncWriteExt;

const UNKNOWN_CITY: &str = "unknown city";
const UNKNOWN_COUNTRY: &str = "unknown country";

pub fn city_from_ip(addr: SocketAddr, reader: OptionalMaxMindReader) -> String {
    let location = if addr.ip().is_loopback() {
        "localhost".to_owned()
    } else if addr.is_ipv4() && addr.ip().to_string().starts_with("192.168.") {
        "local network".to_owned()
    } else if let Some(reader) = reader {
        match reader.lookup::<geoip2::City>(addr.ip()) {
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
    } else {
        "unknown location (no geo ip database)".to_owned()
    };
    format!("{location} ({})", addr.ip())
}

#[tracing::instrument(name = "Request", level = "debug", skip_all, fields(ip=%addr, uri = %req.uri(), method = %req.method()))]
pub async fn print_request_response(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request<Body>,
    next: Next<Body>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let (parts, body) = req.into_parts();

    let body = buffer_body(body).await?;
    tracing::debug!(
        "\nREQUEST\n⇨ headers: {:?}\n⇨ body: {}",
        parts.headers,
        body.0
    );

    /* UNCOMMENT THIS TO WRITE TESTS ALMOST AUTOMATICALLY
    let request_to_test = format!(
        r#"
    #[tokio::test]
    async fn test_() -> Result<()> {{
        let app = TestApp::spawn(None).await;
        let url = format!("http://{}:{{}}{}", app.port);
        let resp = app
        .client
        .request(
            {},
            url,
        )
        {}
        .body(b"{}".to_vec()).send().await?;
    "#,
        parts
            .headers
            .get("host")
            .unwrap()
            .to_str()
            .unwrap()
            .split(":")
            .collect::<Vec<&str>>()[0],
        parts.uri,
        parts.method,
        parts
            .headers
            .iter()
            .map(|h| { format!(".header({:?}, {:?})", h.0, h.1) })
            .collect::<Vec<String>>()
            .join("\n"),
        body.0
    );

    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .append(true)
        .create(true)
        .open("my_test.rs")
        .await
        .unwrap();
    file.write_all(request_to_test.as_bytes()).await.unwrap();
    */

    let req = Request::from_parts(parts, Body::from(body.1));

    let res = next.run(req).await;

    let (parts, body) = res.into_parts();
    let body = buffer_body(body).await?;
    tracing::debug!(
        "\nRESPONSE\n⇨ status: {}\n⇨ headers: {:?}\n⇨ body: {}",
        parts.status,
        parts.headers,
        body.0
    );

    /* UNCOMMENT THIS TO WRITE TESTS ALMOST AUTOMATICALLY
    let response_to_test = format!(
        r#"
        assert_eq!(resp.status(), {});
        {}
        let body = resp.text().await?;
        assert!(body.contains("{}"));
        Ok(())
    }}
    "#,
        parts.status.as_u16(),
        parts
            .headers
            .iter()
            .map(|h| {
                format!(
                    "assert_eq!(resp.headers().get({:?}).unwrap().to_str().unwrap(), {:?});",
                    h.0, h.1
                )
            })
            .collect::<Vec<String>>()
            .join("\n"),
        body.0
    );

    file.write_all(response_to_test.as_bytes()).await.unwrap();
    */

    let res = Response::from_parts(parts, Body::from(body.1));

    Ok(res)
}

async fn buffer_body<B>(body: B) -> Result<(String, Bytes), (StatusCode, String)>
where
    B: axum::body::HttpBody<Data = Bytes>,
    B::Error: std::fmt::Display,
{
    let bytes = match hyper::body::to_bytes(body).await {
        Ok(bytes) => bytes,
        Err(err) => {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("failed to read body: {}", err),
            ));
        }
    };

    let body_str = std::str::from_utf8(&bytes).unwrap_or("NOT UTF-8 (probably binary)");

    Ok((body_str.to_owned(), bytes))
}
