use crate::{
    appstate::{ConfigState, MAXMIND_READER},
    configuration::OpenIdConfig,
    errors::ErrResponse,
    extract::Host,
    users::{ADMINS_ROLE, User, UserInfo, create_user_cookie, user_to_token},
    utils::select_entries_by_value,
};
use axum::{
    body::Body,
    extract::{ConnectInfo, Query, State},
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::cookie::{Cookie, CookieJar, PrivateCookieJar};
use http::{HeaderValue, Request, StatusCode, Uri, header::AUTHORIZATION};
use http_body_util::BodyExt;
use hyper::body::Buf;
use hyper_rustls::HttpsConnectorBuilder;
use hyper_util::{
    client::legacy::{Client, connect::HttpConnector},
    rt::TokioExecutor,
};
use oauth2::{
    AsyncHttpClient, AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointNotSet,
    EndpointSet, HttpRequest, HttpResponse, RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, net::SocketAddr, ops::Deref, pin::Pin};

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
                );
            });
    }
}
async fn openid_configuration_internal(cfg: &mut Option<OpenIdConfig>) -> Result<(), ErrResponse> {
    let cfg = cfg
        .as_mut()
        .ok_or(ErrResponse::S500("no open id configuration"))?;
    let url = cfg
        .openid_configuration_url
        .as_ref()
        .ok_or(ErrResponse::S500("no open id configuration url"))?;

    let client = HyperOAuth2Client::new(cfg.insecure_skip_verify);
    let req = Request::builder()
        .uri(url)
        .body(Body::empty())
        .map_err(|_| ErrResponse::S500("could not get build OAuth2 client"))?;
    let res = client
        .request(req)
        .await
        .map_err(|_| ErrResponse::S500("error communicating with OpenID configuration endpoint"))?;

    let body = res
        .into_body()
        .collect()
        .await
        .map_err(|_| {
            ErrResponse::S500("error getting response from OpenID configuration endpoint")
        })?
        .aggregate();
    let urls: OpenIdUrls = serde_json::from_reader(body.reader())
        .map_err(|_| ErrResponse::S500("error parsing OpenID configuration endpoint response"))?;

    cfg.auth_url = urls.authorization_endpoint;
    cfg.token_url = urls.token_endpoint;
    cfg.userinfo_url = urls.userinfo_endpoint;
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

type BasicClient = oauth2::basic::BasicClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointSet,
>;

fn oauth_client_internal(
    config: OpenIdConfig,
    redirect_url: String,
) -> Result<BasicClient, oauth2::url::ParseError> {
    Ok(
        oauth2::basic::BasicClient::new(ClientId::new(config.client_id))
            .set_client_secret(ClientSecret::new(config.client_secret))
            .set_auth_uri(AuthUrl::new(config.auth_url)?)
            .set_token_uri(TokenUrl::new(config.token_url)?)
            .set_redirect_uri(RedirectUrl::new(redirect_url)?),
    )
}

fn oauth_client(config: OpenIdConfig, redirect_url: String) -> Result<BasicClient, ErrResponse> {
    oauth_client_internal(config, redirect_url)
        .map_err(|_| ErrResponse::S500("could not parse OpenID configuration"))
}

pub async fn oauth2_login(
    State(config): State<ConfigState>,
    jar: CookieJar,
) -> Result<impl IntoResponse, ErrResponse> {
    let openid_config = config
        .openid_config
        .as_ref()
        .ok_or(ErrResponse::S500("OpenID configuration is not available"))?;
    let client = oauth_client(
        openid_config.clone(),
        format!("{}/auth/oauth2callback", config.full_domain()),
    )?;

    let mut client = client.authorize_url(CsrfToken::new_random);

    for s in &openid_config.scopes {
        client = client.add_scope(Scope::new(s.clone()));
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
    host: Host,
) -> Result<(PrivateCookieJar, Redirect), ErrResponse> {
    let oidc_config = config
        .openid_config
        .as_ref()
        .ok_or(ErrResponse::S500("OpenID configuration is not available"))?;
    let oauth_client = oauth_client(
        oidc_config.clone(),
        format!("{}/auth/oauth2callback", config.full_domain()),
    )?;

    // Check the state
    if jar.get(STATE_COOKIE).is_none()
        || jar.get(STATE_COOKIE).map_or("bad-state", |c| c.value()) != query.state
    {
        return Err(ErrResponse::S403("OAuth2 state does not match"));
    }

    let client = HyperOAuth2Client::new(oidc_config.insecure_skip_verify);
    // Get an auth token
    let token = oauth_client
        .exchange_code(AuthorizationCode::new(query.code.clone()))
        .request_async(&client)
        .await
        .map_err(|_| ErrResponse::S500("could not get OAuth2 token"))?;

    // Fetch user data
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
        .body(Body::empty())
        .map_err(|_| ErrResponse::S500("could not create user info request"))?;
    let res = client
        .request(req)
        .await
        .map_err(|_| ErrResponse::S500("could not make user info request"))?;
    let user_data = res
        .into_body()
        .collect()
        .await
        .map_err(|_| ErrResponse::S500("could not get user info response body"))?
        .aggregate();
    let user_data: OAuthUser = serde_json::from_reader(user_data.reader())
        .map_err(|_| ErrResponse::S500("could not retrieve user from user info endpoint"))?;

    // Map roles
    let user_roles = user_data
        .member_of
        .iter()
        .map(|e| {
            e.split(',')
                .next()
                .expect("first entry")
                .trim_start_matches("CN=")
        })
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
        &host,
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

struct HyperOAuth2Client(Client<hyper_rustls::HttpsConnector<HttpConnector>, Body>);

impl Deref for HyperOAuth2Client {
    type Target = Client<hyper_rustls::HttpsConnector<HttpConnector>, Body>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl HyperOAuth2Client {
    fn new(insecure_skip_verify: bool) -> Self {
        let https = if insecure_skip_verify {
            HttpsConnectorBuilder::new()
                .with_tls_config(crate::appstate::get_rustls_config_dangerous())
        } else {
            HttpsConnectorBuilder::new().with_webpki_roots()
        };

        let https = https.https_or_http().enable_http1().build();
        let client: Client<_, Body> = Client::builder(TokioExecutor::new()).build(https);
        Self(client)
    }
}

impl<'c> AsyncHttpClient<'c> for HyperOAuth2Client {
    type Error = ErrResponse;
    type Future = Pin<
        Box<dyn std::future::Future<Output = Result<HttpResponse, Self::Error>> + Send + Sync + 'c>,
    >;

    fn call(&'c self, request: HttpRequest) -> Self::Future {
        Box::pin(async move {
            let (parts, body) = request.into_parts();
            let req = Request::from_parts(parts, Body::from(body.clone()));
            let res = self
                .request(req)
                .await
                .map_err(|_| ErrResponse::S500("could not make OAuth2 request"))?;
            let (parts, body) = res.into_parts();
            let body = body
                .collect()
                .await
                .map_err(|_| ErrResponse::S500("could not get body from OAuth2 response"))?
                .to_bytes();
            Ok(HttpResponse::from_parts(parts, body.to_vec()))
        })
    }
}
