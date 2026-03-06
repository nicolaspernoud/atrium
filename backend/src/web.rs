use axum::{
    body::Body,
    http::{HeaderMap, StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "dist/"]
pub struct Assets;

pub async fn static_handler(uri: Uri, headers: HeaderMap) -> Response {
    let mut path = uri.path().trim_start_matches('/').to_string();

    if path.is_empty() {
        path = "index.html".to_string();
    }

    let accept_encoding = headers
        .get(header::ACCEPT_ENCODING)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    if !accept_encoding.contains("gzip") {
        return (StatusCode::INTERNAL_SERVER_ERROR, "client must accept gzip").into_response();
    }

    // Try finding the file
    let asset = Assets::get(&format!("{path}.gz"));

    if let Some(content) = asset {
        let mime = mime_guess::from_path(&path).first_or_octet_stream();

        let mut response = Body::from(content.data.into_owned()).into_response();
        if let Ok(mime_header) = header::HeaderValue::from_str(mime.as_ref()) {
            response
                .headers_mut()
                .insert(header::CONTENT_TYPE, mime_header);
        }
        response.headers_mut().insert(
            header::CONTENT_ENCODING,
            header::HeaderValue::from_static("gzip"),
        );
        response
    } else {
        // SPA fallback: return index.html for non-file paths or if not found
        if !path.contains('.') || path == "index.html" {
            let asset = Assets::get("index.html.gz");

            if let Some(content) = asset {
                let mut response = Body::from(content.data.into_owned()).into_response();
                response.headers_mut().insert(
                    header::CONTENT_TYPE,
                    header::HeaderValue::from_static("text/html"),
                );
                response.headers_mut().insert(
                    header::CONTENT_ENCODING,
                    header::HeaderValue::from_static("gzip"),
                );
                return response;
            }
        }
        StatusCode::NOT_FOUND.into_response()
    }
}
