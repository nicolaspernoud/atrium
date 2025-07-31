use crate::{
    appstate::ConfigState,
    errors::ErrResponse,
    utils::{is_default, query_pairs_or_error},
};
use axum::{
    Json,
    body::Body,
    extract::{RawQuery, State},
    response::{Html, IntoResponse},
};
use http::{Method, Request, StatusCode, header, header::CONTENT_TYPE};
use hyper_rustls::HttpsConnectorBuilder;
use hyper_util::{client::legacy::Client, rt::TokioExecutor};
use jsonwebtoken_aws_lc::{EncodingKey, Header, encode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::fs::{self};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlyOfficeConfiguration<'a> {
    pub document: Document<'a>,
    pub editor_config: EditorConfig<'a>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub token: &'a str,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Document<'a> {
    pub file_type: &'a str,
    pub key: &'a str,
    pub title: &'a str,
    pub url: &'a str,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorConfig<'a> {
    pub lang: &'a str,
    pub callback_url: &'a str,
    pub customization: Customization,
    pub user: OOUser<'a>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Customization {
    pub autosave: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OOUser<'a> {
    pub id: &'a str,
    pub name: &'a str,
}

const QUERY_ERROR: (http::StatusCode, &str) =
    (StatusCode::INTERNAL_SERVER_ERROR, "query is malformed");

// onlyoffice_page opens the main onlyoffice  window
pub async fn onlyoffice_page(
    State(config): State<ConfigState>,
    RawQuery(query): RawQuery,
) -> Result<Html<String>, (StatusCode, &'static str)> {
    let ooq = query_pairs_or_error(query.as_deref())?;
    let file = ooq.get("file").ok_or(QUERY_ERROR)?;
    let share_token = ooq.get("share_token").ok_or(QUERY_ERROR)?;
    let mtime = ooq.get("mtime").ok_or(QUERY_ERROR)?;
    let oo_user = ooq.get("user").ok_or(QUERY_ERROR)?;

    if let Some(server) = &config.onlyoffice_config.as_ref().map(|c| c.server.clone()) {
        let template = fs::read_to_string("./web/onlyoffice/index.tmpl")
            .await
            .map_err(|_| ErrResponse::S500("couldn't read onlyoffice template file"))?;
        let title = config
            .onlyoffice_config
            .as_ref()
            .ok_or(ErrResponse::S500("could not get onlyoffice configuration"))?
            .title
            .as_ref()
            .unwrap_or(&"AtriumOffice".to_owned())
            .to_owned();
        let path = std::path::Path::new(file);
        let extension = path
            .extension()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default();
        let filename = urlencoding::decode(
            path.file_stem()
                .unwrap_or_default()
                .to_str()
                .unwrap_or_default(),
        )
        .map_err(|_| QUERY_ERROR)?;
        let url = format!("{file}?token={share_token}");

        let mut hasher = Sha256::new();
        hasher.update(format!("{file}{mtime}"));
        let key: String = format!("{:X}", hasher.finalize());

        let mut ooconf = OnlyOfficeConfiguration {
            document: Document {
                file_type: extension,
                key: &key,
                title: &filename,
                url: &url,
            },
            editor_config: EditorConfig {
                lang: "fr-FR",
                callback_url: &format!("{}/onlyoffice/save?{}", &config.full_domain(), url),
                customization: Customization { autosave: false },
                user: OOUser {
                    id: oo_user,
                    name: oo_user,
                },
            },
            token: "",
        };

        let j = serde_json::to_string(&ooconf).map_err(ooconf_to_json_error)?;

        let token = encode(
            &Header::default(),
            &j,
            &EncodingKey::from_secret(
                config
                    .onlyoffice_config
                    .as_ref()
                    .ok_or(ErrResponse::S500("could not get onlyoffice configuration"))?
                    .jwt_secret
                    .as_ref(),
            ),
        )
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "couldn't sign JWT"))?;

        ooconf.token = &token;
        let j = serde_json::to_string(&ooconf).map_err(ooconf_to_json_error)?;

        let response = template
            .replace("{{.Title}}", &title)
            .replace("{{.OnlyOfficeServer}}", server)
            .replace("{{.OnlyOfficeConfiguration}}", &j);
        Ok(Html(response))
    } else {
        Ok(Html("OnlyOffice is not fully configured !".to_owned()))
    }
}

fn ooconf_to_json_error(_: serde_json::Error) -> ErrResponse {
    ErrResponse::S500("couldn't create OnlyOffice configuration json")
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
    RawQuery(query): RawQuery,
    Json(payload): Json<OnlyOfficeCallback>,
) -> Result<impl IntoResponse, (StatusCode, &'static str)> {
    // Case of document closed after editing
    if payload.status == 2 && payload.url.is_some() && query.is_some() {
        let https = HttpsConnectorBuilder::new()
            .with_webpki_roots()
            .https_or_http()
            .enable_http1()
            .build();

        let client: Client<_, Body> = Client::builder(TokioExecutor::new()).build(https);

        // GET the binary content from url
        let request = Request::get(
            payload
                .url
                .ok_or(ErrResponse::S500("could not get url from payload"))?,
        )
        .body(Body::empty())
        .map_err(|_| ErrResponse::S500("could not create GET request to OnlyOffice server"))?;
        let response = client
            .request(request)
            .await
            .map_err(|_| ErrResponse::S500("could not send GET request to OnlyOffice server"))?;
        if !response.status().is_success() {
            return Err((
                response.status(),
                "could not get document from OnlyOffice server",
            ));
        }

        // PUT the content on the ressource gotten from the query
        let request = Request::builder()
            .method(Method::PUT)
            .uri(query.ok_or(ErrResponse::S500("could not get uri from query"))?)
            .header(CONTENT_TYPE, "application/octet-stream")
            .body(Body::new(response.into_body()))
            .map_err(|_| ErrResponse::S500("could not create PUT request to atrium file server"))?;
        let response = client
            .request(request)
            .await
            .map_err(|_| ErrResponse::S500("could not send PUT request to atrium file server"))?;
        if !response.status().is_success() {
            return Err((
                response.status(),
                "could not push the document to atrium file server",
            ));
        }
    }
    Ok((
        [(header::CONTENT_TYPE, "application/json")],
        "{\"error\":0}",
    ))
}
