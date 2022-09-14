use axum::{
    body::{boxed, Body, BoxBody},
    http::{Request, Response, StatusCode, Uri},
};
use tower::ServiceExt;
use tower_http::services::ServeDir;

use crate::configuration::HostType;

pub async fn dir_handler(
    uri: Uri,
    app: HostType,
) -> Result<Response<BoxBody>, (StatusCode, String)> {
    let app = match app {
        HostType::StaticApp(app) => app,
        _ => panic!("Service is not a static app !"),
    };
    let req = Request::builder().uri(uri).body(Body::empty()).unwrap();

    match ServeDir::new(app.target).oneshot(req).await {
        Ok(res) => Ok(res.map(boxed)),
        Err(err) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {}", err),
        )),
    }
}
