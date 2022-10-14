use std::{net::SocketAddr, sync::Arc};

use axum::{
    extract::{ConnectInfo, Host, Query},
    response::{IntoResponse, Redirect},
    Extension,
};
use axum_extra::extract::{cookie::Cookie, CookieJar, PrivateCookieJar};
use http::StatusCode;
use maxminddb::Reader;
use oauth2::{
    basic::BasicClient, reqwest::async_http_client, AuthUrl, AuthorizationCode, ClientId,
    ClientSecret, CsrfToken, RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use serde::Deserialize;

use crate::{
    configuration::{Config, OpenIdConfig},
    users::{create_user_cookie, user_to_token, User, ADMINS_ROLE},
};

const STATE_COOKIE: &'static str = "ATRIUM_OAUTH2_STATE";

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

fn oauth_client(
    config: OpenIdConfig,
    redirect_url: String,
) -> Result<BasicClient, (StatusCode, &'static str)> {
    Ok(oauth_client_internal(config, redirect_url).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "could not parse OpenID configuration",
        )
    })?)
}

pub async fn oauth2_login(
    Extension(config): Extension<Arc<Config>>,
    jar: CookieJar,
) -> Result<impl IntoResponse, impl IntoResponse> {
    if config.openid_config.is_none() {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "OpenID configuration is not available",
        ));
    }
    let client = oauth_client(
        config.openid_config.as_ref().unwrap().clone(),
        format!("{}/auth/oauth2callback", config.full_hostname()),
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
    Extension(reader): Extension<Arc<Option<Reader<Vec<u8>>>>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    jar: CookieJar,
    private_jar: PrivateCookieJar,
    Extension(config): Extension<Arc<Config>>,
    Host(hostname): Host,
) -> Result<(PrivateCookieJar, Redirect), (StatusCode, &'static str)> {
    if config.openid_config.is_none() {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "OpenID configuration is not available",
        ));
    }
    let oidc_config = config.openid_config.as_ref().unwrap();
    let oauth_client = oauth_client(
        oidc_config.clone(),
        format!("{}/auth/oauth2callback", config.full_hostname()),
    )?;

    // Check the state
    if jar.get(STATE_COOKIE).is_none() || jar.get(STATE_COOKIE).unwrap().value() != query.state {
        return Err((StatusCode::FORBIDDEN, "OAuth2 state does not match"));
    }

    // Get an auth token
    let token = oauth_client
        .exchange_code(AuthorizationCode::new(query.code.clone()))
        .request_async(async_http_client)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "could not get OAuth2 token",
            )
        })?;

    // Fetch user data from discord
    let client = reqwest::Client::new();
    let user_data: OAuthUser = client
        .get(&oidc_config.userinfo_url)
        .bearer_auth(token.access_token().secret())
        .send()
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "could not retrieve user from user info endpoint",
            )
        })?
        .json::<OAuthUser>()
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "could not retrieve user from user info endpoint",
            )
        })?;

    let mut user = User {
        login: user_data.login,
        password: "".to_owned(),
        roles: user_data
            .member_of
            .iter()
            .map(|e| e.trim_start_matches("CN=").to_owned())
            .collect(),
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
    let cookie = create_user_cookie(&user_token, hostname, &config, addr, reader, &user)?;

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
