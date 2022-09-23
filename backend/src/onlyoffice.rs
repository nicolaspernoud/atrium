use axum::extract::Query;
use axum::response::IntoResponse;
use axum::{response::Html, Extension, Json};
use http::{header, StatusCode};
use reqwest::Body;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::fs::{self};

use crate::configuration::Config;

// onlyoffice_page opens the main onlyoffice  window
pub async fn onlyoffice_page(
    Extension(config): Extension<Arc<Config>>,
) -> Result<Html<String>, (StatusCode, &'static str)> {
    if let (Some(title), Some(server)) = (&config.onlyoffice_title, &config.onlyoffice_server) {
        let template = fs::read_to_string("./web/onlyoffice/index.tmpl")
            .await
            .map_err(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "couldn't read onlyoffice template file",
                )
            })?;
        let response = template
            .replace("{{.Title}}", title)
            .replace("{{.OnlyOfficeServer}}", server)
            .replace("{{.Hostname}}", &config.full_hostname());
        Ok(Html(response))
    } else {
        Ok(Html("OnlyOffice is not fully configured !".to_owned()))
    }
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct OnlyOfficeCallback {
    pub key: String,
    pub status: i64,
    pub url: String,
}

#[derive(Deserialize)]
pub struct Target {
    file: usize,
    token: usize,
}

// onlyoffice_callback is the callback function wanted by onlyoffice to allow saving a document
// the body provides information on where to get the altered document, and the query provides information on where to put it
pub async fn onlyoffice_callback(
    Json(payload): Json<OnlyOfficeCallback>,
    target: Query<Target>,
) -> Result<impl IntoResponse, (StatusCode, &'static str)> {
    // Case of document closed after editing
    if payload.status == 2 {
        // Get the binary content from url
        let response = reqwest::get(payload.url).await.map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "could not get document from OnlyOffice server",
            )
        })?;
        // PUT the content on the ressource gotten from the query
        let target_url = format!("{}?token={}", target.file, target.token);
        let client = reqwest::Client::new();
        client
            .put(target_url)
            .body(Body::wrap_stream(response.bytes_stream()))
            .send()
            .await
            .map_err(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "could not push the document to atrium file server",
                )
            })?;
    }
    Ok((
        [(header::CONTENT_TYPE, "application/json")],
        "{\"error\":0}",
    ))
}
