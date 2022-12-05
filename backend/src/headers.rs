extern crate headers;
extern crate http;

use headers::{Header, HeaderName, HeaderValue};

pub struct XSRFToken(pub String);

static XSRF_TOKEN: HeaderName = HeaderName::from_static("xsrf-token");

impl Header for XSRFToken {
    fn name() -> &'static HeaderName {
        &XSRF_TOKEN
    }

    fn decode<'i, I>(values: &mut I) -> Result<Self, headers::Error>
    where
        I: Iterator<Item = &'i HeaderValue>,
    {
        let value = values.next().ok_or_else(headers::Error::invalid)?;
        let v = value.to_str().map_err(|_| headers::Error::invalid())?;

        Ok(XSRFToken(v.to_owned()))
    }

    fn encode<E>(&self, values: &mut E)
    where
        E: Extend<HeaderValue>,
    {
        if let Ok(v) = HeaderValue::from_str(&self.0) {
            values.extend(std::iter::once(v));
        }
    }
}
