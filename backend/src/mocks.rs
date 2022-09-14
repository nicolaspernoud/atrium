use axum::{routing::get, Router};

use std::net::TcpListener;

pub async fn mock_proxied_server(listener: TcpListener) {
    let port = listener.local_addr().unwrap().port();
    let message = format!("Hello world from mock server on port {port}!");
    let app = Router::new().route("/", get(move || async { message }));

    axum::Server::from_tcp(listener)
        .expect("failed to build mock server")
        .serve(app.into_make_service())
        .await
        .unwrap();
}
