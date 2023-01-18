use crate::{appstate::ConfigState, configuration::HostType};
use axum::{
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use http::{HeaderValue, Method};

pub async fn cors_middleware<B>(
    State(cfg): State<ConfigState>,
    req: Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    let origin = req.headers().get("origin").map(|o| o.to_owned());
    let hostname = if origin.is_some() && {
        let this = &origin.as_ref().unwrap().to_str();
        let f = |o: &str| o.contains(&cfg.domain);
        matches!(this, Ok(x) if f(x))
    } {
        origin.unwrap()
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

pub async fn debug_cors_middleware<B>(
    req: Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
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
            .unwrap(),
    );
    headers.insert("Access-Control-Allow-Headers", "Accept, Content-Type, Content-Length, Accept-Encoding, XSRF-TOKEN, Authorization, Depth, Destination, Overwrite, X-OC-Mtime".parse().unwrap());
    headers.insert("Access-Control-Allow-Credentials", "true".parse().unwrap());
}

pub async fn inject_security_headers<B>(
    State(cfg): State<ConfigState>,
    host_type: Option<HostType>,
    req: Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode>
where
    B: std::marker::Send,
{
    let inject = host_type
        .map(|app| app.inject_security_headers())
        .unwrap_or(true);
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
        .get("Content-Security-Policy")
        .and_then(|h| h.to_str().ok())
        .map(|h| h.to_owned())
    {
        // If it exists, alter it to inject the atrium main hostname in authorized frame ancestors
        Some(csp) => {
            if csp.contains("frame-ancestors") {
                headers.insert(
                    "Content-Security-Policy",
                    HeaderValue::from_str(&csp.replacen(
                        "frame-ancestors",
                        &format!("frame-ancestors {source}"),
                        1,
                    ))
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
                );
            } else {
                headers.insert(
                    "Content-Security-Policy",
                    HeaderValue::from_str(&format!("{csp}; frame-ancestors {source}"))
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
                );
            }
        }
        // If not, forge a default CSP Header
        None => {
            headers.insert("Content-Security-Policy", 
            HeaderValue::from_str(&format!("default-src 'self' {source} https://unpkg.com https://fonts.gstatic.com; script-src 'self' {source} 'wasm-unsafe-eval' https://cdn.jsdelivr.net https://unpkg.com; style-src 'self' {source} 'unsafe-inline'; frame-src {source}; frame-ancestors {source}; img-src 'self' {source} blob:"))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,);
        }
    }
    headers.insert(
        "X-XSS-Protection",
        HeaderValue::from_static("1; mode=block"),
    );
    headers.insert("Referrer-Policy", HeaderValue::from_static("strict-origin"));
    headers.insert(
        "X-Content-Type-Options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        "Strict-Transport-Security",
        HeaderValue::from_static("max-age=63072000"),
    );
    Ok(())
}
