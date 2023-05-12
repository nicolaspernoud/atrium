use std::{
    error::Error,
    fmt::{self, Display, Formatter},
};

use axum::response::{IntoResponse, Response};
use http::StatusCode;

#[derive(Debug)]
pub enum ErrResponse {
    S403(&'static str),
    S500(&'static str),
}

impl From<ErrResponse> for (StatusCode, &'static str) {
    fn from(err: ErrResponse) -> Self {
        match err {
            ErrResponse::S500(message) => (StatusCode::INTERNAL_SERVER_ERROR, message),
            ErrResponse::S403(message) => (StatusCode::FORBIDDEN, message),
        }
    }
}

impl IntoResponse for ErrResponse {
    fn into_response(self) -> Response {
        Into::<(StatusCode, &'static str)>::into(self).into_response()
    }
}

impl Display for ErrResponse {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Error for ErrResponse {}
