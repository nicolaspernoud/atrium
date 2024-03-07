use async_tungstenite::tokio::{accept_async, connect_async};
use atrium::{apps::App, configuration::Config};
use futures::SinkExt;
use http::{
    header::{CONNECTION, HOST, UPGRADE},
    HeaderValue,
};
use tokio_stream::StreamExt;

use tracing::debug;
use tungstenite::{client::IntoClientRequest, Message};

use crate::helpers::TestApp;

#[tokio::test]
async fn test_get_error_502() {
    // Arrange (proxy to unexisting service)
    let config = Config {
        apps: vec![App {
            id: 1,
            name: "App 1".to_owned(),
            icon: "web_asset".to_owned(),
            color: 4292030255,
            is_proxy: true,
            host: "app1".to_owned(),
            target: format!("localhost:9999"),
            secured: false,
            login: "".to_owned(),
            password: "".to_owned(),
            openpath: "".to_owned(),
            roles: vec![],
            inject_security_headers: false,
            ..Default::default()
        }],
        ..Default::default()
    };
    let app = TestApp::spawn(Some(config)).await;

    // Act
    let response = app
        .client
        .get(format!("http://app1.atrium.io:{}/502", app.port))
        .header("keep-alive", "treu")
        .send()
        .await
        .expect("failed to execute request");

    // Assert
    assert_eq!(response.status(), 502);
}

#[tokio::test]
async fn test_upgrade_mismatch() {
    // Arrange
    let app = TestApp::spawn(None).await;

    // Act
    let response = app
        .client
        .get(format!("http://app1.atrium.io:{}/ws", app.port))
        .header(CONNECTION, "Upgrade")
        .header(UPGRADE, "websocket")
        .send()
        .await
        .expect("failed to execute request");

    // Assert
    assert_eq!(response.status(), 502);
}

#[tokio::test]
async fn test_upgrade_unrequested() {
    // Arrange
    let app = TestApp::spawn(None).await;

    // Act
    let response = app
        .client
        .get(format!("http://app1.atrium.io:{}/ws", app.port))
        .send()
        .await
        .expect("failed to execute request");

    // Assert
    assert_eq!(response.status(), 502);
}

#[tokio::test]
async fn proxy_test() {
    // Arrange
    let app = TestApp::spawn(None).await;

    // Act
    let response = app
        .client
        .get(format!("http://atrium.io:{}", app.port))
        .send()
        .await
        .expect("failed to execute request");

    // Assert
    assert_eq!(response.status(), 200);
    assert!(response.headers().contains_key("Content-Security-Policy"));
    assert!(response
        .text()
        .await
        .unwrap()
        .contains("Hello world from main server !"));

    // Act
    let response = app
        .client
        .get(format!("http://app1.atrium.io:{}", app.port))
        .send()
        .await
        .expect("failed to execute request");

    // Assert
    assert_eq!(response.status(), 200);
    assert!(!response.headers().contains_key("Content-Security-Policy"));
    assert!(response
        .text()
        .await
        .unwrap()
        .contains("Hello world from mock server"));

    // Act
    let response = app
        .client
        .get(format!("http://app1.atrium.io:{}/foo", app.port))
        .send()
        .await
        .expect("failed to execute request");

    // Assert
    assert!(response.status().is_success());
    assert!(!response.headers().contains_key("Content-Security-Policy"));
    let response_content = response.text().await.unwrap();
    debug!("Response : {}", response_content);
    assert!(response_content.contains("bar"));

    // Act
    let response = app
        .client
        .get(format!("http://app2.atrium.io:{}", app.port))
        .send()
        .await
        .expect("failed to execute request");

    // Assert
    assert!(response.status().is_success());
    assert!(response.headers().contains_key("Content-Security-Policy"));
    assert!(response
        .text()
        .await
        .unwrap()
        .contains("Hello world from mock server"));
}

#[tokio::test]
async fn test_websocket() {
    // Arrange
    // Set up websocket server to proxy
    let mock_ws_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("failed to bind to random port");
    let mock_ws_listener_port = mock_ws_listener.local_addr().unwrap().port();

    let _ = tokio::spawn(async move {
        if let Ok((stream, _)) = mock_ws_listener.accept().await {
            let mut websocket = accept_async(stream).await.unwrap();

            let msg = websocket.next().await.unwrap().unwrap();
            assert!(
                matches!(&msg, Message::Ping(inner) if inner == "hello".as_bytes()),
                "did not get ping, but: {:?}",
                msg
            );
            // Tungstenite will auto send a Pong as a response to a Ping
            websocket
                .send(Message::Text("Handshake OK".to_string()))
                .await
                .unwrap();
        }
    });

    // Set up reverse proxy
    let config = Config {
        apps: vec![App {
            id: 1,
            name: "App 1".to_owned(),
            icon: "web_asset".to_owned(),
            color: 4292030255,
            is_proxy: true,
            host: "app1".to_owned(),
            target: format!("localhost:{mock_ws_listener_port}"),
            secured: false,
            login: "".to_owned(),
            password: "".to_owned(),
            openpath: "".to_owned(),
            roles: vec![],
            inject_security_headers: false,
            ..Default::default()
        }],
        ..Default::default()
    };
    let app = TestApp::spawn(Some(config)).await;

    // Act : websocket request
    let mut request = reqwest::Url::parse(&format!("ws://127.0.0.1:{}", app.port))
        .unwrap()
        .into_client_request()
        .unwrap();
    request.headers_mut().insert(
        HOST,
        HeaderValue::from_str(&format!("app1.atrium.io:{}", app.port)).unwrap(),
    );
    let (mut client, _) = connect_async(request).await.unwrap();

    client.send(Message::Ping("hello".into())).await.unwrap();
    let msg = client.next().await.unwrap().unwrap();

    // Assert
    assert!(
        matches!(&msg, Message::Text(inner) if inner == "Handshake OK"),
        "did not get text, but {:?}",
        msg
    );

    let msg = client.next().await.unwrap().unwrap();

    assert!(
        matches!(&msg, Message::Pong(inner) if inner == "hello".as_bytes()),
        "did not get pong, but {:?}",
        msg
    );
}
