use crate::{
    apps::{App, AppWithUri},
    appstate::{ConfigMap, ConfigState},
    davs::model::Dav,
    oauth2::{openid_configuration, RolesMap},
    users::User,
    utils::{is_default, option_string_trim, string_trim},
};
use anyhow::Result;
use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
};
use http::request::Parts;
use hyper::StatusCode;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};

fn http_port() -> u16 {
    8080
}

fn hostname() -> String {
    "atrium.io".to_owned()
}

#[derive(Deserialize, Serialize, Debug, Default, PartialEq, Eq, Clone)]
pub struct OnlyOfficeConfig {
    #[serde(default, skip_serializing_if = "is_default")]
    pub title: Option<String>,
    pub server: String,
    pub jwt_secret: String,
}

#[derive(Deserialize, Serialize, Debug, Default, PartialEq, Eq, Clone)]
pub struct OpenIdConfig {
    pub client_id: String,
    pub client_secret: String,
    #[serde(default, skip_serializing_if = "is_default")]
    pub auth_url: String,
    #[serde(default, skip_serializing_if = "is_default")]
    pub token_url: String,
    #[serde(default, skip_serializing_if = "is_default")]
    pub userinfo_url: String,
    #[serde(default, skip_serializing_if = "is_default")]
    pub openid_configuration_url: Option<String>,
    #[serde(default)]
    pub roles_map: RolesMap,
    #[serde(default = "crate::oauth2::default_scopes")]
    #[serde(skip_serializing_if = "crate::oauth2::is_default_scopes")]
    pub scopes: Vec<String>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub insecure_skip_verify: bool,
}

#[derive(Deserialize, Serialize, Debug, Default, PartialEq, Eq, Clone)]
pub enum TlsMode {
    #[default]
    No,
    BehindProxy,
    Auto,
}

impl TlsMode {
    pub fn is_secure(&self) -> bool {
        match self {
            TlsMode::No => false,
            TlsMode::BehindProxy => true,
            TlsMode::Auto => true,
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Default, PartialEq, Eq, Clone)]
pub struct Config {
    #[serde(default = "hostname", deserialize_with = "string_trim")]
    pub hostname: String,
    #[serde(default, skip_serializing_if = "is_default")]
    pub domain: String,
    #[serde(default, skip_serializing_if = "is_default")]
    pub debug_mode: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    pub single_proxy: bool,
    #[serde(default = "http_port")]
    pub http_port: u16,
    #[serde(default)]
    pub tls_mode: TlsMode,
    #[serde(
        default,
        skip_serializing_if = "is_default",
        deserialize_with = "string_trim"
    )]
    pub letsencrypt_email: String,
    #[serde(
        default,
        skip_serializing_if = "is_default",
        deserialize_with = "option_string_trim"
    )]
    pub cookie_key: Option<String>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub log_to_file: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    pub session_duration_days: Option<i64>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub onlyoffice_config: Option<OnlyOfficeConfig>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub openid_config: Option<OpenIdConfig>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub apps: Vec<App>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub davs: Vec<Dav>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub users: Vec<User>,
}

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
        mut self,
        filepath: &str,
    ) -> Result<(), (StatusCode, &'static str)> {
        self.apps.sort_by(|a, b| a.id.partial_cmp(&b.id).unwrap());
        self.davs.sort_by(|a, b| a.id.partial_cmp(&b.id).unwrap());
        self.to_file(filepath).await.map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "could not save configuration",
            )
        })?;
        Ok(())
    }

    pub fn scheme(&self) -> &str {
        if self.tls_mode.is_secure() {
            "https"
        } else {
            "http"
        }
    }

    pub fn full_domain(&self) -> String {
        format!(
            "{s}://{h}{p}",
            s = self.scheme(),
            h = self.domain,
            p = &(if self.tls_mode == TlsMode::No {
                format!(":{}", self.http_port)
            } else {
                "".to_owned()
            })
        )
    }

    pub fn domains(&self) -> Vec<String> {
        let mut domains = filter_services(&self.apps, &self.hostname, &self.domain)
            .map(|app| format!("{}.{}", trim_host(&app.host), self.hostname))
            .chain(
                filter_services(&self.davs, &self.hostname, &self.domain)
                    .map(|dav| format!("{}.{}", trim_host(&dav.host), self.hostname)),
            )
            .collect::<Vec<String>>();
        domains.insert(0, self.hostname.to_owned());
        // Insert apps subdomains
        for app in filter_services(&self.apps, &self.hostname, &self.domain) {
            for domain in app.subdomains.as_ref().unwrap_or(&Vec::new()) {
                domains.push(format!(
                    "{}.{}.{}",
                    domain,
                    trim_host(&app.host),
                    self.hostname
                ));
            }
        }
        domains
    }
}

pub async fn load_config(config_file: &str) -> Result<(ConfigState, ConfigMap), anyhow::Error> {
    let mut config = Config::from_file(config_file).await?;
    // if the cookie encryption key is not present, generate it and store it
    if config.cookie_key.is_none() {
        config.cookie_key = Some(crate::utils::random_string(64));
        config.to_file(config_file).await?;
    }
    // Allow overriding the hostname with env variable
    if let Ok(h) = std::env::var("MAIN_HOSTNAME") {
        config.hostname = h
    }
    if is_default(&config.domain) {
        config.domain = config.hostname.clone()
    };
    let port = if config.tls_mode.is_secure() {
        None
    } else {
        Some(config.http_port)
    };
    // If OpenID configuration url is set, override the auth, token and userinfo urls with the one gotten from the configuration url
    openid_configuration(&mut config.openid_config).await;
    let mut hashmap: HashMap<String, HostType> =
        filter_services(&config.apps, &config.hostname, &config.domain)
            .map(|app| {
                (
                    format!("{}.{}", trim_host(&app.host), config.hostname),
                    app_to_host_type(app, port),
                )
            })
            .chain(
                filter_services(&config.davs, &config.hostname, &config.domain).map(|dav| {
                    let mut dav = dav.clone();
                    dav.compute_key();
                    (
                        format!("{}.{}", trim_host(&dav.host), config.hostname),
                        HostType::Dav(dav),
                    )
                }),
            )
            .collect();
    // If atrium is in single proxy mode, insert an app matching on the main hostname
    if config.single_proxy {
        hashmap.insert(
            config.hostname.clone(),
            app_to_host_type(&config.apps[0], port),
        );
    }
    // Insert apps subdomains
    for app in filter_services(&config.apps, &config.hostname, &config.domain) {
        for domain in app.subdomains.as_ref().unwrap_or(&Vec::new()) {
            hashmap.insert(
                format!("{}.{}.{}", domain, trim_host(&app.host), config.hostname),
                app_to_host_type(app, port),
            );
        }
    }
    Ok((Arc::new(config), Arc::new(hashmap)))
}

pub(crate) fn trim_host(host: &str) -> String {
    host.split_once('.').unwrap_or((host, "")).0.to_owned()
}

fn app_to_host_type(app: &App, port: Option<u16>) -> HostType {
    if app.is_proxy {
        if app.insecure_skip_verify {
            return HostType::SkipVerifyReverseApp(Box::new(AppWithUri::from_app(
                app.clone(),
                port,
            )));
        }
        HostType::ReverseApp(Box::new(AppWithUri::from_app(app.clone(), port)))
    } else {
        HostType::StaticApp(app.clone())
    }
}

pub async fn config_or_error(config_file: &str) -> Result<Config, (StatusCode, &'static str)> {
    let config = Config::from_file(config_file).await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "could not read config file",
        )
    })?;
    Ok(config)
}

pub trait Service {
    fn host(&self) -> &str;
}

impl Service for App {
    fn host(&self) -> &str {
        &self.host
    }
}

impl Service for Dav {
    fn host(&self) -> &str {
        &self.host
    }
}

fn filter_services<'a, T: Service + 'a>(
    services: &'a [T],
    hostname: &'a str,
    domain: &'a str,
) -> impl Iterator<Item = &'a T> {
    services.iter().filter(move |s| {
        if hostname == domain {
            // If domain == hostname, we keep all the apps that do not contain another hostname
            !s.host().contains(hostname)
        } else {
            // else we keep only the apps that DO contain another hostname (a subdomain)
            s.host().contains(hostname)
        }
    })
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub enum HostType {
    StaticApp(App),
    ReverseApp(Box<AppWithUri>),
    SkipVerifyReverseApp(Box<AppWithUri>),
    Dav(Dav),
}

impl HostType {
    pub fn host(&self) -> &str {
        match self {
            HostType::ReverseApp(app) => &app.inner.host,
            HostType::SkipVerifyReverseApp(app) => &app.inner.host,
            HostType::Dav(dav) => &dav.host,
            HostType::StaticApp(app) => &app.host,
        }
    }

    pub fn roles(&self) -> &Vec<String> {
        match self {
            HostType::ReverseApp(app) => &app.inner.roles,
            HostType::SkipVerifyReverseApp(app) => &app.inner.roles,
            HostType::Dav(dav) => &dav.roles,
            HostType::StaticApp(app) => &app.roles,
        }
    }

    pub fn secured(&self) -> bool {
        match self {
            HostType::ReverseApp(app) => app.inner.secured,
            HostType::SkipVerifyReverseApp(app) => app.inner.secured,
            HostType::Dav(dav) => dav.secured,
            HostType::StaticApp(app) => app.secured,
        }
    }

    pub fn inject_security_headers(&self) -> bool {
        match self {
            HostType::ReverseApp(app) => app.inner.inject_security_headers,
            HostType::SkipVerifyReverseApp(app) => app.inner.secured,
            HostType::Dav(_dav) => true,
            HostType::StaticApp(app) => app.inject_security_headers,
        }
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for HostType
where
    S: Send + Sync,
    ConfigMap: FromRef<S>,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let configmap = ConfigMap::from_ref(state);

        let host = axum::extract::Host::from_request_parts(parts, state)
            .await
            .map_err(|_| StatusCode::NOT_FOUND)?;

        let hostname = host.0.split_once(':').unwrap_or((&host.0, "")).0;

        // Work out where to target to
        let target = configmap
            .get(hostname)
            .ok_or(())
            .map_err(|_| StatusCode::NOT_FOUND)?;
        let target = (*target).clone();

        Ok(target)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::{
        apps::App,
        configuration::{Config, TlsMode},
        davs::model::Dav,
        users::User,
    };

    #[tokio::test]
    async fn test_config_to_file_and_back() {
        // Arrange

        let apps = vec![
            App {
                id: 1,
                name: "App 1".to_owned(),
                icon: "web_asset".to_owned(),
                color: 4292030255,
                is_proxy: true,
                host: "app1".to_owned(),
                target: "192.168.1.8".to_owned(),
                secured: true,
                login: "admin".to_owned(),
                password: "ff54fds6f".to_owned(),
                openpath: "".to_owned(),
                roles: vec!["ADMINS".to_owned(), "USERS".to_owned()],
                ..Default::default()
            },
            App {
                id: 2,
                name: "App 2".to_owned(),
                icon: "web_asset".to_owned(),
                color: 4292030255,
                is_proxy: false,
                host: "app2".to_owned(),
                target: "localhost:8081".to_owned(),
                secured: true,
                login: "admin".to_owned(),
                password: "ff54fds6f".to_owned(),
                openpath: "/javascript_simple.html".to_owned(),
                roles: vec!["ADMINS".to_owned()],
                ..Default::default()
            },
        ];

        let davs = vec![
            Dav {
                id: 1,
                host: "files1".to_owned(),
                directory: "/data/file1".to_owned(),
                writable: true,
                name: "Files 1".to_owned(),
                icon: "folder".to_owned(),
                color: 4292030255,
                secured: true,
                allow_symlinks: false,
                roles: vec!["ADMINS".to_owned(), "USERS".to_owned()],
                passphrase: Some("ABCD123".to_owned()),
                key: None,
            },
            Dav {
                id: 2,
                host: "files2".to_owned(),
                directory: "/data/file2".to_owned(),
                writable: true,
                name: "Files 2".to_owned(),
                icon: "folder".to_owned(),
                color: 4292030255,
                secured: true,
                allow_symlinks: true,
                roles: vec!["USERS".to_owned()],
                passphrase: None,
                key: None,
            },
        ];

        let users = vec![
            User {
                login: "admin".to_owned(),
                password: "password".to_owned(),
                roles: vec!["ADMINS".to_owned()],
                info: None,
            },
            User {
                login: "user".to_owned(),
                password: "password".to_owned(),
                roles: vec!["USERS".to_owned()],
                info: None,
            },
        ];

        let config = Config {
            hostname: "atrium.io".to_owned(),
            domain: "".to_owned(),
            debug_mode: false,
            http_port: 8080,
            tls_mode: TlsMode::No,
            letsencrypt_email: "foo@bar.com".to_owned(),
            cookie_key: None,
            log_to_file: false,
            apps,
            davs,
            users,
            session_duration_days: None,
            onlyoffice_config: None,
            openid_config: None,
            single_proxy: false,
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
