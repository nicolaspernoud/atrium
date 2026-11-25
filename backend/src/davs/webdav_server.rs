// Some of this code is loosely inspired from https://github.com/sigoden/dufs
/*
The MIT License (MIT)

Copyright (c) sigoden(2022)

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
*/
use super::{dav_file::decrypted_size_from_file, headers::Depth, model::Dav, zip};
use crate::{
    davs::{dav_file::DavFile, headers::Overwrite},
    utils::extract_query_pairs,
};
use async_walkdir::WalkDir;
use axum::body::Body;
use chrono::{DateTime, Duration, TimeZone, Utc};
use dashmap::{DashMap, Entry};
use futures_util::{FutureExt, StreamExt, future::BoxFuture};
use headers::{
    AcceptRanges, ContentType, HeaderMap, HeaderMapExt, IfModifiedSince, IfNoneMatch, IfRange,
    Range,
};
use http_body_util::BodyExt;
use hyper::{
    Method, StatusCode, Uri,
    header::{
        CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, HeaderValue, RANGE,
    },
};
use quick_xml::{
    Reader,
    escape::escape,
    events::{BytesText, Event},
};
use serde::Serialize;
use std::{
    borrow::Cow,
    io::{Error, SeekFrom},
    marker::PhantomData,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};
use tokio::io::AsyncWriteExt;
use tokio::{
    fs,
    io::{AsyncReadExt, AsyncSeekExt},
};
use tokio_util::io::{ReaderStream, StreamReader};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error};
use uuid::Uuid;

pub type Request = hyper::Request<Body>;
pub type Response = hyper::Response<Body>;

pub type BoxResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
static APPLICATION_JSON: HeaderValue = HeaderValue::from_static("application/json");
static ACCEPTED: HeaderValue = HeaderValue::from_static("accepted");

const LOCK_TIMEOUT: i64 = 24 * 60 * 60; // 24 hours in seconds
const BUF_SIZE: usize = 65536;
const MAX_PROP_FIND_ENTRIES: usize = 10000;

#[derive(Debug)]
pub struct LockInfo {
    token: String,
    expires_at: DateTime<Utc>,
}

pub type LockMap = Arc<DashMap<PathBuf, LockInfo>>;

pub enum RealPathError {
    Forbidden,
    NotFound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PathKind {
    Dir,
    File,
    Other,
    Miss,
}

/// A method's requirement on the dav and on the resource, evaluated by
/// [`gate`].
///
/// Implemented by zero-sized marker types, one per distinct requirement:
/// the dav-side permission check (403) and the resource-state check (404)
/// live together, so each method gates with a single type. Today every
/// permission relies on the same `writable` flag; when the dav model splits
/// it, only these impls change.
pub trait Gate {
    /// Whether the dav grants the permission the method needs. Default:
    /// the method is allowed on any dav.
    fn permission_granted(_dav: &Dav, _real_path: &RealPath) -> bool {
        true
    }
    /// Whether the resource is in the state the method requires.
    fn state_holds(real_path: &RealPath) -> bool;
}

/// PUT and MKCOL: the dav must accept uploads; overwriting a non-empty file
/// additionally requires the delete permission. No state requirement.
pub struct Upload;
impl Gate for Upload {
    fn permission_granted(dav: &Dav, real_path: &RealPath) -> bool {
        // Both permissions currently derive from the same `writable` flag.
        let can_upload = dav.writable;
        let can_delete = dav.writable;
        if real_path.is_file() && real_path.size() > 0 {
            // Overwriting a non-empty file also requires the delete
            // permission.
            can_upload && can_delete
        } else {
            can_upload
        }
    }
    fn state_holds(_real_path: &RealPath) -> bool {
        true
    }
}

/// DELETE: the dav must accept deletions, and the resource must exist.
pub struct Delete;
impl Gate for Delete {
    fn permission_granted(dav: &Dav, _real_path: &RealPath) -> bool {
        dav.writable
    }
    fn state_holds(real_path: &RealPath) -> bool {
        !real_path.is_miss()
    }
}

/// POST `?extract`: the dav must accept uploads, and the resource must be an
/// existing regular file.
pub struct Extract;
impl Gate for Extract {
    fn permission_granted(dav: &Dav, _real_path: &RealPath) -> bool {
        dav.writable
    }
    fn state_holds(real_path: &RealPath) -> bool {
        real_path.is_file()
    }
}

/// COPY: the dav must accept uploads, and the resource must exist.
pub struct Copy;
impl Gate for Copy {
    fn permission_granted(dav: &Dav, _real_path: &RealPath) -> bool {
        dav.writable
    }
    fn state_holds(real_path: &RealPath) -> bool {
        !real_path.is_miss()
    }
}

/// MOVE: the dav must accept both uploads and deletions, and the resource
/// must exist.
pub struct Move;
impl Gate for Move {
    fn permission_granted(dav: &Dav, _real_path: &RealPath) -> bool {
        dav.writable
    }
    fn state_holds(real_path: &RealPath) -> bool {
        !real_path.is_miss()
    }
}

/// GET, HEAD, PROPFIND: the resource must be an existing directory or file.
pub struct Read;
impl Gate for Read {
    fn state_holds(real_path: &RealPath) -> bool {
        real_path.is_dir() || real_path.is_file()
    }
}

/// PROPPATCH: the resource must be an existing file. Changing the mtime is
/// allowed on any dav, regardless of writability (existing behavior).
pub struct Proppatch;
impl Gate for Proppatch {
    fn state_holds(real_path: &RealPath) -> bool {
        real_path.is_file()
    }
}

/// UNLOCK: the resource must exist.
pub struct Unlock;
impl Gate for Unlock {
    fn state_holds(real_path: &RealPath) -> bool {
        !real_path.is_miss()
    }
}

#[derive(Debug, Clone)]
pub struct RealPath {
    path: PathBuf,
    kind: PathKind,
    size: u64,
    base_path: PathBuf,
    allow_symlinks: bool,
}

impl RealPath {
    pub async fn new(
        dav_path: &str,
        url_path: &str,
        allow_symlinks: bool,
    ) -> Result<Self, RealPathError> {
        let path = Self::extract_path(url_path, dav_path).ok_or(RealPathError::Forbidden)?;
        Self::new_from_path(path, dav_path.into(), allow_symlinks).await
    }

    pub async fn new_from_path(
        path: PathBuf,
        base_path: PathBuf,
        allow_symlinks: bool,
    ) -> Result<Self, RealPathError> {
        let (kind, size) = match fs::metadata(&path).await.ok() {
            Some(meta) => {
                let kind = if meta.is_dir() {
                    PathKind::Dir
                } else if meta.is_file() {
                    PathKind::File
                } else {
                    PathKind::Other
                };
                (kind, meta.len())
            }
            None => (PathKind::Miss, 0),
        };

        if !allow_symlinks && kind != PathKind::Miss && !is_root_contained(&path, &base_path).await
        {
            return Err(RealPathError::NotFound);
        }

        Ok(Self {
            path,
            kind,
            size,
            base_path,
            allow_symlinks,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn is_miss(&self) -> bool {
        self.kind == PathKind::Miss
    }

    pub fn is_dir(&self) -> bool {
        self.kind == PathKind::Dir
    }

    pub fn is_file(&self) -> bool {
        self.kind == PathKind::File
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn base_path(&self) -> &Path {
        &self.base_path
    }

    pub fn allow_symlinks(&self) -> bool {
        self.allow_symlinks
    }

    pub fn extract_path(wanted_path: &str, dav_path: &str) -> Option<PathBuf> {
        let mut decoded_path = decode_uri(wanted_path.get(1..)?)?.into_owned();
        // Prevent double-encoding attacks: decode until stable
        loop {
            let next = decode_uri(&decoded_path).map(|s| s.into_owned());
            match next {
                Some(n) if n != decoded_path => decoded_path = n,
                _ => break,
            }
        }
        let stripped_path = Path::new(&decoded_path).components().collect::<PathBuf>();
        // Reject any '..' components explicitly
        if stripped_path
            .components()
            .any(|c| c == std::path::Component::ParentDir || c == std::path::Component::RootDir)
        {
            return None;
        }
        Some(Path::new(dav_path).join(stripped_path))
    }
}

async fn is_root_contained(path: &Path, directory: &Path) -> bool {
    let (path_canon, dir_canon) = tokio::join!(fs::canonicalize(path), fs::canonicalize(directory));
    let dir = match dir_canon {
        Ok(dir) => dir,
        Err(_) => return false,
    };
    path_canon
        .ok()
        .map(|v| v.starts_with(dir))
        .unwrap_or_default()
}

pub struct WebdavServer {
    locks: LockMap,
}

impl WebdavServer {
    pub fn new() -> Self {
        Self {
            locks: Arc::new(DashMap::new()),
        }
    }

    pub async fn call(&self, req: Request, addr: SocketAddr, dav: &Dav) -> Response {
        let method = req.method().clone();
        let uri = req.uri().clone();

        match self.handle(req, dav).await {
            Ok(res) => {
                debug!(r#"{} "{} {}" - {}"#, addr.ip(), method, uri, res.status());
                res
            }
            Err(err) => {
                let mut res = Response::default();
                let status = StatusCode::INTERNAL_SERVER_ERROR;
                *res.status_mut() = status;
                error!(
                    r#"{} "{} {}" - {} {}"#,
                    addr.ip(),
                    method,
                    uri,
                    res.status(),
                    err
                );
                res
            }
        }
    }

    pub async fn handle(&self, req: Request, dav: &Dav) -> BoxResult<Response> {
        let mut res = Response::default();
        let head_only = req.method() == Method::HEAD;

        let real_path =
            match RealPath::new(&dav.directory, req.uri().path(), dav.allow_symlinks).await {
                Ok(v) => v,
                Err(RealPathError::Forbidden) => {
                    status_forbid(&mut res);
                    return Ok(res);
                }
                Err(RealPathError::NotFound) => {
                    status_not_found(&mut res);
                    return Ok(res);
                }
            };

        let query = extract_query_pairs(req.uri().query().unwrap_or_default());

        let allow_search = true;

        // Perform WebDAV lock checking before dispatching
        let lock_check_result = match req.method() {
            &Method::PUT | &Method::DELETE | &Method::POST => {
                self.check_lock(real_path.path(), req.headers())
            }
            m => match m.as_str() {
                "PROPPATCH" | "MKCOL" => self.check_lock(real_path.path(), req.headers()),
                "COPY" => {
                    if let Some(dest) = self.extract_dest(req.headers(), &dav.directory).await {
                        self.check_lock(dest.path(), req.headers())
                    } else {
                        Ok(())
                    }
                }
                "MOVE" => {
                    if let Err(status) = self.check_lock(real_path.path(), req.headers()) {
                        Err(status)
                    } else if let Some(dest) =
                        self.extract_dest(req.headers(), &dav.directory).await
                    {
                        self.check_lock(dest.path(), req.headers())
                    } else {
                        Ok(())
                    }
                }
                _ => Ok(()),
            },
        };

        if let Err(status) = lock_check_result {
            *res.status_mut() = status;
            *res.body_mut() = Body::from("Resource is locked");
            return Ok(res);
        }

        match req.method() {
            &Method::GET | &Method::HEAD => {
                let Some(ctx) = gate::<Read>(&real_path, dav, &mut res) else {
                    return Ok(res);
                };
                if ctx.real_path.is_dir() {
                    if let Some(search_str) = query.get("q") {
                        if allow_search {
                            let q = decode_uri(search_str).unwrap_or_default();
                            self.handle_query_dir(&ctx, &q, &mut res).await?;
                        }
                    } else if query.contains_key("diskusage") {
                        self.handle_disk_usage(&ctx, &mut res).await?;
                    } else {
                        Self::handle_zip_dir(&ctx, head_only, &mut res)?;
                    }
                } else {
                    let inline = query.contains_key("inline");
                    self.handle_send_file(&ctx, req.headers(), head_only, &mut res, inline)
                        .await?;
                }
            }
            &Method::OPTIONS => {
                set_webdav_headers(&mut res);
            }
            &Method::PUT => {
                let Some(ctx) = gate::<Upload>(&real_path, dav, &mut res) else {
                    return Ok(res);
                };
                self.handle_upload(&ctx, req, &mut res).await?;
            }
            &Method::DELETE => {
                let Some(ctx) = gate::<Delete>(&real_path, dav, &mut res) else {
                    return Ok(res);
                };
                self.handle_delete(&ctx, &mut res).await?;
            }
            &Method::POST => {
                if query.contains_key("extract") {
                    let Some(ctx) = gate::<Extract>(&real_path, dav, &mut res) else {
                        return Ok(res);
                    };
                    self.handle_extract_zip(&ctx, req.headers(), &mut res)
                        .await?;
                } else {
                    status_method_not_allowed(&mut res);
                }
            }
            method => match method.as_str() {
                "PROPFIND" => {
                    let Some(ctx) = gate::<Read>(&real_path, dav, &mut res) else {
                        return Ok(res);
                    };
                    if ctx.real_path.is_dir() {
                        self.handle_propfind_dir(&ctx, req.headers(), &mut res)
                            .await?;
                    } else {
                        self.handle_propfind_file(&ctx, &mut res).await?;
                    }
                }
                "PROPPATCH" => {
                    let Some(ctx) = gate::<Proppatch>(&real_path, dav, &mut res) else {
                        return Ok(res);
                    };
                    self.handle_proppatch(req, &ctx, &mut res).await?;
                }
                "MKCOL" => {
                    let Some(ctx) = gate::<Upload>(&real_path, dav, &mut res) else {
                        return Ok(res);
                    };
                    // An existing resource is a 405 for MKCOL, not a 404, so
                    // the state check stays out of the gate.
                    if !ctx.real_path.is_miss() {
                        status_method_not_allowed(&mut res);
                    } else if !req.into_body().collect().await?.to_bytes().is_empty() {
                        *res.status_mut() = StatusCode::UNSUPPORTED_MEDIA_TYPE;
                        *res.body_mut() = Body::from("Unsupported Media Type");
                    } else {
                        self.handle_mkcol(&ctx, &mut res).await?;
                    }
                }
                "COPY" => {
                    let Some(ctx) = gate::<Copy>(&real_path, dav, &mut res) else {
                        return Ok(res);
                    };
                    self.handle_copymove(&ctx, req, &mut res).await?;
                }
                "MOVE" => {
                    let Some(ctx) = gate::<Move>(&real_path, dav, &mut res) else {
                        return Ok(res);
                    };
                    self.handle_copymove(&ctx, req, &mut res).await?;
                }
                "LOCK" => {
                    self.handle_lock(&real_path, req, &mut res)?;
                }
                "UNLOCK" => {
                    let Some(ctx) = gate::<Unlock>(&real_path, dav, &mut res) else {
                        return Ok(res);
                    };
                    self.handle_unlock(&ctx, req, &mut res)?;
                }
                _ => {
                    status_method_not_allowed(&mut res);
                }
            },
        }
        Ok(res)
    }

    async fn handle_upload(
        &self,
        ctx: &Gated<'_, Upload>,
        req: Request,
        res: &mut Response,
    ) -> BoxResult<()> {
        let real_path = ctx.real_path;
        let key = ctx.dav.key;
        ensure_path_parent(real_path.path()).await?;

        let mut file = if let Ok(v) = DavFile::create(real_path.path(), key).await {
            v
        } else {
            {
                *res.status_mut() = StatusCode::CONFLICT;
                *res.body_mut() = Body::from("Cannot create file at given path");
            };
            return Ok(());
        };

        let (parts, body) = req.into_parts();
        let body_stream = body.into_data_stream();
        let body_with_io_error = body_stream.map(|result| result.map_err(std::io::Error::other));
        let mut body_reader = StreamReader::new(body_with_io_error);

        if let Err(e) = tokio::io::copy(&mut body_reader, &mut file).await {
            *res.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
            *res.body_mut() = Body::from("error writing file");
            error!(
                "WARNING: The file creation on {}{} encountered an error: {}. The file was not created or was deleted !",
                parts
                    .headers
                    .get(http::header::HOST)
                    .unwrap_or(&HeaderValue::from_static("<no host>"))
                    .to_str()
                    .unwrap_or_default(),
                parts.uri,
                e
            );
            if let Err(e) = fs::remove_file(real_path.path()).await {
                error!(
                    "Failed to remove partial file {:?}: {}",
                    real_path.path(),
                    e
                );
            }
            return Ok(());
        };

        file.shutdown().await?;

        if let Some(h) = parts.headers.get("X-OC-Mtime")
            && let Ok(h) = h.to_str()
            && let Ok(t) = h.parse::<i64>()
            && t >= 0 // Reject negative timestamps (before Unix epoch)
            && t <= Utc::now().timestamp() + 86400 // Reject timestamps more than 1 day in the future
            && std::fs::set_times(
                real_path.path(),
                std::fs::FileTimes::new().set_modified(
                    SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(t as u64),
                ),
            )
            .is_ok()
        {
            // Respond with the appropriate header on success
            res.headers_mut().insert("X-OC-Mtime", ACCEPTED.clone());
        };

        *res.status_mut() = StatusCode::CREATED;
        Ok(())
    }

    async fn handle_delete(&self, ctx: &Gated<'_, Delete>, res: &mut Response) -> BoxResult<()> {
        let real_path = ctx.real_path;
        let path = real_path.path();
        if real_path.is_dir() {
            fs::remove_dir_all(path).await?;
            self.locks.retain(|p, _| !p.starts_with(path));
        } else {
            fs::remove_file(path).await?;
            self.locks.remove(path);
        }

        status_no_content(res);
        Ok(())
    }

    async fn handle_query_dir(
        &self,
        ctx: &Gated<'_, Read>,
        query: &str,
        res: &mut Response,
    ) -> BoxResult<()> {
        let real_path = ctx.real_path;
        let key = ctx.dav.key;
        let mut paths: Vec<PathItem> = vec![];
        let mut walkdir = WalkDir::new(real_path.path());
        const MAX_ENTRIES: usize = 10000;
        while let Some(entry) = walkdir.next().await {
            if paths.len() >= MAX_ENTRIES {
                break;
            }
            if let Ok(entry) = entry {
                if !entry
                    .file_name()
                    .to_string_lossy()
                    .to_lowercase()
                    .contains(&query.to_lowercase())
                {
                    continue;
                }
                if let Ok(child_real_path) = RealPath::new_from_path(
                    entry.path(),
                    real_path.base_path().to_path_buf(),
                    real_path.allow_symlinks(),
                )
                .await
                    && let Ok(Some(item)) = self.to_pathitem(&child_real_path, &key).await
                {
                    paths.push(item);
                }
            }
        }
        let j = serde_json::to_string(&paths)?;
        res.headers_mut()
            .insert(CONTENT_TYPE, APPLICATION_JSON.clone());
        *res.body_mut() = Body::from(j);
        Ok(())
    }

    async fn handle_disk_usage(
        &self,
        ctx: &Gated<'_, Read>,
        res: &mut http::Response<Body>,
    ) -> BoxResult<()> {
        let full_path = fs::canonicalize(ctx.real_path.path()).await?;
        let du = crate::sysinfo::disk_info(full_path).await?;
        let j = serde_json::to_string(&du)?;
        res.headers_mut()
            .insert(CONTENT_TYPE, APPLICATION_JSON.clone());
        *res.body_mut() = Body::from(j);
        Ok(())
    }

    fn handle_zip_dir(ctx: &Gated<'_, Read>, head_only: bool, res: &mut Response) -> BoxResult<()> {
        let key = ctx.dav.key;
        let path = ctx.real_path.path();
        let (writer, reader) = tokio::io::duplex(BUF_SIZE);
        let filename = get_file_name(path)?;
        res.headers_mut().insert(
            CONTENT_DISPOSITION,
            HeaderValue::from_str(&format!(
                "attachment; filename=\"{}.zip\"",
                encode_uri(filename),
            ))?,
        );
        res.headers_mut()
            .insert(CONTENT_TYPE, HeaderValue::from_static("application/zip"));
        if head_only {
            return Ok(());
        }
        let token = CancellationToken::new();
        let path = path.to_owned();
        let token_clone = token.clone();
        tokio::spawn(async move {
            if let Err(e) = zip::zip_dir(writer, &path, key, token_clone).await {
                error!("Failed to zip {}, {}", path.display(), e);
            }
        });
        *res.body_mut() = Body::from_stream(CancelOnDrop::new(ReaderStream::new(reader), token));
        Ok(())
    }

    async fn handle_send_file(
        &self,
        ctx: &Gated<'_, Read>,
        headers: &HeaderMap<HeaderValue>,
        head_only: bool,
        res: &mut Response,
        inline_file: bool,
    ) -> BoxResult<()> {
        let path = ctx.real_path.path();
        let key = ctx.dav.key;
        let mut file = DavFile::open(path, key).await?;

        let mut use_range = true;
        if let Some((etag, last_modified)) = file.cache_headers().await {
            let cached = {
                if let Some(if_none_match) = headers.typed_get::<IfNoneMatch>() {
                    !if_none_match.precondition_passes(&etag)
                } else if let Some(if_modified_since) = headers.typed_get::<IfModifiedSince>() {
                    !if_modified_since.is_modified(last_modified.into())
                } else {
                    false
                }
            };
            if cached {
                *res.status_mut() = StatusCode::NOT_MODIFIED;
                return Ok(());
            }

            res.headers_mut().typed_insert(last_modified);
            res.headers_mut().typed_insert(etag.clone());

            if headers.typed_get::<Range>().is_some() {
                use_range = headers.typed_get::<IfRange>().is_none_or(|if_range| {
                    !if_range.is_modified(Some(&etag), Some(&last_modified))
                });
            } else {
                use_range = false;
            }
        }

        let range = if use_range {
            parse_range(headers)
        } else {
            None
        };

        if let Some(mime) = mime_guess::from_path(path).first() {
            res.headers_mut().typed_insert(ContentType::from(mime));
        } else {
            res.headers_mut().insert(
                CONTENT_TYPE,
                HeaderValue::from_static("application/octet-stream"),
            );
        }

        let filename = get_file_name(path)?;
        if inline_file {
            res.headers_mut()
                .insert(CONTENT_DISPOSITION, HeaderValue::from_str("inline")?);
        } else {
            res.headers_mut().insert(
                CONTENT_DISPOSITION,
                HeaderValue::from_str(&format!(
                    "attachment; filename=\"{}\"",
                    encode_uri(filename),
                ))?,
            );
        }

        res.headers_mut().typed_insert(AcceptRanges::bytes());

        let size = file.len().await;

        if let Some(range) = range {
            debug!("Requesting range: {:?}", range);
            let (start, end) = match range {
                RangeValue::Range(start, end) => (
                    start,
                    end.unwrap_or(size.saturating_sub(1))
                        .min(size.saturating_sub(1)),
                ),
                RangeValue::Suffix(suffix) => {
                    let suffix = suffix.min(size);
                    (size.saturating_sub(suffix), size.saturating_sub(1))
                }
            };

            if start > end || start >= size {
                *res.status_mut() = StatusCode::RANGE_NOT_SATISFIABLE;
                res.headers_mut()
                    .insert(CONTENT_RANGE, format!("bytes */{size}").parse()?);
                return Ok(());
            }

            let part_size = end - start + 1;
            *res.status_mut() = StatusCode::PARTIAL_CONTENT;
            let content_range = format!("bytes {}-{}/{}", start, end, size);
            res.headers_mut()
                .insert(CONTENT_RANGE, content_range.parse()?);
            res.headers_mut()
                .insert(CONTENT_LENGTH, format!("{part_size}").parse()?);
            if head_only {
                return Ok(());
            }

            file.seek(SeekFrom::Start(start)).await?;
            let reader = file.take(part_size);
            *res.body_mut() = Body::from_stream(ReaderStream::new(reader));
        } else {
            res.headers_mut()
                .insert(CONTENT_LENGTH, format!("{size}").parse()?);
            if head_only {
                return Ok(());
            }
            // Buffer the file to reduce syscalls and improve throughput
            let reader = tokio::io::BufReader::with_capacity(BUF_SIZE, file);
            *res.body_mut() = Body::from_stream(ReaderStream::new(reader));
        }
        Ok(())
    }

    async fn handle_propfind_dir(
        &self,
        ctx: &Gated<'_, Read>,
        headers: &HeaderMap<HeaderValue>,
        res: &mut Response,
    ) -> BoxResult<()> {
        let real_path = ctx.real_path;
        let key = ctx.dav.key;
        let depth: u32 = match headers.get("depth") {
            Some(v) => {
                let v_str = v.to_str().unwrap_or_default();
                if v_str == "infinity" {
                    u32::MAX
                } else if let Ok(v) = v_str.parse::<u32>() {
                    v
                } else {
                    *res.status_mut() = StatusCode::BAD_REQUEST;
                    return Ok(());
                }
            }
            None => 1,
        };
        let mut paths = vec![
            self.to_pathitem(real_path, &key)
                .await?
                .ok_or(Error::other("no paths"))?,
        ];
        if depth != 0 {
            if let Ok(child) = self
                .list_dir_recursive(
                    real_path,
                    &key,
                    depth,
                    MAX_PROP_FIND_ENTRIES.saturating_sub(paths.len()),
                )
                .await
            {
                paths.extend(child);
            } else {
                status_forbid(res);
                return Ok(());
            }
        }
        let output = paths
            .iter()
            .map(|v| v.to_dav_xml("/"))
            .fold(String::new(), |mut acc, v| {
                acc.push_str(&v);
                acc
            });
        res_multistatus(res, &output);
        Ok(())
    }

    async fn handle_propfind_file(
        &self,
        ctx: &Gated<'_, Read>,
        res: &mut Response,
    ) -> BoxResult<()> {
        let self_uri_prefix = "/";
        if let Some(pathitem) = self.to_pathitem(ctx.real_path, &ctx.dav.key).await? {
            res_multistatus(res, &pathitem.to_dav_xml(self_uri_prefix));
        } else {
            status_not_found(res);
        }
        Ok(())
    }

    async fn handle_mkcol(&self, ctx: &Gated<'_, Upload>, res: &mut Response) -> BoxResult<()> {
        let path = ctx.real_path.path();
        if fs::create_dir(path).await.is_ok() {
            *res.status_mut() = StatusCode::CREATED;
            Ok(())
        } else {
            *res.status_mut() = StatusCode::CONFLICT;
            Ok(())
        }
    }

    fn check_lock(&self, path: &Path, headers: &HeaderMap) -> Result<(), StatusCode> {
        clean_expired_locks(&self.locks);

        // Extract all submitted lock tokens from the If header
        let tokens = Self::extract_if_lock_tokens(headers);

        // Check if the path itself or any of its parent directories are locked
        let mut current = Some(path);
        while let Some(p) = current {
            if let Some(lock) = self.locks.get(p) {
                // If a lock is found on the path or any parent directory,
                // the lock's token MUST be present in the If header
                if !tokens.contains(&lock.token) {
                    return Err(StatusCode::LOCKED);
                }
            }
            current = p.parent();
        }

        Ok(())
    }

    fn extract_if_lock_tokens(headers: &HeaderMap) -> Vec<String> {
        let mut tokens = Vec::new();
        let Some(value) = headers.get("If").and_then(|v| v.to_str().ok()) else {
            return tokens;
        };

        let mut in_parenthesis = false;
        let mut chars = value.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '(' => in_parenthesis = true,
                ')' => in_parenthesis = false,
                '<' if in_parenthesis => {
                    // Extract token until '>'
                    let mut token = String::new();
                    for next_c in chars.by_ref() {
                        if next_c == '>' {
                            break;
                        }
                        token.push(next_c);
                    }
                    tokens.push(token);
                }
                _ => {}
            }
        }
        tokens
    }

    fn handle_lock(&self, real_path: &RealPath, req: Request, res: &mut Response) -> BoxResult<()> {
        let path = real_path.path();
        debug!("LockMap on lock: {:?}", self.locks);
        clean_expired_locks(&self.locks);

        let timeout_secs = parse_timeout_header(req.headers());
        let token = format!("opaquelocktoken:{}", Uuid::new_v4());

        match self.locks.entry(path.to_path_buf()) {
            Entry::Occupied(_) => {
                *res.status_mut() = StatusCode::LOCKED;
                *res.body_mut() = Body::from("Resource is already locked");
                return Ok(());
            }
            Entry::Vacant(v) => {
                v.insert(LockInfo {
                    token: token.clone(),
                    expires_at: Utc::now() + Duration::seconds(timeout_secs),
                });
            }
        }

        // Timeout header in response
        let timeout_resp = if timeout_secs == i64::MAX {
            "Infinite".to_string()
        } else {
            format!("Second-{timeout_secs}")
        };
        res.headers_mut().insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/xml; charset=utf-8"),
        );
        res.headers_mut()
            .insert("lock-token", format!("<{token}>").parse()?);
        res.headers_mut()
            .insert("Timeout", HeaderValue::from_str(&timeout_resp)?);

        *res.body_mut() = Body::from(format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<D:prop xmlns:D="DAV:"><D:lockdiscovery><D:activelock>
<D:locktoken><D:href>{}</D:href></D:locktoken>
<D:lockroot><D:href>{}</D:href></D:lockroot>
<D:timeout>{}</D:timeout>
</D:activelock></D:lockdiscovery></D:prop>"#,
            token,
            req.uri().path(),
            timeout_resp
        ));
        if real_path.is_miss() {
            *res.status_mut() = StatusCode::CREATED;
        }
        Ok(())
    }

    fn handle_unlock(
        &self,
        ctx: &Gated<'_, Unlock>,
        req: Request,
        res: &mut Response,
    ) -> BoxResult<()> {
        let path = ctx.real_path.path();
        clean_expired_locks(&self.locks);
        debug!("LockMap on lock: {:?}", self.locks);

        // Extract Lock-Token header
        let token_header = req
            .headers()
            .get("Lock-Token")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.trim_matches(['<', '>'].as_ref()));
        let token_header = if let Some(t) = token_header {
            t
        } else {
            *res.status_mut() = StatusCode::BAD_REQUEST;
            *res.body_mut() = Body::from("Missing Lock-Token header");
            return Ok(());
        };

        // Check if lock exists and token matches
        match self.locks.entry(path.to_path_buf()) {
            Entry::Occupied(entry) => {
                if entry.get().token == token_header {
                    entry.remove();
                    *res.status_mut() = StatusCode::NO_CONTENT;
                } else {
                    *res.status_mut() = StatusCode::FORBIDDEN;
                    *res.body_mut() = Body::from("Lock-Token does not match");
                }
            }
            Entry::Vacant(_) => {
                *res.status_mut() = StatusCode::NOT_FOUND;
                *res.body_mut() = Body::from("No lock found on resource");
            }
        }
        Ok(())
    }

    async fn handle_proppatch(
        &self,
        req: Request,
        ctx: &Gated<'_, Proppatch>,
        res: &mut Response,
    ) -> BoxResult<()> {
        let path = ctx.real_path.path();
        let output: String;
        let req_path = req.uri().path().to_string();
        if let Ok(modtime) = Self::extract_lastmodified_value(req).await {
            // Validate timestamp is within a reasonable range
            let now = Utc::now().timestamp();
            if modtime < 0 || modtime > now + 86400 {
                status_forbid(res);
                return Ok(());
            }
            std::fs::set_times(
                path,
                std::fs::FileTimes::new().set_modified(
                    SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(modtime as u64),
                ),
            )?;
            output = format!(
                r#"
                    <D:response>
                      <D:href>{req_path}</D:href>
                      <D:propstat>
                        <D:prop>
                          <D:lastmodified xmlns="DAV:">{modtime}</D:lastmodified>
                        </D:prop>
                        <D:status>HTTP/1.1 200 OK</D:status>
                      </D:propstat>
                    </D:response>
                "#
            );
        } else {
            output = format!(
                r#"
                <D:response>
                    <D:href>{req_path}</D:href>
                    <D:propstat>
                        <D:prop>
                        </D:prop>
                        <D:status>HTTP/1.1 403 Forbidden</D:status>
                    </D:propstat>
                </D:response>
                "#
            );
        }
        res_multistatus(res, &output);
        Ok(())
    }

    async fn extract_lastmodified_value(req: Request) -> BoxResult<i64> {
        let body = req.into_body().collect().await?.to_bytes();
        let xml_body = String::from_utf8(body.to_vec())?;

        let mut in_lastmodified_tag = false;
        let mut lastmodified_value = BytesText::new("");

        let mut reader = Reader::from_str(&xml_body);
        loop {
            match reader.read_event() {
                Err(e) => return Err(Box::new(e)),
                Ok(Event::Eof) => break,
                Ok(Event::Start(e)) => {
                    in_lastmodified_tag = e.name().as_ref() == "lastmodified";
                }
                Ok(Event::End(_)) => {
                    in_lastmodified_tag = false;
                }
                Ok(Event::Text(e)) if in_lastmodified_tag => {
                    lastmodified_value = e;
                    break;
                }
                _ => (),
            }
        }
        Ok(lastmodified_value.parse::<i64>()?)
    }

    async fn extract_dest(
        &self,
        headers: &HeaderMap<HeaderValue>,
        dav_path: &str,
    ) -> Option<Destination> {
        let dest = headers.get("Destination")?.to_str().ok()?;
        let uri: Uri = dest.parse().ok()?;
        match RealPath::extract_path(uri.path(), dav_path) {
            Some(dest) => Some(Destination::new(dest, uri.to_string().ends_with('/')).await),
            None => None,
        }
    }

    fn list_dir_recursive<'a>(
        &'a self,
        real_path: &'a RealPath,
        key: &'a Option<[u8; 32]>,
        depth: u32,
        max_entries: usize,
    ) -> BoxFuture<'a, BoxResult<Vec<PathItem>>> {
        async move {
            let mut paths: Vec<PathItem> = vec![];
            if max_entries == 0 {
                return Ok(paths);
            }
            let mut rd = fs::read_dir(real_path.path()).await?;
            while let Ok(Some(entry)) = rd.next_entry().await {
                if paths.len() >= max_entries {
                    break;
                }
                let entry_path = entry.path();
                if let Ok(child_real_path) = RealPath::new_from_path(
                    entry_path,
                    real_path.base_path().to_path_buf(),
                    real_path.allow_symlinks(),
                )
                .await
                    && let Ok(Some(item)) = self.to_pathitem(&child_real_path, key).await
                {
                    let is_dir = item.is_dir();
                    paths.push(item);
                    if is_dir
                        && depth > 1
                        && let Ok(child) = self
                            .list_dir_recursive(
                                &child_real_path,
                                key,
                                depth - 1,
                                max_entries.saturating_sub(paths.len()),
                            )
                            .await
                    {
                        paths.extend(child);
                    }
                }
            }
            Ok(paths)
        }
        .boxed()
    }

    async fn to_pathitem(
        &self,
        real_path: &RealPath,
        key: &Option<[u8; 32]>,
    ) -> BoxResult<Option<PathItem>> {
        let rel_path = real_path.path().strip_prefix(real_path.base_path())?;
        let (meta, meta2) = tokio::join!(
            fs::metadata(real_path.path()),
            fs::symlink_metadata(real_path.path())
        );
        let (meta, meta2) = (meta?, meta2?);
        let is_symlink = meta2.is_symlink();
        if !real_path.allow_symlinks()
            && is_symlink
            && !is_root_contained(real_path.path(), real_path.base_path()).await
        {
            return Ok(None);
        }
        let is_dir = meta.is_dir();
        let path_type = match (is_symlink, is_dir) {
            (true, true) => PathType::SymlinkDir,
            (false, true) => PathType::Dir,
            (true, false) => PathType::SymlinkFile,
            (false, false) => PathType::File,
        };
        let mtime = to_timestamp(&meta.modified()?);
        let size = match path_type {
            PathType::Dir | PathType::SymlinkDir => None,
            PathType::File | PathType::SymlinkFile => Some(if key.is_some() {
                decrypted_size_from_file(real_path.path(), meta.len()).await
            } else {
                meta.len()
            }),
        };
        let name = normalize_path(rel_path);
        Ok(Some(PathItem {
            path_type,
            name,
            mtime,
            size,
        }))
    }

    // `G` is left generic because COPY and MOVE share this handler but gate
    // with different requirements.
    async fn handle_copymove<G: Gate>(
        &self,
        ctx: &Gated<'_, G>,
        req: Request,
        res: &mut Response,
    ) -> BoxResult<()> {
        let real_path = ctx.real_path;
        let dav_path = ctx.dav.directory.as_str();
        let path = real_path.path();
        // get and check headers.
        let overwrite = req.headers().typed_get::<Overwrite>().is_none_or(|o| o.0);
        let depth = match req.headers().typed_get::<Depth>() {
            Some(Depth::Infinity) | None => Depth::Infinity,
            Some(Depth::Zero) if req.method().as_str() == "COPY" => Depth::Zero,
            _ => {
                *res.status_mut() = StatusCode::BAD_REQUEST;
                return Ok(());
            }
        };

        // decode and validate destination.
        let mut dest = if let Some(dest) = self.extract_dest(req.headers(), dav_path).await {
            dest
        } else {
            *res.status_mut() = StatusCode::FORBIDDEN;
            return Ok(());
        };

        // Fails if we try to move a folder in place of the root directory itself
        if real_path.is_dir() && dest.path() == &PathBuf::from(dav_path) {
            *res.status_mut() = StatusCode::FORBIDDEN;
            return Ok(());
        }

        // Fails if collection parent does not exist
        if dest.path().parent().is_none()
            || !dest
                .path()
                .parent()
                .ok_or(Error::other("no parent"))?
                .exists()
        {
            *res.status_mut() = StatusCode::CONFLICT;
            return Ok(());
        }

        // Fails if exists and overwrite is false
        if !overwrite && dest.exists() {
            *res.status_mut() = StatusCode::PRECONDITION_FAILED;
            return Ok(());
        }

        // Fails if source == dest
        if path == dest.path() {
            *res.status_mut() = StatusCode::FORBIDDEN;
            return Ok(());
        }

        // see if we need to delete the destination first
        if real_path.is_dir()
            && dest.exists()
            && dest.is_dir()
            && overwrite
            && depth != Depth::Zero
            && fs::remove_dir_all(dest.path()).await.is_err()
        {
            *res.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
            return Ok(());
        }

        // COPY or MOVE
        if req.method().as_str() == "COPY" {
            Self::do_copy(path, dest.path(), dest.path(), dest.is_dir(), depth).await?;
            if overwrite && dest.exists() {
                *res.status_mut() = StatusCode::NO_CONTENT;
            } else {
                *res.status_mut() = StatusCode::CREATED;
            }
        } else {
            // if the source is a file but the destination is a directory, alter the destination
            if real_path.is_file() && dest.is_dir() {
                dest.push(
                    real_path
                        .path()
                        .file_name()
                        .ok_or(Error::other("no path"))?
                        .into(),
                );
            }
            if real_path.is_dir() && dest.is_file() && dest.exists() {
                *res.status_mut() = StatusCode::NO_CONTENT;
            } else {
                fs::rename(path, dest.path()).await?;
                if real_path.is_dir() {
                    self.locks.retain(|p, _| !p.starts_with(path));
                } else {
                    self.locks.remove(path);
                }
                if dest.exists() {
                    *res.status_mut() = StatusCode::NO_CONTENT;
                } else {
                    *res.status_mut() = StatusCode::CREATED;
                }
            }
        }
        Ok(())
    }

    fn do_copy<'a>(
        source: &'a Path,
        topdest: &'a Path,
        dest: &'a Path,
        dest_is_dir_or_to_be: bool,
        depth: Depth,
    ) -> BoxFuture<'a, Result<(), std::io::Error>> {
        async move {
            // when doing "COPY /a/b /a/b/c make sure we don't recursively
            // copy /a/b/c/ into /a/b/c.
            if source == topdest {
                return Ok(());
            }

            // source must exist.
            let meta = match fs::metadata(source).await {
                Err(e) => return Err(e),
                Ok(m) => m,
            };

            // create dest if directory
            if dest_is_dir_or_to_be {
                fs::create_dir(dest).await.ok();
            }

            // if it's a file we can overwrite it.
            if meta.is_file() {
                if dest_is_dir_or_to_be {
                    let destfile = dest.join(
                        source
                            .file_name()
                            .ok_or_else(|| Error::other("could not extract file name"))?,
                    );
                    return match fs::copy(source, destfile).await {
                        Ok(_) => Ok(()),
                        Err(e) => {
                            debug!("do_copy: fs::copy error: {:?}", e);
                            Err(e)
                        }
                    };
                } else {
                    return match fs::copy(source, dest).await {
                        Ok(_) => Ok(()),
                        Err(e) => {
                            debug!("do_copy: fs::copy error: {:?}", e);
                            Err(e)
                        }
                    };
                }
            }

            // only recurse when Depth > 0.
            if depth == Depth::Zero {
                return Ok(());
            }

            let mut entries = match fs::read_dir(source).await {
                Ok(entries) => entries,
                Err(e) => {
                    debug!("do_copy: fs::read_dir error: {:?}", e);
                    return Err(e);
                }
            };

            while let Some(dirent) = entries.next_entry().await? {
                // NOTE: dirent.metadata() behaves like symlink_metadata()
                let meta = match dirent.metadata().await {
                    Ok(meta) => meta,
                    Err(e) => return Err(e),
                };
                let name = dirent.file_name();
                let nsrc = source.join(&name);
                let ndest = dest.join(&name);

                // recurse
                Self::do_copy(&nsrc, topdest, &ndest, meta.is_dir(), depth).await?;
            }

            Ok(())
        }
        .boxed()
    }

    /// `POST /path/to/archive.zip?extract`
    ///
    /// Extracts the archive into a sibling folder named after it.
    async fn handle_extract_zip(
        &self,
        ctx: &Gated<'_, Extract>,
        headers: &HeaderMap,
        res: &mut Response,
    ) -> BoxResult<()> {
        let real_path = ctx.real_path;
        let key = ctx.dav.key;
        let path = real_path.path();

        // The resource (already gated as an existing file) must be a .zip archive.
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_none_or(|name| !name.to_lowercase().ends_with(".zip"))
        {
            status_text(res, StatusCode::BAD_REQUEST, "File is not a zip archive");
            return Ok(());
        }

        // The archive is extracted into a sibling folder named after its stem.
        let (parent_dir, folder_name) =
            match (path.parent(), path.file_stem().and_then(|s| s.to_str())) {
                (Some(parent), Some(name)) if !name.is_empty() => (parent, name),
                _ => {
                    status_text(res, StatusCode::BAD_REQUEST, "Invalid archive name");
                    return Ok(());
                }
            };
        let target_dir = parent_dir.join(folder_name);

        // The extraction target (and its ancestors) must not be locked.
        if let Err(status) = self.check_lock(&target_dir, headers) {
            status_text(res, status, "Target directory is locked");
            return Ok(());
        }

        // Create the target directory and enforce the dav symlink policy on it.
        if let Err(e) = prepare_extract_target(
            &target_dir,
            real_path.base_path(),
            real_path.allow_symlinks(),
        )
        .await
        {
            match e {
                TargetError::Conflicts => {
                    status_text(
                        res,
                        StatusCode::CONFLICT,
                        "A file with the target folder name already exists",
                    );
                }
                TargetError::Unsafe | TargetError::PermissionDenied => {
                    status_forbid(res);
                }
                TargetError::Io(msg) => {
                    error!(
                        "Failed to prepare extraction target {}: {msg}",
                        target_dir.display()
                    );
                    status_text(
                        res,
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to create directory",
                    );
                }
            }
            return Ok(());
        }

        match zip::extract(path, &target_dir, key).await {
            Ok(()) => status_text(res, StatusCode::OK, "Extraction successful"),
            Err(zip::ExtractError::BadArchive(msg)) => {
                status_text(
                    res,
                    StatusCode::BAD_REQUEST,
                    format!("Invalid zip archive: {msg}"),
                );
            }
            Err(zip::ExtractError::PermissionDenied) => {
                status_forbid(res);
            }
            Err(zip::ExtractError::Io(msg)) => {
                error!(
                    "Failed to extract {} into {}: {msg}",
                    path.display(),
                    target_dir.display()
                );
                status_text(res, StatusCode::INTERNAL_SERVER_ERROR, "Extraction failed");
            }
        }
        Ok(())
    }
}

/// Error returned by [`prepare_extract_target`].
enum TargetError {
    /// A file already occupies the target path.
    Conflicts,
    /// The target escapes the dav base, violating the dav symlink policy.
    Unsafe,
    /// The process lacks the permission to write the target.
    PermissionDenied,
    /// A filesystem error occurred.
    Io(String),
}

/// Creates the directory a zip archive is extracted into, if it does not
/// exist yet.
///
/// A pre-existing target must be a directory that stays within the dav base
/// (e.g. a symlinked directory pointing outside is rejected when the dav
/// does not allow symlinks). A freshly created target is one level under the
/// zip file's parent, which was already validated by [`RealPath`], so it can
/// never escape the base.
async fn prepare_extract_target(
    target: &Path,
    base: &Path,
    allow_symlinks: bool,
) -> Result<(), TargetError> {
    // Classify the target, following symlinks like the rest of the server.
    let existing = match fs::metadata(target).await {
        Ok(meta) => Some(meta),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            return Err(TargetError::PermissionDenied);
        }
        Err(e) => return Err(TargetError::Io(e.to_string())),
    };

    if let Some(meta) = existing {
        // A file (or a symlink to a file) occupying the target path is a
        // conflict.
        if !meta.is_dir() {
            return Err(TargetError::Conflicts);
        }
        if !allow_symlinks && !is_root_contained(target, base).await {
            return Err(TargetError::Unsafe);
        }
        return Ok(());
    }

    fs::create_dir_all(target).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            TargetError::PermissionDenied
        } else {
            TargetError::Io(e.to_string())
        }
    })
}

#[derive(Debug, Serialize, Eq, PartialEq, Ord, PartialOrd)]
struct PathItem {
    path_type: PathType,
    name: String,
    mtime: u64,
    size: Option<u64>,
}

impl PathItem {
    pub fn is_dir(&self) -> bool {
        self.path_type == PathType::Dir || self.path_type == PathType::SymlinkDir
    }

    pub fn to_dav_xml(&self, prefix: &str) -> String {
        let mtime_str = match i64::try_from(self.mtime)
            .ok()
            .and_then(|t| Utc.timestamp_millis_opt(t).single())
        {
            Some(dt) => {
                let mut s = dt.to_rfc2822();
                s.truncate(s.len() - 6);
                format!("{s} GMT")
            }
            None => "Thu, 01 Jan 1970 00:00:00 GMT".to_string(),
        };
        let mut href = encode_uri(&format!("{}{}", prefix, self.name));
        if self.is_dir() && !href.ends_with('/') {
            href.push('/');
        }
        let href = escape(&href);
        let displayname = escape(self.base_name());
        match self.path_type {
            PathType::Dir | PathType::SymlinkDir => format!(
                r#"<D:response>
<D:href>{href}</D:href>
<D:propstat>
<D:prop>
<D:displayname>{displayname}</D:displayname>
<D:getlastmodified>{mtime_str}</D:getlastmodified>
<D:resourcetype><D:collection/></D:resourcetype>
</D:prop>
<D:status>HTTP/1.1 200 OK</D:status>
</D:propstat>
</D:response>"#
            ),
            PathType::File | PathType::SymlinkFile => format!(
                r#"<D:response>
<D:href>{}</D:href>
<D:propstat>
<D:prop>
<D:displayname>{}</D:displayname>
<D:getcontentlength>{}</D:getcontentlength>
<D:getlastmodified>{}</D:getlastmodified>
<D:resourcetype></D:resourcetype>
</D:prop>
<D:status>HTTP/1.1 200 OK</D:status>
</D:propstat>
</D:response>"#,
                href,
                displayname,
                self.size.unwrap_or_default(),
                mtime_str
            ),
        }
    }
    fn base_name(&self) -> &str {
        Path::new(&self.name)
            .file_name()
            .and_then(|v| v.to_str())
            .unwrap_or_default()
    }
}

#[derive(Debug, Serialize, Eq, PartialEq, Ord, PartialOrd)]
enum PathType {
    Dir,
    SymlinkDir,
    File,
    SymlinkFile,
}

fn to_timestamp(time: &SystemTime) -> u64 {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |t| t.as_millis()) as u64
}

fn normalize_path<P: AsRef<Path>>(path: P) -> String {
    let path = path.as_ref().to_str().unwrap_or_default();
    if cfg!(windows) {
        path.replace('\\', "/")
    } else {
        path.to_string()
    }
}

async fn ensure_path_parent(path: &Path) -> BoxResult<()> {
    if let Some(parent) = path.parent()
        && fs::symlink_metadata(parent).await.is_err()
    {
        fs::create_dir_all(&parent).await?;
    }

    Ok(())
}

fn res_multistatus(res: &mut Response, content: &str) {
    *res.status_mut() = StatusCode::MULTI_STATUS;
    res.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/xml; charset=utf-8"),
    );
    *res.body_mut() = Body::from(format!(
        r#"<?xml version="1.0" encoding="utf-8" ?>
<D:multistatus xmlns:D="DAV:">
{content}
</D:multistatus>"#,
    ));
}

#[derive(Debug)]
enum RangeValue {
    Range(u64, Option<u64>),
    Suffix(u64),
}

fn parse_range(headers: &HeaderMap<HeaderValue>) -> Option<RangeValue> {
    let range_hdr = headers.get(RANGE)?;
    let hdr = range_hdr.to_str().ok()?;
    let mut sp = hdr.splitn(2, '=');
    let units = sp.next()?;
    if units == "bytes" {
        let range = sp.next()?;
        let mut sp_range = range.splitn(2, '-');
        let start_str = sp_range.next().unwrap_or("");
        let end_str = sp_range.next().unwrap_or("");

        if start_str.is_empty() {
            // Suffix range: bytes=-N
            let suffix: u64 = end_str.parse().ok()?;
            Some(RangeValue::Suffix(suffix))
        } else {
            let start: u64 = start_str.parse().ok()?;
            let end: Option<u64> = if end_str.is_empty() {
                None
            } else {
                Some(end_str.parse().ok()?)
            };
            Some(RangeValue::Range(start, end))
        }
    } else {
        None
    }
}

/// Sets `status` and a plain-text `body` on the response.
fn status_text(res: &mut Response, status: StatusCode, body: impl Into<Body>) {
    *res.status_mut() = status;
    *res.body_mut() = body.into();
}

fn status_forbid(res: &mut Response) {
    status_text(res, StatusCode::FORBIDDEN, "Forbidden");
}

fn status_not_found(res: &mut Response) {
    status_text(res, StatusCode::NOT_FOUND, "Not Found");
}

/// The request resource of a method that has passed the [`Gate`] of its
/// requirement type `G`: the dav, the resolved path, and the evidence that
/// the method's permission and state checks hold.
///
/// Handlers that need more than the path-resolution checks of [`RealPath`]
/// receive the resource only through this type, never as a bare `&RealPath`
/// (the exceptions are LOCK and OPTIONS, which have no method-level check).
///
/// Typestate: the type parameter records the requirement the context was
/// gated for. The token can only be produced by [`gate`], and handlers
/// declare their own requirement in their signature, so the compiler
/// rejects a context gated for a different requirement.
struct Gated<'a, G: Gate> {
    _gate: PhantomData<G>,
    /// The request resource, validated against the dav base.
    real_path: &'a RealPath,
    /// The dav the request is served from.
    dav: &'a Dav,
}

/// Evaluates the [`Gate`] requirement `G` for a method.
///
/// When a check fails, the rejection (403 or 404) is written to `res` and
/// `None` is returned; when both pass, the returned [`Gated`] token may only
/// be passed to a handler declaring the same requirement.
///
/// The lock gate is a separate server-state concern and stays in
/// [`WebdavServer::check_lock`], evaluated before dispatch.
fn gate<'a, G: Gate>(
    real_path: &'a RealPath,
    dav: &'a Dav,
    res: &mut Response,
) -> Option<Gated<'a, G>> {
    if !G::permission_granted(dav, real_path) {
        status_forbid(res);
        return None;
    }
    if !G::state_holds(real_path) {
        status_not_found(res);
        return None;
    }
    Some(Gated {
        _gate: PhantomData,
        real_path,
        dav,
    })
}

fn status_no_content(res: &mut Response) {
    *res.status_mut() = StatusCode::NO_CONTENT;
}

fn status_method_not_allowed(res: &mut Response) {
    *res.status_mut() = StatusCode::METHOD_NOT_ALLOWED;
    *res.body_mut() = Body::from("Method not allowed");
}

fn get_file_name(path: &Path) -> BoxResult<&str> {
    path.file_name()
        .and_then(|v| v.to_str())
        .ok_or_else(|| format!("Failed to get file name of `{}`", path.display()).into())
}

fn set_webdav_headers(res: &mut Response) {
    res.headers_mut().insert(
        "Allow",
        HeaderValue::from_static("GET,HEAD,PUT,OPTIONS,DELETE,PROPFIND,COPY,MOVE"),
    );
    res.headers_mut()
        .insert("DAV", HeaderValue::from_static("1,2"));
}

pub fn encode_uri(v: &str) -> String {
    let parts: Vec<_> = v.split('/').map(urlencoding::encode).collect();
    parts.join("/")
}

pub fn decode_uri(v: &str) -> Option<Cow<'_, str>> {
    percent_encoding::percent_decode(v.as_bytes())
        .decode_utf8()
        .ok()
}

use futures_util::task::{Context, Poll};
use std::pin::Pin;

pub struct CancelOnDrop<S> {
    inner: S,
    token: CancellationToken,
}

impl<S> CancelOnDrop<S> {
    pub fn new(inner: S, token: CancellationToken) -> Self {
        Self { inner, token }
    }
}

impl<S: futures_util::Stream + Unpin> futures_util::Stream for CancelOnDrop<S> {
    type Item = S::Item;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.inner).poll_next(cx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<S> Drop for CancelOnDrop<S> {
    fn drop(&mut self) {
        self.token.cancel();
    }
}

enum Destination {
    ExistingDir(PathBuf),
    DirToBe(PathBuf),
    ExistingFile(PathBuf),
    FileToBe(PathBuf),
}

impl Destination {
    async fn new(dest: PathBuf, is_new_dir: bool) -> Destination {
        if let Ok(meta) = fs::symlink_metadata(&dest).await {
            if meta.is_symlink() {
                if let Ok(m) = fs::metadata(&dest).await {
                    if m.is_file() {
                        return Destination::ExistingFile(dest);
                    }
                    if m.is_dir() {
                        return Destination::ExistingDir(dest);
                    }
                }
                // Broken symlink - treat as existing file entry
                return Destination::ExistingFile(dest);
            }

            if meta.is_file() {
                Destination::ExistingFile(dest)
            } else {
                Destination::ExistingDir(dest)
            }
        } else {
            if is_new_dir {
                return Destination::DirToBe(dest);
            }
            Destination::FileToBe(dest)
        }
    }

    fn exists(&self) -> bool {
        match self {
            Destination::ExistingDir(_) | Destination::ExistingFile(_) => true,
            Destination::DirToBe(_) | Destination::FileToBe(_) => false,
        }
    }

    fn is_dir(&self) -> bool {
        match self {
            Destination::ExistingDir(_) | Destination::DirToBe(_) => true,
            Destination::ExistingFile(_) | Destination::FileToBe(_) => false,
        }
    }

    fn is_file(&self) -> bool {
        !self.is_dir()
    }

    fn path(&self) -> &PathBuf {
        match self {
            Destination::ExistingDir(p)
            | Destination::DirToBe(p)
            | Destination::ExistingFile(p)
            | Destination::FileToBe(p) => p,
        }
    }

    fn push(&mut self, path: PathBuf) {
        match self {
            Destination::ExistingDir(p)
            | Destination::DirToBe(p)
            | Destination::ExistingFile(p)
            | Destination::FileToBe(p) => p.push(path),
        }
    }
}

fn clean_expired_locks(locks: &DashMap<PathBuf, LockInfo>) {
    let now = Utc::now();
    locks.retain(|_, info| now < info.expires_at);
}

fn parse_timeout_header(headers: &HeaderMap<HeaderValue>) -> i64 {
    if let Some(timeout_header) = headers.get("Timeout")
        && let Ok(header_val) = timeout_header.to_str()
    {
        // Support comma-separated list, take the first understood
        for val in header_val.split(',') {
            let val = val.trim();
            if let Some(secs) = val.strip_prefix("Second-")
                && let Ok(secs) = secs.parse::<i64>()
                && secs > 0
            {
                return secs.min(LOCK_TIMEOUT);
            }
        }
    }
    LOCK_TIMEOUT
}
