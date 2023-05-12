use crate::{
    appstate::{ConfigState, MAXMIND_READER},
    configuration::OpenIdConfig,
    errors::ErrResponse,
    users::{create_user_cookie, user_to_token, User, ADMINS_ROLE},
};
use axum::{
    extract::{ConnectInfo, Host, Query, State},
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::{cookie::Cookie, CookieJar, PrivateCookieJar};
use http::{header::AUTHORIZATION, HeaderValue, Request, Uri};
use hyper::{Body, Client};
use hyper_rustls::HttpsConnectorBuilder;
use oauth2::{
    basic::BasicClient, AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, HttpRequest,
    HttpResponse, RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use serde::Deserialize;
use std::{net::SocketAddr, sync::Arc};

const STATE_COOKIE: &str = "ATRIUM_OAUTH2_STATE";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthUser {
    pub display_name: String,
    pub member_of: Vec<String>,
    pub id: String,
    pub login: String,
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

    let (auth_url, csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("login".to_string()))
        .add_scope(Scope::new("memberOf".to_string()))
        .add_scope(Scope::new("displayName".to_string()))
        .add_scope(Scope::new("email".to_string()))
        .url();

    Ok((
        jar.add(Cookie::new(STATE_COOKIE, csrf_token.secret().clone())),
        Redirect::to(auth_url.as_ref()),
    ))
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
        .request_async(hyper_oauth2_client)
        .await
        .map_err(|_| ErrResponse::S500("could not get OAuth2 token"))?;

    // Fetch user data
    let client = hyper_client();

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

    let mut user = User {
        login: user_data.login,
        password: "".to_owned(),
        roles: user_data
            .member_of
            .iter()
            .map(|e| e.trim_start_matches("CN=").to_owned())
            .collect(),
        ..Default::default()
    };
    // Map admins_group to ADMINS_ROLE if not already present
    if oidc_config.admins_group.is_some()
        && user
            .roles
            .contains(oidc_config.admins_group.as_ref().unwrap())
        && !user.roles.contains(&ADMINS_ROLE.to_owned())
    {
        user.roles.push(ADMINS_ROLE.to_owned());
    }
    let user_token = user_to_token(&user, &config);
    let cookie = create_user_cookie(
        &user_token,
        hostname,
        &config,
        addr,
        Arc::clone(&MAXMIND_READER),
        &user,
    )?;

    Ok((
        private_jar.add(cookie),
        Redirect::to(&format!(
            "/oauth2/oauth2.html?is_admin={}&xsrf_token={}&user={}",
            user.roles.contains(&ADMINS_ROLE.to_owned()),
            user_token.xsrf_token,
            user.login
        )),
    ))
}

fn hyper_client() -> Client<hyper_rustls::HttpsConnector<hyper::client::HttpConnector>> {
    let https = HttpsConnectorBuilder::new()
        .with_webpki_roots()
        .https_or_http()
        .enable_http1()
        .build();
    let client: Client<_, hyper::Body> = Client::builder().build(https);
    client
}

pub async fn hyper_oauth2_client(request: HttpRequest) -> Result<HttpResponse, ErrResponse> {
    let client = hyper_client();

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
