use atrium::{
    configuration::{Config, OpenIdConfig},
    mocks::mock_oauth2_server,
    users::User,
};
use hyper::StatusCode;

use crate::helpers::TestApp;

#[tokio::test]
async fn log_with_oidc_as_user() {
    // Arrange
    let app = TestApp::spawn(None).await;
    // Create a client with redirect enabled
    let client = reqwest::Client::builder()
        .resolve(
            "atrium.io",
            format!("127.0.0.1:{}", app.port).parse().unwrap(),
        )
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

    // Act and Assert : Test that the whoami route sends back who we are
    let response = client
        .get(format!("http://atrium.io:{}/api/user/whoami", app.port))
        .header("Content-Type", "application/json")
        .send()
        .await
        .expect("failed to execute request");
    assert_eq!(response.status(), StatusCode::OK);
    let user = response.json::<User>().await.unwrap();
    assert_eq!(user.login, "USER");
    assert_eq!(user.password, "REDACTED");
    let info = user.info.unwrap();
    assert_eq!(info.given_name, "Us");
    assert_eq!(info.family_name, "ER");
    assert_eq!(info.email, "user@atrium.io");
}

#[tokio::test]
async fn log_with_oidc_wrong_state() {
    // Arrange
    let mock_oauth2_listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("failed to bind to random port");
    let mock_oauth2_port = mock_oauth2_listener.local_addr().unwrap().port();
    tokio::spawn(mock_oauth2_server(mock_oauth2_listener));
    let config = Config {
        openid_config: Some(OpenIdConfig {
            auth_url: format!("http://localhost:{mock_oauth2_port}/authorize_wrong_state"),
            token_url: format!("http://localhost:{mock_oauth2_port}/token"),
            userinfo_url: format!("http://localhost:{mock_oauth2_port}/userinfo"),
            ..Default::default()
        }),
        ..Default::default()
    };
    let app = TestApp::spawn(Some(config)).await;
    // Create a client with redirect enabled
    let client = reqwest::Client::builder()
        .resolve(
            "atrium.io",
            format!("127.0.0.1:{}", app.port).parse().unwrap(),
        )
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
        std::net::TcpListener::bind("127.0.0.1:0").expect("failed to bind to random port");
    let mock_oauth2_port = mock_oauth2_listener.local_addr().unwrap().port();
    tokio::spawn(mock_oauth2_server(mock_oauth2_listener));
    let config = Config {
        openid_config: Some(OpenIdConfig {
            auth_url: format!("http://localhost:{mock_oauth2_port}/authorize"),
            token_url: format!("http://localhost:{mock_oauth2_port}/token"),
            userinfo_url: format!("http://localhost:{mock_oauth2_port}/admininfo"),
            ..Default::default()
        }),
        ..Default::default()
    };
    let app = TestApp::spawn(Some(config)).await;
    // Create a client with redirect enabled
    let client = reqwest::Client::builder()
        .resolve(
            "atrium.io",
            format!("127.0.0.1:{}", app.port).parse().unwrap(),
        )
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

#[tokio::test]
async fn configuration_from_well_known_override() {
    // Arrange
    let mock_oauth2_listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("failed to bind to random port");
    let mock_oauth2_port = mock_oauth2_listener.local_addr().unwrap().port();
    tokio::spawn(mock_oauth2_server(mock_oauth2_listener));
    let config = Config {
        openid_config: Some(OpenIdConfig {
            userinfo_url: format!("http://localhost:{mock_oauth2_port}/admininfo"),
            openid_configuration_url: Some(format!(
                "http://localhost:{mock_oauth2_port}/.well-known/openid-configuration"
            )),
            ..Default::default()
        }),
        ..Default::default()
    };
    let app = TestApp::spawn(Some(config)).await;
    // Create a client with redirect enabled
    let client = reqwest::Client::builder()
        .resolve(
            "atrium.io",
            format!("127.0.0.1:{}", app.port).parse().unwrap(),
        )
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
async fn oidc_is_available() {
    // Arrange
    let app = TestApp::spawn(None).await;
    // Create a client with redirect enabled
    let client = reqwest::Client::builder()
        .resolve(
            "atrium.io",
            format!("127.0.0.1:{}", app.port).parse().unwrap(),
        )
        .cookie_store(true)
        .build()
        .unwrap();
    // Test that OIDC is available
    let response = client
        .get(format!(
            "http://atrium.io:{}/auth/oauth2available",
            app.port
        ))
        .send()
        .await
        .expect("failed to execute request");
    assert!(response.status().is_success());
}

#[tokio::test]
async fn oidc_is_not_available() {
    // Arrange
    let app = TestApp::spawn(None).await;
    // Create a client with redirect enabled
    let client = reqwest::Client::builder()
        .resolve(
            "atrium.io",
            format!("127.0.0.1:{}", app.port).parse().unwrap(),
        )
        .cookie_store(true)
        .build()
        .unwrap();
    let config = Config {
        openid_config: None,
        ..Default::default()
    };
    let app = TestApp::spawn(Some(config)).await;
    // Test that OIDC is not available
    let response = client
        .get(format!(
            "http://atrium.io:{}/auth/oauth2available",
            app.port
        ))
        .send()
        .await
        .expect("failed to execute request");
    assert!(!response.status().is_success());
}
