use anyhow::Result;
use atrium::{
    configuration::{Config, TlsMode},
    mocks::{mock_oauth2_server, mock_proxied_server},
    server::Server,
};
use axum::{BoxError, handler::HandlerWithoutStateExt, response::Redirect};
use axum_extra::extract::Host;
use axum_server::Handle;
use http::{StatusCode, Uri};
use rustls::ServerConfig;
use rustls_acme::{AcmeConfig, caches::DirCache};
use std::{
    fs::File,
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::{net::TcpListener, signal, sync::broadcast};
use tokio_stream::StreamExt;
use tracing::{error, info};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, fmt::time::OffsetTime, prelude::*};

#[cfg(feature = "self_signed")]
pub mod self_signed;

pub const CONFIG_FILE: &str = "atrium.yaml";

fn main() -> Result<()> {
    // println!("MiMalloc version: {}", mimalloc::MiMalloc.version()); // mimalloc = { version = "0.1", features = ["extended"] } in Cargo.toml to use this
    // We need to work out the local time offset before entering multi-threaded context
    let cfg: Config = if let Ok(file) = File::open(CONFIG_FILE) {
        serde_yaml_ng::from_reader(file).expect("failed to parse configuration file")
    } else {
        println!("Configuration file not found, trying to create default configuration file.");
        File::create(CONFIG_FILE).expect("could not create default configuration file");
        Config::default()
    };
    let _log_guards = setup_logger(cfg.debug_mode, cfg.log_to_file);
    run()
}

#[tokio::main]
async fn run() -> Result<()> {
    let debug_mode = Config::from_file(CONFIG_FILE).await?.debug_mode;
    let ip_bind = if cfg!(windows) {
        "0.0.0.0"
    } else {
        "[::]" // On linux bind to ipv6 binds to ipv4 as well
    };
    if debug_mode {
        let mock1_listener = TcpListener::bind(format!("{ip_bind}:8081"))
            .await
            .expect("failed to bind to port");
        tokio::spawn(mock_proxied_server(mock1_listener));
        let mock2_listener = TcpListener::bind(format!("{ip_bind}:8082"))
            .await
            .expect("failed to bind to port");
        tokio::spawn(mock_proxied_server(mock2_listener));
        let mock_oauth2_listener = TcpListener::bind(format!("{ip_bind}:8090"))
            .await
            .expect("failed to bind to port");
        tokio::spawn(mock_oauth2_server(mock_oauth2_listener));
    }

    let config = atrium::configuration::load_config(CONFIG_FILE).await?;

    let reload_loop = Arc::new(AtomicBool::new(true));
    let (tx, _) = broadcast::channel(16);

    info!("Starting server...");
    loop {
        let reload_loop = reload_loop.clone();
        if !(reload_loop.load(Ordering::Relaxed)) {
            break;
        };

        let server = Server::build(CONFIG_FILE, tx.clone()).await?;

        let app = server
            .router
            .into_make_service_with_connect_info::<SocketAddr>();

        let handle = Handle::new();
        let shutdown_handle = handle.clone();
        let mut rx = tx.subscribe();

        tokio::spawn(async move {
            tokio::select! {
                _ = rx.recv() => {
                    info!("Reloading configuration...");
                    shutdown_handle.graceful_shutdown(Some(Duration::from_secs(1)));
                },
                _ = shutdown_signal() => {
                        info!("Shutting down...");
                        reload_loop.store(false, Ordering::Relaxed);
                        shutdown_handle.graceful_shutdown(Some(Duration::from_secs(10)));
                },
            }
        });

        match config.0.tls_mode {
            TlsMode::Auto => {
                let config = atrium::configuration::load_config(CONFIG_FILE).await?;
                let domains: Vec<String> = config.0.domains();
                info!(
                    "Getting let's encrypt certificates for FQDNs : {:?}",
                    domains
                );
                let mut state = AcmeConfig::new(domains)
                    .contact_push(format!("mailto:{}", config.0.letsencrypt_email))
                    .directory_lets_encrypt(true)
                    .cache(DirCache::new("./letsencrypt_cache"))
                    .state();

                let mut rustls_config = ServerConfig::builder()
                    .with_no_client_auth()
                    .with_cert_resolver(state.resolver());
                rustls_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
                let acceptor = state.axum_acceptor(Arc::new(rustls_config));

                tokio::spawn(async move {
                    loop {
                        match state.next().await.expect("could not start ACME loop") {
                            Ok(ok) => info!("ACME (let's encrypt) event: {:?}", ok),
                            Err(err) => error!("ACME (let's encrypt) error: {:?}", err),
                        }
                    }
                });

                // Spawn a server to redirect HTTP to HTTPS
                tokio::spawn(redirect_http_to_https(handle.clone()));

                // Main server
                let addr = format!("[::]:{}", 443).parse::<std::net::SocketAddr>()?;

                axum_server::bind(addr)
                    .acceptor(acceptor)
                    .handle(handle)
                    .serve(app)
                    .await?;
            }
            #[cfg(feature = "self_signed")]
            TlsMode::SelfSigned => {
                self_signed::serve_with_self_signed_cert(ip_bind, &server.port, handle, app)
                    .await?;
            }
            _ => {
                let addr = format!("{ip_bind}:{}", server.port).parse::<std::net::SocketAddr>()?;
                axum_server::bind(addr).handle(handle).serve(app).await?;
            }
        }
    }

    info!("Graceful shutdown done !");

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("signal received, starting graceful shutdown");
}

fn setup_logger(debug_mode: bool, log_to_file: bool) -> Vec<WorkerGuard> {
    let mut guards = Vec::new();
    let time_format =
        time::format_description::parse("[year]-[month]-[day] [hour]:[minute]:[second]")
            .expect("format string should be valid!");
    let offset = time::UtcOffset::current_local_offset().expect("should get local offset!");
    let timer = OffsetTime::new(offset, time_format);

    let (non_blocking, guard) = tracing_appender::non_blocking(std::io::stdout());
    guards.push(guard);
    let stdout_writer = fmt::Layer::new()
        .with_timer(timer.clone())
        .with_writer(non_blocking);

    let file_writer = if log_to_file {
        let (non_blocking, guard) = tracing_appender::non_blocking(
            tracing_appender::rolling::daily("./logs/", "atrium.log"),
        );
        guards.push(guard);
        let file_writer = fmt::Layer::new()
            .with_ansi(false)
            .with_timer(timer)
            .with_writer(non_blocking);
        Some(file_writer)
    } else {
        None
    };

    let _tracing = tracing_subscriber::registry()
        .with(stdout_writer)
        .with(file_writer);

    if debug_mode {
        _tracing
            .with(tracing_subscriber::EnvFilter::new(
                "atrium=debug,tower_http=debug",
            ))
            .init();
    } else {
        _tracing
            .with(tracing_subscriber::EnvFilter::new("atrium=info"))
            .init();
    }

    guards
}

async fn redirect_http_to_https(handle: Handle) -> tokio::io::Result<()> {
    fn make_https(host: &str, uri: Uri) -> Result<Uri, BoxError> {
        let mut parts = uri.into_parts();
        parts.scheme = Some(axum::http::uri::Scheme::HTTPS);
        if parts.path_and_query.is_none() {
            parts.path_and_query = Some("/".parse()?);
        }
        parts.authority = Some(host.parse()?);
        Ok(Uri::from_parts(parts)?)
    }

    async fn redirect(Host(host): Host, uri: Uri) -> Result<Redirect, (StatusCode, &'static str)> {
        match make_https(&host, uri) {
            Ok(uri) => Ok(Redirect::permanent(&uri.to_string())),
            Err(_) => Err((StatusCode::BAD_REQUEST, "redirect to HTTPS failed")),
        }
    }

    let addr = format!("[::]:{}", 80)
        .parse::<std::net::SocketAddr>()
        .unwrap();
    axum_server::bind(addr)
        .handle(handle)
        .serve(redirect.into_make_service())
        .await
}
