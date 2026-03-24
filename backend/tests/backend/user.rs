use atrium::{
    sysinfo::SystemInfo,
    auth::{User, share::ShareResponse},
};
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
        "xsrf token not provided or not matching"
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
        .json::<atrium::auth::AuthResponse>()
        .await
        .unwrap()
        .xsrf_token
        .unwrap();

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
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        response.text().await.unwrap(),
        "xsrf token not provided or not matching"
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
    assert_eq!(
        response.text().await.unwrap(),
        "xsrf token not provided or not matching"
    );

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
        .json::<atrium::auth::AuthResponse>()
        .await
        .unwrap()
        .xsrf_token
        .unwrap();

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
    // Act and Assert : Get the a share token for an unsecured host
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
    assert_eq!(response.status(), StatusCode::OK);
    // Act and Assert : Get the a share token for an host which the user has no rights for
    let response = app
        .client
        .post(format!(
            "http://atrium.io:{}/api/user/get_share_token",
            app.port
        ))
        .header("Content-Type", "application/json")
        .header("xsrf-token", &xsrf_token)
        .body(r#"{"hostname":"secured-files.atrium.io","path":"/file1"}"#)
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
        .json::<atrium::auth::AuthResponse>()
        .await
        .unwrap()
        .xsrf_token
        .unwrap();

    let dir = "A' directory with special chars like é or è";
    let resource = format!("{dir}/A' file with special chars like é or è.txt");

    // Create a dir with special characters
    let url = format!("http://secured-files.atrium.io:{}/{dir}", app.port);
    let resp = crate::davs::mkcol(&app, &url)
        .header("xsrf-token", &xsrf_token)
        .send()
        .await
        .expect("could not create directory");
    assert_eq!(resp.status(), 201);

    // Create a file with special characters
    let url = format!("http://secured-files.atrium.io:{}/{resource}", app.port);
    let resp = app
        .client
        .put(&url)
        .header("xsrf-token", &xsrf_token)
        .body(b"abc".to_vec())
        .send()
        .await
        .expect("could not create file");
    assert_eq!(resp.status(), 201);

    // Get the a share token for an host which the user has the rights for
    let response = app
        .client
        .post(format!(
            "http://atrium.io:{}/api/user/get_share_token",
            app.port
        ))
        .header("Content-Type", "application/json")
        .header("xsrf-token", &xsrf_token)
        .body(format!(
            r#"{{"hostname":"secured-files.atrium.io","path":"/{resource}"}}"#
        ))
        .send()
        .await
        .expect("failed to execute request");
    assert_eq!(response.status(), StatusCode::OK);
    let share_token = response.json::<ShareResponse>().await.unwrap().token;

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
    let url = format!("http://secured-files.atrium.io:{}/{resource}", app.port);
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
        "http://secured-files-2.atrium.io:{}/{resource}?token={share_token}",
        app.port
    );
    let resp = client.get(url).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // Try to use the share token for the right host and the right path (must pass)
    let url = format!(
        "http://secured-files.atrium.io:{}/{resource}?token={share_token}",
        app.port
    );
    let resp = client.get(url).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Wait for 2 seconds and try to reuse the share token which must be expired
    std::thread::sleep(std::time::Duration::from_secs(3));
    let url = format!(
        "http://secured-files.atrium.io:{}/{resource}?token={share_token}",
        app.port
    );
    let resp = client.get(url).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn share_token_security_test() {
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
    assert_eq!(response.status(), StatusCode::OK);

    let xsrf_token: String = response
        .json::<atrium::auth::AuthResponse>()
        .await
        .unwrap()
        .xsrf_token
        .unwrap();

    // 1. Create a read-only share token for /dira
    let response = app
        .client
        .post(format!(
            "http://atrium.io:{}/api/user/get_share_token",
            app.port
        ))
        .header("Content-Type", "application/json")
        .header("xsrf-token", &xsrf_token)
        .body(r#"{"hostname":"secured-files.atrium.io","path":"/dira","writable":false,"share_for_days":1}"#)
        .send()
        .await
        .expect("failed to execute request");
    assert_eq!(response.status(), StatusCode::OK);
    let share_response = response.json::<ShareResponse>().await.unwrap();
    let share_token = share_response.token;
    let share_xsrf_token = share_response.xsrf_token.unwrap();

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .resolve(
            "secured-files.atrium.io",
            format!("127.0.0.1:{}", app.port).parse().unwrap(),
        )
        .resolve(
            "atrium.io",
            format!("127.0.0.1:{}", app.port).parse().unwrap(),
        )
        .cookie_store(false)
        .build()
        .unwrap();

    // 2. Verify read-only: GET should work, PUT should fail
    let url_get = format!(
        "http://secured-files.atrium.io:{}/dira/file1?token={share_token}",
        app.port
    );
    let resp = client.get(url_get).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let url_put = format!(
        "http://secured-files.atrium.io:{}/dira/file1?token={share_token}",
        app.port
    );
    let resp = client.put(url_put).body("data").send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // 3. Try to generate a NEW share token from the existing share token
    // a) Try to make it read-write (should fail)
    let resp = client
        .post(format!(
            "http://atrium.io:{}/api/user/get_share_token",
            app.port
        ))
        .header("Content-Type", "application/json")
        .header("xsrf-token", &share_xsrf_token)
        .header("Cookie", format!("ATRIUM_AUTH={share_token}"))
        .body(r#"{"hostname":"secured-files.atrium.io","path":"/dira","writable":true,"share_for_days":1}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // b) Try to change hostname (should fail)
    let resp = client
        .post(format!(
            "http://atrium.io:{}/api/user/get_share_token",
            app.port
        ))
        .header("Content-Type", "application/json")
        .header("xsrf-token", &share_xsrf_token)
        .header("Cookie", format!("ATRIUM_AUTH={share_token}"))
        .body(
            r#"{"hostname":"files2.atrium.io","path":"/dira","writable":false,"share_for_days":1}"#,
        )
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // c) Try to share a subpath (should work)
    let resp = client
        .post(format!(
            "http://atrium.io:{}/api/user/get_share_token",
            app.port
        ))
        .header("Content-Type", "application/json")
        .header("xsrf-token", &share_xsrf_token)
        .header("Cookie", format!("ATRIUM_AUTH={share_token}"))
        .body(r#"{"hostname":"secured-files.atrium.io","path":"/dira/subdira","writable":false,"share_for_days":1}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // d) Try to share a parent path (should fail)
    let resp = client
        .post(format!(
            "http://atrium.io:{}/api/user/get_share_token",
            app.port
        ))
        .header("Content-Type", "application/json")
        .header("xsrf-token", &share_xsrf_token)
        .header("Cookie", format!("ATRIUM_AUTH={share_token}"))
        .body(r#"{"hostname":"secured-files.atrium.io","path":"/","writable":false,"share_for_days":1}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // e) Try to share a parent path with .. (should fail)
    let resp = client
        .post(format!(
            "http://atrium.io:{}/api/user/get_share_token",
            app.port
        ))
        .header("Content-Type", "application/json")
        .header("xsrf-token", &share_xsrf_token)
        .header("Cookie", format!("ATRIUM_AUTH={share_token}"))
        .body(r#"{"hostname":"secured-files.atrium.io","path":"/dira/..","writable":false,"share_for_days":1}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // 4. Try to access the services list (should fail)
    let response = app
        .client
        .get(format!(
            "http://atrium.io:{}/api/user/list_services",
            app.port
        ))
        .header("xsrf-token", &share_xsrf_token)
        .header("Cookie", format!("ATRIUM_AUTH={share_token}"))
        .send()
        .await
        .expect("failed to execute request");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    // 5. Try to access a secured app (should fail)
    let resp = app
        .client
        .get(format!("http://secured-app.atrium.io:{}", app.port))
        .header("xsrf-token", &share_xsrf_token)
        .header("Cookie", format!("ATRIUM_AUTH={share_token}"))
        .send()
        .await
        .expect("failed to execute request");
    // Assert that is impossible
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
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
        .json::<atrium::auth::AuthResponse>()
        .await
        .unwrap()
        .xsrf_token
        .unwrap();

    // Test the system info route
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

    // Act and Assert : Test that the whoami route sends back who we are
    let response = app
        .client
        .get(format!("http://atrium.io:{}/api/user/whoami", app.port))
        .header("Content-Type", "application/json")
        .send()
        .await
        .expect("failed to execute request");
    assert_eq!(response.status(), StatusCode::OK);
    let user = response.json::<User>().await.unwrap();
    assert_eq!(user.login, "user");
    assert_eq!(user.password, "REDACTED");
}

#[tokio::test]
async fn logout_test() {
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

    // Test that we can log out
    let response = app
        .client
        .get(format!("http://atrium.io:{}/auth/logout", app.port))
        .send()
        .await
        .expect("failed to execute request");
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response
            .headers()
            .get("set-cookie")
            .unwrap()
            .to_str()
            .unwrap()
            .contains("ATRIUM_AUTH=; Path=/; Domain=atrium.io; Max-Age=0;")
    );
}
