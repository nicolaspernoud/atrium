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
use super::{
    dav_file::{decrypted_size, Streamer, BUF_SIZE},
    headers::Depth,
    model::Dav,
};
use crate::davs::{dav_file::DavFile, headers::Overwrite};
use async_walkdir::WalkDir;
use async_zip::{Compression, ZipEntryBuilder, tokio::write::ZipFileWriter};
use chrono::{TimeZone, Utc};
use futures::TryStreamExt;
use futures_util::{future::BoxFuture, FutureExt, StreamExt};
use headers::{
    AcceptRanges, ContentType, HeaderMap, HeaderMapExt, IfModifiedSince, IfNoneMatch, IfRange,
    Range,
};
use hyper::{
    header::{
        HeaderValue, CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, RANGE,
    },
    Body, Method, StatusCode, Uri,
};
use quick_xml::escape::escape;
use serde::Serialize;
use std::{
    borrow::Cow,
    io::Error,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};
use tokio::{fs, io, io::AsyncWrite};
use tokio_util::io::StreamReader;
use tracing::{debug, error};
use uuid::Uuid;

pub type Request = hyper::Request<Body>;
pub type Response = hyper::Response<Body>;

pub type BoxResult<T> = Result<T, Box<dyn std::error::Error>>;
static APPLICATION_JSON: HeaderValue = HeaderValue::from_static("application/json");

pub struct WebdavServer {}

impl WebdavServer {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn call(
        self: Arc<Self>,
        req: Request,
        addr: SocketAddr,
        dav: &Dav,
    ) -> Result<Response, hyper::Error> {
        let method = req.method().clone();
        let uri = req.uri().clone();

        let res = match self.handle(req, dav).await {
            Ok(res) => {
                let status = res.status().as_u16();
                debug!(r#"{} "{} {}" - {}"#, addr.ip(), method, uri, status,);
                res
            }
            Err(err) => {
                let mut res = Response::default();
                let status = StatusCode::INTERNAL_SERVER_ERROR;
                *res.status_mut() = status;
                let status = status.as_u16();
                error!(r#"{} "{} {}" - {} {}"#, addr.ip(), method, uri, status, err);
                res
            }
        };

        Ok(res)
    }

    pub async fn handle(self: Arc<Self>, mut req: Request, dav: &Dav) -> BoxResult<Response> {
        let mut res = Response::default();

        let req_path = &req.uri().path();
        let headers = req.headers();
        let method = req.method().clone();

        let head_only = method == Method::HEAD;

        let path = match self.extract_path(req_path, &dav.directory) {
            Some(v) => v,
            None => {
                status_forbid(&mut res);
                return Ok(res);
            }
        };

        let path = path.as_path();

        let query = req.uri().query().unwrap_or_default();

        let (is_miss, is_dir, is_file, size) = match fs::metadata(path).await.ok() {
            Some(meta) => (false, meta.is_dir(), meta.is_file(), meta.len()),
            None => (true, false, false, 0),
        };

        let allow_upload = dav.writable;
        let allow_delete = dav.writable;
        let allow_search = true;
        let key = dav.key;

        if !dav.allow_symlinks
            && !is_miss
            && !self
                .is_root_contained(path, Path::new(&dav.directory))
                .await
        {
            status_not_found(&mut res);
            return Ok(res);
        }

        match method {
            Method::GET | Method::HEAD => {
                if is_dir {
                    if let Some(stripped) = query.strip_prefix("q=") {
                        if allow_search {
                            let q = decode_uri(stripped).unwrap_or_default();
                            self.handle_query_dir(
                                path,
                                &q,
                                &mut res,
                                &dav.directory,
                                dav.allow_symlinks,
                                key,
                            )
                            .await?;
                        }
                    } else if query.starts_with("diskusage") {
                        self.handle_disk_usage(path, &mut res).await?;
                    } else {
                        self.handle_zip_dir(path, head_only, &mut res, key).await?;
                    }
                } else if is_file {
                    self.handle_send_file(path, headers, head_only, &mut res, key)
                        .await?;
                } else {
                    status_not_found(&mut res);
                }
            }
            Method::OPTIONS => {
                set_webdav_headers(&mut res);
            }
            Method::PUT => {
                if !allow_upload || (!allow_delete && is_file && size > 0) {
                    status_forbid(&mut res);
                } else {
                    self.handle_upload(path, req, &mut res, key).await?;
                }
            }
            Method::DELETE => {
                if !allow_delete {
                    status_forbid(&mut res);
                } else if !is_miss {
                    self.handle_delete(path, is_dir, &mut res).await?
                } else {
                    status_not_found(&mut res);
                }
            }
            method => match method.as_str() {
                "PROPFIND" => {
                    if is_dir {
                        self.handle_propfind_dir(
                            path,
                            headers,
                            &mut res,
                            &dav.directory,
                            dav.allow_symlinks,
                            key,
                        )
                        .await?;
                    } else if is_file {
                        self.handle_propfind_file(
                            path,
                            &mut res,
                            &dav.directory,
                            dav.allow_symlinks,
                            key,
                        )
                        .await?;
                    } else {
                        status_not_found(&mut res);
                    }
                }
                "PROPPATCH" => {
                    if is_file {
                        self.handle_proppatch(req_path, &mut res).await?;
                    } else {
                        status_not_found(&mut res);
                    }
                }
                "MKCOL" => {
                    if !allow_upload {
                        status_forbid(&mut res);
                    } else if !is_miss {
                        status_method_not_allowed(&mut res);
                    } else if axum::body::HttpBody::data(&mut req).await.is_some() {
                        *res.status_mut() = StatusCode::UNSUPPORTED_MEDIA_TYPE;
                        *res.body_mut() = Body::from("Unsupported Media Type");
                    } else {
                        self.handle_mkcol(path, &mut res).await?;
                    }
                }
                "COPY" => {
                    if !allow_upload {
                        status_forbid(&mut res);
                    } else if is_miss {
                        status_not_found(&mut res);
                    } else {
                        self.handle_copymove(path, req, method, &mut res, &dav.directory)
                            .await?
                    }
                }
                "MOVE" => {
                    if !allow_upload || !allow_delete {
                        status_forbid(&mut res);
                    } else if is_miss {
                        status_not_found(&mut res);
                    } else {
                        self.handle_copymove(path, req, method, &mut res, &dav.directory)
                            .await?
                    }
                }
                "LOCK" => {
                    // Fake lock
                    if is_dir {
                        status_not_found(&mut res);
                    } else {
                        self.handle_lock(req_path, is_miss, &mut res).await?;
                    }
                }
                "UNLOCK" => {
                    // Fake unlock
                    if is_miss {
                        status_not_found(&mut res);
                    }
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
        path: &Path,
        mut req: Request,
        res: &mut Response,
        key: Option<[u8; 32]>,
    ) -> BoxResult<()> {
        ensure_path_parent(path).await?;

        let file = match DavFile::create(path, key).await {
            Ok(v) => v,
            Err(_) => {
                status_forbid(res);
                return Ok(());
            }
        };

        let body_with_io_error = req
            .body_mut()
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err));

        let mut body_reader = StreamReader::new(body_with_io_error);

        file.copy_from(&mut body_reader).await?;

        // If the X-OC-Mtime header is present, alter the file modified time according to that header's value.
        if let Some(h) = req.headers().get("X-OC-Mtime") {
            if let Ok(h) = h.to_str() {
                if let Ok(t) = h.parse::<i64>() {
                    // If it fails, we do nothing
                    _ = filetime::set_file_mtime(path, filetime::FileTime::from_unix_time(t, 0));
                }
            }
        };

        *res.status_mut() = StatusCode::CREATED;
        Ok(())
    }

    async fn handle_delete(&self, path: &Path, is_dir: bool, res: &mut Response) -> BoxResult<()> {
        match is_dir {
            true => fs::remove_dir_all(path).await?,
            false => fs::remove_file(path).await?,
        }

        status_no_content(res);
        Ok(())
    }

    async fn handle_query_dir(
        &self,
        path: &Path,
        query: &str,
        res: &mut Response,
        directory: &str,
        allow_symlinks: bool,
        key: Option<[u8; 32]>,
    ) -> BoxResult<()> {
        let mut paths: Vec<PathItem> = vec![];
        let mut walkdir = WalkDir::new(path);
        while let Some(entry) = walkdir.next().await {
            if let Ok(entry) = entry {
                if !entry
                    .file_name()
                    .to_string_lossy()
                    .to_lowercase()
                    .contains(&query.to_lowercase())
                {
                    continue;
                }
                if fs::symlink_metadata(entry.path()).await.is_err() {
                    continue;
                }
                if let Ok(Some(item)) = self
                    .to_pathitem(
                        entry.path(),
                        path.to_path_buf(),
                        directory,
                        allow_symlinks,
                        &key,
                    )
                    .await
                {
                    paths.push(item);
                }
            }
        }
        let j = serde_json::to_string(&paths)?;
        res.headers_mut()
            .insert(CONTENT_TYPE, APPLICATION_JSON.to_owned());
        *res.body_mut() = Body::from(j);
        Ok(())
    }

    async fn handle_disk_usage(
        &self,
        path: &Path,
        res: &mut http::Response<Body>,
    ) -> BoxResult<()> {
        let full_path = fs::canonicalize(path).await?;
        let du = crate::sysinfo::disk_info(full_path).await?;
        let j = serde_json::to_string(&du)?;
        res.headers_mut()
            .insert(CONTENT_TYPE, APPLICATION_JSON.to_owned());
        *res.body_mut() = Body::from(j);
        Ok(())
    }

    async fn handle_zip_dir(
        &self,
        path: &Path,
        head_only: bool,
        res: &mut Response,
        key: Option<[u8; 32]>,
    ) -> BoxResult<()> {
        let (mut writer, reader) = tokio::io::duplex(BUF_SIZE);
        let filename = get_file_name(path)?;
        res.headers_mut().insert(
            CONTENT_DISPOSITION,
            HeaderValue::from_str(&format!(
                "attachment; filename=\"{}.zip\"",
                encode_uri(filename),
            ))
            .unwrap(),
        );
        res.headers_mut()
            .insert(CONTENT_TYPE, HeaderValue::from_static("application/zip"));
        if head_only {
            return Ok(());
        }
        let path = path.to_owned();
        tokio::spawn(async move {
            if let Err(e) = zip_dir(&mut writer, &path, key).await {
                error!("Failed to zip {}, {}", path.display(), e);
            }
        });
        let reader = Streamer::new(reader, BUF_SIZE);
        *res.body_mut() = Body::wrap_stream(reader.into_stream());
        Ok(())
    }

    async fn handle_send_file(
        &self,
        path: &Path,
        headers: &HeaderMap<HeaderValue>,
        head_only: bool,
        res: &mut Response,
        key: Option<[u8; 32]>,
    ) -> BoxResult<()> {
        let file = DavFile::open(path, key).await?;

        let mut use_range = true;
        if let Some((etag, last_modified)) = file.cache_headers() {
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
                use_range = headers
                    .typed_get::<IfRange>()
                    .map(|if_range| !if_range.is_modified(Some(&etag), Some(&last_modified)))
                    // Always be fresh if there is no validators
                    .unwrap_or(true);
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
        res.headers_mut().insert(
            CONTENT_DISPOSITION,
            HeaderValue::from_str(&format!(
                "attachment; filename=\"{}\"",
                encode_uri(filename),
            ))
            .unwrap(),
        );

        res.headers_mut().typed_insert(AcceptRanges::bytes());

        let size = file.len();

        if let Some(range) = range {
            debug!("Requesting range: {:?}", range);
            if range
                .end
                .map_or_else(|| range.start < size, |v| v >= range.start)
            {
                let end = range.end.unwrap_or(size - 1).min(size - 1);
                let part_size = end - range.start + 1;
                *res.status_mut() = StatusCode::PARTIAL_CONTENT;
                let content_range = format!("bytes {}-{}/{}", range.start, end, size);
                res.headers_mut()
                    .insert(CONTENT_RANGE, content_range.parse()?);
                res.headers_mut()
                    .insert(CONTENT_LENGTH, format!("{}", part_size).parse()?);
                if head_only {
                    return Ok(());
                }

                *res.body_mut() = file.into_body_sized(range.start, part_size).await?;
            } else {
                *res.status_mut() = StatusCode::RANGE_NOT_SATISFIABLE;
                res.headers_mut()
                    .insert(CONTENT_RANGE, format!("bytes */{}", size).parse()?);
            }
        } else {
            res.headers_mut()
                .insert(CONTENT_LENGTH, format!("{}", size).parse()?);
            if head_only {
                return Ok(());
            }
            *res.body_mut() = file.into_body().await;
        }
        Ok(())
    }

    async fn handle_propfind_dir(
        &self,
        path: &Path,
        headers: &HeaderMap<HeaderValue>,
        res: &mut Response,
        directory: &str,
        allow_symlinks: bool,
        key: Option<[u8; 32]>,
    ) -> BoxResult<()> {
        let base_path = Path::new(directory);
        let depth: u32 = match headers.get("depth") {
            Some(v) => match v.to_str().ok().and_then(|v| v.parse().ok()) {
                Some(v) => v,
                None => {
                    *res.status_mut() = StatusCode::BAD_REQUEST;
                    return Ok(());
                }
            },
            None => 1,
        };
        let mut paths = vec![self
            .to_pathitem(path, base_path, directory, allow_symlinks, &key)
            .await?
            .unwrap()];
        if depth != 0 {
            match self
                .list_dir(path, base_path, directory, allow_symlinks, &key)
                .await
            {
                Ok(child) => paths.extend(child),
                Err(_) => {
                    status_forbid(res);
                    return Ok(());
                }
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
        path: &Path,
        res: &mut Response,
        directory: &str,
        allow_symlinks: bool,
        key: Option<[u8; 32]>,
    ) -> BoxResult<()> {
        let base_path = Path::new(directory);
        let self_uri_prefix = "/";
        if let Some(pathitem) = self
            .to_pathitem(path, base_path, directory, allow_symlinks, &key)
            .await?
        {
            res_multistatus(res, &pathitem.to_dav_xml(self_uri_prefix));
        } else {
            status_not_found(res);
        }
        Ok(())
    }

    async fn handle_mkcol(&self, path: &Path, res: &mut Response) -> BoxResult<()> {
        match fs::create_dir(path).await {
            Ok(_) => {
                *res.status_mut() = StatusCode::CREATED;
                Ok(())
            }
            Err(_) => {
                *res.status_mut() = StatusCode::CONFLICT;
                Ok(())
            }
        }
    }

    async fn handle_lock(
        &self,
        req_path: &str,
        is_miss: bool,
        res: &mut Response,
    ) -> BoxResult<()> {
        let token = format!("opaquelocktoken:{}", Uuid::new_v4());

        res.headers_mut().insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/xml; charset=utf-8"),
        );
        res.headers_mut()
            .insert("lock-token", format!("<{}>", token).parse().unwrap());

        *res.body_mut() = Body::from(format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<D:prop xmlns:D="DAV:"><D:lockdiscovery><D:activelock>
<D:locktoken><D:href>{}</D:href></D:locktoken>
<D:lockroot><D:href>{}</D:href></D:lockroot>
</D:activelock></D:lockdiscovery></D:prop>"#,
            token, req_path
        ));
        if is_miss {
            *res.status_mut() = StatusCode::CREATED;
        }
        Ok(())
    }

    async fn handle_proppatch(&self, req_path: &str, res: &mut Response) -> BoxResult<()> {
        let output = format!(
            r#"<D:response>
<D:href>{}</D:href>
<D:propstat>
<D:prop>
</D:prop>
<D:status>HTTP/1.1 403 Forbidden</D:status>
</D:propstat>
</D:response>"#,
            req_path
        );
        res_multistatus(res, &output);
        Ok(())
    }

    async fn is_root_contained(&self, path: &Path, directory: &Path) -> bool {
        let (path, dir) = tokio::join!(fs::canonicalize(path), fs::canonicalize(directory));
        let dir = match dir {
            Ok(dir) => dir,
            Err(_err) => return false,
        };
        path.ok().map(|v| v.starts_with(dir)).unwrap_or_default()
    }

    async fn extract_dest(
        &self,
        headers: &HeaderMap<HeaderValue>,
        dav_path: &str,
    ) -> Option<Destination> {
        let dest = headers.get("Destination")?.to_str().ok()?;
        let uri: Uri = dest.parse().ok()?;
        match self.extract_path(uri.path(), dav_path) {
            Some(dest) => Some(Destination::new(dest, uri.to_string().ends_with('/')).await),
            None => None,
        }
    }

    fn extract_path(&self, wanted_path: &str, dav_path: &str) -> Option<PathBuf> {
        let decoded_path = decode_uri(&wanted_path[1..])?.into_owned();
        let stripped_path = Path::new(&decoded_path).components().collect::<PathBuf>();
        let self_path = Path::new(dav_path);
        Some(self_path.join(stripped_path))
    }

    async fn list_dir(
        &self,
        entry_path: &Path,
        base_path: &Path,
        directory: &str,
        allow_symlinks: bool,
        key: &Option<[u8; 32]>,
    ) -> BoxResult<Vec<PathItem>> {
        let mut paths: Vec<PathItem> = vec![];
        let mut rd = fs::read_dir(entry_path).await?;
        while let Ok(Some(entry)) = rd.next_entry().await {
            let entry_path = entry.path();
            if let Ok(Some(item)) = self
                .to_pathitem(
                    entry_path.as_path(),
                    base_path,
                    directory,
                    allow_symlinks,
                    key,
                )
                .await
            {
                paths.push(item);
            }
        }
        Ok(paths)
    }

    async fn to_pathitem<P: AsRef<Path>>(
        &self,
        path: P,
        base_path: P,
        directory: &str,
        allow_symlinks: bool,
        key: &Option<[u8; 32]>,
    ) -> BoxResult<Option<PathItem>> {
        let path = path.as_ref();
        let rel_path = path.strip_prefix(&base_path).unwrap();
        let (meta, meta2) = tokio::join!(fs::metadata(&path), fs::symlink_metadata(&path));
        let (meta, meta2) = (meta?, meta2?);
        let is_symlink = meta2.is_symlink();
        if !allow_symlinks
            && is_symlink
            && !self.is_root_contained(path, Path::new(directory)).await
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
            PathType::File | PathType::SymlinkFile => Some(if let Some(_key) = key {
                decrypted_size(meta.len())
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

    async fn handle_copymove(
        &self,
        path: &Path,
        req: Request,
        method: Method,
        res: &mut Response,
        dav_path: &str,
    ) -> BoxResult<()> {
        // get and check headers.
        let overwrite = req.headers().typed_get::<Overwrite>().map_or(true, |o| o.0);
        let depth = match req.headers().typed_get::<Depth>() {
            Some(Depth::Infinity) | None => Depth::Infinity,
            Some(Depth::Zero) if method.as_str() == "COPY" => Depth::Zero,
            _ => {
                *res.status_mut() = StatusCode::BAD_REQUEST;
                return Ok(());
            }
        };

        // decode and validate destination.
        let mut dest = match self.extract_dest(req.headers(), dav_path).await {
            Some(dest) => dest,
            None => {
                *res.status_mut() = StatusCode::FORBIDDEN;
                return Ok(());
            }
        };

        // Fails if we try to move a folder in place of the root directory itself
        if path.is_dir() && dest.path() == &PathBuf::from(dav_path) {
            *res.status_mut() = StatusCode::FORBIDDEN;
            return Ok(());
        }

        // Fails if collection parent does not exist
        if dest.path().parent().is_none() || !dest.path().parent().unwrap().exists() {
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
        if path.is_dir()
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
        if method.as_str() == "COPY" {
            Self::do_copy(path, dest.path(), dest.path(), dest.is_dir(), depth).await?;
            if overwrite && dest.exists() {
                *res.status_mut() = StatusCode::NO_CONTENT;
            } else {
                *res.status_mut() = StatusCode::CREATED;
            }
        } else {
            // if the source is a file but the destination is a directory, alter the destination
            if path.is_file() && dest.is_dir() {
                dest.push(path.file_name().unwrap().into());
            }
            if path.is_dir() && dest.is_file() && dest.exists() {
                *res.status_mut() = StatusCode::NO_CONTENT;
            } else {
                fs::rename(path, dest.path()).await?;
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
                    let destfile = dest.join(source.file_name().ok_or_else(|| {
                        Error::new(io::ErrorKind::Other, "could not extract file name")
                    })?);
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

            let mut retval = Ok(());
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
                if let Err(e) = Self::do_copy(&nsrc, topdest, &ndest, meta.is_dir(), depth).await {
                    retval = Err(e);
                }
            }

            retval
        }
        .boxed()
    }
}

#[derive(Debug, Serialize)]
struct IndexData {
    href: String,
    uri_prefix: String,
    paths: Vec<PathItem>,
    allow_upload: bool,
    allow_delete: bool,
    allow_search: bool,
    dir_exists: bool,
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
        let mut mtime = Utc
            .timestamp_millis_opt(self.mtime as i64)
            .unwrap()
            .to_rfc2822();
        mtime.truncate(mtime.len() - 6);
        let mtime = format!("{} GMT", mtime);
        let mut href = encode_uri(&format!("{}{}", prefix, &self.name));
        if self.is_dir() && !href.ends_with('/') {
            href.push('/');
        }
        let displayname = escape(self.base_name());
        match self.path_type {
            PathType::Dir | PathType::SymlinkDir => format!(
                r#"<D:response>
<D:href>{}</D:href>
<D:propstat>
<D:prop>
<D:displayname>{}</D:displayname>
<D:getlastmodified>{}</D:getlastmodified>
<D:resourcetype><D:collection/></D:resourcetype>
</D:prop>
<D:status>HTTP/1.1 200 OK</D:status>
</D:propstat>
</D:response>"#,
                href, displayname, mtime
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
                mtime
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
        .unwrap()
        .as_millis() as u64
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
    if let Some(parent) = path.parent() {
        if fs::symlink_metadata(parent).await.is_err() {
            fs::create_dir_all(&parent).await?;
        }
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
{}
</D:multistatus>"#,
        content,
    ));
}

async fn zip_dir<W: AsyncWrite + Unpin>(
    writer: &mut W,
    dir: &Path,
    key: Option<[u8; 32]>,
) -> BoxResult<()> {
    let mut writer = ZipFileWriter::with_tokio(writer);
    let mut walkdir = WalkDir::new(dir);
    while let Some(entry) = walkdir.next().await {
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

            let file = DavFile::open(&entry_path, key).await?;

            let builder = ZipEntryBuilder::new(filename.into(), Compression::Deflate);
            let entry_writer = writer.write_entry_stream(builder).await?;
            let mut entry_writer_compat = tokio_util::compat::FuturesAsyncWriteCompatExt::compat_write(entry_writer);

            file.copy_to(&mut entry_writer_compat).await?;

            entry_writer_compat.into_inner().close().await?;
        }
    }
    writer.close().await?;
    Ok(())
}

#[derive(Debug)]
struct RangeValue {
    start: u64,
    end: Option<u64>,
}

fn parse_range(headers: &HeaderMap<HeaderValue>) -> Option<RangeValue> {
    let range_hdr = headers.get(RANGE)?;
    let hdr = range_hdr.to_str().ok()?;
    let mut sp = hdr.splitn(2, '=');
    let units = sp.next().unwrap();
    if units == "bytes" {
        let range = sp.next()?;
        let mut sp_range = range.splitn(2, '-');
        let start: u64 = sp_range.next().unwrap().parse().ok()?;
        let end: Option<u64> = if let Some(end) = sp_range.next() {
            if end.is_empty() {
                None
            } else {
                Some(end.parse().ok()?)
            }
        } else {
            None
        };
        Some(RangeValue { start, end })
    } else {
        None
    }
}

fn status_forbid(res: &mut Response) {
    *res.status_mut() = StatusCode::FORBIDDEN;
    *res.body_mut() = Body::from("Forbidden");
}

fn status_not_found(res: &mut Response) {
    *res.status_mut() = StatusCode::NOT_FOUND;
    *res.body_mut() = Body::from("Not Found");
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

pub fn decode_uri(v: &str) -> Option<Cow<str>> {
    percent_encoding::percent_decode(v.as_bytes())
        .decode_utf8()
        .ok()
}

enum Destination {
    ExistingDir(PathBuf),
    DirToBe(PathBuf),
    ExistingFile(PathBuf),
    FileToBe(PathBuf),
}

impl Destination {
    async fn new(dest: PathBuf, is_new_dir: bool) -> Destination {
        match fs::symlink_metadata(&dest).await {
            Ok(meta) => {
                if meta.is_symlink() {
                    if let Ok(m) = fs::metadata(&dest).await {
                        if m.is_file() {
                            return Destination::ExistingFile(dest);
                        }
                        if m.is_dir() {
                            return Destination::ExistingDir(dest);
                        }
                    }
                }
                if meta.is_file() {
                    Destination::ExistingFile(dest)
                } else {
                    Destination::ExistingDir(dest)
                }
            }
            Err(_) => {
                if is_new_dir {
                    return Destination::DirToBe(dest);
                }
                Destination::FileToBe(dest)
            }
        }
    }

    fn exists(&self) -> bool {
        match self {
            Destination::ExistingDir(_) => true,
            Destination::DirToBe(_) => false,
            Destination::ExistingFile(_) => true,
            Destination::FileToBe(_) => false,
        }
    }

    fn is_dir(&self) -> bool {
        match self {
            Destination::ExistingDir(_) => true,
            Destination::DirToBe(_) => true,
            Destination::ExistingFile(_) => false,
            Destination::FileToBe(_) => false,
        }
    }

    fn is_file(&self) -> bool {
        !self.is_dir()
    }

    fn path(&self) -> &PathBuf {
        match self {
            Destination::ExistingDir(p) => p,
            Destination::DirToBe(p) => p,
            Destination::ExistingFile(p) => p,
            Destination::FileToBe(p) => p,
        }
    }

    fn push(&mut self, path: PathBuf) {
        match self {
            Destination::ExistingDir(p) => p.push(path),
            Destination::DirToBe(p) => p.push(path),
            Destination::ExistingFile(p) => p.push(path),
            Destination::FileToBe(p) => p.push(path),
        }
    }
}
