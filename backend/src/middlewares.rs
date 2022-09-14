use axum::{
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use http::{HeaderValue, Method};

pub async fn cors_middleware<B>(
    req: Request<B>,
    next: Next<B>,
    hostname: HeaderValue,
) -> Result<Response, StatusCode> {
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
