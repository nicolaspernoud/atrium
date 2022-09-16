use axum::{
    handler::Handler,
    middleware,
    response::Html,
    routing::{any, delete, get, get_service, post},
    Extension, Router,
};

use http::StatusCode;
use hyper::{Body, Request};
use tokio::sync::broadcast::Sender;

use tower::{ServiceBuilder, ServiceExt};

use tower_http::services::ServeDir;

use crate::{
    apps::{add_app, delete_app, get_apps, proxy_handler},
    configuration::{load_config, ConfigFile, HostType},
    davs::{
        model::{add_dav, delete_dav, get_davs},
        webdav_handler,
    },
    dir_server::dir_handler,
    middlewares::{
        cors_middleware, debug_cors_middleware, inject_security_headers,
        inject_security_headers_for_apps,
    },
    sysinfo::system_info,
    users::{
        add_user, cookie_to_body, delete_user, get_share_token, get_users, list_services,
        local_auth,
    },
};

pub struct Server {
    pub router: Router,
    pub port: u16,
}

impl Server {
    pub async fn build(config_file: &str, tx: Sender<()>) -> Result<Self, anyhow::Error> {
        // Configure Maxmind GeoLite2 City Database as shared state
        let maxmind_reader =
            std::sync::Arc::new(maxminddb::Reader::open_readfile("GeoLite2-City.mmdb").ok());

        let config = load_config(config_file).await?;

        let config_file: ConfigFile = config_file.to_owned();
        let key = axum_extra::extract::cookie::Key::from(
            config.0.cookie_key.as_ref().unwrap().as_bytes(),
        );

        let main_hostname = config.0.get_hostname();
        let main_hostname_header = config.0.get_hostname_header();

        let user_router = Router::new()
            .route("/list_services", get(list_services))
            .route("/system_info", get(system_info))
            .route(
                "/get_share_token",
                post(get_share_token.layer(middleware::from_fn(cookie_to_body))),
            );

        let admin_router = Router::new()
            .route("/users", get(get_users).post(add_user))
            .route("/users/:user_login", delete(delete_user))
            .route("/apps", get(get_apps).post(add_app))
            .route("/apps/:app_id", delete(delete_app))
            .route("/davs", get(get_davs).post(add_dav))
            .route("/davs/:dav_id", delete(delete_dav));

        let hn = main_hostname.clone();
        let main_router = Router::new()
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
            .nest("/api/admin", admin_router)
            .nest("/api/user", user_router)
            .fallback(get_service(ServeDir::new("web")).handle_error(error_500))
            .layer(middleware::from_fn(move |req, next| {
                inject_security_headers(req, next, hn.clone(), false)
            }));

        let hn = main_hostname.clone();
        let proxy_router =
            Router::new()
                .route("/*path", any(proxy_handler))
                .layer(middleware::from_fn(move |req, next| {
                    inject_security_headers_for_apps(req, next, hn.clone(), true)
                }));

        let hn = main_hostname.clone();
        let webdav_router = Router::new()
            .route("/*path", any(webdav_handler))
            .layer(middleware::from_fn(move |req, next| {
                cors_middleware(req, next, main_hostname_header.clone())
            }))
            .layer(middleware::from_fn(move |req, next| {
                inject_security_headers(req, next, hn.clone(), false)
            }));

        let hn = main_hostname.clone();
        let dir_router =
            Router::new()
                .route("/*path", any(dir_handler))
                .layer(middleware::from_fn(move |req, next| {
                    inject_security_headers_for_apps(req, next, hn.clone(), true)
                }));

        let mut router = Router::new()
            .route(
                "/*path",
                any(
                    |hostype: Option<HostType>, request: Request<Body>| async move {
                        match hostype {
                            Some(HostType::StaticApp(_)) => dir_router.oneshot(request).await,
                            Some(HostType::ReverseApp(_)) => proxy_router.oneshot(request).await,
                            Some(HostType::Dav(_)) => webdav_router.oneshot(request).await,
                            None => main_router.oneshot(request).await,
                        }
                    },
                ),
            )
            .layer(
                ServiceBuilder::new()
                    .layer(Extension(key))
                    .layer(Extension(config.1))
                    .layer(Extension(config_file))
                    .layer(Extension(maxmind_reader)),
            );

        if config.0.debug_mode {
            router = router
                .layer(middleware::from_fn(move |req, next| {
                    debug_cors_middleware(req, next)
                }))
                .layer(axum::middleware::from_fn(
                    crate::logger::print_request_response,
                ));
        }

        Ok(Server {
            router: router,
            port: config.0.http_port,
        })
    }
}

async fn error_500(_err: std::io::Error) -> impl axum::response::IntoResponse {
    (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong...")
}
