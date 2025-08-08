use crate::davs::error::DavFileError;
use async_stream::stream;
use axum::body::Body;
use chacha20poly1305::{
    aead::{
        stream::{NewStream, StreamPrimitive},
        KeyInit, stream,
    },
    XChaCha20Poly1305,
};
use futures::{Stream, StreamExt};
use headers::{ETag, LastModified};
use rand::{rngs::OsRng, TryRngCore};
use std::pin::Pin;
use std::{fs::Metadata, io, path::Path, time::SystemTime};
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::{
    fs::{self, File},
    io::{AsyncRead, AsyncSeekExt, AsyncWrite},
};

const PLAIN_CHUNK_SIZE: usize = 1_000_000; // 1 MByte
const ENCRYPTION_OVERHEAD: usize = 16;
const ENCRYPTED_CHUNK_SIZE: usize = PLAIN_CHUNK_SIZE + ENCRYPTION_OVERHEAD;
const NONCE_SIZE: usize = 19;

pub const BUF_SIZE: usize = 65536;

pub struct DavFile {
    file: File,
    key: Option<[u8; 32]>,
    metadata: Option<Metadata>,
}

impl DavFile {
    pub async fn create(path: &Path, key: Option<[u8; 32]>) -> io::Result<DavFile> {
        let file = fs::File::create(&path).await?;
        Ok(DavFile {
            file,
            key,
            metadata: None,
        })
    }

    pub async fn open(path: impl AsRef<Path>, key: Option<[u8; 32]>) -> io::Result<DavFile> {
        let (file, meta) = tokio::join!(fs::File::open(&path), fs::metadata(&path));
        let (file, meta) = (file?, meta?);
        Ok(DavFile {
            file,
            key,
            metadata: Some(meta),
        })
    }

    pub fn cache_headers(&self) -> Option<(ETag, LastModified)> {
        let mtime = self.metadata.as_ref()?.modified().ok()?;
        let timestamp = mtime
            .duration_since(SystemTime::UNIX_EPOCH)
            .ok()?
            .as_millis() as u64;
        let size = self.metadata.as_ref()?.len();
        if let Ok(etag) = format!(r#""{timestamp}-{size}""#).parse::<ETag>() {
            let last_modified = LastModified::from(mtime);
            Some((etag, last_modified))
        } else {
            None
        }
    }

    pub fn len(&self) -> u64 {
        let encrypted_size = self.metadata.as_ref().map_or(0, |v| v.len());
        if self.key.is_some() {
            decrypted_size(encrypted_size)
        } else {
            encrypted_size
        }
    }

    pub async fn copy_from<R>(mut self, reader: &mut R) -> Result<(), DavFileError>
    where
        R: AsyncRead + Unpin + ?Sized,
    {
        if let Some(key) = self.key {
            let mut enc_file = EncryptedStreamer::new(self.file, key);
            enc_file.copy_from(reader).await?;
            enc_file.inner.flush().await?;
        } else {
            tokio::io::copy(reader, &mut self.file).await?;
            self.file.flush().await?;
        }
        Ok(())
    }

    pub async fn copy_to<W>(mut self, writer: &mut W) -> Result<(), DavFileError>
    where
        W: AsyncWrite + Unpin + ?Sized,
    {
        if let Some(key) = self.key {
            let encrypted_file = EncryptedStreamer::new(self.file, key);
            encrypted_file.copy_to(writer).await.map(|_| ())?;
        } else {
            tokio::io::copy(&mut self.file, writer).await?;
        }
        writer.flush().await?;
        Ok(())
    }

    pub async fn into_body_sized(mut self, start: u64, max_length: u64) -> Result<Body, io::Error> {
        if let Some(key) = self.key {
            let encrypted_file = EncryptedStreamer::new(self.file, key);
            Ok(Body::from_stream(
                encrypted_file.into_stream_sized(start, max_length),
            ))
        } else {
            self.file.seek(std::io::SeekFrom::Start(start)).await?;
            let reader = Streamer::new(self.file, BUF_SIZE);
            Ok(Body::from_stream(reader.into_stream_sized(max_length)))
        }
    }

    pub fn into_body(self) -> Body {
        if let Some(key) = self.key {
            let encrypted_file = EncryptedStreamer::new(self.file, key);
            Body::from_stream(encrypted_file.into_stream())
        } else {
            let reader = Streamer::new(self.file, BUF_SIZE);
            Body::from_stream(reader.into_stream())
        }
    }
}

pub struct Streamer<R>
where
    R: AsyncRead + Unpin + Send + 'static,
{
    reader: R,
    buf_size: usize,
}

impl<R> Streamer<R>
where
    R: AsyncRead + Unpin + Send + 'static,
{
    #[inline]
    pub fn new(reader: R, buf_size: usize) -> Self {
        Self { reader, buf_size }
    }
    pub fn into_stream(
        mut self,
    ) -> Pin<Box<impl ?Sized + Stream<Item = Result<Vec<u8>, io::Error>> + 'static>> {
        let stream = stream! {
            loop {
                let mut buf = vec![0; self.buf_size];
                let r = self.reader.read(&mut buf).await?;
                if r == 0 {
                    break
                }
                buf.truncate(r);
                yield Ok(buf);
            }
        };
        stream.boxed()
    }

    // allow truncation as truncated remaining is always less than buf_size: usize
    fn into_stream_sized(
        mut self,
        max_length: u64,
    ) -> Pin<Box<impl ?Sized + Stream<Item = Result<Vec<u8>, io::Error>> + 'static>> {
        let stream = stream! {
        let mut remaining = max_length;
            loop {
                if remaining == 0 {
                    break;
                }
                let bs = if remaining >= self.buf_size as u64 {
                    self.buf_size
                } else {
                    remaining as usize
                };
                let mut buf = vec![0; bs];
                let r = self.reader.read(&mut buf).await?;
                if r == 0 {
                    break;
                } else {
                    buf.truncate(r);
                    yield Ok(buf);
                }
                remaining -= r as u64;
            }
        };
        stream.boxed()
    }
}

struct EncryptedStreamer<I>
where
    I: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    inner: I,
    key: [u8; 32],
}

impl<I> EncryptedStreamer<I>
where
    I: AsyncRead + AsyncWrite + AsyncSeekExt + Unpin + Send + 'static,
{
    #[inline]
    fn new(inner: I, key: [u8; 32]) -> Self {
        Self { inner, key }
    }

    async fn copy_from<R>(&mut self, mut reader: &mut R) -> Result<u64, DavFileError>
    where
        R: AsyncRead + Unpin + ?Sized,
    {
        let mut nonce = [0; NONCE_SIZE];
        OsRng
            .try_fill_bytes(&mut nonce)
            .map_err(|e| DavFileError::NonceGeneration(e.to_string()))?;
        let aead = XChaCha20Poly1305::new(self.key.as_ref().into());
        let mut stream_encryptor = stream::EncryptorBE32::from_aead(aead, nonce.as_ref().into());

        // Write the nonce as stream header
        self.inner.write_all(&nonce).await?;
        let mut total_count = 0;

        loop {
            let mut buffer = Vec::with_capacity(PLAIN_CHUNK_SIZE);
            let mut chunked_reader = reader.take(PLAIN_CHUNK_SIZE as u64);

            let read_count = chunked_reader.read_to_end(&mut buffer).await?;
            total_count += read_count;

            reader = chunked_reader.into_inner();
            buffer.truncate(read_count);

            if read_count == PLAIN_CHUNK_SIZE {
                let ciphertext = stream_encryptor
                    .encrypt_next(buffer.as_slice())
                    .map_err(|e| DavFileError::Encryption(e.to_string()))?;
                self.inner.write_all(&ciphertext).await?;
            } else {
                let ciphertext = stream_encryptor
                    .encrypt_last(buffer.get(..read_count).ok_or(DavFileError::Slice)?)
                    .map_err(|e| DavFileError::Encryption(e.to_string()))?;
                self.inner.write_all(&ciphertext).await?;
                break;
            }
        }
        self.inner.flush().await?;
        Ok(total_count as u64)
    }

    async fn copy_to<W>(mut self, writer: &mut W) -> Result<u64, DavFileError>
    where
        W: AsyncWrite + Unpin + ?Sized,
    {
        let nonce = self.retrieve_nonce().await?;
        let aead = XChaCha20Poly1305::new(self.key.as_ref().into());

        let mut stream_decryptor = stream::DecryptorBE32::from_aead(aead, nonce.as_ref().into());

        let mut total_count = 0;

        loop {
            let mut buffer = Vec::with_capacity(ENCRYPTED_CHUNK_SIZE);
            let mut reader = self.inner.take(ENCRYPTED_CHUNK_SIZE as u64);

            let read_count = reader.read_to_end(&mut buffer).await?;
            total_count += read_count;

            self.inner = reader.into_inner();
            buffer.truncate(read_count);

            if read_count == ENCRYPTED_CHUNK_SIZE {
                let plaintext = stream_decryptor
                    .decrypt_next(buffer.as_slice())
                    .map_err(|e| DavFileError::Decryption(e.to_string()))?;
                writer.write_all(&plaintext).await?;
            } else if read_count == 0 {
                break;
            } else {
                let plaintext = stream_decryptor
                    .decrypt_last(buffer.get(..read_count).ok_or(DavFileError::Slice)?)
                    .map_err(|e| DavFileError::Decryption(e.to_string()))?;
                writer.write_all(&plaintext).await?;
                break;
            }
        }
        Ok(total_count as u64)
    }

    fn into_stream(
        mut self,
    ) -> Pin<Box<impl ?Sized + Stream<Item = Result<Vec<u8>, io::Error>> + 'static>> {
        let stream = stream! {
            let aead = XChaCha20Poly1305::new(self.key.as_ref().into());
            let nonce = self.retrieve_nonce().await?;
            let mut stream_decryptor = stream::DecryptorBE32::from_aead(aead, nonce.as_ref().into());

             loop {
                let mut buffer = Vec::with_capacity(ENCRYPTED_CHUNK_SIZE);
                let mut reader = self.inner.take(ENCRYPTED_CHUNK_SIZE as u64);

                let read_count = reader.read_to_end(&mut buffer).await?;

                self.inner = reader.into_inner();
                buffer.truncate(read_count);

                if read_count == ENCRYPTED_CHUNK_SIZE {
                    let plaintext = match stream_decryptor
                        .decrypt_next(buffer.as_slice()) {
                            Ok(plaintext) => plaintext,
                            Err(e) => {yield Err(DavFileError::Decryption(e.to_string()).into());break;}
                        };
                    yield Ok(plaintext);
                } else if read_count == 0 {
                    break;
                } else {
                    let plaintext = match stream_decryptor
                    .decrypt_last(buffer.get(..read_count).ok_or(DavFileError::Slice)?){
                            Ok(plaintext) => plaintext,
                            Err(e) => {yield Err(DavFileError::Decryption(e.to_string()).into());break;}
                        };
                        yield Ok(plaintext);
                     break;
                }
            }

        };
        stream.boxed()
    }

    /// Creates a stream that reads and decrypts a sized portion of the file.
    /// This is used to handle HTTP Range requests on encrypted files.
    /// The implementation is complex because it needs to map a plaintext range
    /// to a range in the encrypted file, which is non-trivial due to the
    /// chunked encryption format.
    fn into_stream_sized(
        mut self,
        start: u64,
        max_length: u64,
    ) -> Pin<Box<impl ?Sized + Stream<Item = Result<Vec<u8>, io::Error>> + 'static>> {
        let stream = stream! {
            // Initialize the decryptor
            let aead = XChaCha20Poly1305::new(self.key.as_ref().into());
            let nonce = self.retrieve_nonce().await?;
            let stream_decryptor = stream::StreamBE32::from_aead(aead, nonce.as_ref().into());

            // Calculate the starting position in the encrypted stream
            let mut chunked_position = ChunkedPosition::new(start);
            self.inner.seek(std::io::SeekFrom::Start(chunked_position.beginning_of_active_chunk)).await?;


        let mut remaining = max_length;
            loop {
                if remaining == 0 {
                    break;
                }
                let mut buffer = Vec::with_capacity(ENCRYPTED_CHUNK_SIZE);
                let mut reader = self.inner.take(ENCRYPTED_CHUNK_SIZE as u64);
                let read_count = reader.read_to_end(&mut buffer).await?;
                self.inner = reader.into_inner();
                buffer.truncate(read_count);

                if read_count == ENCRYPTED_CHUNK_SIZE {
                    let mut plaintext = match Self::decrypt_chunk(&stream_decryptor, &buffer, chunked_position.active_chunk_counter as u32, false) {
                        Ok(plaintext) => plaintext,
                        Err(e) => {
                            yield Err(e.into());
                            break;
                        }
                    };

                        chunked_position.active_chunk_counter+= 1;

                        if start != 0 {
                            plaintext.drain(0..chunked_position.offset_in_active_chunk as usize);
                            chunked_position.offset_in_active_chunk = 0;
                        }
                        if (remaining as usize) < plaintext.len()   {
                            plaintext.truncate(remaining as usize);
                             yield Ok(plaintext);
                            break;
                        } else {
                            remaining -= plaintext.len() as u64;
                        }

                    yield Ok(plaintext);

                } else if read_count == 0 {
                    break;
                } else {
                    let mut plaintext = match Self::decrypt_chunk(&stream_decryptor, &buffer, chunked_position.active_chunk_counter as u32, true) {
                        Ok(plaintext) => plaintext,
                        Err(e) => {
                            yield Err(e.into());
                            break;
                        }
                    };

                        if start != 0 {
                            plaintext.drain(0..chunked_position.offset_in_active_chunk as usize);
                        }
                        if (remaining as usize) < plaintext.len()   {

                            plaintext.truncate(remaining as usize);
                        }
                        yield Ok(plaintext);
                     break;
                }

            }
        };
        stream.boxed()
    }

    fn decrypt_chunk(
        decryptor: &stream::StreamBE32<XChaCha20Poly1305>,
        buffer: &[u8],
        position: u32,
        is_last: bool,
    ) -> Result<Vec<u8>, DavFileError> {
        decryptor
            .decrypt(position, is_last, buffer)
            .map_err(|e| DavFileError::Decryption(e.to_string()))
    }

    async fn retrieve_nonce(&mut self) -> Result<[u8; NONCE_SIZE], std::io::Error> {
        let mut nonce = [0u8; NONCE_SIZE];
        self.inner.read_exact(&mut nonce).await?;
        Ok(nonce)
    }
}

pub fn decrypted_size(enc_size: u64) -> u64 {
    if enc_size == 0 {
        return 0;
    }
    let number_of_chunks = {
        let enc_size_without_nonce = enc_size - NONCE_SIZE as u64;
        let d = enc_size_without_nonce / ENCRYPTED_CHUNK_SIZE as u64;
        let r = enc_size_without_nonce % ENCRYPTED_CHUNK_SIZE as u64;
        if r > 0 { d + 1 } else { d }
    };
    enc_size - ENCRYPTION_OVERHEAD as u64 * number_of_chunks - NONCE_SIZE as u64
}

fn encrypted_offset(dec_offset: u64) -> u64 {
    let number_of_chunks = dec_offset / PLAIN_CHUNK_SIZE as u64 + 1;
    dec_offset + ENCRYPTION_OVERHEAD as u64 * number_of_chunks + NONCE_SIZE as u64
}

/// Represents a position within the encrypted file, mapped from a plaintext offset.
#[derive(PartialEq, Debug)]
struct ChunkedPosition {
    /// The byte offset of the beginning of the chunk that contains the target plaintext offset.
    beginning_of_active_chunk: u64,
    /// The byte offset within the decrypted chunk where the target plaintext offset is located.
    offset_in_active_chunk: u64,
    /// The index of the chunk that contains the target plaintext offset.
    active_chunk_counter: u64,
}

impl ChunkedPosition {
    /// Creates a new `ChunkedPosition` from a plaintext offset.
    fn new(plain_offset: u64) -> Self {
        // Calculate which chunk the plaintext offset falls into.
        let active_chunk_counter = plain_offset / PLAIN_CHUNK_SIZE as u64;
        // Calculate the starting position of that chunk in the encrypted file.
        let beginning_of_active_chunk =
            active_chunk_counter * ENCRYPTED_CHUNK_SIZE as u64 + NONCE_SIZE as u64;
        // Calculate the encrypted offset corresponding to the plaintext offset.
        let start = encrypted_offset(plain_offset);
        // Calculate the offset within the decrypted chunk.
        let offset_in_active_chunk =
            start - (beginning_of_active_chunk + ENCRYPTION_OVERHEAD as u64);
        Self {
            beginning_of_active_chunk,
            offset_in_active_chunk,
            active_chunk_counter,
        }
    }
}
#[cfg(test)]
mod tests {
    use crate::davs::dav_file::{
        ChunkedPosition, ENCRYPTED_CHUNK_SIZE, ENCRYPTION_OVERHEAD, NONCE_SIZE, PLAIN_CHUNK_SIZE,
        decrypted_size,
    };

    #[test]
    fn test_decrypted_size() {
        let nonce_size = NONCE_SIZE as u64;
        let encryption_overhead = ENCRYPTION_OVERHEAD as u64;
        let encrypted_chunk_size = ENCRYPTED_CHUNK_SIZE as u64;
        let plain_chunk_size = PLAIN_CHUNK_SIZE as u64;

        assert_eq!(decrypted_size(0), 0);
        assert_eq!(decrypted_size(nonce_size + encryption_overhead), 0);
        assert_eq!(
            decrypted_size(nonce_size + 3 * encrypted_chunk_size),
            3 * plain_chunk_size
        );
        assert_eq!(
            decrypted_size(
                nonce_size + 3 * encrypted_chunk_size + ENCRYPTION_OVERHEAD as u64 + 150
            ),
            3 * plain_chunk_size + 150
        );
    }

    #[test]
    fn test_chunked_position() {
        let nonce_size = NONCE_SIZE as u64;
        let encrypted_chunk_size = ENCRYPTED_CHUNK_SIZE as u64;
        let plain_chunk_size = PLAIN_CHUNK_SIZE as u64;

        assert_eq!(
            ChunkedPosition::new(0),
            ChunkedPosition {
                beginning_of_active_chunk: nonce_size,
                offset_in_active_chunk: 0,
                active_chunk_counter: 0
            }
        );

        assert_eq!(
            ChunkedPosition::new(100),
            ChunkedPosition {
                beginning_of_active_chunk: nonce_size,
                offset_in_active_chunk: 100,
                active_chunk_counter: 0
            }
        );

        assert_eq!(
            ChunkedPosition::new(100 + 2 * plain_chunk_size),
            ChunkedPosition {
                beginning_of_active_chunk: nonce_size + 2 * encrypted_chunk_size,
                offset_in_active_chunk: 100,
                active_chunk_counter: 2
            }
        );
    }
}
