use crate::{headers::XSRFToken, users::UserToken};
use axum::{extract::Request, http::StatusCode, middleware::Next, response::Response};
use axum_extra::TypedHeader;

pub async fn xsrf_middleware(
    xsrf_token: Option<TypedHeader<XSRFToken>>,
    user: Option<UserToken>,
    req: Request,
    next: Next,
) -> Result<Response, (StatusCode, &'static str)> {
    if let Some(user) = user
        && let Some(user_xsrf) = user.xsrf_token
        && xsrf_token.as_ref().map(|v| &v.0.0) != Some(&user_xsrf)
    {
        Err((
            StatusCode::FORBIDDEN,
            "xsrf token not provided or not matching",
        ))
    } else {
        Ok(next.run(req).await)
    }
}
