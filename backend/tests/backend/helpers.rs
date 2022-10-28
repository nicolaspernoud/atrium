use reqwest::Client;
use std::{fs, net::SocketAddr};
use tokio::sync::broadcast;
use tracing::info;

use atrium::{
    apps::App,
    configuration::{Config, OnlyOfficeConfig, OpenIdConfig, TlsMode},
    davs::model::Dav,
    mocks::{mock_oauth2_server, mock_proxied_server},
    server::Server,
    users::User,
    utils::random_string,
};

use anyhow::Result;

pub struct TestApp {
    pub client: Client,
    pub id: String,
    pub port: u16,
    pub server_started: tokio::sync::broadcast::Receiver<()>,
}

impl TestApp {
    pub async fn is_ready(&mut self) {
        self.server_started
            .recv()
            .await
            .expect("could not start server");
    }

    pub async fn spawn(config: Option<Config>) -> Self {
        let id = random_string(16);
        create_test_tree(&id).ok();
        let main_listener =
            std::net::TcpListener::bind("127.0.0.1:0").expect("failed to bind to random port");

        let main_addr = (&main_listener).local_addr().unwrap();
        let main_port = main_addr.port();
        let mock1_listener =
            std::net::TcpListener::bind("127.0.0.1:0").expect("failed to bind to random port");
        let mock1_port = mock1_listener.local_addr().unwrap().port();
        let mock2_listener =
            std::net::TcpListener::bind("127.0.0.1:0").expect("failed to bind to random port");
        let mock2_port = mock2_listener.local_addr().unwrap().port();
        let mock_oauth2_listener =
            std::net::TcpListener::bind("127.0.0.1:0").expect("failed to bind to random port");
        let mock_oauth2_port = mock_oauth2_listener.local_addr().unwrap().port();

        let mut config = config.unwrap_or(create_default_config(
            &id,
            &main_port,
            &mock1_port,
            &mock2_port,
            &mock_oauth2_port,
        ));

        if config.hostname.is_empty() {
            config.hostname = "atrium.io".to_owned();
            config.http_port = main_port;
        }

        create_apps_file(&id, config).await;

        tokio::spawn(mock_proxied_server(mock1_listener));
        tokio::spawn(mock_proxied_server(mock2_listener));
        tokio::spawn(mock_oauth2_server(mock_oauth2_listener));

        let (tx, _) = broadcast::channel(16);
        let fp = format!("{}.yaml", &id);

        let (server_status, server_started) = broadcast::channel(16);

        let _ = tokio::spawn(async move {
            drop(main_listener);
            loop {
                info!("Configuration read !");
                let mut rx = tx.subscribe();
                let app = Server::build(&fp, tx.clone())
                    .await
                    .expect("could not build server from configuration");
                let server = axum::Server::bind(&main_addr)
                    .serve(
                        app.router
                            .into_make_service_with_connect_info::<SocketAddr>(),
                    )
                    .with_graceful_shutdown(async move {
                        rx.recv().await.expect("Could not receive reload command!");
                    });
                server_status.send(()).expect("could not send message");
                server.await.expect("could not start server");
            }
        });

        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .resolve("atrium.io", main_addr)
            .resolve("app1.atrium.io", main_addr)
            .resolve("app2.atrium.io", main_addr)
            .resolve("app2-altered.atrium.io", main_addr)
            .resolve("secured-app.atrium.io", main_addr)
            .resolve("static-app.atrium.io", main_addr)
            .resolve("files1.atrium.io", main_addr)
            .resolve("files2.atrium.io", main_addr)
            .resolve("files3.atrium.io", main_addr)
            .resolve("secured-files.atrium.io", main_addr)
            .resolve("fwdtoredirect.atrium.io", main_addr)
            .resolve("relativeredirect.atrium.io", main_addr)
            .resolve("absoluteredirect.atrium.io", main_addr)
            .resolve("app1-subdomain1.app1.atrium.io", main_addr)
            .resolve("app1.subdomain2.app1.atrium.io", main_addr)
            .cookie_store(true)
            .build()
            .unwrap();

        let mut test_app = TestApp {
            client: client,
            id: id,
            port: main_port,
            server_started: server_started,
        };

        test_app.is_ready().await;

        test_app
    }
}

impl Drop for TestApp {
    fn drop(&mut self) {
        std::fs::remove_file(&format!("{}.yaml", self.id)).ok();
        std::fs::remove_dir_all(&format!("./data/{}", self.id)).ok();
    }
}

pub async fn create_apps_file(id: &str, config: Config) {
    let filepath = format!("{}.yaml", &id);
    config.to_file(&filepath).await.unwrap();
}

pub fn create_default_config(
    id: &str,
    main_port: &u16,
    mock1_port: &u16,
    mock2_port: &u16,
    mock_oauth2_port: &u16,
) -> Config {
    let apps = vec![
        App {
            id: 1,
            name: "App 1".to_owned(),
            icon: 0xf53f,
            color: 4292030255,
            is_proxy: true,
            host: "app1".to_owned(),
            target: format!("localhost:{mock1_port}"),
            secured: false,
            login: "admin".to_owned(),
            password: "ff54fds6f".to_owned(),
            openpath: "".to_owned(),
            roles: vec!["ADMINS".to_owned(), "USERS".to_owned()],
            inject_security_headers: false,
            subdomains: Some(vec![
                "app1-subdomain1".to_owned(),
                "app1.subdomain2".to_owned(),
            ]),
        },
        App {
            id: 2,
            name: "App 2".to_owned(),
            icon: 0xf53f,
            color: 4292030255,
            is_proxy: true,
            host: "app2".to_owned(),
            target: format!("localhost:{mock2_port}"),
            secured: false,
            login: "admin".to_owned(),
            password: "ff54fds6f".to_owned(),
            openpath: "/javascript_simple.html".to_owned(),
            roles: vec!["ADMINS".to_owned()],
            inject_security_headers: true,
            subdomains: None,
        },
        App {
            id: 3,
            name: "Secured App".to_owned(),
            icon: 0xf53f,
            color: 4292030255,
            is_proxy: true,
            host: "secured-app".to_owned(),
            target: format!("localhost:{mock2_port}"),
            secured: true,
            login: "".to_owned(),
            password: "".to_owned(),
            openpath: "".to_owned(),
            roles: vec!["ADMINS".to_owned()],
            inject_security_headers: true,
            subdomains: None,
        },
        App {
            id: 4,
            name: "Static App".to_owned(),
            icon: 0xf53f,
            color: 4292030255,
            is_proxy: false,
            host: "static-app".to_owned(),
            target: "tests/data".to_owned(),
            secured: true,
            login: "".to_owned(),
            password: "".to_owned(),
            openpath: "".to_owned(),
            roles: vec!["ADMINS".to_owned()],
            inject_security_headers: true,
            subdomains: None,
        },
    ];

    let davs = vec![
        Dav {
            id: 1,
            host: "files1".to_owned(),
            directory: format!("./data/{id}/dir1"),
            writable: true,
            name: "Files 1".to_owned(),
            icon: 0xf0330,
            color: 4292030255,
            secured: false,
            allow_symlinks: false,
            roles: vec!["ADMINS".to_owned(), "USERS".to_owned()],
            passphrase: None,
            key: None,
        },
        Dav {
            id: 2,
            host: "files2".to_owned(),
            directory: format!("./data/{id}/dir2"),
            writable: true,
            name: "Files 2".to_owned(),
            icon: 0xf0330,
            color: 4292030255,
            secured: false,
            allow_symlinks: true,
            roles: vec!["ADMINS".to_owned()],
            passphrase: Some("ABCD123".to_owned()),
            key: None,
        },
        Dav {
            id: 3,
            host: "files3".to_owned(),
            directory: format!("./data/{id}/dir3"),
            writable: false,
            name: "Files 3".to_owned(),
            icon: 0xf0330,
            color: 4292030255,
            secured: false,
            allow_symlinks: true,
            roles: vec!["ADMINS".to_owned(), "USERS".to_owned()],
            passphrase: None,
            key: None,
        },
        Dav {
            id: 4,
            host: "secured-files".to_owned(),
            directory: format!("./data/{id}/dir3"),
            writable: false,
            name: "Secured Files".to_owned(),
            icon: 0xf0330,
            color: 4292030255,
            secured: true,
            allow_symlinks: true,
            roles: vec!["ADMINS".to_owned()],
            passphrase: None,
            key: None,
        },
        Dav {
            id: 5,
            host: "secured-files-2".to_owned(),
            directory: format!("./data/{id}/dir3"),
            writable: false,
            name: "Secured Files 2".to_owned(),
            icon: 0xf0330,
            color: 4292030255,
            secured: true,
            allow_symlinks: true,
            roles: vec!["ADMINS".to_owned()],
            passphrase: None,
            key: None,
        },
    ];

    let users = vec![
        User {
            login: "admin".to_owned(),
            password: "$argon2id$v=19$m=4096,t=3,p=1$QWsdpHrjCaPwy3IODegzNA$dqyioLh9ndJ3V7OoKpkCaczJmGNKjuG99F5hisd3bPs".to_owned(),
            roles: vec!["ADMINS".to_owned()],
            ..Default::default()
        },
        User {
            login: "user".to_owned(),
            password: "$argon2id$v=19$m=4096,t=3,p=1$ZH9ZFCT6YjYQpxkNt3SQgQ$g3DQawMEWlU1rnMAserFAzUg3Lg2O80s8eH+PrvmUo0".to_owned(),
            roles: vec!["USERS".to_owned()],
            ..Default::default()
        },
    ];

    Config {
        hostname: "atrium.io".to_owned(),
        domain: "".to_owned(),
        debug_mode: false,
        tls_mode: TlsMode::No,
        letsencrypt_email: "foo@bar.com".to_owned(),
        http_port: *main_port,
        cookie_key: None,
        log_to_file: false,
        apps: apps,
        davs: davs,
        users: users,
        session_duration_days: None,
        onlyoffice_config: Some(OnlyOfficeConfig {
            title: Some("AtriumOffice".to_owned()),
            server: "http://onlyoffice.atrium.io".to_owned(),
            jwt_secret: "CHANGE_ME_IN_PRODUCTION".to_owned(),
        }),
        openid_config: Some(OpenIdConfig {
            client_id: "dummy".to_owned(),
            client_secret: "dummy".to_owned(),
            auth_url: format!("http://localhost:{mock_oauth2_port}/authorize"),
            token_url: format!("http://localhost:{mock_oauth2_port}/token"),
            userinfo_url: format!("http://localhost:{mock_oauth2_port}/userinfo"),
            admins_group: Some("TO_BECOME_ADMINS".to_owned()),
        }),
    }
}

fn create_test_tree(base: &str) -> Result<()> {
    for dir in vec!["dir1", "dir2", "dir3"] {
        fs::create_dir_all(format!("./data/{base}/{dir}/dira"))?;
        fs::create_dir_all(format!("./data/{base}/{dir}/dirb"))?;
        fs::create_dir_all(format!("./data/{base}/{dir}/dira/subdira"))?;
    }
    // Create files only for non encrypted davs
    for dir in vec!["dir1", "dir3"] {
        for subdir in vec!["dira", "dirb", "dira/subdira"] {
            for file in vec!["file1", "file2"] {
                fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(format!("./data/{base}/{dir}/{subdir}/{file}"))
                    .ok();
            }
        }
    }
    Ok(())
}

pub fn encode_uri(v: &str) -> String {
    let parts: Vec<_> = v.split('/').map(urlencoding::encode).collect();
    parts.join("/")
}
