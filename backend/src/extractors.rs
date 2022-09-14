// Inspired from https://github.com/Owez/axum-auth
// Copyright 2022 Owen Griffiths
// MIT License

use axum::{
    async_trait,
    extract::{FromRequest, RequestParts},
    http::{header::AUTHORIZATION, StatusCode},
};
use base64ct::Encoding;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct AuthBasic(pub (String, Option<String>));

#[async_trait]
impl<B> FromRequest<B> for AuthBasic
where
    B: Send,
{
    type Rejection = StatusCode;

    async fn from_request(req: &mut RequestParts<B>) -> std::result::Result<Self, Self::Rejection> {
        // Get authorisation header
        let authorisation = req
            .headers()
            .get(AUTHORIZATION)
            .ok_or(StatusCode::BAD_REQUEST)?
            .to_str()
            .map_err(|_| StatusCode::BAD_REQUEST)?;

        // Check that its a well-formed basic auth then decode and return
        let split = authorisation.split_once(' ');
        match split {
            Some((name, contents)) if name == "Basic" => decode_basic(contents),
            _ => Err(StatusCode::BAD_REQUEST),
        }
    }
}

/// Decodes basic auth, returning the full tuple if present
fn decode_basic(input: &str) -> Result<AuthBasic, StatusCode> {
    const ERR: StatusCode = StatusCode::BAD_REQUEST;

    // Decode from base64 into a string
    let decoded = base64ct::Base64::decode_vec(input).map_err(|_| ERR)?;
    let decoded = String::from_utf8(decoded).map_err(|_| ERR)?;

    // Return depending on if password is present
    Ok(AuthBasic(
        if let Some((id, password)) = decoded.split_once(':') {
            (id.to_string(), Some(password.to_string()))
        } else {
            (decoded, None)
        },
    ))
}
