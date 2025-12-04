use crate::appstate::OptionalMaxMindReader;
use axum::{
    body::{Body, Bytes},
    extract::ConnectInfo,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use http_body_util::BodyExt;
use maxminddb::geoip2;
use std::net::{
    IpAddr::{V4, V6},
    SocketAddr,
};

const UNKNOWN_CITY: &str = "unknown city";
const UNKNOWN_COUNTRY: &str = "unknown country";
const LOCALHOST: &str = "localhost";
const LOCAL_NETWORK: &str = "local network";

pub fn city_from_ip(addr: SocketAddr, reader: OptionalMaxMindReader) -> String {
    let ip = addr.ip();
    let location = match ip {
        // Replace with https://doc.rust-lang.org/std/net/enum.IpAddr.html#method.is_global when stable
        V4(ip) if ip.is_private() => LOCAL_NETWORK.to_owned(),
        V6(ip) if ip.to_ipv4_mapped().is_some_and(|ip| ip.is_private()) => LOCAL_NETWORK.to_owned(),
        _ if ip.is_loopback() => LOCALHOST.to_owned(), // Does not work for ipv4 mapped addresses
        V6(ip) if ip.to_ipv4_mapped().is_some_and(|ip| ip.is_loopback()) => LOCALHOST.to_owned(),
        _ => {
            if let Some(reader) = reader {
                if let Ok(result) = reader.lookup(ip)
                    && let Ok(Some(city)) = result.decode::<geoip2::City<'_>>()
                {
                    format!(
                        "{}, {}",
                        city.city.names.english.unwrap_or(UNKNOWN_CITY),
                        city.country.names.english.unwrap_or(UNKNOWN_COUNTRY),
                    )
                } else {
                    "unknown location".to_owned()
                }
            } else {
                "unknown location (no geo ip database)".to_owned()
            }
        }
    };
    format!("{location} ({ip})")
}

#[tracing::instrument(name = "Request", level = "debug", skip_all, fields(ip=%addr, uri = %req.uri(), method = %req.method()))]
pub async fn print_request_response(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request<Body>,
    next: Next,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let (parts, body) = req.into_parts();

    let body = buffer_body(body).await?;
    tracing::debug!(
        "\nREQUEST\n⇨ uri: {}\n⇨ headers: {:?}\n⇨ body: {}",
        parts.uri,
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
    let bytes = body
        .collect()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("failed to read body: {e}")))?
        .to_bytes();

    let body_str = std::str::from_utf8(&bytes).unwrap_or("NOT UTF-8 (probably binary)");

    Ok((body_str.to_owned(), bytes))
}
