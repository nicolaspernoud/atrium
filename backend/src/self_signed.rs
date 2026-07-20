use crate::CONFIG_FILE;
use atrium::errors::Error;
use axum::{extract::connect_info::IntoMakeServiceWithConnectInfo, routing::MethodRouter};
use axum_server::{Handle, tls_rustls::RustlsConfig};
use std::net::SocketAddr;
use tokio::fs;
use tracing::info;

const CERT_PATH: &str = "cert.pem";
const KEY_PATH: &str = "key.pem";

pub async fn serve_with_self_signed_cert(
    ip: &str,
    port: &u16,
    handle: Handle<SocketAddr>,
    app: IntoMakeServiceWithConnectInfo<MethodRouter, SocketAddr>,
) -> Result<(), Error> {
    // Certificates
    let (cert, key) = load_or_generate_cert().await?;
    let rustls_config = RustlsConfig::from_pem(cert, key).await?;

    // Main server
    let addr = format!("{ip}:{port}").parse::<std::net::SocketAddr>()?;

    // Start the server with TLS
    Ok(axum_server::bind_rustls(addr, rustls_config)
        .handle(handle)
        .serve(app)
        .await?)
}

/// Load or generate a self-signed certificate and private key
async fn load_or_generate_cert() -> Result<(Vec<u8>, Vec<u8>), Error> {
    match (fs::read(CERT_PATH).await, fs::read(KEY_PATH).await) {
        (Ok(cert), Ok(key)) => Ok((cert, key)),
        (Err(e1), Err(e2))
            if e1.kind() == std::io::ErrorKind::NotFound
                || e2.kind() == std::io::ErrorKind::NotFound =>
        {
            let (cert, key) = generate_self_signed_cert().await?;
            persist_cert_and_key(&cert, &key).await?;
            Ok((cert, key))
        }
        (Err(e), _) | (_, Err(e)) => Err(e.into()),
    }
}

/// Generate a self-signed certificate and private key
async fn generate_self_signed_cert() -> Result<(Vec<u8>, Vec<u8>), Error> {
    let config = atrium::configuration::load_config(CONFIG_FILE).await?;
    let domains: Vec<String> = config.0.domains();
    // Generate a self-signed certificate using rcgen
    let cert = rcgen::generate_simple_self_signed(domains)?;
    Ok((
        cert.cert.pem().into_bytes(),
        cert.signing_key.serialize_pem().into_bytes(),
    ))
}

/// Persist the certificate and key to files
async fn persist_cert_and_key(cert: &[u8], key: &[u8]) -> Result<(), Error> {
    info!("Persisting certificate and key to disk...");
    fs::write(CERT_PATH, cert).await?;
    fs::write(KEY_PATH, key).await?;
    Ok(())
}
