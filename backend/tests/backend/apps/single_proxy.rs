use atrium::{
    apps::App,
    configuration::{Config, OpenIdConfig},
    mocks::{mock_oauth2_server, mock_proxied_server},
};
use reqwest::redirect::Policy;
use tokio::net::TcpListener;

use crate::helpers::TestApp;

#[tokio::test]
async fn unsecured_single_proxy_test() {
    // Arrange
    let mock_listener =
        TcpListener::bind("127.0.0.1:0").await.expect("failed to bind to random port");
    let mock_port = mock_listener.local_addr().unwrap().port();
    tokio::spawn(mock_proxied_server(mock_listener));
    let config = Config {
        single_proxy: true,
        apps: vec![App {
            id: 1,
            is_proxy: true,
            host: "app1".to_owned(),
            target: format!("localhost:{mock_port}"),
            ..Default::default()
        }],
        ..Default::default()
    };
    let app = TestApp::spawn(Some(config)).await;
    let client = reqwest::Client::builder()
        .resolve(
            "atrium.io",
            format!("127.0.0.1:{}", app.port).parse().unwrap(),
        )
        .cookie_store(true)
        .build()
        .unwrap();

    // Act
    let response = client
        .get(format!("http://atrium.io:{}", app.port))
        .send()
        .await
        .expect("failed to execute request");

    // Assert
    assert!(response.status().is_success());
    assert!(response.text().await.unwrap().contains(&format!(
        "Hello world from mock server on port {mock_port}!"
    )));

    // Act
    let response = client
        .get(format!("http://atrium.io:{}/healthcheck", app.port))
        .send()
        .await
        .expect("failed to execute request");
    // Assert
    assert!(response.status().is_success());
    assert!(response.text().await.unwrap().contains(&"OK".to_string()));
}

#[tokio::test]
async fn secured_single_proxy_test() {
    // Arrange
    let mock_oauth2_listener =
        TcpListener::bind("127.0.0.1:0").await.expect("failed to bind to random port");
    let mock_oauth2_port = mock_oauth2_listener.local_addr().unwrap().port();
    tokio::spawn(mock_oauth2_server(mock_oauth2_listener));
    let mock_listener =
        TcpListener::bind("127.0.0.1:0").await.expect("failed to bind to random port");
    let mock_port = mock_listener.local_addr().unwrap().port();
    tokio::spawn(mock_proxied_server(mock_listener));
    let config = Config {
        single_proxy: true,
        apps: vec![App {
            id: 1,
            is_proxy: true,
            host: "app1".to_owned(),
            secured: true,
            roles: vec!["ADMINS".to_owned(), "USERS".to_owned()],
            target: format!("localhost:{mock_port}"),
            ..Default::default()
        }],
        openid_config: Some(OpenIdConfig {
            openid_configuration_url: Some(format!(
                "http://localhost:{mock_oauth2_port}/.well-known/openid-configuration"
            )),
            ..Default::default()
        }),
        ..Default::default()
    };
    let app = TestApp::spawn(Some(config)).await;

    // Act
    let client = reqwest::Client::builder()
        .resolve(
            "atrium.io",
            format!("127.0.0.1:{}", app.port).parse().unwrap(),
        )
        .redirect(Policy::none())
        .cookie_store(true)
        .build()
        .unwrap();
    let response = client
        .get(format!("http://atrium.io:{}", app.port))
        .send()
        .await
        .expect("failed to execute request");

    // Assert redirect to OAuth2 login
    assert!(response.status().is_redirection());

    // Act
    let client = reqwest::Client::builder()
        .resolve(
            "atrium.io",
            format!("127.0.0.1:{}", app.port).parse().unwrap(),
        )
        .cookie_store(true)
        .build()
        .unwrap();

    let response = client
        .get(format!("http://atrium.io:{}", app.port))
        .send()
        .await
        .expect("failed to execute request");

    // Assert that service is available after redirects
    assert!(response.status().is_success());
    assert!(response.text().await.unwrap().contains(&format!(
        "Hello world from mock server on port {mock_port}!"
    )));
}
