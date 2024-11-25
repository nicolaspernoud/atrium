use crate::CONFIG_FILE;
use anyhow::Result;
use axum::{extract::connect_info::IntoMakeServiceWithConnectInfo, routing::MethodRouter};
use axum_server::{tls_rustls::RustlsConfig, Handle};
use std::{net::SocketAddr, path::Path};
use tokio::fs;
use tracing::info;

const CERT_PATH: &str = "cert.pem";
const KEY_PATH: &str = "key.pem";

pub async fn serve_with_self_signed_cert(
    ip: &str,
    port: &u16,
    handle: Handle,
    app: IntoMakeServiceWithConnectInfo<MethodRouter, SocketAddr>,
) -> anyhow::Result<()> {
    // Certificates
    let (cert, key) = load_or_generate_cert().await?;
    let rustls_config = RustlsConfig::from_pem(cert, key).await?;

    // Main server
    let addr = format!("{ip}:{}", port).parse::<std::net::SocketAddr>()?;

    // Start the server with TLS
    Ok(axum_server::bind_rustls(addr, rustls_config)
        .handle(handle)
        .serve(app)
        .await?)
}

/// Load or generate a self-signed certificate and private key
async fn load_or_generate_cert() -> Result<(Vec<u8>, Vec<u8>)> {
    if Path::new(CERT_PATH).exists() && Path::new(KEY_PATH).exists() {
        info!("Loading existing certificate and key from disk...");
        let cert = fs::read(CERT_PATH).await?;
        let key = fs::read(KEY_PATH).await?;
        Ok((cert, key))
    } else {
        info!("Generating new self-signed certificate and key...");
        let (cert, key) = generate_self_signed_cert().await?;
        persist_cert_and_key(&cert, &key).await?;
        Ok((cert, key))
    }
}

/// Generate a self-signed certificate and private key
async fn generate_self_signed_cert() -> Result<(Vec<u8>, Vec<u8>)> {
    let config = atrium::configuration::load_config(CONFIG_FILE).await?;
    let domains: Vec<String> = config.0.domains();
    // Generate a self-signed certificate using rcgen
    let cert = rcgen::generate_simple_self_signed(domains)?;
    Ok((
        cert.cert.pem().into_bytes(),
        cert.key_pair.serialize_pem().into_bytes(),
    ))
}

/// Persist the certificate and key to files
async fn persist_cert_and_key(cert: &[u8], key: &[u8]) -> Result<()> {
    info!("Persisting certificate and key to disk...");
    fs::write(CERT_PATH, cert).await?;
    fs::write(KEY_PATH, key).await?;
    Ok(())
}
