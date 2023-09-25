use crate::{
    appstate::{ConfigState, MAXMIND_READER},
    configuration::OpenIdConfig,
    errors::ErrResponse,
    users::{create_user_cookie, user_to_token, User, UserInfo, ADMINS_ROLE},
    utils::select_entries_by_value,
};
use anyhow::Result;
use axum::{
    extract::{ConnectInfo, Host, Query, State},
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::{cookie::Cookie, CookieJar, PrivateCookieJar};
use http::{header::AUTHORIZATION, HeaderValue, Request, StatusCode, Uri};
use hyper::{Body, Client};
use hyper_rustls::HttpsConnectorBuilder;
use oauth2::{
    basic::BasicClient, AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, HttpRequest,
    HttpResponse, RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, net::SocketAddr};

const STATE_COOKIE: &str = "ATRIUM_OAUTH2_STATE";

#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
pub struct OpenIdUrls {
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub userinfo_endpoint: String,
}

pub fn is_default_scopes(vec: &Vec<String>) -> bool {
    *vec == default_scopes()
}

pub fn default_scopes() -> Vec<String> {
    vec!["openid".to_string()]
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Eq)]
pub struct RolesMap(HashMap<String, String>);

impl Default for RolesMap {
    fn default() -> Self {
        RolesMap(HashMap::from([
            ("ADMINS".to_owned(), "ADMINS".to_owned()),
            ("USERS".to_owned(), "USERS".to_owned()),
        ]))
    }
}

// Override the openid urls with the ones provided by the well known configuration endpoint, if it fails, the previous values are kept
pub async fn openid_configuration(cfg: &mut Option<OpenIdConfig>) {
    if cfg.is_some() {
        openid_configuration_internal(cfg)
            .await
            .unwrap_or_else(|error| {
                tracing::info!(
                    "Could not set up Open ID Connect configuration from well-known url: {}",
                    error
                )
            });
    }
}
async fn openid_configuration_internal(cfg: &mut Option<OpenIdConfig>) -> Result<()> {
    let url = cfg
        .as_ref()
        .ok_or(anyhow::Error::msg("no open id configuration"))?
        .openid_configuration_url
        .as_ref()
        .ok_or(anyhow::Error::msg("no open id configuration url"))?;

    // Unwrap is ok since we tested before that the option was Some...
    let client = hyper_client(cfg.as_ref().unwrap().insecure_skip_verify);
    let req = Request::builder().uri(url).body(Body::empty())?;
    let res = client.request(req).await?;

    let data = hyper::body::to_bytes(res.into_body()).await?;
    let urls: OpenIdUrls = serde_json::from_slice(&data)?;

    // Unwrap is ok since we tested before that the option was Some...
    cfg.as_mut().unwrap().auth_url = urls.authorization_endpoint;
    cfg.as_mut().unwrap().token_url = urls.token_endpoint;
    cfg.as_mut().unwrap().userinfo_url = urls.userinfo_endpoint;
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct OAuthUser {
    pub login: String,
    #[serde(rename = "memberOf")]
    pub member_of: Vec<String>,
    #[serde(default)]
    pub given_name: String,
    #[serde(default)]
    pub family_name: String,
    #[serde(default)]
    pub email: String,
}

fn oauth_client_internal(
    config: OpenIdConfig,
    redirect_url: String,
) -> Result<BasicClient, oauth2::url::ParseError> {
    Ok(BasicClient::new(
        ClientId::new(config.client_id),
        Some(ClientSecret::new(config.client_secret)),
        AuthUrl::new(config.auth_url)?,
        Some(TokenUrl::new(config.token_url)?),
    )
    .set_redirect_uri(RedirectUrl::new(redirect_url)?))
}

fn oauth_client(config: OpenIdConfig, redirect_url: String) -> Result<BasicClient, ErrResponse> {
    oauth_client_internal(config, redirect_url)
        .map_err(|_| ErrResponse::S500("could not parse OpenID configuration"))
}

pub async fn oauth2_login(
    State(config): State<ConfigState>,
    jar: CookieJar,
) -> Result<impl IntoResponse, ErrResponse> {
    if config.openid_config.is_none() {
        return Err(ErrResponse::S500("OpenID configuration is not available"));
    }
    let client = oauth_client(
        config.openid_config.as_ref().unwrap().clone(),
        format!("{}/auth/oauth2callback", config.full_domain()),
    )?;

    let mut client = client.authorize_url(CsrfToken::new_random);

    for s in &config.as_ref().openid_config.as_ref().unwrap().scopes {
        client = client.add_scope(Scope::new(s.to_string()));
    }

    let (auth_url, csrf_token) = client.url();

    Ok((
        jar.add(Cookie::new(STATE_COOKIE, csrf_token.secret().clone())),
        Redirect::to(auth_url.as_ref()),
    ))
}

pub async fn oauth2_available(
    State(config): State<ConfigState>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    if config.openid_config.is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            "OpenID configuration is not available",
        ));
    }
    Ok((StatusCode::OK, "OpenID configuration found"))
}

#[derive(Debug, Deserialize)]

pub struct AuthRequest {
    pub code: String,
    pub state: String,
}

pub async fn oauth2_callback(
    Query(query): Query<AuthRequest>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    jar: CookieJar,
    private_jar: PrivateCookieJar,
    State(config): State<ConfigState>,
    Host(hostname): Host,
) -> Result<(PrivateCookieJar, Redirect), ErrResponse> {
    if config.openid_config.is_none() {
        return Err(ErrResponse::S500("OpenID configuration is not available"));
    }
    let oidc_config = config.openid_config.as_ref().unwrap();
    let oauth_client = oauth_client(
        oidc_config.clone(),
        format!("{}/auth/oauth2callback", config.full_domain()),
    )?;

    // Check the state
    if jar.get(STATE_COOKIE).is_none() || jar.get(STATE_COOKIE).unwrap().value() != query.state {
        return Err(ErrResponse::S403("OAuth2 state does not match"));
    }

    // Get an auth token
    let token = oauth_client
        .exchange_code(AuthorizationCode::new(query.code.clone()))
        .request_async(|r| hyper_oauth2_client(oidc_config.insecure_skip_verify, r))
        .await
        .map_err(|_| ErrResponse::S500("could not get OAuth2 token"))?;

    // Fetch user data
    let client = hyper_client(oidc_config.insecure_skip_verify);

    let userinfo_uri = oidc_config
        .userinfo_url
        .parse::<Uri>()
        .map_err(|_| ErrResponse::S500("could not parse oidc user info url"))?;
    let req = Request::builder()
        .uri(userinfo_uri)
        .header(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", token.access_token().secret())).map_err(
                |_| ErrResponse::S500("could not create bearer header from access token"),
            )?,
        )
        .body(hyper::Body::empty())
        .map_err(|_| ErrResponse::S500("could not create user info request"))?;
    let res = client
        .request(req)
        .await
        .map_err(|_| ErrResponse::S500("could not make user info request"))?;
    let user_data = hyper::body::to_bytes(res.into_body())
        .await
        .map_err(|_| ErrResponse::S500("could not get user info response body"))?;
    let user_data: OAuthUser = serde_json::from_slice(&user_data)
        .map_err(|_| ErrResponse::S500("could not retrieve user from user info endpoint"))?;

    // Map roles
    let user_roles = user_data
        .member_of
        .iter()
        .map(|e| e.split(',').collect::<Vec<&str>>()[0].trim_start_matches("CN="))
        .collect();
    let mapped_roles = select_entries_by_value(&oidc_config.roles_map.0, user_roles);

    let user = User {
        login: user_data.login,
        password: "".to_owned(),
        roles: mapped_roles,
        info: Some(UserInfo {
            given_name: user_data.given_name,
            family_name: user_data.family_name,
            email: user_data.email,
        }),
    };

    let user_token = user_to_token(&user, &config);
    let cookie = create_user_cookie(
        &user_token,
        hostname,
        &config,
        addr,
        MAXMIND_READER.get(),
        &user,
    )?;

    Ok((
        private_jar.add(cookie),
        if config.single_proxy {
            Redirect::to("/")
        } else {
            Redirect::to(&format!(
                "/oauth2/oauth2.html?is_admin={}&xsrf_token={}&user={}",
                user.roles.contains(&ADMINS_ROLE.to_owned()),
                user_token.xsrf_token,
                user.login
            ))
        },
    ))
}

fn hyper_client(
    insecure_skip_verify: bool,
) -> Client<hyper_rustls::HttpsConnector<hyper::client::HttpConnector>> {
    let https;

    if insecure_skip_verify {
        https = HttpsConnectorBuilder::new()
            .with_tls_config(crate::appstate::get_rustls_config_dangerous());
    } else {
        https = HttpsConnectorBuilder::new().with_webpki_roots();
    }

    let https = https.https_or_http().enable_http1().build();
    let client: Client<_, hyper::Body> = Client::builder().build(https);
    client
}

pub async fn hyper_oauth2_client(
    insecure_skip_verify: bool,
    request: HttpRequest,
) -> Result<HttpResponse, ErrResponse> {
    let client = hyper_client(insecure_skip_verify);

    let mut req = Request::builder()
        .uri(request.url.as_str())
        .method(request.method);

    for (name, value) in &request.headers {
        req = req.header(name.as_str(), value.as_bytes());
    }

    let req = req
        .body(Body::from(request.body))
        .map_err(|_| ErrResponse::S500("could not create OAuth2 request"))?;
    let mut res = client
        .request(req)
        .await
        .map_err(|_| ErrResponse::S500("could not make OAuth2 request"))?;
    let status_code = res.status();
    let headers = std::mem::take(res.headers_mut());
    let body = hyper::body::to_bytes(res.into_body())
        .await
        .map_err(|_| ErrResponse::S500("could not get body from OAuth2 response"))?;

    Ok(HttpResponse {
        status_code,
        headers,
        body: body.into(),
    })
}
