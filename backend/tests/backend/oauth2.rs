use atrium::{
    configuration::{Config, OpenIdConfig},
    mocks::mock_oauth2_server,
};
use hyper::StatusCode;

use crate::helpers::TestApp;

#[tokio::test]
async fn log_with_oidc_as_user() {
    // Arrange
    let app = TestApp::spawn(None).await;
    // Create a client with redirect enabled
    let client = reqwest::Client::builder()
        .resolve("atrium.io", format!("[::1]:{}", app.port).parse().unwrap())
        .cookie_store(true)
        .build()
        .unwrap();
    // Log as user
    let response = client
        .get(format!("http://atrium.io:{}/auth/oauth2login", app.port))
        .send()
        .await
        .expect("failed to execute request");
    assert!(response.status().is_success());
    assert!(response.url().as_str().contains("is_admin=false"));
    assert!(response.text().await.unwrap().contains("Auth OK"));
}

#[tokio::test]
async fn log_with_oidc_wrong_state() {
    // Arrange
    let mock_oauth2_listener =
        std::net::TcpListener::bind("[::]:0").expect("failed to bind to random port");
    let mock_oauth2_port = mock_oauth2_listener.local_addr().unwrap().port();
    tokio::spawn(mock_oauth2_server(mock_oauth2_listener));
    let config = Config {
        openid_config: Some(OpenIdConfig {
            client_id: "dummy".to_owned(),
            client_secret: "dummy".to_owned(),
            auth_url: format!("http://localhost:{mock_oauth2_port}/authorize_wrong_state"),
            token_url: format!("http://localhost:{mock_oauth2_port}/token"),
            userinfo_url: format!("http://localhost:{mock_oauth2_port}/userinfo"),
            admins_group: Some("TO_BECOME_ADMINS".to_owned()),
        }),
        ..Default::default()
    };
    let app = TestApp::spawn(Some(config)).await;
    // Create a client with redirect enabled
    let client = reqwest::Client::builder()
        .resolve("atrium.io", format!("[::1]:{}", app.port).parse().unwrap())
        .cookie_store(true)
        .build()
        .unwrap();
    // Log as user
    let response = client
        .get(format!("http://atrium.io:{}/auth/oauth2login", app.port))
        .send()
        .await
        .expect("failed to execute request");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn log_with_oidc_as_admin() {
    // Arrange
    let mock_oauth2_listener =
        std::net::TcpListener::bind("[::]:0").expect("failed to bind to random port");
    let mock_oauth2_port = mock_oauth2_listener.local_addr().unwrap().port();
    tokio::spawn(mock_oauth2_server(mock_oauth2_listener));
    let config = Config {
        openid_config: Some(OpenIdConfig {
            client_id: "dummy".to_owned(),
            client_secret: "dummy".to_owned(),
            auth_url: format!("http://localhost:{mock_oauth2_port}/authorize"),
            token_url: format!("http://localhost:{mock_oauth2_port}/token"),
            userinfo_url: format!("http://localhost:{mock_oauth2_port}/admininfo"),
            admins_group: Some("TO_BECOME_ADMINS".to_owned()),
        }),
        ..Default::default()
    };
    let app = TestApp::spawn(Some(config)).await;
    // Create a client with redirect enabled
    let client = reqwest::Client::builder()
        .resolve("atrium.io", format!("[::1]:{}", app.port).parse().unwrap())
        .cookie_store(true)
        .build()
        .unwrap();
    // Log as user
    let response = client
        .get(format!("http://atrium.io:{}/auth/oauth2login", app.port))
        .send()
        .await
        .expect("failed to execute request");
    assert!(response.status().is_success());
    assert!(response.url().as_str().contains("is_admin=true"));
    assert!(response.text().await.unwrap().contains("Auth OK"));
}
