use crate::configuration::HostType;
use axum::{
    body::Body,
    http::{Request, Response, StatusCode},
    response::IntoResponse,
};
use tower::util::ServiceExt;
use tower_http::services::ServeDir;

pub async fn dir_handler(
    app: HostType,
    req: Request<Body>,
) -> Result<impl IntoResponse, Response<Body>> {
    let app = match app {
        HostType::StaticApp(app) => app,
        _ => panic!("Service is not a static app !"),
    };

    match ServeDir::new(app.target).oneshot(req).await {
        Ok(res) => Ok(res),
        Err(_) => Err(Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body("could not serve dir".into())
            .expect("infallible")),
    }
}
