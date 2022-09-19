use crate::{
    apps::{App, AppWithUri},
    davs::model::Dav,
    users::User,
    utils::{option_string_trim, string_trim},
};
use anyhow::Result;
use axum::{
    async_trait,
    extract::{FromRequest, RequestParts},
    Extension, TypedHeader,
};

use hyper::StatusCode;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};

fn http_port() -> u16 {
    8080
}

fn hostname() -> String {
    "atrium.io".to_owned()
}

#[derive(Deserialize, Serialize, Debug, Default, PartialEq, Clone)]
pub struct Config {
    #[serde(default = "hostname", deserialize_with = "string_trim")]
    pub hostname: String,
    #[serde(default)]
    pub debug_mode: bool,
    #[serde(default = "http_port")]
    pub http_port: u16,
    #[serde(default)]
    pub auto_tls: bool,
    #[serde(deserialize_with = "string_trim")]
    pub letsencrypt_email: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "option_string_trim"
    )]
    pub cookie_key: Option<String>,
    #[serde(default)]
    pub log_to_file: bool,
    pub apps: Vec<App>,
    pub davs: Vec<Dav>,
    pub users: Vec<User>,
    pub session_duration_days: Option<i64>,
}

pub type ConfigMap = HashMap<String, HostType>;

pub type ConfigFile = String;

impl Config {
    pub async fn from_file(filepath: &str) -> Result<Self> {
        let data = tokio::fs::read_to_string(filepath).await?;
        let config = serde_yaml::from_str::<Config>(&data)?;
        Ok(config)
    }

    pub async fn to_file(&self, filepath: &str) -> Result<()> {
        let contents = serde_yaml::to_string::<Config>(self)?;
        tokio::fs::write(filepath, contents).await?;
        Ok(())
    }

    pub async fn to_file_or_internal_server_error(
        self,
        filepath: &str,
    ) -> Result<(), (StatusCode, &'static str)> {
        self.to_file(filepath).await.map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "could not save configuration",
            )
        })?;
        Ok(())
    }

    pub fn domains(&self) -> Vec<String> {
        let mut domains = self
            .apps
            .iter()
            .map(|app| format!("{}.{}", app.host.to_owned(), self.hostname))
            .chain(
                self.davs
                    .iter()
                    .map(|dav| format!("{}.{}", dav.host.to_owned(), self.hostname)),
            )
            .collect::<Vec<String>>();
        domains.insert(0, self.hostname.to_owned());
        domains
    }
}

pub async fn load_config(
    config_file: &str,
) -> Result<(Arc<Config>, Arc<ConfigMap>), anyhow::Error> {
    let mut config = Config::from_file(config_file).await?;
    // if the cookie encryption key is not present, generate it and store it
    if config.cookie_key.is_none() {
        config.cookie_key = Some(crate::utils::random_string(64));
        config.to_file(config_file).await?;
    }
    // Allow overriding the hostname with env variable
    if let Some(h) = std::env::var("HOSTNAME").ok() {
        config.hostname = h
    }
    let hashmap = config
        .apps
        .iter()
        .map(|app| {
            let port = if config.auto_tls {
                None
            } else {
                Some(config.http_port)
            };
            (
                format!("{}.{}", app.host.to_owned(), config.hostname),
                if app.is_proxy {
                    HostType::ReverseApp(AppWithUri::from_app_domain_and_http_port(
                        app.clone(),
                        &config.hostname,
                        port,
                    ))
                } else {
                    HostType::StaticApp(app.clone())
                },
            )
        })
        .chain(config.davs.iter().map(|dav| {
            let mut dav = dav.clone();
            dav.compute_key();
            (
                format!("{}.{}", dav.host.to_owned(), config.hostname),
                HostType::Dav(dav),
            )
        }))
        .collect();
    Ok((Arc::new(config), Arc::new(hashmap)))
}

pub async fn config_or_error(config_file: &str) -> Result<Config, (StatusCode, &'static str)> {
    let config = Config::from_file(&config_file).await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "could not read config file",
        )
    })?;
    Ok(config)
}

#[derive(PartialEq, Debug, Clone)]
pub enum HostType {
    StaticApp(App),
    ReverseApp(AppWithUri),
    Dav(Dav),
}

impl HostType {
    pub fn host(&self) -> &str {
        match self {
            HostType::ReverseApp(app) => &app.inner.host,
            HostType::Dav(dav) => &dav.host,
            HostType::StaticApp(app) => &app.host,
        }
    }

    pub fn roles(&self) -> &Vec<String> {
        match self {
            HostType::ReverseApp(app) => &app.inner.roles,
            HostType::Dav(dav) => &dav.roles,
            HostType::StaticApp(app) => &app.roles,
        }
    }

    pub fn secured(&self) -> bool {
        match self {
            HostType::ReverseApp(app) => app.inner.secured,
            HostType::Dav(dav) => dav.secured,
            HostType::StaticApp(app) => app.secured,
        }
    }

    pub fn inject_security_headers(&self) -> bool {
        match self {
            HostType::ReverseApp(app) => app.inner.inject_security_headers,
            HostType::Dav(_dav) => true,
            HostType::StaticApp(app) => app.inject_security_headers,
        }
    }
}

#[async_trait]
impl<B> FromRequest<B> for HostType
where
    B: Send,
{
    type Rejection = StatusCode;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let Extension(configmap) = Extension::<Arc<HashMap<String, HostType>>>::from_request(req)
            .await
            .expect("`Config` extension is missing");

        let host = TypedHeader::<headers::Host>::from_request(req)
            .await
            .map_err(|_| StatusCode::NOT_FOUND)?;

        let host = host.hostname();

        // Work out where to target to
        let target = configmap
            .get(host)
            .ok_or(())
            .map_err(|_| StatusCode::NOT_FOUND)?;
        let target = (*target).clone();

        Ok(target)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::{apps::App, configuration::Config, davs::model::Dav, users::User};

    lazy_static::lazy_static! {
        static ref APPS: Vec<App> = {
            vec![
                App {
                    id: 1,
                    name: "App 1".to_owned(),
                    icon: 0xf53f,
                    color: 4292030255,
                    is_proxy: true,
                    host: "app1".to_owned(),
                    target: "192.168.1.8".to_owned(),
                    secured: true,
                    login: "admin".to_owned(),
                    password: "ff54fds6f".to_owned(),
                    openpath: "".to_owned(),
                    roles: vec!["ADMINS".to_owned(), "USERS".to_owned()],
                    inject_security_headers: true,
                },
                App {
                    id: 2,
                    name: "App 2".to_owned(),
                    icon: 0xf53f,
                    color: 4292030255,
                    is_proxy: false,
                    host: "app2".to_owned(),
                    target: "localhost:8081".to_owned(),
                    secured: true,
                    login: "admin".to_owned(),
                    password: "ff54fds6f".to_owned(),
                    openpath: "/javascript_simple.html".to_owned(),
                    roles: vec!["ADMINS".to_owned()],
                    inject_security_headers: true,
                },
            ]
        };

        static ref DAVS: Vec<Dav> = {
            vec![
                    Dav {
                    id: 1,
                    host: "files1".to_owned(),
                    directory: "/data/file1".to_owned(),
                    writable: true,
                    name: "Files 1".to_owned(),
                    icon: 0xf0330,
                    color: 4292030255,
                    secured: true,
                    allow_symlinks: false,
                    roles: vec!["ADMINS".to_owned(),"USERS".to_owned()],
                    passphrase: Some("ABCD123".to_owned()),
                    key: None
                },
                Dav {
                    id: 2,
                    host: "files2".to_owned(),
                    directory: "/data/file2".to_owned(),
                    writable: true,
                    name: "Files 2".to_owned(),
                    icon: 0xf0330,
                    color: 4292030255,
                    secured: true,
                    allow_symlinks: true,
                    roles: vec!["USERS".to_owned()],
                    passphrase: None,
                    key: None
                },
            ]
        };

        static ref USERS: Vec<User> = {
            vec![
                User {
                    login: "admin".to_owned(),
                    password: "password".to_owned(),
                    roles: vec!["ADMINS".to_owned()],
                },
                User {
                    login: "user".to_owned(),
                    password: "password".to_owned(),
                    roles: vec!["USERS".to_owned()],
                },
            ]
        };
    }

    #[tokio::test]
    async fn test_config_to_file_and_back() {
        // Arrange
        let config = Config {
            hostname: "atrium.io".to_owned(),
            debug_mode: false,
            http_port: 8080,
            auto_tls: false,
            letsencrypt_email: "foo@bar.com".to_owned(),
            cookie_key: None,
            log_to_file: false,
            apps: APPS.clone(),
            davs: DAVS.clone(),
            users: USERS.clone(),
            session_duration_days: None,
        };

        // Act
        let filepath = "config_test.yaml";
        config.to_file(filepath).await.unwrap();
        let new_config = Config::from_file(filepath).await.unwrap();

        // Assert
        assert_eq!(new_config, config);

        // Tidy
        fs::remove_file(filepath).unwrap();
    }
}
