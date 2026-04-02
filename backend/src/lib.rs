pub mod apps;
pub mod appstate;
pub mod configuration;
pub mod davs;
pub mod dir_server;
pub mod errors;
pub mod extract;
pub mod headers;
#[cfg(target_os = "linux")]
pub mod jail;
#[cfg(target_os = "linux")]
pub type OptionalJail = Option<std::sync::Arc<jail::Jail>>;
// TODO : remove the OptionalJail when cfg conditionals are supported in where clauses
#[cfg(not(target_os = "linux"))]
pub type OptionalJail = ();
pub mod logger;
pub mod middlewares;
pub mod mocks;
pub mod oauth2;
pub mod onlyoffice;
pub mod server;
pub mod sysinfo;
pub mod auth;
pub mod utils;
pub mod web;
