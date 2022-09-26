use axum::extract::RawQuery;
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
    if let Some(server) = &config
        .onlyoffice_config
        .as_ref()
        .map(|c| c.server.to_owned())
    {
        let template = fs::read_to_string("./web/onlyoffice/index.tmpl")
            .await
            .map_err(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "couldn't read onlyoffice template file",
                )
            })?;
        let title = config
            .onlyoffice_config
            .as_ref()
            .unwrap()
            .title
            .as_ref()
            .unwrap_or(&"AtriumOffice".to_owned())
            .to_owned();
        let response = template
            .replace("{{.Title}}", &title)
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
    pub url: Option<String>,
}

// onlyoffice_callback is the callback function wanted by onlyoffice to allow saving a document
// the body provides information on where to get the altered document, and the query provides information on where to put it
pub async fn onlyoffice_callback(
    Json(payload): Json<OnlyOfficeCallback>,
    RawQuery(query): RawQuery,
) -> Result<impl IntoResponse, (StatusCode, &'static str)> {
    // Case of document closed after editing
    if payload.status == 2 && payload.url.is_some() && query.is_some() {
        // Get the binary content from url
        let response = reqwest::get(payload.url.unwrap()).await.map_err(|e| {
            tracing::log::error!("ERROR: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "could not get document from OnlyOffice server",
            )
        })?;
        // PUT the content on the ressource gotten from the query
        let client = reqwest::Client::new();
        client
            .put(query.unwrap())
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
