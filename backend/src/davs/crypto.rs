use chacha20poly1305::{
    XChaCha20Poly1305,
    aead::{
        KeyInit,
        stream::{self, StreamBE32, NewStream, StreamPrimitive},
    },
};

pub const DEFAULT_PLAIN_CHUNK_SIZE: usize = 1_000_000; // 1 MByte

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CipherType {
    XChaCha20Poly1305 = 0,
}

impl CipherType {
    pub fn from_u8(v: u8) -> Result<Self, String> {
        match v {
            0 => Ok(CipherType::XChaCha20Poly1305),
            _ => Err(format!("Unknown cipher type: {}", v)),
        }
    }

    pub fn nonce_size(&self) -> usize {
        match self {
            CipherType::XChaCha20Poly1305 => 19,
        }
    }

    pub fn overhead(&self) -> usize {
        match self {
            CipherType::XChaCha20Poly1305 => 16,
        }
    }
}

pub trait Cipher: Send + Sync {
    fn encrypt(&mut self, chunk_idx: u32, is_last: bool, plaintext: &[u8]) -> Result<Vec<u8>, String>;
    fn decrypt(&mut self, chunk_idx: u32, is_last: bool, ciphertext: &[u8]) -> Result<Vec<u8>, String>;
    fn plain_chunk_size(&self) -> usize;
    fn cipher_type(&self) -> CipherType;
}

struct XChaCha20Poly1305Cipher {
    stream_encryptor: StreamBE32<XChaCha20Poly1305>,
    plain_chunk_size: usize,
}

impl Cipher for XChaCha20Poly1305Cipher {
    fn encrypt(&mut self, chunk_idx: u32, is_last: bool, plaintext: &[u8]) -> Result<Vec<u8>, String> {
        self.stream_encryptor
            .encrypt(chunk_idx, is_last, plaintext)
            .map_err(|e: chacha20poly1305::aead::Error| e.to_string())
    }

    fn decrypt(&mut self, chunk_idx: u32, is_last: bool, ciphertext: &[u8]) -> Result<Vec<u8>, String> {
        self.stream_encryptor
            .decrypt(chunk_idx, is_last, ciphertext)
            .map_err(|e: chacha20poly1305::aead::Error| e.to_string())
    }

    fn plain_chunk_size(&self) -> usize {
        self.plain_chunk_size
    }

    fn cipher_type(&self) -> CipherType {
        CipherType::XChaCha20Poly1305
    }
}

pub fn create_cipher(
    cipher_type: CipherType,
    key: &[u8; 32],
    nonce: &[u8],
    plain_chunk_size: usize,
) -> Box<dyn Cipher> {
    match cipher_type {
        CipherType::XChaCha20Poly1305 => {
            let aead = XChaCha20Poly1305::new(key.into());
            let stream_encryptor = stream::StreamBE32::from_aead(aead, nonce.into());
            Box::new(XChaCha20Poly1305Cipher {
                stream_encryptor,
                plain_chunk_size,
            })
        }
    }
}
