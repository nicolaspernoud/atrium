use std::fmt;
use std::io;

const SLICE_ERR: &str = "could not get buffer slice for decrypting";

#[derive(Debug)]
pub struct CryptoError<E>(pub E);

impl<E: fmt::Display> fmt::Display for CryptoError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<E: fmt::Debug + fmt::Display> std::error::Error for CryptoError<E> {}

#[derive(Debug)]
pub enum DavFileError {
    FileCreate(io::Error),
    FileRead(io::Error),
    FileWrite(io::Error),
    FileTruncate(io::Error),
    NonceGeneration(Box<dyn std::error::Error + Send + Sync>),
    Encryption(Box<dyn std::error::Error + Send + Sync>),
    Decryption(Box<dyn std::error::Error + Send + Sync>),
    AuthFailed,
    InvalidLength,
    TruncatedChunk,
    Slice,
}

impl fmt::Display for DavFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DavFileError::FileCreate(e) => write!(f, "error creating file: {e}"),
            DavFileError::FileRead(e) => write!(f, "error reading file: {e}"),
            DavFileError::FileWrite(e) => write!(f, "error writing file: {e}"),
            DavFileError::FileTruncate(e) => write!(f, "error truncating file: {e}"),
            DavFileError::NonceGeneration(e) => write!(f, "error generating nonce: {e}"),
            DavFileError::Encryption(e) => write!(f, "error encrypting plaintext: {e}"),
            DavFileError::Decryption(e) => write!(f, "error decrypting ciphertext: {e}"),
            DavFileError::AuthFailed => write!(
                f,
                "error decrypting ciphertext: authentication failed (integrity check)"
            ),
            DavFileError::InvalidLength => write!(f, "invalid length for cryptographic operation"),
            DavFileError::TruncatedChunk => write!(f, "detected truncated or corrupted chunk"),
            DavFileError::Slice => write!(f, "{}", SLICE_ERR),
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
            DavFileError::FileCreate(e)
            | DavFileError::FileRead(e)
            | DavFileError::FileWrite(e)
            | DavFileError::FileTruncate(e) => Some(e),
            DavFileError::NonceGeneration(e)
            | DavFileError::Encryption(e)
            | DavFileError::Decryption(e) => Some(e.as_ref()),
            DavFileError::AuthFailed
            | DavFileError::InvalidLength
            | DavFileError::TruncatedChunk
            | DavFileError::Slice => None,
        }
    }
}

impl From<DavFileError> for io::Error {
    fn from(err: DavFileError) -> Self {
        match err {
            DavFileError::FileCreate(ref e)
            | DavFileError::FileRead(ref e)
            | DavFileError::FileWrite(ref e)
            | DavFileError::FileTruncate(ref e) => io::Error::new(e.kind(), err),
            _ => io::Error::other(err),
        }
    }
}
