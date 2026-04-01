use super::crypto::{Cipher, CipherType, create_cipher, DEFAULT_PLAIN_CHUNK_SIZE};
use futures::ready;
use headers::{ETag, LastModified};
use rand::{TryRng, rngs::SysRng};
use std::io::{self, SeekFrom};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::{path::Path, time::SystemTime};
use tokio::fs::{self, File};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeek, AsyncWrite, AsyncWriteExt, ReadBuf};


const BUFFER_ERROR: &str = "buffer error for encryption or decryption";

pub enum DavFile {
    Plain(File),
    Encrypted {
        file: Box<File>,
        key: [u8; 32],
        read_buffer: Vec<u8>,
        encrypted_read_buffer: Vec<u8>,
        write_buffer: Vec<u8>,
        pos: u64,
        decrypted_len: u64,
        nonce: Vec<u8>,
        offset_in_chunk: u32,
        read_chunk_idx: u32,
        write_chunk_idx: u32,
        seeked_after_open: bool,
        cipher: Box<dyn Cipher>,
        write_op_in_progress: bool,
    },
}

impl DavFile {
    pub async fn create(path: &Path, key: Option<[u8; 32]>) -> io::Result<DavFile> {
        Self::create_with_chunk_size(path, key, DEFAULT_PLAIN_CHUNK_SIZE).await
    }

    pub async fn create_with_chunk_size(
        path: &Path,
        key: Option<[u8; 32]>,
        plain_chunk_size: usize,
    ) -> io::Result<DavFile> {
        let mut file = fs::File::create(&path).await?;

        match key {
            Some(key) => {
                let cipher_type = CipherType::XChaCha20Poly1305;
                let nonce_size = cipher_type.nonce_size();
                let mut nonce = vec![0u8; nonce_size];
                TryRng::try_fill_bytes(&mut SysRng, &mut nonce)
                    .map_err(|e| io::Error::other(e.to_string()))?;

                // Header: plain_chunk_size (u32), cipher_type (u8), nonce
                file.write_all(&(plain_chunk_size as u32).to_be_bytes())
                    .await?;
                file.write_all(&[cipher_type as u8]).await?;
                file.write_all(&nonce).await?;
                file.flush().await?;

                let cipher = create_cipher(cipher_type, &key, &nonce, plain_chunk_size);

                Ok(DavFile::Encrypted {
                    file: Box::new(file),
                    key,
                    read_buffer: Vec::new(),
                    encrypted_read_buffer: Vec::new(),
                    write_buffer: Vec::new(),
                    pos: 0,
                    decrypted_len: 0,
                    nonce,
                    offset_in_chunk: 0,
                    read_chunk_idx: 0,
                    write_chunk_idx: 0,
                    seeked_after_open: false,
                    cipher,
                    write_op_in_progress: false,
                })
            }
            None => Ok(DavFile::Plain(file)),
        }
    }

    pub async fn open(path: impl AsRef<Path>, key: Option<[u8; 32]>) -> io::Result<DavFile> {
        let mut file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .await?;
        let metadata = file.metadata().await?;
        match key {
            Some(key) => {
                if metadata.len() == 0 {
                    let cipher_type = CipherType::XChaCha20Poly1305;
                    let plain_chunk_size = DEFAULT_PLAIN_CHUNK_SIZE;
                    let nonce_size = cipher_type.nonce_size();
                    let nonce = vec![0u8; nonce_size];
                    let cipher = create_cipher(cipher_type, &key, &nonce, plain_chunk_size);
                    return Ok(DavFile::Encrypted {
                        file: Box::new(file),
                        key,
                        read_buffer: Vec::new(),
                        encrypted_read_buffer: Vec::new(),
                        write_buffer: Vec::new(),
                        pos: 0,
                        decrypted_len: 0,
                        nonce,
                        offset_in_chunk: 0,
                        read_chunk_idx: 0,
                        write_chunk_idx: 0,
                        seeked_after_open: false,
                        cipher,
                        write_op_in_progress: false,
                    });
                }

                let mut plain_chunk_size_bytes = [0u8; 4];
                file.read_exact(&mut plain_chunk_size_bytes).await?;
                let plain_chunk_size = u32::from_be_bytes(plain_chunk_size_bytes) as usize;

                let mut cipher_type_byte = [0u8; 1];
                file.read_exact(&mut cipher_type_byte).await?;
                let cipher_type = CipherType::from_u8(cipher_type_byte[0])
                    .map_err(|e| io::Error::other(e))?;

                let nonce_size = cipher_type.nonce_size();
                let mut nonce = vec![0u8; nonce_size];
                file.read_exact(&mut nonce).await?;

                let overhead = cipher_type.overhead();
                let encrypted_chunk_size = plain_chunk_size + overhead;
                let header_size = 4 + 1 + nonce_size;

                let enc_size_without_header = metadata.len().saturating_sub(header_size as u64);
                let write_chunk_idx_initial =
                    (enc_size_without_header / encrypted_chunk_size as u64) as u32;

                let cipher = create_cipher(cipher_type, &key, &nonce, plain_chunk_size);

                Ok(DavFile::Encrypted {
                    file: Box::new(file),
                    key,
                    read_buffer: Vec::new(),
                    encrypted_read_buffer: Vec::new(),
                    write_buffer: Vec::new(),
                    pos: 0,
                    decrypted_len: decrypted_size_ext(
                        metadata.len(),
                        plain_chunk_size,
                        cipher_type,
                    ),
                    nonce,
                    offset_in_chunk: 0,
                    read_chunk_idx: 0,
                    write_chunk_idx: write_chunk_idx_initial,
                    seeked_after_open: false,
                    cipher,
                    write_op_in_progress: false,
                })
            }
            None => Ok(DavFile::Plain(file)),
        }
    }

    pub async fn len(&self) -> u64 {
        match self {
            DavFile::Plain(file) => file.metadata().await.map_or(0, |m| m.len()),
            DavFile::Encrypted { decrypted_len, .. } => *decrypted_len,
        }
    }

    pub async fn is_empty(&self) -> bool {
        self.len().await == 0
    }

    pub async fn cache_headers(&self) -> Option<(ETag, LastModified)> {
        let metadata = match match self {
            DavFile::Plain(file) => file.metadata().await,
            DavFile::Encrypted { file, .. } => file.metadata().await,
        } {
            Ok(m) => m,
            Err(_) => return None,
        };
        let mtime = metadata.modified().ok()?;
        let timestamp = mtime
            .duration_since(SystemTime::UNIX_EPOCH)
            .ok()?
            .as_millis() as u64;
        let size = self.len().await;
        if let Ok(etag) = format!(r#""{timestamp}-{size}""#).parse::<ETag>() {
            let last_modified = LastModified::from(mtime);
            Some((etag, last_modified))
        } else {
            None
        }
    }
}

impl AsyncRead for DavFile {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.get_mut() {
            DavFile::Plain(file) => Pin::new(file).poll_read(cx, buf),
            DavFile::Encrypted {
                file,
                read_buffer,
                encrypted_read_buffer,
                pos,
                offset_in_chunk,
                read_chunk_idx,
                cipher,
                ..
            } => {
                let plain_chunk_size = cipher.plain_chunk_size();
                let cipher_type = cipher.cipher_type();
                let encrypted_chunk_size = plain_chunk_size + cipher_type.overhead();
                // first, return any leftover plaintext
                if !read_buffer.is_empty() {
                    let len = std::cmp::min(buf.remaining(), read_buffer.len());
                    buf.put_slice(
                        read_buffer
                            .get(..len)
                            .ok_or(io::Error::other(BUFFER_ERROR))?,
                    );
                    read_buffer.drain(..len);
                    *pos += len as u64;
                    return Poll::Ready(Ok(()));
                }

                // fill encrypted_read_buffer to at least one chunk
                while encrypted_read_buffer.len() < encrypted_chunk_size {
                    let mut tmp = vec![0u8; encrypted_chunk_size - encrypted_read_buffer.len()];
                    let mut read_buf = ReadBuf::new(&mut tmp);
                    match Pin::new(&mut **file).poll_read(cx, &mut read_buf) {
                        Poll::Ready(Ok(())) => {
                            let n = read_buf.filled().len();
                            if n == 0 {
                                break; // EOF
                            }
                            encrypted_read_buffer.extend_from_slice(
                                tmp.get(..n).ok_or(io::Error::other(BUFFER_ERROR))?,
                            );
                        }
                        Poll::Ready(Err(e)) => {
                            return Poll::Ready(Err(e));
                        }
                        Poll::Pending => return Poll::Pending,
                    }
                }

                if encrypted_read_buffer.is_empty() {
                    return Poll::Ready(Ok(()));
                }

                let is_last = encrypted_read_buffer.len() < encrypted_chunk_size;

                let mut plaintext = cipher
                    .decrypt(*read_chunk_idx, is_last, encrypted_read_buffer.as_slice())
                    .map_err(|e| io::Error::other(format!("Decryption error: {e}")))?;

                encrypted_read_buffer.clear();
                *read_chunk_idx += 1;

                // apply offset_in_chunk if needed
                if *offset_in_chunk > 0 {
                    let offset = *offset_in_chunk as usize;
                    if offset < plaintext.len() {
                        plaintext.drain(..offset);
                    } else {
                        plaintext.clear();
                    }
                    *offset_in_chunk = 0;
                }

                // fill the user buffer and keep remainder in read_buffer
                let len = std::cmp::min(buf.remaining(), plaintext.len());
                buf.put_slice(plaintext.get(..len).ok_or(io::Error::other(BUFFER_ERROR))?);
                if len < plaintext.len() {
                    read_buffer.extend_from_slice(
                        plaintext.get(len..).ok_or(io::Error::other(BUFFER_ERROR))?,
                    );
                }
                *pos += len as u64;

                Poll::Ready(Ok(()))
            }
        }
    }
}

impl AsyncWrite for DavFile {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            DavFile::Plain(file) => Pin::new(file).poll_write(cx, buf),
            DavFile::Encrypted {
                file,
                write_buffer,
                decrypted_len,
                write_chunk_idx,
                seeked_after_open,
                cipher,
                write_op_in_progress,
                ..
            } => {
                if *seeked_after_open {
                    return Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::Unsupported,
                        "writing after seek is not supported for encrypted files",
                    )));
                }

                if !*write_op_in_progress {
                    write_buffer.extend_from_slice(buf);
                    *decrypted_len += buf.len() as u64;
                }

                *write_op_in_progress = true;

                match poll_write_chunks(
                    cx,
                    file,
                    write_buffer,
                    write_chunk_idx,
                    cipher,
                    false,
                ) {
                    Poll::Ready(Ok(())) => {
                        *write_op_in_progress = false;
                        Poll::Ready(Ok(buf.len()))
                    }
                    Poll::Ready(Err(e)) => {
                        *write_op_in_progress = false;
                        Poll::Ready(Err(e))
                    }
                    Poll::Pending => Poll::Pending,
                }
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            DavFile::Plain(file) => Pin::new(file).poll_flush(cx),
            DavFile::Encrypted {
                file,
                write_buffer,
                write_chunk_idx,
                seeked_after_open,
                cipher,
                ..
            } => {
                if *seeked_after_open {
                    return Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::Unsupported,
                        "writing after seek is not supported for encrypted files",
                    )));
                }
                ready!(poll_write_chunks(
                    cx,
                    file,
                    write_buffer,
                    write_chunk_idx,
                    cipher,
                    false
                ))?;
                Pin::new(&mut **file).poll_flush(cx)
            }
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        ready!(self.as_mut().poll_flush(cx))?;
        let me = self.get_mut();
        match me {
            DavFile::Plain(file) => Pin::new(file).poll_shutdown(cx),
            DavFile::Encrypted {
                file,
                write_buffer,
                write_chunk_idx,
                seeked_after_open,
                cipher,
                ..
            } => {
                if *seeked_after_open {
                    return Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::Unsupported,
                        "writing after seek is not supported for encrypted files",
                    )));
                }
                ready!(poll_write_chunks(
                    cx,
                    file,
                    write_buffer,
                    write_chunk_idx,
                    cipher,
                    true
                ))?;
                Pin::new(&mut **file).poll_shutdown(cx)
            }
        }
    }
}

impl AsyncSeek for DavFile {
    fn start_seek(self: Pin<&mut Self>, position: SeekFrom) -> io::Result<()> {
        match self.get_mut() {
            DavFile::Plain(file) => Pin::new(file).start_seek(position),
            DavFile::Encrypted {
                file,
                pos,
                decrypted_len,
                read_buffer,
                encrypted_read_buffer,
                offset_in_chunk,
                read_chunk_idx,
                seeked_after_open,
                cipher,
                ..
            } => {
                *seeked_after_open = true;
                let new_pos = match position {
                    SeekFrom::Start(p) => p as i64,
                    SeekFrom::End(p) => *decrypted_len as i64 + p,
                    SeekFrom::Current(p) => *pos as i64 + p,
                };

                if new_pos < 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "invalid seek to a negative position",
                    ));
                }

                *pos = new_pos as u64;
                read_buffer.clear();
                encrypted_read_buffer.clear();

                let plain_chunk_size = cipher.plain_chunk_size();
                let cipher_type = cipher.cipher_type();
                let encrypted_pos = encrypted_chunk_start_ext(*pos, plain_chunk_size, cipher_type);
                *offset_in_chunk = (*pos % plain_chunk_size as u64) as u32;
                *read_chunk_idx = (*pos / plain_chunk_size as u64) as u32;
                Pin::new(&mut **file).start_seek(SeekFrom::Start(encrypted_pos))
            }
        }
    }

    fn poll_complete(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<u64>> {
        match self.get_mut() {
            DavFile::Plain(file) => Pin::new(file).poll_complete(cx),
            DavFile::Encrypted { file, pos, .. } => {
                ready!(Pin::new(&mut **file).poll_complete(cx))?;
                Poll::Ready(Ok(*pos))
            }
        }
    }
}

fn encrypted_chunk_start_ext(dec_offset: u64, plain_chunk_size: usize, cipher_type: CipherType) -> u64 {
    let encrypted_chunk_size = plain_chunk_size + cipher_type.overhead();
    let header_size = 4 + 1 + cipher_type.nonce_size();
    let chunk_idx = dec_offset / plain_chunk_size as u64;
    header_size as u64 + chunk_idx * encrypted_chunk_size as u64
}

pub fn decrypted_size_ext(enc_size: u64, plain_chunk_size: usize, cipher_type: CipherType) -> u64 {
    let header_size = 4 + 1 + cipher_type.nonce_size();
    let overhead = cipher_type.overhead();
    let encrypted_chunk_size = plain_chunk_size + overhead;

    if enc_size <= header_size as u64 {
        return 0;
    }
    let enc_size_without_header = enc_size - header_size as u64;
    let num_chunks = enc_size_without_header.div_ceil(encrypted_chunk_size as u64);
    if num_chunks == 0 {
        return 0;
    }
    let last_chunk_size = enc_size_without_header - (num_chunks - 1) * encrypted_chunk_size as u64;
    (num_chunks - 1) * plain_chunk_size as u64
        + (last_chunk_size.saturating_sub(overhead as u64))
}

pub fn decrypted_size(enc_size: u64) -> u64 {
    decrypted_size_ext(
        enc_size,
        DEFAULT_PLAIN_CHUNK_SIZE,
        CipherType::XChaCha20Poly1305,
    )
}

fn poll_write_chunks(
    cx: &mut Context<'_>,
    file: &mut File,
    write_buffer: &mut Vec<u8>,
    write_chunk_idx: &mut u32,
    cipher: &mut Box<dyn Cipher>,
    finalize: bool,
) -> Poll<io::Result<()>> {
    let plain_chunk_size = cipher.plain_chunk_size();
    while write_buffer.len() >= plain_chunk_size || (finalize && !write_buffer.is_empty()) {
        let is_last = finalize && write_buffer.len() <= plain_chunk_size;
        let chunk_len = std::cmp::min(write_buffer.len(), plain_chunk_size);
        let chunk = write_buffer
            .get(..chunk_len)
            .ok_or(io::Error::other(BUFFER_ERROR))?;

        let ciphertext = cipher
            .encrypt(*write_chunk_idx, is_last, chunk)
            .map_err(|e| io::Error::other(format!("Encryption error: {}", e)))?;

        // write the ciphertext fully to disk
        let mut written = 0;
        while written < ciphertext.len() {
            let bytes_written = ready!(
                Pin::new(&mut *file).poll_write(
                    cx,
                    ciphertext
                        .get(written..)
                        .ok_or(io::Error::other(BUFFER_ERROR))?
                )
            )?;
            if bytes_written == 0 {
                return Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "failed to write whole chunk",
                )));
            }
            written += bytes_written;
        }
        write_buffer.drain(..chunk_len);
        *write_chunk_idx += 1;
    }
    Poll::Ready(Ok(()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

    #[test]
    fn test_decrypted_size() {
        let cipher_type = CipherType::XChaCha20Poly1305;
        let nonce_size = cipher_type.nonce_size() as u64;
        let overhead = cipher_type.overhead() as u64;
        let plain_chunk_size = DEFAULT_PLAIN_CHUNK_SIZE as u64;
        let encrypted_chunk_size = plain_chunk_size + overhead;
        let header_size = 4 + 1 + nonce_size;

        assert_eq!(decrypted_size_ext(0, plain_chunk_size as usize, cipher_type), 0);
        assert_eq!(decrypted_size_ext(header_size + overhead, plain_chunk_size as usize, cipher_type), 0);
        assert_eq!(
            decrypted_size_ext(header_size + 3 * encrypted_chunk_size, plain_chunk_size as usize, cipher_type),
            3 * plain_chunk_size
        );
        assert_eq!(
            decrypted_size_ext(
                header_size + 3 * encrypted_chunk_size + overhead + 150,
                plain_chunk_size as usize,
                cipher_type
            ),
            3 * plain_chunk_size + 150
        );
    }

    #[tokio::test]
    async fn test_plain_file() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("test.txt");

        let mut file = DavFile::create(&path, None).await?;
        file.write_all(b"hello world").await?;
        file.shutdown().await?;

        let mut file = DavFile::open(&path, None).await?;
        let mut contents = String::new();
        file.read_to_string(&mut contents).await?;
        assert_eq!(contents, "hello world");

        Ok(())
    }

    #[tokio::test]
    async fn test_encrypted_file() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("test.txt.enc");
        let key = [42u8; 32];

        let mut file = DavFile::create(&path, Some(key)).await?;
        file.write_all(b"hello encrypted world").await?;
        file.shutdown().await?;

        let mut file = DavFile::open(&path, Some(key)).await?;
        let mut contents = String::new();
        file.read_to_string(&mut contents).await?;
        assert_eq!(contents, "hello encrypted world");

        Ok(())
    }

    #[tokio::test]
    async fn test_encrypted_file_seek() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("test.txt.enc");
        let key = [42u8; 32];
        let content = b"hello encrypted world, this is a long sentence to test seeking.";

        let mut file = DavFile::create(&path, Some(key)).await?;
        file.write_all(content).await?;
        file.shutdown().await?;

        let mut file = DavFile::open(&path, Some(key)).await?;

        file.seek(SeekFrom::Start(6)).await?;
        let mut contents = String::new();
        file.read_to_string(&mut contents).await?;
        assert_eq!(
            contents,
            "encrypted world, this is a long sentence to test seeking."
        );

        file.seek(SeekFrom::Start(0)).await?;
        let mut contents = String::new();
        file.read_to_string(&mut contents).await?;
        assert_eq!(
            contents,
            "hello encrypted world, this is a long sentence to test seeking."
        );

        file.seek(SeekFrom::End(-10)).await?;
        let mut contents = String::new();
        file.read_to_string(&mut contents).await?;
        assert_eq!(contents, "t seeking.");

        Ok(())
    }

    #[tokio::test]
    async fn test_large_encrypted_file() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("large.txt.enc");
        let key = [42u8; 32];
        let content = vec![0xAB; DEFAULT_PLAIN_CHUNK_SIZE * 3 + 123];

        let mut file = DavFile::create(&path, Some(key)).await?;
        file.write_all(&content).await?;
        file.shutdown().await?;

        let mut file = DavFile::open(&path, Some(key)).await?;
        let mut read_content = Vec::new();
        file.read_to_end(&mut read_content).await?;

        assert_eq!(content.len(), read_content.len());
        assert_eq!(content, read_content);

        Ok(())
    }

    #[tokio::test]
    async fn test_encrypted_file_truncated() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("trunc.txt.enc");
        let key = [7u8; 32];
        let content = b"this will be truncated at the end for testing";

        // create and write normally
        {
            let mut file = DavFile::create(&path, Some(key)).await?;
            file.write_all(content).await?;
            file.shutdown().await?;
        }

        // Truncate the underlying file by removing the last N bytes of the file
        // (simulate corruption)
        let stdf = std::fs::OpenOptions::new().write(true).open(&path)?;
        let meta = stdf.metadata()?;
        let new_len = meta.len().saturating_sub(5); // remove 5 bytes
        stdf.set_len(new_len)?;
        stdf.sync_all()?;

        // now open with our reader and try to read; because last ciphertext chunk is truncated
        let mut file = DavFile::open(&path, Some(key)).await?;
        let mut out = Vec::new();
        let res = file.read_to_end(&mut out).await;
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Other);
        assert!(err.to_string().contains("Decryption error"));
        Ok(())
    }

    #[tokio::test]
    async fn test_encrypted_file_seek_across_chunk_boundary() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("large.txt.enc");
        let key = [42u8; 32];
        let plain_chunk_size = DEFAULT_PLAIN_CHUNK_SIZE;
        let mut content = vec![0u8; plain_chunk_size * 2];
        for i in 0..plain_chunk_size {
            content[i] = 0xAA;
        }
        for i in plain_chunk_size..(plain_chunk_size * 2) {
            content[i] = 0xBB;
        }

        let mut file = DavFile::create(&path, Some(key)).await?;
        file.write_all(&content).await?;
        file.shutdown().await?;

        let mut file = DavFile::open(&path, Some(key)).await?;
        file.seek(SeekFrom::Start((plain_chunk_size - 5) as u64))
            .await?;
        let mut buf = [0u8; 10];
        file.read_exact(&mut buf).await?;

        assert_eq!(&buf, &content[plain_chunk_size - 5..plain_chunk_size + 5]);

        Ok(())
    }

    #[tokio::test]
    async fn test_large_encrypted_file_read_byte_by_byte() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("large_byte_by_byte.txt.enc");
        let key = [88u8; 32];
        let content = vec![0xAB; DEFAULT_PLAIN_CHUNK_SIZE * 2 + 555];

        let mut file = DavFile::create(&path, Some(key)).await?;
        file.write_all(&content).await?;
        file.shutdown().await?;

        let mut file = DavFile::open(&path, Some(key)).await?;
        let mut read_content = Vec::new();
        let mut byte = [0u8; 1];
        while file.read(&mut byte).await? > 0 {
            read_content.push(byte[0]);
        }

        assert_eq!(content.len(), read_content.len());
        assert_eq!(content, read_content);

        Ok(())
    }

    #[tokio::test]
    async fn test_encrypted_file_different_chunk_size() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("small_chunks.txt.enc");
        let key = [99u8; 32];
        let plain_chunk_size = 1024; // 1 KB chunks
        let content = vec![0xCD; 5000]; // ~5 chunks

        let mut file = DavFile::create_with_chunk_size(&path, Some(key), plain_chunk_size).await?;
        file.write_all(&content).await?;
        file.shutdown().await?;

        // Verify file size on disk to ensure it's using small chunks
        let meta = fs::metadata(&path).await?;
        let cipher_type = CipherType::XChaCha20Poly1305;
        let expected_size = (4 + 1 + cipher_type.nonce_size()) as u64
            + (content.len() as u64 / plain_chunk_size as u64) * (plain_chunk_size + cipher_type.overhead()) as u64
            + (content.len() as u64 % plain_chunk_size as u64) + cipher_type.overhead() as u64;
        assert_eq!(meta.len(), expected_size);

        let mut file = DavFile::open(&path, Some(key)).await?;
        let mut read_content = Vec::new();
        file.read_to_end(&mut read_content).await?;

        assert_eq!(content.len(), read_content.len());
        assert_eq!(content, read_content);

        Ok(())
    }
}
