use crate::{appstate::ConfigState, configuration::HostType};
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use http::{HeaderValue, Method};

pub async fn cors_middleware(
    State(cfg): State<ConfigState>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let hostname = if let Some(origin_header) = req.headers().get("origin")
        && let Ok(origin) = origin_header.to_str()
        && origin.contains(&cfg.domain)
    {
        origin_header.clone()
    } else {
        cfg.full_domain()
            .parse()
            .expect("could not parse hostname : invalid format")
    };
    let mut resp = next.run(req).await;
    let headers = resp.headers_mut();
    headers.insert("Access-Control-Allow-Origin", hostname);
    allow_methods_headers_credentials(headers);
    Ok(resp)
}

pub async fn debug_cors_middleware(req: Request, next: Next) -> Result<Response, StatusCode> {
    let method = req.method().clone();
    let origin = req.headers().get("origin").map(|o| o.to_owned());
    let mut resp = next.run(req).await;
    if let Some(origin) = origin {
        let headers = resp.headers_mut();
        headers.insert("Access-Control-Allow-Origin", origin);
        allow_methods_headers_credentials(headers);
        if method == Method::OPTIONS {
            *resp.status_mut() = StatusCode::OK;
        }
    }
    Ok(resp)
}

fn allow_methods_headers_credentials(headers: &mut http::HeaderMap) {
    headers.insert(
        "Access-Control-Allow-Methods",
        "POST, GET, OPTIONS, PUT, DELETE, PROPFIND, PROPPATCH, MKCOL, MOVE, COPY"
            .parse()
            .expect("infallible"),
    );
    headers.insert("Access-Control-Allow-Headers", "Accept, Content-Type, Content-Length, Accept-Encoding, XSRF-TOKEN, Authorization, Depth, Destination, Overwrite, X-OC-Mtime".parse().expect("infallible"));
    headers.insert(
        "Access-Control-Allow-Credentials",
        "true".parse().expect("infallible"),
    );
}

pub async fn inject_security_headers(
    State(cfg): State<ConfigState>,
    host_type: Option<HostType>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let inject = host_type.is_none_or(|app| app.inject_security_headers());
    if inject {
        let source = {
            format!(
                "{s}://{h}:* {s}://*.{h}:*",
                s = cfg.scheme(),
                h = cfg.domain,
            )
        };
        let mut resp = next.run(req).await;
        inject_security_headers_internal(&mut resp, &source)?;
        Ok(resp)
    } else {
        Ok(next.run(req).await)
    }
}

fn inject_security_headers_internal(resp: &mut Response, source: &str) -> Result<(), StatusCode> {
    let headers = resp.headers_mut();
    match headers
        .remove(http::header::CONTENT_SECURITY_POLICY)
        .as_ref()
        .and_then(|h| h.to_str().ok())
    {
        // If it exists, alter it to inject the atrium main hostname in authorized frame ancestors
        Some(csp) => {
            let new_csp = if csp.contains("frame-ancestors") {
                csp.replacen("frame-ancestors", &format!("frame-ancestors {source}"), 1)
            } else {
                format!("{csp}; frame-ancestors {source}")
            };
            headers.insert(
                http::header::CONTENT_SECURITY_POLICY,
                HeaderValue::from_str(&new_csp)
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
            );
        }
        // If not, forge a default CSP Header
        None => {
            headers.insert(
                http::header::CONTENT_SECURITY_POLICY,
                HeaderValue::from_str(&format!("default-src 'self' {source} https://unpkg.com https://*.gstatic.com blob:; script-src 'self' {source} 'wasm-unsafe-eval' https://cdn.jsdelivr.net https://unpkg.com https://*.gstatic.com; style-src 'self' {source} 'unsafe-inline'; frame-src {source}; frame-ancestors {source}"))
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
            );
        }
    }
    headers.insert("Referrer-Policy", HeaderValue::from_static("strict-origin"));
    headers.insert(
        "X-Content-Type-Options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        "Strict-Transport-Security",
        HeaderValue::from_static("max-age=63072000; includeSubDomains"),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inject_security_headers_internal_none() {
        let mut resp = Response::builder().body(axum::body::Body::empty()).unwrap();
        let source = "http://example.com:* http://*.example.com:*";

        inject_security_headers_internal(&mut resp, source).unwrap();

        let csp = resp
            .headers()
            .get(http::header::CONTENT_SECURITY_POLICY)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(csp.contains(source));
        assert!(csp.contains("frame-ancestors"));
    }

    #[test]
    fn test_inject_security_headers_internal_existing_with_frame_ancestors() {
        let mut resp = Response::builder()
            .header(
                "Content-Security-Policy",
                "default-src 'self'; frame-ancestors 'self'",
            )
            .body(axum::body::Body::empty())
            .unwrap();
        let source = "http://example.com:* http://*.example.com:*";

        inject_security_headers_internal(&mut resp, source).unwrap();

        let csp = resp
            .headers()
            .get(http::header::CONTENT_SECURITY_POLICY)
            .unwrap()
            .to_str()
            .unwrap();
        assert_eq!(
            csp,
            format!("default-src 'self'; frame-ancestors {source} 'self'")
        );
    }

    #[test]
    fn test_inject_security_headers_internal_existing_without_frame_ancestors() {
        let mut resp = Response::builder()
            .header("content-security-policy", "default-src 'self'")
            .body(axum::body::Body::empty())
            .unwrap();
        let source = "http://example.com:* http://*.example.com:*";

        inject_security_headers_internal(&mut resp, source).unwrap();

        let csp = resp
            .headers()
            .get(http::header::CONTENT_SECURITY_POLICY)
            .unwrap()
            .to_str()
            .unwrap();
        assert_eq!(csp, format!("default-src 'self'; frame-ancestors {source}"));
    }
}
