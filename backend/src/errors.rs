use std::fmt::{self, Display, Formatter};

use axum::response::{IntoResponse, Response};
use http::StatusCode;
use tracing::error;

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
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ErrResponse {}

#[derive(Debug)]
pub struct Error(pub &'static str);

impl From<Error> for ErrResponse {
    fn from(err: Error) -> Self {
        ErrResponse::S500(err.0)
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        error!("IO error: {value}");
        Error("IO error")
    }
}

impl From<serde_yaml_ng::Error> for Error {
    fn from(value: serde_yaml_ng::Error) -> Self {
        error!("serde yaml error: {value}");
        Error("serde yaml error")
    }
}

impl From<std::net::AddrParseError> for Error {
    fn from(value: std::net::AddrParseError) -> Self {
        error!("error parsing IP address: {value}");
        Error("error parsing IP address")
    }
}

impl From<rcgen::Error> for Error {
    fn from(value: rcgen::Error) -> Self {
        error!("rcgen error: {value}");
        Error("rcgen error")
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for Error {}
