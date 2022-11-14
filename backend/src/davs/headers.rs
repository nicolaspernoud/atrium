use headers::Header;
use hyper::header::{HeaderName, HeaderValue};

static DEPTH: HeaderName = HeaderName::from_static("depth");
static OVERWRITE: HeaderName = HeaderName::from_static("overwrite");

// helper.
fn one<'i, I>(values: &mut I) -> Result<&'i HeaderValue, headers::Error>
where
    I: Iterator<Item = &'i HeaderValue>,
{
    let v = values.next().ok_or_else(invalid)?;
    if values.next().is_some() {
        Err(invalid())
    } else {
        Ok(v)
    }
}

// helper
fn invalid() -> headers::Error {
    headers::Error::invalid()
}

/// Depth: header.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Depth {
    Zero,
    One,
    Infinity,
}

impl Header for Depth {
    fn name() -> &'static HeaderName {
        &DEPTH
    }

    fn decode<'i, I>(values: &mut I) -> Result<Self, headers::Error>
    where
        I: Iterator<Item = &'i HeaderValue>,
    {
        let value = one(values)?;
        match value.as_bytes() {
            b"0" => Ok(Depth::Zero),
            b"1" => Ok(Depth::One),
            b"infinity" | b"Infinity" => Ok(Depth::Infinity),
            _ => Err(invalid()),
        }
    }

    fn encode<E>(&self, values: &mut E)
    where
        E: Extend<HeaderValue>,
    {
        let value = match *self {
            Depth::Zero => "0",
            Depth::One => "1",
            Depth::Infinity => "Infinity",
        };
        values.extend(std::iter::once(HeaderValue::from_static(value)));
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Overwrite(pub bool);

impl Header for Overwrite {
    fn name() -> &'static HeaderName {
        &OVERWRITE
    }

    fn decode<'i, I>(values: &mut I) -> Result<Self, headers::Error>
    where
        I: Iterator<Item = &'i HeaderValue>,
    {
        let line = one(values)?;
        match line.as_bytes() {
            b"F" => Ok(Overwrite(false)),
            b"T" => Ok(Overwrite(true)),
            _ => Err(invalid()),
        }
    }

    fn encode<E>(&self, values: &mut E)
    where
        E: Extend<HeaderValue>,
    {
        let value = match self.0 {
            true => "T",
            false => "F",
        };
        values.extend(std::iter::once(HeaderValue::from_static(value)));
    }
}
