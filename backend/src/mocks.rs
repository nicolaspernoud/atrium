use crate::middlewares::debug_cors_middleware;
use axum::{
    extract::{Host, Query},
    middleware,
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
    Extension, Router,
};
use http::{header, HeaderMap, HeaderValue, StatusCode};
use serde::Deserialize;
use tokio::net::TcpListener;

pub async fn mock_proxied_server(listener: TcpListener) {
    let port = listener.local_addr().unwrap().port();
    let app = Router::new()
        .route("/", get(message))
        .layer(Extension(port))
        .route("/foo", get(move || async { "bar" }))
        .route("/headers", get(headers))
        .route(
            "/ws",
            get(move || async { StatusCode::SWITCHING_PROTOCOLS }),
        );

    axum::serve(listener, app)
        .await
        .expect("failed to build mock server");
}

async fn message(port: Extension<u16>, headers: HeaderMap) -> impl IntoResponse {
    format!(
        r#"
        Hello world from mock server on port {}!
        "host": {:?}
        "x-forwarded-host": {:?}
        "#,
        *port,
        headers
            .get("host")
            .unwrap_or(&HeaderValue::from_static("no host header")),
        headers
            .get("x-forwarded-host")
            .unwrap_or(&HeaderValue::from_static("no x-forwarded-host header"))
    )
}

async fn headers(headers: HeaderMap) -> impl IntoResponse {
    format!("HEADERS: {:?}", headers)
}

pub async fn mock_oauth2_server(listener: TcpListener) {
    let app = Router::new()
        .route("/.well-known/openid-configuration", get(well_known_openid))
        .route("/authorize", get(authorize))
        .route("/authorize_wrong_state", get(authorize_wrong_state))
        .route("/authorize_manual_action", get(authorize_manual_action))
        .route("/token", post(token))
        .route("/userinfo", get(userinfo))
        .route("/admininfo", get(admininfo))
        .route("/logout", get(logout))
        .layer(middleware::from_fn(move |req, next| {
            debug_cors_middleware(req, next)
        }));

    axum::serve(listener, app)
        .await
        .expect("failed to build mock server");
}

async fn well_known_openid(Host(host): Host) -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/json")],
        format!(
            r#"{{
			"response_types_supported": [
			  "code",
			  "id_token",
			  "token",
			  "code id_token",
			  "code token",
			  "token id_token",
			  "code id_token token"
			],
			"request_parameter_supported": true,
			"request_uri_parameter_supported": false,
			"jwks_uri": "http://localhost:8090/jwk",
			"subject_types_supported": [
			  "public"
			],
			"id_token_signing_alg_values_supported": [
			  "RS512"
			],
			"registration_endpoint": "http://{host}/register",
			"issuer": "http://{host}",
			"authorization_endpoint": "http://{host}/authorize",
			"token_endpoint": "http://{host}/token",
			"userinfo_endpoint": "http://{host}/userinfo"
		  }}"#
        ),
    )
}

#[derive(Deserialize)]
struct AuthorizeQuery {
    redirect_uri: String,
    state: String,
}

async fn authorize_manual_action(q: Query<AuthorizeQuery>) -> Html<String> {
    Html(format!(
        r#"<button onclick="location.href='{}?state={}&code=mock_code';">Authenticate and log in...</button>"#,
        q.redirect_uri, q.state
    ))
}

async fn authorize(q: Query<AuthorizeQuery>) -> impl IntoResponse {
    Redirect::to(&format!(
        "{}?state={}&code=mock_code",
        q.redirect_uri, q.state
    ))
}

async fn authorize_wrong_state(q: Query<AuthorizeQuery>) -> impl IntoResponse {
    Redirect::to(&format!(
        "{}?state={}&code=mock_code",
        q.redirect_uri, "not_the_expected_state"
    ))
}

async fn token() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/json")],
        r#"
        {
            "access_token":"mock_token",
            "token_type":"Bearer",
            "expires_in":3600,
            "scope":"login"
        }
        "#,
    )
}

async fn userinfo() -> impl IntoResponse {
    (
        // Complete user infos
        [(header::CONTENT_TYPE, "application/json")],
        r#"{
			"displayName": "Us ER",
            "given_name": "Us",
            "family_name": "ER",
            "email": "user@atrium.io",
			"memberOf": [
				"CN=USERS",
				"CN=OTHER_GROUP"
			],
			"id": "1000",
			"login": "USER"
		}"#,
    )
}

async fn admininfo() -> impl IntoResponse {
    (
        // No user infos, that should still work
        [(header::CONTENT_TYPE, "application/json")],
        r#"{
			"memberOf": [
				"CN=ADMINS,other_infos_to_discard,",
				"CN=OTHER_GROUP"
			],
			"id": "1",
			"login": "ADMIN"
		}"#,
    )
}

async fn logout() -> impl IntoResponse {
    "Logout OK"
}
