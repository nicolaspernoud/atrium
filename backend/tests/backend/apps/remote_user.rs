use atrium::{apps::AUTHENTICATED_USER_MAIL_HEADER, users::AUTH_COOKIE};

use crate::helpers::TestApp;

#[tokio::test]
async fn headers_reflect() {
    // Arrange
    let app = TestApp::spawn(None).await;

    // Act
    let response = app
        .client
        .get(format!("http://app1.atrium.io:{}/headers", app.port))
        .header("random-header", "PlayingFair")
        .send()
        .await
        .expect("failed to execute request");

    // Assert that we get the request headers reflected in the response
    assert_eq!(response.status(), 200);
    assert!(response.text().await.unwrap().contains("random-header"));
}

#[tokio::test]
async fn no_atrium_cookie() {
    // Arrange
    let app = TestApp::spawn(None).await;

    // Log as admin
    let response = app
        .client
        .post(format!("http://atrium.io:{}/auth/local", app.port))
        .body(r#"{"login":"admin","password":"password"}"#)
        .header("Content-Type", "application/json")
        .send()
        .await
        .expect("failed to execute request");
    assert!(response.status().is_success());
    // Act : try to access app as admin
    let response = app
        .client
        .get(format!("http://secured-app.atrium.io:{}/headers", app.port))
        .send()
        .await
        .expect("failed to execute request");

    // Assert that we get the request headers reflected in the response
    assert_eq!(response.status(), 200);
    let response_text = response.text().await.unwrap();
    assert!(response_text.contains(r#""host": "localhost""#));

    // Assert that we DO NOT get the authentication cookie header reflected in the response
    assert!(!response_text.contains(AUTH_COOKIE));
}

#[tokio::test]
async fn remote_user_removed() {
    // Arrange
    let app = TestApp::spawn(None).await;

    // Log as admin
    let response = app
        .client
        .post(format!("http://atrium.io:{}/auth/local", app.port))
        .body(r#"{"login":"admin","password":"password"}"#)
        .header("Content-Type", "application/json")
        .send()
        .await
        .expect("failed to execute request");
    assert!(response.status().is_success());
    // Act : try to access app as admin
    let response = app
        .client
        .get(format!("http://secured-app.atrium.io:{}/headers", app.port))
        .header(AUTHENTICATED_USER_MAIL_HEADER, "TryingToHack")
        .header("remote-User", "TryingToHack")
        .header("remote-user", "TryingToHack")
        .header("random-header", "PlayingFair")
        .send()
        .await
        .expect("failed to execute request");

    // Assert that we get the request headers reflected in the response
    assert_eq!(response.status(), 200);
    let response_text = response.text().await.unwrap();
    assert!(response_text.contains(r#""host": "localhost""#));
    assert!(response_text.contains("PlayingFair"));

    // Assert that we DO NOT get the remote user in the response
    assert!(!response_text.contains(AUTHENTICATED_USER_MAIL_HEADER));
    assert!(!response_text.contains("TryingToHack"));

    // Assert that we do not get the correct remote user for this app (as forward_user_mail is false)
    assert!(!response_text.contains("admin@atrium.io"));
}

#[tokio::test]
async fn remote_user_populated() {
    // Arrange
    let app = TestApp::spawn(None).await;

    // Log as admin
    let response = app
        .client
        .post(format!("http://atrium.io:{}/auth/local", app.port))
        .body(r#"{"login":"admin","password":"password"}"#)
        .header("Content-Type", "application/json")
        .send()
        .await
        .expect("failed to execute request");
    assert!(response.status().is_success());
    // Act : try to access app as admin
    let response = app
        .client
        .get(format!("http://app2.atrium.io:{}/headers", app.port))
        .header(AUTHENTICATED_USER_MAIL_HEADER, "TryingToHack")
        .send()
        .await
        .expect("failed to execute request");

    // Assert that we get the request headers reflected in the response
    assert_eq!(response.status(), 200);
    let response_text = response.text().await.unwrap();
    assert!(response_text.contains(r#""host": "localhost""#));

    // Assert that we only get the remote user populated by atrium in the response
    assert!(response_text.contains("admin@atrium.io"));
    assert!(!response_text.contains("TryingToHack"));
}
