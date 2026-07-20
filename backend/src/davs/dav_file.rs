use super::crypto::{Cipher, CipherType};
use super::errors::DavFileError;
use futures::ready;
use headers::{ETag, LastModified};
use rand::{TryRng, rngs::SysRng};
use std::fmt;
use std::io::{self, SeekFrom};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::{path::Path, time::SystemTime};
use tokio::fs::{self, File};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeek, AsyncWrite, AsyncWriteExt, ReadBuf};

const BUFFER_ERROR: &str = "buffer error for encryption or decryption";

pub enum DavFile {
    Plain(File),
    Encrypted(EncryptedFile),
}

impl fmt::Debug for DavFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DavFile::Plain(_) => write!(f, "DavFile::Plain"),
            DavFile::Encrypted(_) => write!(f, "DavFile::Encrypted"),
        }
    }
}

pub struct EncryptedFile {
    file: Box<File>,
    read_buffer: Vec<u8>,
    encrypted_read_buffer: Vec<u8>,
    write_buffer: Vec<u8>,
    pos: u64,
    decrypted_len: u64,
    offset_in_chunk: u32,
    read_chunk_idx: u32,
    write_chunk_idx: u32,
    seeked_after_open: bool,
    cipher: Box<dyn Cipher>,
}

impl fmt::Debug for EncryptedFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EncryptedFile")
    }
}

impl EncryptedFile {
    fn new(file: File, cipher: Box<dyn Cipher>, decrypted_len: u64, write_chunk_idx: u32) -> Self {
        let plain_chunk_size = cipher.plain_chunk_size();
        let encrypted_chunk_size = plain_chunk_size + cipher.cipher_type().overhead();
        Self {
            file: Box::new(file),
            cipher,
            decrypted_len,
            write_chunk_idx,
            read_buffer: Vec::with_capacity(plain_chunk_size),
            encrypted_read_buffer: Vec::with_capacity(encrypted_chunk_size),
            write_buffer: Vec::with_capacity(plain_chunk_size),
            pos: 0,
            offset_in_chunk: 0,
            read_chunk_idx: 0,
            seeked_after_open: false,
        }
    }
}

impl DavFile {
    pub async fn create(path: &Path, key: Option<[u8; 32]>) -> io::Result<DavFile> {
        Self::create_with_cipher_type(path, key, CipherType::XChaCha20Poly1305_1M).await
    }

    pub async fn create_with_cipher_type(
        path: &Path,
        key: Option<[u8; 32]>,
        cipher_type: CipherType,
    ) -> io::Result<DavFile> {
        let mut file = fs::File::create(&path).await?;

        match key {
            Some(key) => {
                let nonce_size = cipher_type.nonce_size();
                let mut nonce = vec![0u8; nonce_size];
                TryRng::try_fill_bytes(&mut SysRng, &mut nonce)
                    .map_err(|e| io::Error::from(DavFileError::NonceGeneration(Box::new(e))))?;

                // Header: cipher_type (u8), nonce
                file.write_all(&[cipher_type as u8]).await?;
                file.write_all(&nonce).await?;
                file.flush().await?;

                let cipher = cipher_type.create_cipher(&key, &nonce)?;

                Ok(DavFile::Encrypted(EncryptedFile::new(file, cipher, 0, 0)))
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
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "encrypted file is empty and missing header",
                    ));
                }

                let mut cipher_type_byte = [0u8; 1];
                file.read_exact(&mut cipher_type_byte).await?;
                let cipher_type =
                    CipherType::from_u8(cipher_type_byte[0]).map_err(io::Error::other)?;

                let nonce_size = cipher_type.nonce_size();
                let mut nonce = vec![0u8; nonce_size];
                file.read_exact(&mut nonce).await?;

                let encrypted_chunk_size = cipher_type.encrypted_chunk_size();
                let header_size = cipher_type.header_size();

                let enc_size_without_header = metadata.len().saturating_sub(header_size as u64);
                let write_chunk_idx_initial =
                    (enc_size_without_header / encrypted_chunk_size as u64) as u32;

                let cipher = cipher_type.create_cipher(&key, &nonce)?;

                Ok(DavFile::Encrypted(EncryptedFile::new(
                    file,
                    cipher,
                    cipher_type.decrypted_size(metadata.len())?,
                    write_chunk_idx_initial,
                )))
            }
            None => Ok(DavFile::Plain(file)),
        }
    }

    pub async fn len(&self) -> u64 {
        match self {
            DavFile::Plain(file) => file.metadata().await.map_or(0, |m| m.len()),
            DavFile::Encrypted(f) => f.decrypted_len,
        }
    }

    pub async fn is_empty(&self) -> bool {
        self.len().await == 0
    }

    pub async fn cache_headers(&self) -> Option<(ETag, LastModified)> {
        let (metadata, size) = match self {
            DavFile::Plain(file) => {
                let m = file.metadata().await.ok()?;
                let s = m.len();
                (m, s)
            }
            DavFile::Encrypted(f) => {
                let m = f.file.metadata().await.ok()?;
                let s = f.decrypted_len;
                (m, s)
            }
        };
        let mtime = metadata.modified().ok()?;
        let timestamp = mtime
            .duration_since(SystemTime::UNIX_EPOCH)
            .ok()?
            .as_millis() as u64;
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
            DavFile::Encrypted(f) => {
                let plain_chunk_size = f.cipher.plain_chunk_size();
                let cipher_type = f.cipher.cipher_type();
                let encrypted_chunk_size = plain_chunk_size + cipher_type.overhead();
                // first, return any leftover plaintext
                if !f.read_buffer.is_empty() {
                    let len = std::cmp::min(buf.remaining(), f.read_buffer.len());
                    buf.put_slice(
                        f.read_buffer
                            .get(..len)
                            .ok_or(io::Error::other(BUFFER_ERROR))?,
                    );
                    f.read_buffer.drain(..len);
                    f.pos += len as u64;
                    return Poll::Ready(Ok(()));
                }

                // fill encrypted_read_buffer to at least one chunk
                while f.encrypted_read_buffer.len() < encrypted_chunk_size {
                    let start = f.encrypted_read_buffer.len();
                    f.encrypted_read_buffer.resize(encrypted_chunk_size, 0);
                    let target = f
                        .encrypted_read_buffer
                        .get_mut(start..)
                        .ok_or(io::Error::other(BUFFER_ERROR))?;
                    let mut read_buf = ReadBuf::new(target);
                    match Pin::new(&mut *f.file).poll_read(cx, &mut read_buf) {
                        Poll::Ready(Ok(())) => {
                            let n = read_buf.filled().len();
                            f.encrypted_read_buffer.truncate(start + n);
                            if n == 0 {
                                break; // EOF
                            }
                        }
                        Poll::Ready(Err(e)) => {
                            f.encrypted_read_buffer.truncate(start);
                            return Poll::Ready(Err(DavFileError::FileRead(e).into()));
                        }
                        Poll::Pending => {
                            f.encrypted_read_buffer.truncate(start);
                            return Poll::Pending;
                        }
                    }
                }

                if f.encrypted_read_buffer.is_empty() {
                    return Poll::Ready(Ok(()));
                }

                let is_last = f.encrypted_read_buffer.len() < encrypted_chunk_size;

                let mut plaintext = f
                    .cipher
                    .decrypt(
                        f.read_chunk_idx,
                        is_last,
                        f.encrypted_read_buffer.as_slice(),
                    )
                    .map_err(|_e| io::Error::from(DavFileError::AuthFailed))?;

                f.encrypted_read_buffer.clear();
                f.read_chunk_idx += 1;

                // apply offset_in_chunk if needed
                if f.offset_in_chunk > 0 {
                    let offset = f.offset_in_chunk as usize;
                    if offset < plaintext.len() {
                        plaintext.drain(..offset);
                    } else {
                        plaintext.clear();
                    }
                    f.offset_in_chunk = 0;
                }

                // fill the user buffer and keep remainder in read_buffer
                let len = std::cmp::min(buf.remaining(), plaintext.len());
                buf.put_slice(plaintext.get(..len).ok_or(io::Error::other(BUFFER_ERROR))?);
                if len < plaintext.len() {
                    f.read_buffer.extend_from_slice(
                        plaintext.get(len..).ok_or(io::Error::other(BUFFER_ERROR))?,
                    );
                }
                f.pos += len as u64;

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
            DavFile::Encrypted(f) => {
                if f.seeked_after_open {
                    return Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::Unsupported,
                        "writing after seek is not supported for encrypted files",
                    )));
                }

                let plain_chunk_size = f.cipher.plain_chunk_size();
                let max_buffer_size = plain_chunk_size * 2;

                // Try to flush existing chunks first if needed
                if f.write_buffer.len() >= plain_chunk_size
                    && poll_write_chunks(
                        cx,
                        &mut f.file,
                        &mut f.write_buffer,
                        &mut f.write_chunk_idx,
                        &mut f.cipher,
                        false,
                    )?
                    .is_pending()
                    && f.write_buffer.len() >= max_buffer_size
                {
                    return Poll::Pending;
                }

                let to_write = std::cmp::min(buf.len(), max_buffer_size - f.write_buffer.len());
                if to_write == 0 && !buf.is_empty() {
                    return Poll::Pending;
                }

                f.write_buffer
                    .extend_from_slice(buf.get(..to_write).ok_or(io::Error::other(BUFFER_ERROR))?);

                // Try to flush again after adding new data
                let _ = poll_write_chunks(
                    cx,
                    &mut f.file,
                    &mut f.write_buffer,
                    &mut f.write_chunk_idx,
                    &mut f.cipher,
                    false,
                )?;

                f.decrypted_len += to_write as u64;
                Poll::Ready(Ok(to_write))
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            DavFile::Plain(file) => Pin::new(file).poll_flush(cx),
            DavFile::Encrypted(f) => {
                if f.seeked_after_open {
                    return Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::Unsupported,
                        "writing after seek is not supported for encrypted files",
                    )));
                }
                ready!(poll_write_chunks(
                    cx,
                    &mut f.file,
                    &mut f.write_buffer,
                    &mut f.write_chunk_idx,
                    &mut f.cipher,
                    false
                ))?;
                Pin::new(&mut *f.file).poll_flush(cx)
            }
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        ready!(self.as_mut().poll_flush(cx))?;
        let me = self.get_mut();
        match me {
            DavFile::Plain(file) => Pin::new(file).poll_shutdown(cx),
            DavFile::Encrypted(f) => {
                if f.seeked_after_open {
                    return Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::Unsupported,
                        "writing after seek is not supported for encrypted files",
                    )));
                }
                ready!(poll_write_chunks(
                    cx,
                    &mut f.file,
                    &mut f.write_buffer,
                    &mut f.write_chunk_idx,
                    &mut f.cipher,
                    true
                ))?;
                Pin::new(&mut *f.file).poll_shutdown(cx)
            }
        }
    }
}

impl AsyncSeek for DavFile {
    fn start_seek(self: Pin<&mut Self>, position: SeekFrom) -> io::Result<()> {
        match self.get_mut() {
            DavFile::Plain(file) => Pin::new(file).start_seek(position),
            DavFile::Encrypted(f) => {
                f.seeked_after_open = true;
                let new_pos = match position {
                    SeekFrom::Start(p) => p as i64,
                    SeekFrom::End(p) => f.decrypted_len as i64 + p,
                    SeekFrom::Current(p) => f.pos as i64 + p,
                };

                if new_pos < 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "invalid seek to a negative position",
                    ));
                }

                f.pos = std::cmp::min(new_pos as u64, f.decrypted_len);
                f.read_buffer.clear();
                f.encrypted_read_buffer.clear();

                let plain_chunk_size = f.cipher.plain_chunk_size();
                let cipher_type = f.cipher.cipher_type();
                let encrypted_pos = cipher_type.encrypted_chunk_start(f.pos);
                f.offset_in_chunk = (f.pos % plain_chunk_size as u64) as u32;
                f.read_chunk_idx = (f.pos / plain_chunk_size as u64) as u32;
                Pin::new(&mut *f.file).start_seek(SeekFrom::Start(encrypted_pos))
            }
        }
    }

    fn poll_complete(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<u64>> {
        match self.get_mut() {
            DavFile::Plain(file) => Pin::new(file).poll_complete(cx),
            DavFile::Encrypted(f) => {
                ready!(Pin::new(&mut *f.file).poll_complete(cx))?;
                Poll::Ready(Ok(f.pos))
            }
        }
    }
}

pub async fn decrypted_size_from_file(path: &Path, enc_size: u64) -> u64 {
    if enc_size == 0 {
        return 0;
    }
    match fs::File::open(path).await {
        Ok(mut file) => {
            let mut cipher_type_byte = [0u8; 1];
            if file.read_exact(&mut cipher_type_byte).await.is_ok()
                && let Ok(cipher_type) = CipherType::from_u8(cipher_type_byte[0])
            {
                return cipher_type.decrypted_size(enc_size).unwrap_or(0);
            }
            0
        }
        Err(_) => 0,
    }
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
            .map_err(|e| io::Error::from(DavFileError::Encryption(e)))?;

        // write the ciphertext fully to disk
        let mut written = 0;
        while written < ciphertext.len() {
            let bytes_written = match ready!(
                Pin::new(&mut *file).poll_write(
                    cx,
                    ciphertext
                        .get(written..)
                        .ok_or(io::Error::other(BUFFER_ERROR))?
                )
            ) {
                Ok(n) => n,
                Err(e) => return Poll::Ready(Err(DavFileError::FileWrite(e).into())),
            };
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
    async fn test_encrypted_file_critically_truncated() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("crit_trunc.txt.enc");
        let key = [7u8; 32];
        let content = b"this will be critically truncated";

        {
            let mut file = DavFile::create(&path, Some(key)).await?;
            file.write_all(content).await?;
            file.shutdown().await?;
        }

        let stdf = std::fs::OpenOptions::new().write(true).open(&path)?;
        // header is 20. overhead is 16. total min 36 for 0 bytes plain.
        // Let's make it 25 bytes total (5 bytes of "ciphertext" which is less than 16 overhead)
        stdf.set_len(25)?;
        stdf.sync_all()?;

        let res = DavFile::open(&path, Some(key)).await;
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Other);
        assert!(
            err.to_string()
                .contains("detected truncated or corrupted chunk")
        );
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
        let content = vec![0xAB; 1_000_000 * 3 + 123];

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
        assert!(err.to_string().contains("error decrypting ciphertext"));
        Ok(())
    }

    #[tokio::test]
    async fn test_encrypted_file_seek_across_chunk_boundary() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("large.txt.enc");
        let key = [42u8; 32];
        let plain_chunk_size = 1_000_000;
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
        let content = vec![0xAB; 1_000_000 * 2 + 555];

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
    async fn test_encrypted_file_write_after_seek_fails() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("seek_write.txt.enc");
        let key = [42u8; 32];

        let mut file = DavFile::create(&path, Some(key)).await?;
        file.write_all(b"initial data").await?;

        // Seek should set seeked_after_open to true
        file.seek(SeekFrom::Start(0)).await?;

        // Attempt to write after seek should fail
        let res = file.write_all(b"more data").await;
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Unsupported);
        assert_eq!(
            err.to_string(),
            "writing after seek is not supported for encrypted files"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_encrypted_file_header_corruption_fails() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("corrupt.txt.enc");
        let key = [42u8; 32];

        // 1. Invalid cipher type
        {
            let mut file = DavFile::create(&path, Some(key)).await?;
            file.write_all(b"data").await?;
            file.shutdown().await?;

            // Corrupt the first byte (cipher type)
            let mut f = std::fs::OpenOptions::new().write(true).open(&path)?;
            use std::io::Write;
            f.write_all(&[255u8])?; // Invalid cipher type
            f.sync_all()?;
            drop(f);

            let res = DavFile::open(&path, Some(key)).await;
            assert!(res.is_err());
        }

        // 2. Truncated header (only cipher type byte, missing nonce)
        {
            // Reset file
            let mut file = DavFile::create(&path, Some(key)).await?;
            file.write_all(b"data").await?;
            file.shutdown().await?;

            let f = std::fs::OpenOptions::new().write(true).open(&path)?;
            f.set_len(1)?; // Only cipher type byte
            f.sync_all()?;
            drop(f);

            let res = DavFile::open(&path, Some(key)).await;
            assert!(res.is_err());
            if let Err(e) = res {
                assert_eq!(e.kind(), io::ErrorKind::UnexpectedEof);
            }
        }
        Ok(())
    }
}
