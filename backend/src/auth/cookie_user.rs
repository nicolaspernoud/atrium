use std::convert::Infallible;

use super::user::{AUTH_COOKIE, UserToken};
use crate::appstate::ConfigState;
use axum::{
    extract::{FromRef, FromRequestParts, OptionalFromRequestParts},
    response::{IntoResponse, Response},
};
use axum_extra::extract::cookie::{Key, PrivateCookieJar};
use http::{StatusCode, request::Parts};

/// A wrapper around `UserToken` that only allows authentication from cookies.
pub struct CookieUserToken(pub UserToken);

impl<S> FromRequestParts<S> for CookieUserToken
where
    S: Send + Sync,
    Key: FromRef<S>,
    ConfigState: FromRef<S>,
    crate::OptionalJail: FromRef<S>,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let jar = PrivateCookieJar::<Key>::from_request_parts(parts, state)
            .await
            .expect("Cookie jar retrieval is Infallible");

        // ONLY Get the serialized user_token from the cookie jar
        if let Some(cookie) = jar.get(AUTH_COOKIE) {
            let serialized_user_token = cookie.value();
            let user_token = UserToken::from_json(serialized_user_token)
                .map_err(|e| (e.0, e.1).into_response())?;
            return Ok(CookieUserToken(user_token));
        }

        Err(StatusCode::UNAUTHORIZED.into_response())
    }
}

impl<S> OptionalFromRequestParts<S> for CookieUserToken
where
    S: Send + Sync,
    Key: FromRef<S>,
    ConfigState: FromRef<S>,
    crate::OptionalJail: FromRef<S>,
{
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Option<Self>, Self::Rejection> {
        Ok(
            <CookieUserToken as FromRequestParts<S>>::from_request_parts(parts, state)
                .await
                .ok(),
        )
    }
}

impl From<CookieUserToken> for UserToken {
    fn from(cookie_token: CookieUserToken) -> Self {
        cookie_token.0
    }
}
