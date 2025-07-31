use crate::{
    appstate::ConfigState,
    configuration::HostType,
    users::{UserTokenWithoutXSRFCheck, authorized_or_redirect_to_login},
};
use axum::{
    body::Body,
    extract::State,
    http::{Request, Response, StatusCode},
    response::IntoResponse,
};
use axum_extra::extract::Host;
use tower::util::ServiceExt;
use tower_http::services::ServeDir;

pub async fn dir_handler(
    user: Option<UserTokenWithoutXSRFCheck>,
    app: HostType,
    Host(hostname): Host,
    State(config): State<ConfigState>,
    req: Request<Body>,
) -> Result<impl IntoResponse, Response<Body>> {
    authorized_or_redirect_to_login(&app, &user, &hostname, &req, &config).map_err(|b| *b)?;

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
