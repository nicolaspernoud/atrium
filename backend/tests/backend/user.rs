use atrium::{sysinfo::SystemInfo, users::User};
use hyper::StatusCode;

use crate::helpers::TestApp;

#[tokio::test]
async fn list_services_api_for_unlogged_user_test() {
    // Arrange
    let app = TestApp::spawn(None).await;
    // Do not log

    // Act and Assert : Get the services (must fail)
    let response = app
        .client
        .get(format!(
            "http://atrium.io:{}/api/user/list_services",
            app.port
        ))
        .send()
        .await
        .expect("failed to execute request");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        response.text().await.unwrap(),
        "no user found or xsrf token not provided"
    );
}

#[tokio::test]
async fn list_services_api_for_normal_user_test() {
    // Arrange
    let app = TestApp::spawn(None).await;
    // Log as user
    let response = app
        .client
        .post(format!("http://atrium.io:{}/auth/local", app.port))
        .body(r#"{"login":"user","password":"password"}"#)
        .header("Content-Type", "application/json")
        .send()
        .await
        .expect("failed to execute request");
    assert_eq!(response.status(), StatusCode::OK);

    // Get XSRF token from response
    let xsrf_token: String = response
        .json::<atrium::users::AuthResponse>()
        .await
        .unwrap()
        .xsrf_token;

    // Get the services without XSRF token
    let response = app
        .client
        .get(format!(
            "http://atrium.io:{}/api/user/list_services",
            app.port
        ))
        .send()
        .await
        .expect("failed to execute request");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        response.text().await.unwrap(),
        "no user found or xsrf token not provided"
    );

    // Get the services with a wrong XSRF token
    let response = app
        .client
        .get(format!(
            "http://atrium.io:{}/api/user/list_services",
            app.port
        ))
        .header("xsrf-token", "randomtoken")
        .send()
        .await
        .expect("failed to execute request");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(response.text().await.unwrap(), "xsrf token doesn't match");

    // Act and Assert : Get the services with XSRF token
    let response = app
        .client
        .get(format!(
            "http://atrium.io:{}/api/user/list_services",
            app.port
        ))
        .header("xsrf-token", &xsrf_token)
        .send()
        .await
        .expect("failed to execute request");
    assert_eq!(response.status(), StatusCode::OK);
    let response_content = response.text().await.unwrap();
    // Assert that apps and davs for users are present
    println!("Response content is: {}", response_content);
    assert!(response_content.contains("app1"));
    assert!(response_content.contains("files1"));
    // Assert that apps and davs for admins are not present
    assert!(!response_content.contains("secured-app"));
    assert!(!response_content.contains("secured-files"));
    assert!(!response_content.contains("ff54fds6f"));
    assert!(!response_content.contains("ABCD123"));
    assert!(response_content.contains(r#""login":"REDACTED""#));
    assert!(response_content.contains(r#""password":"REDACTED""#));
    assert!(!response_content.contains(r#"passphrase"#));
}

#[tokio::test]
async fn get_share_token_test() {
    // Arrange
    let app = TestApp::spawn(None).await;
    // Log as user
    let response = app
        .client
        .post(format!("http://atrium.io:{}/auth/local", app.port))
        .body(r#"{"login":"user","password":"password"}"#)
        .header("Content-Type", "application/json")
        .send()
        .await
        .expect("failed to execute request");
    assert_eq!(response.status(), StatusCode::OK);

    // Get XSRF token from response
    let xsrf_token: String = response
        .json::<atrium::users::AuthResponse>()
        .await
        .unwrap()
        .xsrf_token;

    // Act and Assert : Get the a share token for an unexisting host
    let response = app
        .client
        .post(format!(
            "http://atrium.io:{}/api/user/get_share_token",
            app.port
        ))
        .header("Content-Type", "application/json")
        .header("xsrf-token", &xsrf_token)
        .body(r#"{"hostname":"files0.atrium.io","path":"/file1"}"#)
        .send()
        .await
        .expect("failed to execute request");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    // Act and Assert : Get the a share token for an host which the user has no rights for
    let response = app
        .client
        .post(format!(
            "http://atrium.io:{}/api/user/get_share_token",
            app.port
        ))
        .header("Content-Type", "application/json")
        .header("xsrf-token", &xsrf_token)
        .body(r#"{"hostname":"files2.atrium.io","path":"/file1"}"#)
        .send()
        .await
        .expect("failed to execute request");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    // Act and Assert : Get the a share token for an host which the user has the rights for
    let response = app
        .client
        .post(format!(
            "http://atrium.io:{}/api/user/get_share_token",
            app.port
        ))
        .header("Content-Type", "application/json")
        .header("xsrf-token", &xsrf_token)
        .body(r#"{"hostname":"files1.atrium.io","path":"/file1"}"#)
        .send()
        .await
        .expect("failed to execute request");
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn use_share_token_test() {
    // Arrange
    let app = TestApp::spawn(None).await;
    // Log as admin (could be an user, it is just because the secured test app service is reserved to admins)
    let response = app
        .client
        .post(format!("http://atrium.io:{}/auth/local", app.port))
        .body(r#"{"login":"admin","password":"password"}"#)
        .header("Content-Type", "application/json")
        .send()
        .await
        .expect("failed to execute request");
    assert_eq!(response.status(), StatusCode::OK);

    // Get XSRF token from response
    let xsrf_token: String = response
        .json::<atrium::users::AuthResponse>()
        .await
        .unwrap()
        .xsrf_token;

    // Get the a share token for an host which the user has the rights for
    let response = app
        .client
        .post(format!(
            "http://atrium.io:{}/api/user/get_share_token",
            app.port
        ))
        .header("Content-Type", "application/json")
        .header("xsrf-token", &xsrf_token)
        .body(r#"{"hostname":"secured-files.atrium.io","path":"/dira/file2"}"#)
        .send()
        .await
        .expect("failed to execute request");
    assert_eq!(response.status(), StatusCode::OK);
    let share_token = response.text().await.unwrap();
    let share_token = share_token.split('=').collect::<Vec<_>>()[1];

    // Create a client without cookie store
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .resolve(
            "atrium.io",
            format!("127.0.0.1:{}", app.port).parse().unwrap(),
        )
        .resolve(
            "secured-files.atrium.io",
            format!("127.0.0.1:{}", app.port).parse().unwrap(),
        )
        .resolve(
            "secured-files-2.atrium.io",
            format!("127.0.0.1:{}", app.port).parse().unwrap(),
        )
        .cookie_store(false)
        .build()
        .unwrap();

    // Try to get the file without share token (must fail)
    let url = format!("http://secured-files.atrium.io:{}/dira/file2", app.port);
    let resp = client.get(url).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // Try to use the share token for the right host but the wrong path (must fail)
    let url = format!(
        "http://secured-files.atrium.io:{}/dira/file1?token={share_token}",
        app.port
    );
    let resp = client.get(url).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // Try to use the share token for the wrong host but the right path (must fail)
    let url = format!(
        "http://secured-files-2.atrium.io:{}/dira/file2?token={share_token}",
        app.port
    );
    let resp = client.get(url).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // Try to use the share token for the right host and the right path (must pass)
    let url = format!(
        "http://secured-files.atrium.io:{}/dira/file2?token={share_token}",
        app.port
    );
    let resp = client.get(url).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Wait for 2 seconds and try to reuse the share token which must be expired
    std::thread::sleep(std::time::Duration::from_secs(3));
    let url = format!(
        "http://secured-files.atrium.io:{}/dira/file2?token={share_token}",
        app.port
    );
    let resp = client.get(url).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn get_system_info_test() {
    // Arrange
    let app = TestApp::spawn(None).await;
    // Log as user
    let response = app
        .client
        .post(format!("http://atrium.io:{}/auth/local", app.port))
        .body(r#"{"login":"user","password":"password"}"#)
        .header("Content-Type", "application/json")
        .send()
        .await
        .expect("failed to execute request");
    assert_eq!(response.status(), StatusCode::OK);

    // Get XSRF token from response
    let xsrf_token: String = response
        .json::<atrium::users::AuthResponse>()
        .await
        .unwrap()
        .xsrf_token;

    // Get the a share token for an host which the user has the rights for
    let response = app
        .client
        .get(format!(
            "http://atrium.io:{}/api/user/system_info",
            app.port
        ))
        .header("xsrf-token", &xsrf_token)
        .send()
        .await
        .expect("failed to execute request");
    assert_eq!(response.status(), StatusCode::OK);
    let system_info = response.json::<SystemInfo>().await.unwrap();
    assert!(system_info.used_memory <= system_info.total_memory);
}

#[tokio::test]
async fn whoami_test() {
    // Arrange
    let app = TestApp::spawn(None).await;
    // Log as user
    let response = app
        .client
        .post(format!("http://atrium.io:{}/auth/local", app.port))
        .body(r#"{"login":"user","password":"password"}"#)
        .header("Content-Type", "application/json")
        .send()
        .await
        .expect("failed to execute request");
    assert_eq!(response.status(), StatusCode::OK);

    // Get XSRF token from response
    let xsrf_token: String = response
        .json::<atrium::users::AuthResponse>()
        .await
        .unwrap()
        .xsrf_token;

    // Act and Assert : Test that the whoami route sends back who we are
    let response = app
        .client
        .get(format!("http://atrium.io:{}/api/user/whoami", app.port))
        .header("xsrf-token", &xsrf_token)
        .header("Content-Type", "application/json")
        .send()
        .await
        .expect("failed to execute request");
    assert_eq!(response.status(), StatusCode::OK);
    let user = response.json::<User>().await.unwrap();
    assert_eq!(user.login, "user");
    assert_eq!(user.password, "REDACTED");
}
