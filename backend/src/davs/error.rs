use std::fmt;
use std::io;

const SLICE_ERR: &str = "could not get buffer slice for decrypting";

#[derive(Debug)]
pub enum DavFileError {
    FileCreate(io::Error),
    NonceGeneration(String),
    Encryption(String),
    Decryption(String),
    Slice,
}

impl fmt::Display for DavFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DavFileError::FileCreate(e) => write!(f, "error creating file: {e}"),
            DavFileError::NonceGeneration(s) => write!(f, "error generating nonce: {s}"),
            DavFileError::Encryption(s) => write!(f, "error encrypting plaintext: {s}"),
            DavFileError::Decryption(s) => write!(f, "error decrypting ciphertext: {s}"),
            DavFileError::Slice => write!(f, "slice error"),
        }
    }
}

impl From<io::Error> for DavFileError {
    fn from(err: io::Error) -> Self {
        DavFileError::FileCreate(err)
    }
}

impl std::error::Error for DavFileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DavFileError::FileCreate(e) => Some(e),
            _ => None,
        }
    }
}

impl From<DavFileError> for io::Error {
    fn from(err: DavFileError) -> Self {
        match err {
            DavFileError::FileCreate(e) => e,
            DavFileError::Slice => io::Error::other(SLICE_ERR),
            _ => io::Error::other(err.to_string()),
        }
    }
}
