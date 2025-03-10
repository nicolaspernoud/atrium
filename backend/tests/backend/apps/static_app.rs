use crate::helpers::TestApp;

#[tokio::test]
async fn static_app_test() {
    // Arrange
    let app = TestApp::spawn(None).await;

    println!(
        "Current directory is: {:?}",
        std::env::current_dir().unwrap()
    );

    // Act
    let response = app
        .client
        .get(format!("http://static-app.atrium.io:{}", app.port))
        .send()
        .await
        .expect("failed to execute request");

    // Assert
    assert!(response.status().is_success());
    assert!(
        response
            .text()
            .await
            .unwrap()
            .contains("This is statically served !")
    );

    // Act
    let response = app
        .client
        .get(format!(
            "http://static-app.atrium.io:{}/lorem.txt",
            app.port
        ))
        .send()
        .await
        .expect("failed to execute request");

    // Assert
    assert!(response.status().is_success());
    assert!(response.headers().contains_key("Content-Security-Policy"));
    assert!(
        response
            .text()
            .await
            .unwrap()
            .contains("Lorem ipsum dolor sit amet")
    );
}

#[tokio::test]
async fn secured_static_app_test() {
    // Arrange
    let app = TestApp::spawn(None).await;

    // Act : try to access app as unlogged user
    let response = app
        .client
        .get(format!("http://secured-static-app.atrium.io:{}", app.port))
        .send()
        .await
        .expect("failed to execute request");

    // Assert that is impossible (redirected to login page)
    assert_eq!(response.status(), 302);
    assert_eq!(response.text().await.unwrap(), "");

    // Log as normal user
    let response = app
        .client
        .post(format!("http://atrium.io:{}/auth/local", app.port))
        .body(r#"{"login":"user","password":"password"}"#)
        .header("Content-Type", "application/json")
        .send()
        .await
        .expect("failed to execute request");
    assert!(response.status().is_success());
    // Act : try to access app as logged user
    let response = app
        .client
        .get(format!("http://secured-static-app.atrium.io:{}", app.port))
        .send()
        .await
        .expect("failed to execute request");
    // Assert that is impossible
    assert!(response.status() == 403);
    assert_eq!(response.text().await.unwrap(), "");

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
        .get(format!("http://secured-static-app.atrium.io:{}", app.port))
        .send()
        .await
        .expect("failed to execute request");
    // Assert that is possible
    assert!(response.status().is_success());
    assert!(
        response
            .text()
            .await
            .unwrap()
            .contains("This is statically served !")
    );
}
