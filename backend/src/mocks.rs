use crate::middlewares::debug_cors_middleware;
use axum::{
    extract::Query,
    middleware,
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
    Router,
};
use http::{header, HeaderMap, StatusCode};
use serde::Deserialize;
use std::net::TcpListener;

pub async fn mock_proxied_server(listener: TcpListener) {
    let port = listener.local_addr().unwrap().port();
    let message = format!("Hello world from mock server on port {port}!");
    let app = Router::new()
        .route("/", get(move || async { message }))
        .route("/foo", get(move || async { "bar" }))
        .route("/headers", get(headers))
        .route(
            "/ws",
            get(move || async { StatusCode::SWITCHING_PROTOCOLS }),
        );

    axum::Server::from_tcp(listener)
        .expect("failed to build mock server")
        .serve(app.into_make_service())
        .await
        .unwrap();
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

    axum::Server::from_tcp(listener)
        .expect("failed to build mock server")
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn well_known_openid() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/json")],
        r#"{
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
			"registration_endpoint": "http://localhost:8090/register",
			"issuer": "http://localhost:8090",
			"authorization_endpoint": "http://localhost:8090/authorize",
			"token_endpoint": "http://localhost:8090/token",
			"userinfo_endpoint": "http://localhost:8090/userinfo"
		  }"#,
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
        [(header::CONTENT_TYPE, "application/json")],
        r#"{
			"displayName": "Us ER",
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
        [(header::CONTENT_TYPE, "application/json")],
        r#"{
			"displayName": "Ad MIN",
			"memberOf": [
				"CN=TO_BECOME_ADMINS",
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
