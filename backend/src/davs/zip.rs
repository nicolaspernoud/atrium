use super::dav_file::DavFile;
use super::webdav_server::BoxResult;
use async_walkdir::WalkDir;
use async_zip::{
    Compression, ZipEntryBuilder,
    tokio::{read::seek::ZipFileReader, write::ZipFileWriter},
};
use futures_util::StreamExt;
use std::path::{Component, Path, PathBuf};
use tokio::{
    fs,
    io::{AsyncWrite, AsyncWriteExt},
};
use tokio_util::compat::{FuturesAsyncReadCompatExt, FuturesAsyncWriteCompatExt};
use tokio_util::sync::CancellationToken;
use tracing::{error, warn};

/// Strips unsafe components from a zip entry path.
///
/// Backslashes are normalized to `/`. Returns `None` when the path is empty,
/// absolute, or contains `..` components (zip-slip protection).
pub fn sanitize_zip_path(raw: &str) -> Option<PathBuf> {
    let normalized = raw.replace('\\', "/");
    let path = Path::new(&normalized);
    let mut clean_path = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(c) => clean_path.push(c),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    if clean_path.as_os_str().is_empty() {
        None
    } else {
        Some(clean_path)
    }
}

/// Streams the regular files of `dir` into a zip archive written to `writer`,
/// with paths relative to `dir`.
pub async fn zip_dir<W: AsyncWrite + Unpin>(
    writer: W,
    dir: &Path,
    key: Option<[u8; 32]>,
    token: CancellationToken,
) -> BoxResult<()> {
    let mut zip_writer = ZipFileWriter::with_tokio(writer);
    let mut walkdir = WalkDir::new(dir);
    while let Some(entry) = walkdir.next().await {
        if token.is_cancelled() {
            return Ok(());
        }
        if let Ok(entry) = entry {
            let entry_path = entry.path();
            let meta = match fs::symlink_metadata(entry.path()).await {
                Ok(meta) => meta,
                Err(_) => continue,
            };
            if !meta.is_file() {
                continue;
            }
            let filename = match entry_path.strip_prefix(dir).ok().and_then(|v| v.to_str()) {
                Some(v) => v,
                None => continue,
            };

            let mut file = DavFile::open(&entry_path, key).await?;

            let builder = ZipEntryBuilder::new(filename.into(), Compression::Deflate);
            let entry_writer = zip_writer.write_entry_stream(builder).await?;
            let mut entry_writer_compat = entry_writer.compat_write();

            tokio::select! {
                _ = token.cancelled() => return Ok(()),
                res = tokio::io::copy(&mut file, &mut entry_writer_compat) => {
                    res?;
                }
            }

            entry_writer_compat.into_inner().close().await?;
        }
    }
    zip_writer.close().await?;
    Ok(())
}

/// Error returned by [`extract`].
pub enum ExtractError {
    /// The file is not a valid (or readable) zip archive.
    BadArchive(String),
    /// The process lacks the permission to write the extraction target.
    PermissionDenied,
    /// A filesystem error occurred during extraction.
    Io(String),
}

fn io_error(e: std::io::Error) -> ExtractError {
    if e.kind() == std::io::ErrorKind::PermissionDenied {
        ExtractError::PermissionDenied
    } else {
        ExtractError::Io(e.to_string())
    }
}

/// Extracts the zip archive at `zip_path` into `target_dir`, which must
/// already exist.
///
/// Files are written with [`DavFile::create`], so archives in encrypted davs
/// are transparently encrypted. Entries with unsafe paths (absolute, `..`)
/// or invalid UTF-8 names are skipped with a warning. Symlink entries are
/// materialized as regular files containing their target's content.
pub async fn extract(
    zip_path: &Path,
    target_dir: &Path,
    key: Option<[u8; 32]>,
) -> Result<(), ExtractError> {
    let dav_file = match DavFile::open(zip_path, key).await {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
            return Err(ExtractError::BadArchive(e.to_string()));
        }
        Err(e) => return Err(io_error(e)),
    };

    let mut zip_reader = ZipFileReader::with_tokio(tokio::io::BufReader::new(dav_file))
        .await
        .map_err(|e| ExtractError::BadArchive(e.to_string()))?;

    let entries_count = zip_reader.file().entries().len();
    for index in 0..entries_count {
        let Some(entry) = zip_reader.file().entries().get(index) else {
            continue;
        };
        let raw_name = match entry.filename().as_str() {
            Ok(s) => s.to_string(),
            Err(e) => {
                warn!("Skipping zip entry with invalid UTF-8 filename: {e}");
                continue;
            }
        };

        let Some(rel_path) = sanitize_zip_path(&raw_name) else {
            warn!("Skipping unsafe zip entry path: {raw_name}");
            continue;
        };

        let dest_path = target_dir.join(&rel_path);
        if !dest_path.starts_with(target_dir) {
            warn!("Skipping zip entry escaping target dir: {raw_name}");
            continue;
        }

        if entry.dir().unwrap_or(false) || raw_name.ends_with('/') {
            fs::create_dir_all(&dest_path).await.map_err(io_error)?;
            continue;
        }

        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent).await.map_err(io_error)?;
        }

        let mut out_file = DavFile::create(&dest_path, key).await.map_err(io_error)?;

        let mut entry_reader = zip_reader
            .reader_without_entry(index)
            .await
            .map_err(|e| ExtractError::Io(e.to_string()))?
            .compat();

        if let Err(e) = tokio::io::copy(&mut entry_reader, &mut out_file).await {
            error!("Failed to extract zip entry {raw_name}: {e}");
            return Err(ExtractError::Io(e.to_string()));
        }
        out_file.shutdown().await.map_err(io_error)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_zip_path() {
        assert_eq!(
            sanitize_zip_path("foo/bar.txt"),
            Some(PathBuf::from("foo/bar.txt"))
        );
        assert_eq!(
            sanitize_zip_path("foo\\bar\\baz.txt"),
            Some(PathBuf::from("foo/bar/baz.txt"))
        );
        assert_eq!(sanitize_zip_path("/etc/passwd"), None);
        assert_eq!(sanitize_zip_path("../evil.txt"), None);
        assert_eq!(sanitize_zip_path("foo/../../evil.txt"), None);
        assert_eq!(sanitize_zip_path(""), None);
        assert_eq!(sanitize_zip_path("."), None);
    }
}
