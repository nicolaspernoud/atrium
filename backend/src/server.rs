use axum::{
    Router,
    body::Body,
    handler::Handler,
    middleware,
    response::Html,
    routing::{MethodRouter, any, delete, get, post},
};

use hyper::Request;
use tokio::sync::broadcast::Sender;

use tower::{ServiceBuilder, ServiceExt};

#[cfg(target_os = "linux")]
use crate::jail::Jail;
use crate::{
    apps::{add_app, delete_app, get_apps, proxy_handler},
    appstate::{AppState, Client, InsecureSkipVerifyClient},
    auth::{
        AdminToken, auth_middleware, cookie_to_body, dav_auth_middleware, get_share_token,
        xsrf_middleware,
    },
    configuration::{HostType, load_config},
    davs::{
        model::{add_dav, delete_dav, get_davs},
        webdav_handler,
    },
    dir_server::dir_handler,
    errors::Error,
    middlewares::{cors_middleware, debug_cors_middleware, inject_security_headers},
};
use crate::{
    auth::{add_user, delete_user, get_users, list_services, local_auth, logout, whoami},
    oauth2::{oauth2_available, oauth2_callback, oauth2_login},
    onlyoffice::{onlyoffice_callback, onlyoffice_page},
    sysinfo::system_info,
};

pub struct Server {
    pub router: MethodRouter,
    pub port: u16,
}

impl Server {
    pub async fn build(config_file: &str, tx: Sender<()>) -> Result<Self, Error> {
        let config = load_config(config_file).await?;
        tracing::info!("Atrium's main hostname: {}", config.0.hostname);

        let debug_mode = config.0.debug_mode;
        let http_port = config.0.http_port;
        let single_proxy = config.0.single_proxy;
        #[cfg(target_os = "linux")]
        let jail = Jail::new_from_config(&config.0.jail).await;

        // Start pruning task once a day
        #[cfg(target_os = "linux")]
        if let Some(jail) = jail.as_ref() {
            let jail_clone = std::sync::Arc::clone(jail);
            tokio::spawn(async move {
                let mut interval =
                    tokio::time::interval(tokio::time::Duration::from_secs(24 * 3600));
                loop {
                    jail_clone.prune_expired_rules().await;
                    interval.tick().await;
                }
            });
        }

        let state = AppState::new(
            axum_extra::extract::cookie::Key::from(
                config.0.cookie_key.as_ref().expect("cookie key").as_bytes(),
            ),
            config.0,
            config.1,
            config_file.to_owned(),
            #[cfg(target_os = "linux")]
            jail,
        );

        let user_router = Router::new()
            .route("/api/user/list_services", get(list_services))
            .route("/api/user/system_info", get(system_info))
            .route(
                "/api/user/get_share_token",
                post(get_share_token).layer(middleware::from_fn_with_state(
                    state.clone(),
                    cookie_to_body,
                )),
            )
            .route_layer(middleware::from_fn_with_state(
                state.clone(),
                xsrf_middleware,
            ))
            .route("/api/user/whoami", get(whoami));

        let admin_router = Router::new()
            .route("/api/admin/users", get(get_users).post(add_user))
            .route("/api/admin/users/{user_login}", delete(delete_user))
            .route("/api/admin/apps", get(get_apps).post(add_app))
            .route("/api/admin/apps/{app_id}", delete(delete_app))
            .route("/api/admin/davs", get(get_davs).post(add_dav))
            .route("/api/admin/davs/{dav_id}", delete(delete_dav))
            .route_layer(
                ServiceBuilder::new()
                    .layer(
                        middleware::from_extractor_with_state::<AdminToken, AppState>(
                            state.clone(),
                        ),
                    )
                    .layer(middleware::from_fn_with_state(
                        state.clone(),
                        xsrf_middleware,
                    )),
            );

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
            .route("/auth/oauth2login", get(oauth2_login))
            .route("/auth/oauth2callback", get(oauth2_callback))
            .route("/auth/oauth2available", get(oauth2_available))
            .route("/auth/logout", get(logout))
            // We use merge instead of nest as it is still a little bit faster
            .merge(admin_router)
            .merge(user_router)
            .route("/onlyoffice/save", post(onlyoffice_callback))
            .route("/onlyoffice", get(onlyoffice_page))
            .route(
                "/healthcheck",
                get(|| async { "OK" }).layer(middleware::from_fn_with_state(
                    state.clone(),
                    debug_cors_middleware,
                )),
            );

        let proxy_router = proxy_handler::<Client>
            .layer(middleware::from_fn_with_state(
                state.clone(),
                auth_middleware,
            ))
            .with_state(state.clone());
        let unsecure_proxy_router = proxy_handler::<InsecureSkipVerifyClient>
            .layer(middleware::from_fn_with_state(
                state.clone(),
                auth_middleware,
            ))
            .with_state(state.clone());
        let router = if single_proxy {
            let main_router = main_router
                .fallback(any(
                    |hostype: Option<HostType>, request: Request<Body>| async move {
                        match hostype {
                            Some(HostType::SkipVerifyReverseApp(_)) => {
                                unsecure_proxy_router.oneshot(request).await
                            }
                            _ => proxy_router.oneshot(request).await,
                        }
                    },
                ))
                .with_state(state.clone());
            any(|_: Option<HostType>, request: Request<Body>| async move {
                main_router.oneshot(request).await
            })
        } else {
            let main_router = main_router
                .fallback(crate::web::static_handler)
                .with_state(state.clone());
            let webdav_router = webdav_handler
                .layer(middleware::from_fn_with_state(
                    state.clone(),
                    xsrf_middleware,
                ))
                .layer(middleware::from_fn_with_state(
                    state.clone(),
                    cors_middleware,
                ))
                .layer(middleware::from_fn_with_state(
                    state.clone(),
                    dav_auth_middleware,
                ))
                .with_state(state.clone());
            let dir_router = dir_handler
                .layer(middleware::from_fn_with_state(
                    state.clone(),
                    auth_middleware,
                ))
                .with_state(state.clone());
            any(
                |hostype: Option<HostType>, request: Request<Body>| async move {
                    match hostype {
                        Some(HostType::ReverseApp(_)) => proxy_router.oneshot(request).await,
                        Some(HostType::StaticApp(_)) => dir_router.oneshot(request).await,
                        Some(HostType::SkipVerifyReverseApp(_)) => {
                            unsecure_proxy_router.oneshot(request).await
                        }
                        Some(HostType::Dav(_)) => webdav_router.oneshot(request).await,
                        None => main_router.oneshot(request).await,
                    }
                },
            )
        };

        let mut router = router
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
