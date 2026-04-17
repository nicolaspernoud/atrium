use chacha20poly1305::{
    XChaCha20Poly1305,
    aead::{
        KeyInit,
        stream::{self, StreamBE32, NewStream, StreamPrimitive},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CipherType {
    XChaCha20Poly1305_1M = 0,
}

impl CipherType {
    pub fn from_u8(v: u8) -> Result<Self, String> {
        match v {
            0 => Ok(CipherType::XChaCha20Poly1305_1M),
            _ => Err(format!("Unknown cipher type: {}", v)),
        }
    }

    pub fn nonce_size(&self) -> usize {
        match self {
            CipherType::XChaCha20Poly1305_1M => 19,
        }
    }

    pub fn overhead(&self) -> usize {
        match self {
            CipherType::XChaCha20Poly1305_1M => 16,
        }
    }

    pub fn plain_chunk_size(&self) -> usize {
        match self {
            CipherType::XChaCha20Poly1305_1M => 1_000_000,
        }
    }

    pub fn encrypted_chunk_size(&self) -> usize {
        self.plain_chunk_size() + self.overhead()
    }

    pub fn header_size(&self) -> usize {
        1 + self.nonce_size()
    }

    pub fn encrypted_chunk_start(&self, dec_offset: u64) -> u64 {
        let chunk_idx = dec_offset / self.plain_chunk_size() as u64;
        self.header_size() as u64 + chunk_idx * self.encrypted_chunk_size() as u64
    }

    pub fn decrypted_size(&self, enc_size: u64) -> u64 {
        let header_size = self.header_size() as u64;
        let encrypted_chunk_size = self.encrypted_chunk_size() as u64;
        let plain_chunk_size = self.plain_chunk_size() as u64;
        let overhead = self.overhead() as u64;

        if enc_size <= header_size {
            return 0;
        }
        let enc_size_without_header = enc_size - header_size;
        let num_chunks = enc_size_without_header.div_ceil(encrypted_chunk_size);
        if num_chunks == 0 {
            return 0;
        }
        let last_chunk_size = enc_size_without_header - (num_chunks - 1) * encrypted_chunk_size;
        (num_chunks - 1) * plain_chunk_size
            + (last_chunk_size.saturating_sub(overhead))
    }

    pub fn create_cipher(&self, key: &[u8; 32], nonce: &[u8]) -> Box<dyn Cipher> {
        match self {
            CipherType::XChaCha20Poly1305_1M => {
                let aead = XChaCha20Poly1305::new(key.into());
                let stream_encryptor = stream::StreamBE32::from_aead(aead, nonce.into());
                Box::new(XChaCha20Poly1305Cipher {
                    stream_encryptor,
                    cipher_type: *self,
                })
            }
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
    cipher_type: CipherType,
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
        self.cipher_type.plain_chunk_size()
    }

    fn cipher_type(&self) -> CipherType {
        self.cipher_type
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decrypted_size() {
        let cipher_type = CipherType::XChaCha20Poly1305_1M;
        let overhead = cipher_type.overhead() as u64;
        let plain_chunk_size = cipher_type.plain_chunk_size() as u64;
        let encrypted_chunk_size = plain_chunk_size + overhead;
        let header_size = cipher_type.header_size() as u64;

        assert_eq!(cipher_type.decrypted_size(0), 0);
        assert_eq!(cipher_type.decrypted_size(header_size + overhead), 0);
        assert_eq!(
            cipher_type.decrypted_size(header_size + 3 * encrypted_chunk_size),
            3 * plain_chunk_size
        );
        assert_eq!(
            cipher_type.decrypted_size(
                header_size + 3 * encrypted_chunk_size + overhead + 150
            ),
            3 * plain_chunk_size + 150
        );
    }

    #[test]
    fn test_encrypted_chunk_start() {
        let cipher_type = CipherType::XChaCha20Poly1305_1M;
        let plain_chunk_size = cipher_type.plain_chunk_size() as u64;
        let encrypted_chunk_size = cipher_type.encrypted_chunk_size() as u64;
        let header_size = cipher_type.header_size() as u64;

        assert_eq!(cipher_type.encrypted_chunk_start(0), header_size);
        assert_eq!(cipher_type.encrypted_chunk_start(plain_chunk_size - 1), header_size);
        assert_eq!(cipher_type.encrypted_chunk_start(plain_chunk_size), header_size + encrypted_chunk_size);
        assert_eq!(cipher_type.encrypted_chunk_start(2 * plain_chunk_size + 500), header_size + 2 * encrypted_chunk_size);
    }
}
