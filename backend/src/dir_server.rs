use crate::{
    appstate::ConfigState,
    configuration::HostType,
    users::{authorized_or_redirect_to_login, UserTokenWithoutXSRFCheck},
};
use axum::{
    body::Body,
    extract::{Host, State},
    http::{Request, Response, StatusCode},
    response::IntoResponse,
};
use tower::ServiceExt;
use tower_http::services::ServeDir;

pub async fn dir_handler(
    user: Option<UserTokenWithoutXSRFCheck>,
    app: HostType,
    Host(hostname): Host,
    State(config): State<ConfigState>,
    req: Request<Body>,
) -> Result<impl IntoResponse, Response<Body>> {
    authorized_or_redirect_to_login(&app, &user, &hostname, &req, &config)?;

    let app = match app {
        HostType::StaticApp(app) => app,
        _ => panic!("Service is not a static app !"),
    };

    match ServeDir::new(app.target).oneshot(req).await {
        Ok(res) => Ok(res),
        Err(err) => Err(Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(format!("Something went wrong: {}", err).into())
            .unwrap()),
    }
}
