use axum::{
    middleware,
    response::{Html, IntoResponse},
    routing::{delete, get, get_service, post},
    Router,
};

use http::StatusCode;
use hyper::{Body, Request};
use tokio::sync::broadcast::Sender;

use tower::ServiceExt;

use tower_http::services::ServeDir;

use crate::{
    apps::{add_app, delete_app, get_apps, proxy_handler},
    appstate::AppState,
    configuration::{load_config, HostType},
    davs::{
        model::{add_dav, delete_dav, get_davs},
        webdav_handler,
    },
    dir_server::dir_handler,
    middlewares::{cors_middleware, debug_cors_middleware, inject_security_headers},
    oauth2::{oauth2_callback, oauth2_login},
    onlyoffice::{onlyoffice_callback, onlyoffice_page},
    sysinfo::system_info,
    users::{
        add_user, cookie_to_body, delete_user, get_share_token, get_users, list_services,
        local_auth, whoami,
    },
};

pub struct Server {
    pub router: Router,
    pub port: u16,
}

impl Server {
    pub async fn build(config_file: &str, tx: Sender<()>) -> Result<Self, anyhow::Error> {
        let config = load_config(config_file).await?;
        tracing::info!("Atrium's main hostname: {}", config.0.hostname);

        let debug_mode = config.0.debug_mode;
        let http_port = config.0.http_port;

        let state = AppState::new(
            axum_extra::extract::cookie::Key::from(
                config.0.cookie_key.as_ref().unwrap().as_bytes(),
            ),
            config.0,
            config.1,
            config_file.to_owned(),
        );

        let user_router = Router::new()
            .route("/whoami", get(whoami))
            .route("/list_services", get(list_services))
            .route("/system_info", get(system_info))
            .route(
                "/get_share_token",
                post(get_share_token).layer(middleware::from_fn(cookie_to_body)),
            );

        let admin_router = Router::new()
            .route("/users", get(get_users).post(add_user))
            .route("/users/:user_login", delete(delete_user))
            .route("/apps", get(get_apps).post(add_app))
            .route("/apps/:app_id", delete(delete_app))
            .route("/davs", get(get_davs).post(add_dav))
            .route("/davs/:dav_id", delete(delete_dav));

        let main_router: Router<()> = Router::new()
            .route(
                "/reload",
                get(|| async move {
                    match tx.send(()) {
                        Ok(_) => Html("Apps reloaded !"),
                        Err(_) => Html("Could not reload apps !"),
                    }
                }),
            )
            .route("/auth/local", post(local_auth))
            .route("/auth/oauth2login", get(oauth2_login))
            .route("/auth/oauth2callback", get(oauth2_callback))
            .nest("/api/admin", admin_router)
            .nest("/api/user", user_router)
            .route("/onlyoffice/save", post(onlyoffice_callback))
            .route("/onlyoffice", get(onlyoffice_page))
            .fallback_service(get_service(ServeDir::new("web")).handle_error(error_500))
            .with_state(state.clone());

        let proxy_router = Router::new()
            .fallback(proxy_handler)
            .with_state(state.clone());

        let webdav_router = Router::new()
            .fallback(webdav_handler)
            .layer(middleware::from_fn_with_state(
                state.clone(),
                cors_middleware,
            ))
            .with_state(state.clone());

        let dir_router = Router::new()
            .fallback(dir_handler)
            .with_state(state.clone());

        let mut router = Router::new()
            .fallback(
                |hostype: Option<HostType>, request: Request<Body>| async move {
                    match hostype {
                        Some(HostType::StaticApp(_)) => dir_router.oneshot(request).await,
                        Some(HostType::ReverseApp(_)) => proxy_router.oneshot(request).await,
                        Some(HostType::Dav(_)) => webdav_router.oneshot(request).await,
                        None => main_router.oneshot(request).await,
                    }
                },
            )
            .layer(middleware::from_fn_with_state(
                state.clone(),
                inject_security_headers,
            ))
            .with_state(state);

        if debug_mode {
            router = router
                .layer(middleware::from_fn(debug_cors_middleware))
                .layer(axum::middleware::from_fn(
                    crate::logger::print_request_response,
                ));
        }

        Ok(Server {
            router,
            port: http_port,
        })
    }
}

async fn error_500(_err: std::io::Error) -> impl IntoResponse {
    (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong...")
}
