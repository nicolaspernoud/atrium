use crate::helpers::{encode_uri, TestApp};
use std::io::{self, BufWriter, Write};

use http::StatusCode;
use hyper::{header::RANGE, Method};
use tokio::fs::File;

use anyhow::Result;
use base64ct::{Base64, Encoding};
use futures::StreamExt;
use sha2::{Digest, Sha512};
use xml::escape::escape_str_pcdata;

#[tokio::test]
async fn put_and_retrieve_tests() -> Result<()> {
    let app = TestApp::spawn().await;
    put_and_get_file(&app, app.port, "lorem.txt", "files1", "text/plain", false).await?;
    put_and_get_file(&app, app.port, "lorem.txt", "files2", "text/plain", true).await?;

    let big_file_path = "tests/data/big_file.bin";
    create_big_binary_file(big_file_path);
    put_and_get_file(
        &app,
        app.port,
        "big_file.bin",
        "files1",
        "application/octet-stream",
        false,
    )
    .await?;
    put_and_get_file(
        &app,
        app.port,
        "big_file.bin",
        "files2",
        "application/octet-stream",
        true,
    )
    .await?;

    std::fs::remove_file(big_file_path).ok();
    Ok(())
}

async fn put_and_get_file(
    app: &TestApp,
    port: u16,
    file_name: &str,
    dav_server: &str,
    wanted_content: &str,
    encrypted: bool,
) -> Result<()> {
    let mut file = std::fs::File::open(format!("tests/data/{file_name}"))?;

    let mut hasher = Sha512::new();
    io::copy(&mut file, &mut hasher)?;
    let hash_source = hasher.finalize();
    println!("Source file hash: {}", Base64::encode_string(&hash_source));

    let file = File::open(format!("tests/data/{file_name}")).await?;
    // Act : send the file
    let resp = app
        .client
        .put(format!("http://{dav_server}.atrium.io:{port}/{file_name}"))
        .body(file_to_body(file))
        .send()
        .await?;
    assert_eq!(resp.status(), 201);

    let stored_file_path = if !encrypted {
        format!("data/{}/dir1/{file_name}", app.id)
    } else {
        format!("data/{}/dir2/{file_name}", app.id)
    };
    let mut stored_file = std::fs::File::open(stored_file_path)?;
    let mut hasher = Sha512::new();
    io::copy(&mut stored_file, &mut hasher)?;
    let hash_stored = hasher.finalize();
    println!("Stored file hash: {}", Base64::encode_string(&hash_stored));
    // Assert that the stored file is the same as the send file... or not if it it encrypted
    if !encrypted {
        assert_eq!(hash_source, hash_stored);
    } else {
        assert!(hash_source != hash_stored);
    }

    // Act : retrieve the file
    let resp = app
        .client
        .get(format!("http://{dav_server}.atrium.io:{port}/{file_name}"))
        .send()
        .await?;
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.headers().get("content-type").unwrap(), wanted_content);
    assert_eq!(resp.headers().get("accept-ranges").unwrap(), "bytes");
    assert!(resp.headers().contains_key("etag"));
    assert!(resp.headers().contains_key("last-modified"));
    assert!(resp.headers().contains_key("content-length"));
    let mut stream = resp.bytes_stream();

    let mut hasher = Sha512::new();
    while let Some(item) = stream.next().await {
        let chunk = item?;
        hasher.write_all(&chunk)?;
    }
    let hash_retrieved = hasher.finalize();
    println!(
        "Retrieved file hash: {}",
        Base64::encode_string(&hash_retrieved)
    );
    // Assert that the retrieved file is the same as the original file
    assert_eq!(hash_source, hash_retrieved);
    Ok(())
}

fn file_to_body(file: File) -> reqwest::Body {
    let stream = tokio_util::codec::FramedRead::new(file, tokio_util::codec::BytesCodec::new());
    let body = reqwest::Body::wrap_stream(stream);
    body
}

fn create_big_binary_file(path: &str) {
    let size = 100_000_000;

    std::fs::remove_file(path).ok();
    let f = std::fs::File::create(path).unwrap();
    let mut writer = BufWriter::new(f);

    let mut rng = rand::thread_rng();
    let mut buffer = [0; 1024];
    let mut remaining_size = size;

    while remaining_size > 0 {
        let to_write = std::cmp::min(remaining_size, buffer.len());
        let buffer = &mut buffer[..to_write];
        rand::Rng::fill(&mut rng, buffer);
        io::Write::write(&mut writer, buffer).unwrap();
        remaining_size -= to_write;
    }
}

#[tokio::test]
async fn get_correct_range() -> Result<()> {
    let app = TestApp::spawn().await;

    let cases = vec!["files1", "files2"];

    for case in cases.iter() {
        let file = File::open(format!("tests/data/lorem.txt")).await?;
        // Act : send the file
        let resp = app
            .client
            .put(format!("http://{case}.atrium.io:{}/{case}", app.port))
            .body(file_to_body(file))
            .send()
            .await?;
        assert_eq!(resp.status(), 201);

        // Act : retrieve the file
        let resp = app
            .client
            .get(format!("http://{case}.atrium.io:{}/{case}", app.port))
            .header(RANGE, "bytes=20000-20050")
            .send()
            .await?;
        assert_eq!(resp.status(), 206);
        assert_eq!(
            resp.text().await?,
            "estie vitae volutpat eget, aliquet ac ipsum. Quisqu"
        );
    }

    Ok(())
}

#[tokio::test]
async fn get_file_range_limit_cases() -> Result<()> {
    let app = TestApp::spawn().await;
    let url = format!(
        "http://files2.atrium.io:{}/get_file_range_limit_cases",
        app.port
    );
    app.client
        .put(&url)
        .body(b"abcdefghijklmnopqrstuvwxyz".to_vec())
        .send()
        .await?;
    let resp = app
        .client
        .get(&url)
        .header(RANGE, "bytes=20-40")
        .send()
        .await?;
    assert_eq!(resp.status(), 206);
    assert_eq!(
        resp.headers().get("content-range").unwrap(),
        "bytes 20-25/26"
    );
    assert_eq!(resp.headers().get("accept-ranges").unwrap(), "bytes");
    assert_eq!(resp.headers().get("content-length").unwrap(), "6");
    assert_eq!(resp.text().await?, "uvwxyz");
    let resp = app
        .client
        .get(&url)
        .header(RANGE, "bytes=30-")
        .send()
        .await?;
    assert_eq!(resp.status(), 416);
    assert_eq!(resp.headers().get("content-range").unwrap(), "bytes */26");
    Ok(())
}

#[tokio::test]
async fn try_to_hack() -> Result<()> {
    let app = TestApp::spawn().await;
    let mut dst = std::fs::File::create(format!("./data/{}/test.txt", app.id))
        .expect("could not create file");
    std::io::Write::write(&mut dst, b"This should not be accessible !!!")
        .expect("failed to write to file");
    let resp = app
        .client
        .get(format!("http://files1.atrium.io:{}/../test.txt", app.port))
        .send()
        .await?;
    assert_eq!(resp.status(), 404);
    Ok(())
}

#[tokio::test]
async fn try_to_use_wrong_key_to_decrypt() -> Result<()> {
    // Arrange
    let mut app = TestApp::spawn().await;

    // Act : send a file
    let url = format!("http://files2.atrium.io:{}/must_have_the_key", app.port);
    app.client
        .put(&url)
        .body(b"abcdefghijklmnopqrstuvwxyz".to_vec())
        .send()
        .await?;
    // Act : alter the key configuration file and reload
    let fp = format!("{}.yaml", &app.id);
    let mut src = std::fs::File::open(&fp).expect("failed to open config file");
    let mut data = String::new();
    std::io::Read::read_to_string(&mut src, &mut data).expect("failed to read config file");
    drop(src);
    let new_data = data.replace("ABCD123", "ABCDEFG");
    let mut dst = std::fs::File::create(&fp).expect("could not create file");
    std::io::Write::write(&mut dst, new_data.as_bytes()).expect("failed to write to file");
    app.client
        .get(format!("http://atrium.io:{}/reload", app.port))
        .send()
        .await
        .expect("failed to execute request");

    app.is_ready().await;

    // Assert that the file cannot be retrieved
    let resp = app.client.get(&url).send().await?;
    assert!(resp.bytes().await.is_err());

    Ok(())
}

#[tokio::test]
async fn get_dir_404() -> Result<()> {
    let app = TestApp::spawn().await;
    let resp = app
        .client
        .get(format!("http://files1.atrium.io:{}/404", app.port))
        .send()
        .await?;
    assert_eq!(resp.status(), 404);
    Ok(())
}

#[tokio::test]
async fn get_dir_zip() -> Result<()> {
    let app = TestApp::spawn().await;
    let resp = app
        .client
        .get(format!("http://files1.atrium.io:{}/dira", app.port))
        .send()
        .await?;
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers().get("content-type").unwrap(),
        "application/zip"
    );
    assert!(resp.headers().contains_key("content-disposition"));
    Ok(())
}

#[tokio::test]
async fn head_dir_zip() -> Result<()> {
    let app = TestApp::spawn().await;
    let resp = app
        .client
        .head(format!("http://files1.atrium.io:{}", app.port))
        .send()
        .await?;
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers().get("content-type").unwrap(),
        "application/zip"
    );
    assert!(resp.headers().contains_key("content-disposition"));
    assert_eq!(resp.text().await?, "");
    Ok(())
}

#[tokio::test]
async fn get_dir_search() -> Result<()> {
    let app = TestApp::spawn().await;

    let resp = app
        .client
        .get(format!(
            "http://files1.atrium.io:{}?q={}",
            app.port, "file1"
        ))
        .send()
        .await?;
    assert_eq!(resp.status(), 200);
    assert!(resp.text().await?.contains("file1"));
    Ok(())
}

#[tokio::test]
async fn get_dir_search_not_existing() -> Result<()> {
    let app = TestApp::spawn().await;
    let resp = app
        .client
        .get(format!(
            "http://files1.atrium.io:{}?q={}",
            app.port, "file3"
        ))
        .send()
        .await?;
    assert_eq!(resp.status(), 200);
    assert!(!resp.text().await?.contains("file3"));
    Ok(())
}

#[tokio::test]
async fn get_disk_usage() -> Result<()> {
    let app = TestApp::spawn().await;
    let resp = app
        .client
        .get(format!("http://files1.atrium.io:{}?diskusage", app.port))
        .send()
        .await?;
    assert_eq!(resp.status(), 200);
    let disk_info = resp.json::<atrium::sysinfo::DiskInfo>().await.unwrap();
    assert!(disk_info.total_space > 0);
    assert!(disk_info.available_space <= disk_info.total_space);
    Ok(())
}

#[tokio::test]
async fn get_file_404() -> Result<()> {
    let app = TestApp::spawn().await;
    let resp = app
        .client
        .get(format!("http://files1.atrium.io:{}/404", app.port))
        .send()
        .await?;
    assert_eq!(resp.status(), 404);
    Ok(())
}

#[tokio::test]
async fn head_file_404() -> Result<()> {
    let app = TestApp::spawn().await;
    let resp = app
        .client
        .head(format!("http://files1.atrium.io:{}/404", app.port))
        .send()
        .await?;
    assert_eq!(resp.status(), 404);
    Ok(())
}

#[tokio::test]
async fn options_dir() -> Result<()> {
    let app = TestApp::spawn().await;
    let resp = app
        .client
        .request(
            hyper::Method::OPTIONS,
            format!("http://files1.atrium.io:{}", app.port),
        )
        .send()
        .await?;
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers().get("allow").unwrap(),
        "GET,HEAD,PUT,OPTIONS,DELETE,PROPFIND,COPY,MOVE"
    );
    assert_eq!(resp.headers().get("dav").unwrap(), "1,2");
    Ok(())
}

#[tokio::test]
async fn put_file() -> Result<()> {
    let app = TestApp::spawn().await;
    let url = format!("http://files1.atrium.io:{}/myfile", app.port);
    let resp = app.client.put(&url).body(b"abc".to_vec()).send().await?;
    assert_eq!(resp.status(), 201);
    let resp = app.client.get(url).send().await?;
    assert_eq!(resp.status(), 200);
    Ok(())
}

#[tokio::test]
async fn put_file_not_writable() -> Result<()> {
    let app = TestApp::spawn().await;
    let url = format!("http://files3.atrium.io:{}/myfile", app.port);
    let resp = app.client.put(&url).body(b"abc".to_vec()).send().await?;
    assert_eq!(resp.status(), 403);
    let resp = app.client.get(url).send().await?;
    assert_eq!(resp.status(), 404);
    Ok(())
}

#[tokio::test]
async fn put_file_create_dir() -> Result<()> {
    let app = TestApp::spawn().await;
    let url = format!(
        "http://files1.atrium.io:{}/xyz/my_file_in_dir.txt",
        app.port
    );
    let resp = app.client.put(&url).body(b"abc".to_vec()).send().await?;
    assert_eq!(resp.status(), 201);
    let resp = app.client.get(url).send().await?;
    assert_eq!(resp.status(), 200);
    Ok(())
}

#[tokio::test]
async fn put_file_conflict_dir() -> Result<()> {
    let app = TestApp::spawn().await;
    let resp = app
        .client
        .put(format!("http://files1.atrium.io:{}/dira", app.port))
        .body(b"abc".to_vec())
        .send()
        .await?;
    assert_eq!(resp.status(), 403);
    Ok(())
}

#[tokio::test]
async fn put_file_alter_modtime() -> Result<()> {
    let app = TestApp::spawn().await;
    let url = format!("http://files1.atrium.io:{}/myfile", app.port);
    let resp = app
        .client
        .put(&url)
        .body(b"abc".to_vec())
        .header("X-OC-Mtime", "405659700")
        .send()
        .await?;
    assert_eq!(resp.status(), 201);
    let resp = app.client.get(&url).send().await?;
    assert_eq!(resp.status(), 200);
    let resp = propfind(&app, &url).send().await?;
    assert_eq!(resp.status(), 207);
    let body = resp.text().await?;
    assert!(body.contains("<D:getlastmodified>Tue, 09 Nov 1982 03:15:00 GMT</D:getlastmodified>"));
    Ok(())
}

#[tokio::test]
async fn delete_file() -> Result<()> {
    let app = TestApp::spawn().await;
    let url = format!(
        "http://files1.atrium.io:{}/xyz/file_to_delete.txt",
        app.port
    );
    app.client.put(&url).body(b"abc".to_vec()).send().await?;
    let resp = app.client.delete(&url).send().await?;
    assert_eq!(resp.status(), 204);
    let resp = app.client.get(url).send().await?;
    assert_eq!(resp.status(), 404);
    Ok(())
}

#[tokio::test]
async fn delete_file_404() -> Result<()> {
    let app = TestApp::spawn().await;
    let resp = app
        .client
        .delete(format!("http://files1.atrium.io:{}/file3", app.port))
        .send()
        .await?;
    assert_eq!(resp.status(), 404);
    Ok(())
}

fn propfind(app: &TestApp, url: &str) -> reqwest::RequestBuilder {
    app.client
        .request(Method::from_bytes(b"PROPFIND").unwrap(), url)
}
fn proppatch(app: &TestApp, url: &str) -> reqwest::RequestBuilder {
    app.client
        .request(Method::from_bytes(b"PROPPATCH").unwrap(), url)
}
fn mkcol(app: &TestApp, url: &str) -> reqwest::RequestBuilder {
    app.client
        .request(Method::from_bytes(b"MKCOL").unwrap(), url)
}
fn copy(app: &TestApp, url: &str) -> reqwest::RequestBuilder {
    app.client
        .request(Method::from_bytes(b"COPY").unwrap(), url)
}
fn mv(app: &TestApp, url: &str) -> reqwest::RequestBuilder {
    app.client
        .request(Method::from_bytes(b"MOVE").unwrap(), url)
}
fn lock(app: &TestApp, url: &str) -> reqwest::RequestBuilder {
    app.client
        .request(Method::from_bytes(b"LOCK").unwrap(), url)
}
fn unlock(app: &TestApp, url: &str) -> reqwest::RequestBuilder {
    app.client
        .request(Method::from_bytes(b"UNLOCK").unwrap(), url)
}

#[tokio::test]
async fn propfind_dir() -> Result<()> {
    let app = TestApp::spawn().await;
    let url = format!("http://files1.atrium.io:{}/dira", app.port);
    let resp = propfind(&app, &url).send().await?;
    assert_eq!(resp.status(), 207);
    let body = resp.text().await?;
    assert!(body.contains("<D:href>/dira/</D:href>"));
    assert!(body.contains("<D:displayname>dira</D:displayname>"));
    assert!(body.contains("<D:getcontentlength>0</D:getcontentlength>"));
    for f in vec!["file1", "file2"] {
        assert!(body.contains(&format!("<D:href>/dira/{}</D:href>", encode_uri(f))));
        assert!(body.contains(&format!(
            "<D:displayname>{}</D:displayname>",
            escape_str_pcdata(f)
        )));
    }
    Ok(())
}

#[tokio::test]
async fn propfind_dir_depth0() -> Result<()> {
    let app = TestApp::spawn().await;
    let url = format!("http://files1.atrium.io:{}/dira", app.port);
    let resp = propfind(&app, &url).header("depth", "0").send().await?;
    assert_eq!(resp.status(), 207);
    let body = resp.text().await?;
    assert!(body.contains("<D:href>/dira/</D:href>"));
    assert!(body.contains("<D:displayname>dira</D:displayname>"));
    assert_eq!(
        body.lines()
            .filter(|v| *v == "<D:status>HTTP/1.1 200 OK</D:status>")
            .count(),
        1
    );
    Ok(())
}

#[tokio::test]
async fn propfind_404() -> Result<()> {
    let app = TestApp::spawn().await;
    let url = format!("http://files1.atrium.io:{}/404", app.port);
    let resp = propfind(&app, &url).send().await?;
    assert_eq!(resp.status(), 404);
    Ok(())
}

#[tokio::test]
async fn propfind_file() -> Result<()> {
    let app = TestApp::spawn().await;
    let url = format!("http://files1.atrium.io:{}/dira/file1", app.port);
    let resp = propfind(&app, &url).send().await?;
    assert_eq!(resp.status(), 207);
    let body = resp.text().await?;
    assert!(body.contains("<D:href>/dira/file1</D:href>"));
    assert!(body.contains("<D:getcontentlength>0</D:getcontentlength>"));
    assert!(body.contains("<D:displayname>file1</D:displayname>"));
    assert_eq!(
        body.lines()
            .filter(|v| *v == "<D:status>HTTP/1.1 200 OK</D:status>")
            .count(),
        1
    );
    Ok(())
}

#[tokio::test]
async fn propfind_file_encrypted() -> Result<()> {
    let app = TestApp::spawn().await;
    let url = format!("http://files2.atrium.io:{}/dira/file1", app.port);
    app.client.put(&url).body(b"abc".to_vec()).send().await?;
    let resp = propfind(&app, &url).send().await?;
    assert_eq!(resp.status(), 207);
    let body = resp.text().await?;
    assert!(body.contains("<D:href>/dira/file1</D:href>"));
    assert!(body.contains("<D:getcontentlength>3</D:getcontentlength>"));
    assert!(body.contains("<D:displayname>file1</D:displayname>"));
    assert_eq!(
        body.lines()
            .filter(|v| *v == "<D:status>HTTP/1.1 200 OK</D:status>")
            .count(),
        1
    );
    // Test on dir
    let resp = propfind(&app, &format!("http://files2.atrium.io:{}/dira", app.port))
        .send()
        .await?;
    let body = resp.text().await?;
    assert!(body.contains("<D:href>/dira/file1</D:href>"));
    assert!(body.contains("<D:getcontentlength>3</D:getcontentlength>"));
    Ok(())
}

#[tokio::test]
async fn proppatch_file() -> Result<()> {
    let app = TestApp::spawn().await;
    let url = format!("http://files1.atrium.io:{}/dira/file1", app.port);
    let resp = proppatch(&app, &url).send().await?;
    assert_eq!(resp.status(), 207);
    let body = resp.text().await?;
    assert!(body.contains("<D:href>/dira/file1</D:href>"));
    Ok(())
}

#[tokio::test]
async fn proppatch_404() -> Result<()> {
    let app = TestApp::spawn().await;
    let url = format!("http://files1.atrium.io:{}/404", app.port);
    let resp = proppatch(&app, &url).send().await?;

    assert_eq!(resp.status(), 404);
    Ok(())
}

#[tokio::test]
async fn mkcol_dir() -> Result<()> {
    let app = TestApp::spawn().await;
    let url = format!("http://files1.atrium.io:{}/newdir", app.port);
    let resp = mkcol(&app, &url).send().await?;
    assert_eq!(resp.status(), 201);
    Ok(())
}

#[tokio::test]
async fn mkcol_not_writable() -> Result<()> {
    let app = TestApp::spawn().await;
    let url = format!("http://files3.atrium.io:{}/newdir", app.port);
    let resp = mkcol(&app, &url).send().await?;
    assert_eq!(resp.status(), 403);
    Ok(())
}

#[tokio::test]
async fn copy_file() -> Result<()> {
    let app = TestApp::spawn().await;
    let url = format!("http://files1.atrium.io:{}/dira/file1", app.port);
    let new_url = format!("http://files1.atrium.io:{}/dira/file1%20(copy)", app.port);
    let resp = copy(&app, &url)
        .header("Destination", &new_url)
        .send()
        .await?;
    assert_eq!(resp.status(), 201);
    let resp = app.client.get(new_url).send().await?;
    assert_eq!(resp.status(), 200);
    Ok(())
}

#[tokio::test]
async fn copy_dir() -> Result<()> {
    let app = TestApp::spawn().await;
    let url = format!("http://files1.atrium.io:{}/dira/", app.port);
    let new_url = format!("http://files1.atrium.io:{}/newdir/", app.port);
    let resp = copy(&app, &url)
        .header("Destination", &new_url)
        .send()
        .await?;
    assert_eq!(resp.status(), 201);
    let mut test_url = format!("http://files1.atrium.io:{}/newdir/subdira/file1", app.port);
    let resp = app.client.get(test_url).send().await?;
    assert_eq!(resp.status(), 200);
    test_url = format!("http://files1.atrium.io:{}/newdir/file1", app.port);
    let resp = app.client.get(test_url).send().await?;
    assert_eq!(resp.status(), 200);
    Ok(())
}

#[tokio::test]
async fn copy_not_writable() -> Result<()> {
    let app = TestApp::spawn().await;
    let url = format!("http://files3.atrium.io:{}/dira/file1", app.port);
    let new_url = format!("http://files3.atrium.io:{}/dira/file1%20(copy)", app.port);
    let resp = copy(&app, &url)
        .header("Destination", &new_url)
        .send()
        .await?;
    assert_eq!(resp.status(), 403);
    let resp = app.client.get(new_url).send().await?;
    assert_eq!(resp.status(), 404);
    Ok(())
}

#[tokio::test]
async fn copy_file_404() -> Result<()> {
    let app = TestApp::spawn().await;
    let url = format!("http://files1.atrium.io:{}/dira/file3", app.port);
    let new_url = format!("http://files1.atrium.io:{}/dira/file3%20(copy)", app.port);
    let resp = copy(&app, &url)
        .header("Destination", new_url)
        .send()
        .await?;
    assert_eq!(resp.status(), 404);
    Ok(())
}

#[tokio::test]
async fn move_file() -> Result<()> {
    let app = TestApp::spawn().await;
    let origin_url = format!("http://files1.atrium.io:{}/dira/file2", app.port);
    let new_url = format!("http://files1.atrium.io:{}/dira/file2%20(moved)", app.port);
    let resp = mv(&app, &origin_url)
        .header("Destination", &new_url)
        .send()
        .await?;
    assert_eq!(resp.status(), 201);
    let resp = app.client.get(new_url).send().await?;
    assert_eq!(resp.status(), 200);
    let resp = app.client.get(origin_url).send().await?;
    assert_eq!(resp.status(), 404);
    Ok(())
}

#[tokio::test]
async fn move_file_to_dir() -> Result<()> {
    let app = TestApp::spawn().await;
    let origin_url = format!("http://files1.atrium.io:{}/dira/file2", app.port);
    let new_url = format!("http://files1.atrium.io:{}/dirb/", app.port);
    let resp = mv(&app, &origin_url)
        .header("Destination", &new_url)
        .send()
        .await?;
    assert_eq!(resp.status(), 201);
    let resp = app.client.get(format!("{new_url}file2")).send().await?;
    assert_eq!(resp.status(), 200);
    let resp = app.client.get(origin_url).send().await?;
    assert_eq!(resp.status(), 404);
    Ok(())
}

#[tokio::test]
async fn move_dir() -> Result<()> {
    let app = TestApp::spawn().await;
    let url = format!("http://files1.atrium.io:{}/dira/", app.port);
    let new_url = format!("http://files1.atrium.io:{}/newdir/", app.port);
    let resp = mv(&app, &url)
        .header("Destination", &new_url)
        .send()
        .await?;
    assert_eq!(resp.status(), 201);
    let mut test_url = format!("http://files1.atrium.io:{}/newdir/subdira/file1", app.port);
    let resp = app.client.get(test_url).send().await?;
    assert_eq!(resp.status(), 200);
    test_url = format!("http://files1.atrium.io:{}/newdir/file1", app.port);
    let resp = app.client.get(test_url).send().await?;
    assert_eq!(resp.status(), 200);
    test_url = format!("http://files1.atrium.io:{}/dira/file1", app.port);
    let resp = app.client.get(test_url).send().await?;
    assert_eq!(resp.status(), 404);
    Ok(())
}

#[tokio::test]
async fn move_file_not_writable() -> Result<()> {
    let app = TestApp::spawn().await;
    let origin_url = format!("http://files3.atrium.io:{}/dira/file2", app.port);
    let new_url = format!("http://files3.atrium.io:{}/dira/file2%20(moved)", app.port);
    let resp = mv(&app, &origin_url)
        .header("Destination", &new_url)
        .send()
        .await?;
    assert_eq!(resp.status(), 403);
    let resp = app.client.get(new_url).send().await?;
    assert_eq!(resp.status(), 404);
    let resp = app.client.get(origin_url).send().await?;
    assert_eq!(resp.status(), 200);
    Ok(())
}

#[tokio::test]
async fn move_file_404() -> Result<()> {
    let app = TestApp::spawn().await;
    let url = format!("http://files1.atrium.io:{}/file3", app.port);
    let new_url = format!("http://files1.atrium.io:{}/file3%20(moved)", app.port);
    let resp = mv(&app, &url).header("Destination", new_url).send().await?;
    assert_eq!(resp.status(), 404);
    Ok(())
}

#[tokio::test]
async fn lock_file() -> Result<()> {
    let app = TestApp::spawn().await;
    let url = format!("http://files1.atrium.io:{}/dira/file1", app.port);
    let resp = lock(&app, &url).send().await?;
    assert_eq!(resp.status(), 200);
    let body = resp.text().await?;
    assert!(body.contains("<D:href>/dira/file1</D:href>"));
    Ok(())
}

#[tokio::test]
async fn lock_file_404() -> Result<()> {
    let app = TestApp::spawn().await;
    let url = format!("http://files1.atrium.io:{}/file3", app.port);
    let resp = lock(&app, &url).send().await?;
    assert_eq!(resp.status(), 404);
    Ok(())
}

#[tokio::test]
async fn unlock_file() -> Result<()> {
    let app = TestApp::spawn().await;
    let url = format!("http://files1.atrium.io:{}/dira/file1", app.port);
    let resp = unlock(&app, &url).send().await?;
    assert_eq!(resp.status(), 200);
    Ok(())
}

#[tokio::test]
async fn unlock_file_404() -> Result<()> {
    let app = TestApp::spawn().await;
    let url = format!("http://files1.atrium.io:{}/file3", app.port);
    let resp = unlock(&app, &url).send().await?;
    assert_eq!(resp.status(), 404);
    Ok(())
}

#[cfg(unix)]
use std::os::unix::fs::symlink as symlink_dir;
#[cfg(windows)]
use std::os::windows::fs::symlink_dir;

#[tokio::test]
async fn default_not_allow_symlinks() -> Result<()> {
    let app = TestApp::spawn().await;
    std::fs::create_dir_all(format!("./data/{}/dir_symlink", app.id))?;
    std::fs::write(
        format!("./data/{}/dir_symlink/file1", app.id),
        b"Lorem ipsum",
    )?;
    let srcdir = std::fs::canonicalize(std::path::PathBuf::from(format!(
        "./data/{}/dir_symlink",
        app.id
    )))
    .expect("couldn't canonicalize path");
    symlink_dir(srcdir, format!("./data/{}/dir1/dirc", app.id)).expect("couldn't create symlink");
    let resp = app
        .client
        .get(format!("http://files1.atrium.io:{}/dirc", app.port))
        .send()
        .await?;
    assert_eq!(resp.status(), 404);
    let resp = app
        .client
        .get(format!("http://files1.atrium.io:{}/dirc/file1", app.port))
        .send()
        .await?;
    assert_eq!(resp.status(), 404);
    Ok(())
}

#[tokio::test]
async fn allow_symlinks() -> Result<()> {
    let app = TestApp::spawn().await;
    std::fs::create_dir_all(format!("./data/{}/dir_symlink", app.id))?;
    std::fs::write(
        format!("./data/{}/dir_symlink/file1", app.id),
        b"Lorem ipsum",
    )?;
    let srcdir = std::fs::canonicalize(std::path::PathBuf::from(format!(
        "./data/{}/dir_symlink",
        app.id
    )))
    .expect("couldn't canonicalize path");
    symlink_dir(srcdir, format!("./data/{}/dir3/dirc", app.id)).expect("couldn't create symlink");
    let resp = app
        .client
        .get(format!("http://files3.atrium.io:{}/dirc", app.port))
        .send()
        .await?;
    assert_eq!(resp.status(), 200);
    let resp = app
        .client
        .get(format!("http://files3.atrium.io:{}/dirc/file1", app.port))
        .send()
        .await?;
    assert_eq!(resp.status(), 200);
    Ok(())
}

#[tokio::test]
async fn secured_dav_test() {
    // Arrange
    let app = TestApp::spawn().await;

    // Act : try to access app as unlogged user
    let response = app
        .client
        .get(format!("http://secured-files.atrium.io:{}", app.port))
        .send()
        .await
        .expect("failed to execute request");

    // Assert that is impossible
    assert!(response.status() == 401);
    assert_eq!(response.text().await.unwrap(), "");

    // Log as normal user
    let response = app
        .client
        .post(format!("http://atrium.io:{}/auth/local", app.port))
        .body(r#"{"login":"user","password":"password"}"#)
        .header("Content-Type", "application/json")
        .send()
        .await
        .expect("failed to execute request");
    assert!(response.status().is_success());
    // Get XSRF token from response
    let xsrf_token: String = response
        .json::<atrium::users::AuthResponse>()
        .await
        .unwrap()
        .xsrf_token;
    // Act : try to access app as logged user
    let response = app
        .client
        .get(format!("http://secured-files.atrium.io:{}", app.port))
        .header("xsrf-token", &xsrf_token)
        .send()
        .await
        .expect("failed to execute request");
    // Assert that is impossible
    assert!(response.status() == 403);

    // Log as admin
    let response = app
        .client
        .post(format!("http://atrium.io:{}/auth/local", app.port))
        .body(r#"{"login":"admin","password":"password"}"#)
        .header("Content-Type", "application/json")
        .send()
        .await
        .expect("failed to execute request");
    assert!(response.status().is_success());
    // Get XSRF token from response
    let xsrf_token: String = response
        .json::<atrium::users::AuthResponse>()
        .await
        .unwrap()
        .xsrf_token;
    // Act : try to access app as admin without XSRF token
    let response = app
        .client
        .get(format!("http://secured-files.atrium.io:{}", app.port))
        .send()
        .await
        .expect("failed to execute request");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    // Act : try to access app as admin with a wrong XSRF token
    let response = app
        .client
        .get(format!("http://secured-files.atrium.io:{}", app.port))
        .header("xsrf-token", "randomtoken")
        .send()
        .await
        .expect("failed to execute request");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    // Act : try to access app as admin
    let response = app
        .client
        .get(format!("http://secured-files.atrium.io:{}", app.port))
        .header("xsrf-token", &xsrf_token)
        .send()
        .await
        .expect("failed to execute request");
    // Assert that is possible
    assert!(response.status().is_success());
}

#[tokio::test]
async fn secured_dav_basic_auth_and_token_test() {
    // Arrange
    let app = TestApp::spawn().await;

    // Create a client without cookie store
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .resolve(
            "atrium.io",
            format!("127.0.0.1:{}", app.port).parse().unwrap(),
        )
        .resolve(
            "secured-files.atrium.io",
            format!("127.0.0.1:{}", app.port).parse().unwrap(),
        )
        .cookie_store(false)
        .build()
        .unwrap();

    // Log as admin
    let response = client
        .post(format!("http://atrium.io:{}/auth/local", app.port))
        .body(r#"{"login":"admin","password":"password"}"#)
        .header("Content-Type", "application/json")
        .send()
        .await
        .expect("failed to execute request");
    assert!(response.status().is_success());
    // Get the token from the cookie
    let token = response.headers().get("set-cookie").unwrap();
    let token = token.to_str().unwrap().to_owned();
    let token = token.split(";").collect::<Vec<_>>()[0]
        .split("=")
        .collect::<Vec<_>>()[1];
    let bauth = format!("dummy:{token}");

    // Try to access app : must fail
    let response = client
        .get(format!("http://secured-files.atrium.io:{}", app.port))
        .send()
        .await
        .expect("failed to execute request");
    assert!(response.status() == 401);
    // Try to access app with the token passed as basic auth : must succeed
    let response = client
        .get(format!("http://secured-files.atrium.io:{}", app.port))
        .header(
            "Authorization",
            format!(
                "Basic {}",
                base64ct::Base64::encode_string(bauth.as_bytes())
            ),
        )
        .send()
        .await
        .expect("failed to execute request");
    assert!(response.status().is_success());
    // Try to access app with the token passed as query : must succeed
    let response = client
        .get(format!(
            "http://secured-files.atrium.io:{}?token={}",
            app.port, token
        ))
        .send()
        .await
        .expect("failed to execute request");
    assert!(response.status().is_success());
    // Try to access app with the login and password passed as basic auth : must succeed
    let response = client
        .get(format!("http://secured-files.atrium.io:{}", app.port))
        .header(
            "Authorization",
            format!(
                "Basic {}",
                base64ct::Base64::encode_string("admin:password".as_bytes())
            ),
        )
        .send()
        .await
        .expect("failed to execute request");
    assert!(response.status().is_success());
    // Try to access app with the login and a WRONG password passed as basic auth : must fail
    let response = client
        .get(format!("http://secured-files.atrium.io:{}", app.port))
        .header(
            "Authorization",
            format!(
                "Basic {}",
                base64ct::Base64::encode_string("admin:badpassword".as_bytes())
            ),
        )
        .send()
        .await
        .expect("failed to execute request");
    assert!(response.status() == StatusCode::UNAUTHORIZED);
}
