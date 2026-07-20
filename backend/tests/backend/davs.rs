use crate::helpers::{TestApp, encode_uri, login_and_get_xsrf_token};
use base64ct::{Base64, Encoding};
use futures::StreamExt;
use http::StatusCode;
use hyper::{Method, header::RANGE};
use quick_xml::escape::escape;
use sha2::{Digest, Sha512};
use std::{
    fs,
    io::{self, BufWriter, Read},
    time::{Duration, Instant},
};
use tokio::fs::File;

type BoxResult<T> = Result<T, Box<dyn std::error::Error>>;

#[tokio::test]
async fn put_and_retrieve_tests() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
    put_and_get_file(&app, app.port, "lorem.txt", "files1", "text/plain", false).await?;
    put_and_get_file(&app, app.port, "lorem.txt", "files2", "text/plain", true).await?;

    let big_file_path = "tests/data/big_file.bin";
    create_big_binary_file(100_000_000, big_file_path);
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

// Run with `cargo test --release --package atrium --test backend -- davs::sized_files_bench --exact --nocapture --ignored |grep "→"`
#[tokio::test]
#[ignore]
async fn sized_files_bench() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;

    let file_sizes_mb = [1, 10, 100, 500, 1000, 3000];

    for size in file_sizes_mb.iter() {
        // Create a file
        let sized_file_name = "sized_file.bin";
        let sized_file_path = &format!("tests/data/{sized_file_name}");
        let downloaded_file_path = "tests/data/downloaded_sized_file.bin";
        create_big_binary_file(size * 1_000_000, sized_file_path);

        // Reference : file copy
        let before = Instant::now();
        std::fs::copy(sized_file_path, downloaded_file_path)?;
        println!(
            "=== Reference: file copy of size {size} Mb in {:.2?} s → {:.2?} Mb/s",
            before.elapsed().as_secs_f32(),
            *size as f32 / before.elapsed().as_secs_f32()
        );

        for dav in ["files1", "files2"] {
            let encrypted = if dav == "files2" { " (encrypted)" } else { "" };
            // Send the file
            let before = Instant::now();
            let file = File::open(sized_file_path).await?;
            let resp = app
                .client
                .put(format!(
                    "http://{dav}.atrium.io:{}/{sized_file_name}",
                    app.port
                ))
                .body(file_to_body(file))
                .send()
                .await?;
            assert_eq!(resp.status(), 201);
            println!(
                "Sent file of size {size} Mb to {dav}{encrypted} in {:.2?} s → {:.2?} Mb/s",
                before.elapsed().as_secs_f32(),
                *size as f32 / before.elapsed().as_secs_f32()
            );

            let before = Instant::now();
            let resp = app
                .client
                .get(format!(
                    "http://{dav}.atrium.io:{}/{sized_file_name}",
                    app.port
                ))
                .send()
                .await?;
            assert_eq!(resp.status(), 200);
            let mut file = std::fs::File::create(downloaded_file_path)?;
            let mut content = io::Cursor::new(resp.bytes().await?);
            std::io::copy(&mut content, &mut file)?;
            println!(
                "Retrieved file of size {size} Mb from {dav}{encrypted} in {:.2?} s → {:.2?} Mb/s",
                before.elapsed().as_secs_f32(),
                *size as f32 / before.elapsed().as_secs_f32()
            );
        }
        std::fs::remove_file(sized_file_path).ok();
        std::fs::remove_file(downloaded_file_path).ok();
    }
    Ok(())
}

async fn put_and_get_file(
    app: &TestApp,
    port: u16,
    file_name: &str,
    dav_server: &str,
    wanted_content: &str,
    encrypted: bool,
) -> BoxResult<()> {
    let file = std::fs::File::open(format!("tests/data/{file_name}"))?;

    let hash_source = file_hash(file)?;
    println!("Source file hash: {}", hash_source);

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
    let stored_file = std::fs::File::open(stored_file_path)?;
    let hash_stored = file_hash(stored_file)?;
    println!("Stored file hash: {}", hash_stored);
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
        hasher.update(&chunk);
    }
    let hash_retrieved = Base64::encode_string(&hasher.finalize());
    println!("Retrieved file hash: {}", hash_retrieved);
    // Assert that the retrieved file is the same as the original file
    assert_eq!(hash_source, hash_retrieved);
    Ok(())
}

fn file_hash(mut file: fs::File) -> Result<String, io::Error> {
    let mut hasher = Sha512::new();
    let mut buffer = [0u8; 8192];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    let hash_source = hasher.finalize();
    let hash_source = Base64::encode_string(&hash_source);
    Ok(hash_source)
}

fn file_to_body(file: File) -> reqwest::Body {
    let stream = tokio_util::codec::FramedRead::new(file, tokio_util::codec::BytesCodec::new());
    reqwest::Body::wrap_stream(stream)
}

fn create_big_binary_file(size: usize, path: &str) {
    std::fs::remove_file(path).ok();
    let f = std::fs::File::create(path).unwrap();
    let mut writer = BufWriter::new(f);

    let mut rng = rand::rng();
    let mut buffer = [0; 1024];
    let mut remaining_size = size;

    while remaining_size > 0 {
        let to_write = std::cmp::min(remaining_size, buffer.len());
        let buffer = &mut buffer[..to_write];
        rand::RngExt::fill(&mut rng, buffer);
        io::Write::write(&mut writer, buffer).unwrap();
        remaining_size -= to_write;
    }
}

#[tokio::test]
async fn get_correct_range() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;

    let cases = ["files1", "files2"];

    for case in cases.iter() {
        let file = File::open("tests/data/lorem.txt").await?;
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

        let file_content = fs::read("tests/data/lorem.txt")?;
        let expected_content = &file_content[20000..=20050];
        assert_eq!(resp.bytes().await?, expected_content);
    }

    Ok(())
}

#[tokio::test]
async fn get_file_range_limit_cases() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
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
async fn check_lock_for_all_write_methods() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
    let url_file = format!("http://files1.atrium.io:{}/locked_file.txt", app.port);
    let url_dir = format!("http://files1.atrium.io:{}/locked_dir/", app.port);

    // Setup: Create a file to lock
    let resp = app
        .client
        .put(&url_file)
        .body(b"initial content".to_vec())
        .send()
        .await?;
    assert_eq!(resp.status(), 201);

    // Setup: Create a directory
    let resp = mkcol(&app, &url_dir).send().await?;
    assert_eq!(resp.status(), 201);

    // Lock the file
    let lock_resp = lock(&app, &url_file).send().await?;
    assert_eq!(lock_resp.status(), 200);
    let lock_token = lock_resp
        .headers()
        .get("lock-token")
        .unwrap()
        .to_str()?
        .to_owned();

    // Lock the directory
    let dir_lock_resp = lock(&app, &url_dir).send().await?;
    assert_eq!(dir_lock_resp.status(), 200);
    let dir_lock_token = dir_lock_resp
        .headers()
        .get("lock-token")
        .unwrap()
        .to_str()?
        .to_owned();

    // 1. PUT on locked file (should fail with 423 Locked without correct lock token, succeed with correct token)
    let resp_put_fail = app
        .client
        .put(&url_file)
        .body(b"new content".to_vec())
        .send()
        .await?;
    assert_eq!(resp_put_fail.status(), StatusCode::LOCKED);

    let resp_put_ok = app
        .client
        .put(&url_file)
        .header("If", format!("({})", lock_token))
        .body(b"new content".to_vec())
        .send()
        .await?;
    assert_eq!(resp_put_ok.status(), 201);

    // 2. DELETE on locked file (should fail with 423 Locked without correct lock token, succeed with correct token)
    let resp_del_fail = app.client.delete(&url_file).send().await?;
    assert_eq!(resp_del_fail.status(), StatusCode::LOCKED);

    // 3. PROPPATCH on locked file (should fail with 423 Locked without correct lock token, succeed with correct token)
    let prop_body = r#"
        <?xml version="1.0" encoding="utf-8" ?>
        <D:propertyupdate xmlns:D="DAV:">
            <D:set>
                <D:prop>
                    <lastmodified xmlns="DAV:">405659400</lastmodified>
                </D:prop>
            </D:set>
        </D:propertyupdate>
    "#;
    let resp_prop_fail = proppatch(&app, &url_file).body(prop_body).send().await?;
    assert_eq!(resp_prop_fail.status(), StatusCode::LOCKED);

    let resp_prop_ok = proppatch(&app, &url_file)
        .header("If", format!("({})", lock_token))
        .body(prop_body)
        .send()
        .await?;
    assert_eq!(resp_prop_ok.status(), 207);

    // 4. MKCOL under a path (should fail with 423 Locked if that exact path is locked)
    let url_locked_miss = format!("{}locked_miss", url_dir);

    // Create a lock on the missing resource (creates empty locked resource)
    let lock_miss_resp = lock(&app, &url_locked_miss).send().await?;
    assert_eq!(lock_miss_resp.status(), 201); // Created lock on unexisting resource
    let lock_miss_token = lock_miss_resp
        .headers()
        .get("lock-token")
        .unwrap()
        .to_str()?
        .to_owned();

    // Trying to MKCOL at url_locked_miss without correct lock tokens should fail (due to lock_miss_token or dir_lock_token parent lock)
    let resp_mkcol_fail = mkcol(&app, &url_locked_miss).send().await?;
    assert_eq!(resp_mkcol_fail.status(), StatusCode::LOCKED);

    // MKCOL fails on a locked-miss if we have only one of the two required lock tokens (either the resource lock token or the parent directory lock token)
    let resp_mkcol_fail = mkcol(&app, &url_locked_miss)
        .header("If", format!("({})", lock_miss_token))
        .send()
        .await?;
    assert_eq!(resp_mkcol_fail.status(), StatusCode::LOCKED);

    let resp_mkcol_fail = mkcol(&app, &url_locked_miss)
        .header("If", format!("({})", dir_lock_token))
        .send()
        .await?;
    assert_eq!(resp_mkcol_fail.status(), StatusCode::LOCKED);

    // MKCOL succeeds on a locked-miss with the correct lock tokens (the resource lock token and parent directory lock token both provided in the If header!)
    let resp_mkcol_ok = mkcol(&app, &url_locked_miss)
        .header("If", format!("({}) ({})", lock_miss_token, dir_lock_token))
        .send()
        .await?;
    assert_eq!(resp_mkcol_ok.status(), 201);

    // 5. COPY with destination locked (should fail with 423 Locked if destination is locked and token is missing, succeed with correct token)
    let src_file = format!("http://files1.atrium.io:{}/src_file.txt", app.port);
    app.client
        .put(&src_file)
        .body(b"src content".to_vec())
        .send()
        .await?;

    let resp_copy_fail = copy(&app, &src_file)
        .header("Destination", &url_file)
        .send()
        .await?;
    assert_eq!(resp_copy_fail.status(), StatusCode::LOCKED);

    let resp_copy_ok = copy(&app, &src_file)
        .header("Destination", &url_file)
        .header("If", format!("({})", lock_token))
        .send()
        .await?;
    assert_eq!(resp_copy_ok.status(), 204); // Overwrite was true (default)

    // 6. MOVE with source and destination locked (should fail with 423 Locked if either is locked, succeed with correct tokens)
    let other_locked_file = format!("http://files1.atrium.io:{}/other_locked.txt", app.port);
    app.client
        .put(&other_locked_file)
        .body(b"other locked content".to_vec())
        .send()
        .await?;
    let other_lock_resp = lock(&app, &other_locked_file).send().await?;
    assert_eq!(other_lock_resp.status(), 200);
    let other_lock_token = other_lock_resp
        .headers()
        .get("lock-token")
        .unwrap()
        .to_str()?
        .to_owned();

    // Source locked, destination unlocked:
    let unlocked_dest = format!("http://files1.atrium.io:{}/unlocked_dest.txt", app.port);
    let resp_move_fail1 = mv(&app, &other_locked_file)
        .header("Destination", &unlocked_dest)
        .send()
        .await?;
    assert_eq!(resp_move_fail1.status(), StatusCode::LOCKED);

    // Source unlocked, destination locked:
    let unlocked_src = format!("http://files1.atrium.io:{}/unlocked_src.txt", app.port);
    app.client
        .put(&unlocked_src)
        .body(b"unlocked source content".to_vec())
        .send()
        .await?;
    let resp_move_fail2 = mv(&app, &unlocked_src)
        .header("Destination", &other_locked_file)
        .send()
        .await?;
    assert_eq!(resp_move_fail2.status(), StatusCode::LOCKED);

    // Source locked and destination locked, correct If lock tokens supplied:
    // Testing multiple tokens extraction: "If: (<token1>) (<token2>)"
    let resp_move_ok = mv(&app, &other_locked_file)
        .header("Destination", &url_file)
        .header("If", format!("({}) ({})", other_lock_token, lock_token))
        .send()
        .await?;
    assert_eq!(resp_move_ok.status(), 204); // Destination was already mapped, overwrite is true, so 204 No Content

    // Finally, clean up by deleting with valid lock token
    let resp_del_ok = app
        .client
        .delete(&url_file)
        .header("If", format!("({})", lock_token))
        .send()
        .await?;
    assert_eq!(resp_del_ok.status(), 204);

    Ok(())
}

#[tokio::test]
async fn check_collection_locking() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
    let url_coll = format!("http://files1.atrium.io:{}/my_locked_coll/", app.port);

    // 1. Create a directory (collection)
    let resp = mkcol(&app, &url_coll).send().await?;
    assert_eq!(resp.status(), 201);

    // 2. LOCK the collection (should succeed with 200 OK because it is an existing directory)
    let lock_resp = lock(&app, &url_coll).send().await?;
    assert_eq!(lock_resp.status(), 200);
    let lock_token = lock_resp
        .headers()
        .get("lock-token")
        .unwrap()
        .to_str()?
        .to_owned();

    // 3. Attempt to PUT a file inside the locked collection without the lock token (should fail with 423 Locked)
    let url_file = format!("{}test_file.txt", url_coll);
    let resp_put_fail = app
        .client
        .put(&url_file)
        .body(b"some content".to_vec())
        .send()
        .await?;
    assert_eq!(resp_put_fail.status(), StatusCode::LOCKED);

    // 4. Attempt to PUT the file with the collection lock token (should succeed with 201 Created)
    let resp_put_ok = app
        .client
        .put(&url_file)
        .header("If", format!("({})", lock_token))
        .body(b"some content".to_vec())
        .send()
        .await?;
    assert_eq!(resp_put_ok.status(), 201);

    // 5. Attempt to create a subdirectory without the lock token (should fail with 423 Locked)
    let url_sub = format!("{}sub_dir/", url_coll);
    let resp_sub_fail = mkcol(&app, &url_sub).send().await?;
    assert_eq!(resp_sub_fail.status(), StatusCode::LOCKED);

    // 6. Attempt to create the subdirectory with the lock token (should succeed with 201 Created)
    let resp_sub_ok = mkcol(&app, &url_sub)
        .header("If", format!("({})", lock_token))
        .send()
        .await?;
    assert_eq!(resp_sub_ok.status(), 201);

    // 7. Attempt to DELETE the file inside without the lock token (should fail with 423 Locked)
    let resp_del_fail = app.client.delete(&url_file).send().await?;
    assert_eq!(resp_del_fail.status(), StatusCode::LOCKED);

    // 8. Attempt to DELETE the file with the lock token (should succeed with 204 No Content)
    let resp_del_ok = app
        .client
        .delete(&url_file)
        .header("If", format!("({})", lock_token))
        .send()
        .await?;
    assert_eq!(resp_del_ok.status(), 204);

    // 9. MOVE a resource out of the locked collection (requires lock token because it alters the locked parent collection)
    let url_file2 = format!("{}test_file_2.txt", url_coll);
    let resp_put2 = app
        .client
        .put(&url_file2)
        .header("If", format!("({})", lock_token))
        .body(b"other content".to_vec())
        .send()
        .await?;
    assert_eq!(resp_put2.status(), 201);

    let url_unlocked_dest = format!("http://files1.atrium.io:{}/unlocked_dest.txt", app.port);
    let resp_move_fail = mv(&app, &url_file2)
        .header("Destination", &url_unlocked_dest)
        .send()
        .await?;
    assert_eq!(resp_move_fail.status(), StatusCode::LOCKED);

    let resp_move_ok = mv(&app, &url_file2)
        .header("Destination", &url_unlocked_dest)
        .header("If", format!("({})", lock_token))
        .send()
        .await?;
    assert_eq!(resp_move_ok.status(), 201);

    Ok(())
}

#[tokio::test]
async fn try_to_hack() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
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
async fn try_to_use_wrong_key_to_decrypt() -> BoxResult<()> {
    // Arrange
    let mut app = TestApp::spawn(None).await;

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

    let xsrf_token = login_and_get_xsrf_token(&app, "admin").await;
    app.client
        .get(format!("http://atrium.io:{}/reload", app.port))
        .header("xsrf-token", &xsrf_token)
        .send()
        .await
        .expect("failed to execute request");
    app.is_ready().await;

    // Assert that the file cannot be retrieved or that the server closes the connection
    if let Ok(response) = app
        .client
        .get(&url)
        .header("xsrf-token", &xsrf_token)
        .send()
        .await
    {
        assert!(response.bytes().await.is_err());
    }

    Ok(())
}

#[tokio::test]
async fn get_dir_404() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
    let resp = app
        .client
        .get(format!("http://files1.atrium.io:{}/404", app.port))
        .send()
        .await?;
    assert_eq!(resp.status(), 404);
    Ok(())
}

#[tokio::test]
async fn get_dir_zip() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
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
async fn head_dir_zip() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
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
async fn get_dir_search() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;

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
async fn get_dir_search_not_existing() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
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
async fn get_dir_search_subdir() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;

    let resp = app
        .client
        .get(format!(
            "http://files1.atrium.io:{}/dira/?q={}",
            app.port, "subdira"
        ))
        .send()
        .await?;
    assert_eq!(resp.status(), 200);
    assert!(resp.text().await?.contains("subdira"));
    Ok(())
}

#[tokio::test]
async fn get_dir_search_wrong_subdir() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;

    let resp = app
        .client
        .get(format!(
            "http://files1.atrium.io:{}/dirb/?q={}",
            app.port, "subdira"
        ))
        .send()
        .await?;
    assert_eq!(resp.status(), 200);
    assert!(!resp.text().await?.contains("subdira"));
    Ok(())
}

#[tokio::test]
async fn get_disk_usage() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
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
async fn get_file_404() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
    let resp = app
        .client
        .get(format!("http://files1.atrium.io:{}/404", app.port))
        .send()
        .await?;
    assert_eq!(resp.status(), 404);
    assert!(resp.headers().contains_key("Content-Security-Policy"));
    Ok(())
}

#[tokio::test]
async fn head_file_404() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
    let resp = app
        .client
        .head(format!("http://files1.atrium.io:{}/404", app.port))
        .send()
        .await?;
    assert_eq!(resp.status(), 404);
    Ok(())
}

#[tokio::test]
async fn options_dir() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
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
async fn put_file() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
    let url = format!("http://files1.atrium.io:{}/myfile", app.port);
    let resp = app.client.put(&url).body(b"abc".to_vec()).send().await?;
    assert_eq!(resp.status(), 201);
    let resp = app.client.get(url).send().await?;
    assert_eq!(resp.status(), 200);
    Ok(())
}

#[tokio::test]
async fn put_file_not_writable() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
    let url = format!("http://files3.atrium.io:{}/myfile", app.port);
    let resp = app.client.put(&url).body(b"abc".to_vec()).send().await?;
    assert_eq!(resp.status(), 403);
    let resp = app.client.get(url).send().await?;
    assert_eq!(resp.status(), 404);
    Ok(())
}

#[tokio::test]
async fn put_file_create_dir() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
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
async fn put_file_conflict_dir() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
    let resp = app
        .client
        .put(format!("http://files1.atrium.io:{}/dira", app.port))
        .body(b"abc".to_vec())
        .send()
        .await?;
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    Ok(())
}

#[tokio::test]
async fn put_file_alter_modtime() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
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
    assert!(body.contains("9 Nov 1982"));
    Ok(())
}

#[tokio::test]
async fn delete_file() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
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
async fn delete_file_404() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
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
pub(crate) fn mkcol(app: &TestApp, url: &str) -> reqwest::RequestBuilder {
    app.client
        .request(Method::from_bytes(b"MKCOL").unwrap(), url)
}
pub(crate) fn copy(app: &TestApp, url: &str) -> reqwest::RequestBuilder {
    app.client
        .request(Method::from_bytes(b"COPY").unwrap(), url)
}
pub(crate) fn mv(app: &TestApp, url: &str) -> reqwest::RequestBuilder {
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
async fn propfind_dir() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
    let url = format!("http://files1.atrium.io:{}/dira", app.port);
    let resp = propfind(&app, &url).send().await?;
    assert_eq!(resp.status(), 207);
    let body = resp.text().await?;
    assert!(body.contains("<D:href>/dira/</D:href>"));
    assert!(body.contains("<D:displayname>dira</D:displayname>"));
    assert!(body.contains("<D:getcontentlength>0</D:getcontentlength>"));
    for f in ["file1", "file2"] {
        assert!(body.contains(&format!("<D:href>/dira/{}</D:href>", encode_uri(f))));
        assert!(body.contains(&format!("<D:displayname>{}</D:displayname>", escape(f))));
    }
    Ok(())
}

#[tokio::test]
async fn propfind_dir_depth0() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
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
async fn propfind_404() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
    let url = format!("http://files1.atrium.io:{}/404", app.port);
    let resp = propfind(&app, &url).send().await?;
    assert_eq!(resp.status(), 404);
    Ok(())
}

#[tokio::test]
async fn propfind_file() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
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
async fn propfind_file_encrypted() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
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
async fn proppatch_file_no_modtime() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
    let url = format!("http://files1.atrium.io:{}/dira/file1", app.port);
    let resp = proppatch(&app, &url).send().await?;
    assert_eq!(resp.status(), 207);
    let body = resp.text().await?;
    assert!(body.contains("<D:href>/dira/file1</D:href>"));
    assert!(body.contains("<D:status>HTTP/1.1 403 Forbidden</D:status>"));
    Ok(())
}

#[tokio::test]
async fn proppatch_file_modtime() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
    let url = format!("http://files1.atrium.io:{}/dira/file1", app.port);
    let resp = proppatch(&app, &url)
        .body(
            r#"
                <?xml version="1.0" encoding="utf-8" ?>
                <D:propertyupdate xmlns:D="DAV:">
                    <D:set>
                        <D:prop>
                            <lastmodified xmlns="DAV:">405659400</lastmodified>
                        </D:prop>
                    </D:set>
                </D:propertyupdate>
            "#,
        )
        .send()
        .await?;
    assert_eq!(resp.status(), 207);
    let body = resp.text().await?;
    assert!(body.contains("<D:href>/dira/file1</D:href>"));
    assert!(body.contains(r#"<D:lastmodified xmlns="DAV:">405659400</D:lastmodified>"#));
    assert!(body.contains("<D:status>HTTP/1.1 200 OK</D:status>"));
    let resp = propfind(&app, &url).send().await?;
    assert_eq!(resp.status(), 207);
    let body = resp.text().await?;
    assert!(body.contains("9 Nov 1982"));
    Ok(())
}

#[tokio::test]
async fn proppatch_404() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
    let url = format!("http://files1.atrium.io:{}/404", app.port);
    let resp = proppatch(&app, &url).send().await?;

    assert_eq!(resp.status(), 404);
    Ok(())
}

#[tokio::test]
async fn mkcol_dir() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
    let url = format!("http://files1.atrium.io:{}/newdir", app.port);
    let resp = mkcol(&app, &url).send().await?;
    assert_eq!(resp.status(), 201);
    Ok(())
}

#[tokio::test]
async fn mkcol_not_writable() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
    let url = format!("http://files3.atrium.io:{}/newdir", app.port);
    let resp = mkcol(&app, &url).send().await?;
    assert_eq!(resp.status(), 403);
    Ok(())
}

#[tokio::test]
async fn copy_file() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
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
async fn copy_dir() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
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
async fn copy_not_writable() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
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
async fn copy_file_404() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
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
async fn move_file() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
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
async fn move_file_to_dir() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
    let origin_url = format!("http://files1.atrium.io:{}/dira/file2", app.port);
    let new_url = format!("http://files1.atrium.io:{}/dirb/", app.port);
    let resp = mv(&app, &origin_url)
        .header("Destination", &new_url)
        .send()
        .await?;
    assert_eq!(resp.status(), 204);
    let resp = app.client.get(format!("{new_url}file2")).send().await?;
    assert_eq!(resp.status(), 200);
    let resp = app.client.get(origin_url).send().await?;
    assert_eq!(resp.status(), 404);
    Ok(())
}

#[tokio::test]
async fn move_dir() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
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
async fn move_dir_root() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
    let url = format!("http://files1.atrium.io:{}/dira/", app.port);
    let new_url = format!("http://files1.atrium.io:{}/", app.port);
    let resp = mv(&app, &url)
        .header("Destination", &new_url)
        .send()
        .await?;
    assert_eq!(resp.status(), 403);
    let resp = app.client.get(url).send().await?;
    assert_eq!(resp.status(), 200);
    Ok(())
}

#[tokio::test]
async fn move_file_root() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
    let url = format!("http://files1.atrium.io:{}/dira/file1", app.port);
    let dest = format!("http://files1.atrium.io:{}/", app.port);
    let new_url = format!("http://files1.atrium.io:{}/file1", app.port);
    let resp = mv(&app, &url).header("Destination", &dest).send().await?;
    assert_eq!(resp.status(), 204);
    let resp = app.client.get(new_url).send().await?;
    assert_eq!(resp.status(), 200);
    Ok(())
}

#[tokio::test]
async fn move_file_not_writable() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
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
async fn move_file_404() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
    let url = format!("http://files1.atrium.io:{}/file3", app.port);
    let new_url = format!("http://files1.atrium.io:{}/file3%20(moved)", app.port);
    let resp = mv(&app, &url).header("Destination", new_url).send().await?;
    assert_eq!(resp.status(), 404);
    Ok(())
}

#[tokio::test]
async fn lock_file() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
    let url = format!("http://files1.atrium.io:{}/dira/file1", app.port);

    // First lock should succeed
    let resp = lock(&app, &url).send().await?;
    assert_eq!(resp.status(), 200);
    // Extract lock token from response
    let _ = resp
        .headers()
        .get("lock-token")
        .expect("Lock-Token header missing")
        .to_str()?;
    let body = resp.text().await?;
    assert!(body.contains("<D:href>/dira/file1</D:href>"));

    // Second lock attempt should fail
    let resp2 = lock(&app, &url).send().await?;
    assert_eq!(resp2.status(), 423); // 423 Locked

    Ok(())
}

#[tokio::test]
async fn unlock_file() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
    let url = format!("http://files1.atrium.io:{}/dira/file1", app.port);

    // Lock first
    let lock_resp = lock(&app, &url).send().await?;
    let lock_token = lock_resp.headers().get("lock-token").unwrap();

    // Proper unlock
    let unlock_resp = unlock(&app, &url)
        .header("Lock-Token", lock_token)
        .send()
        .await?;
    assert_eq!(unlock_resp.status(), 204);

    // Should be able to lock again after unlock
    let relock_resp = lock(&app, &url).send().await?;
    assert_eq!(relock_resp.status(), 200);

    Ok(())
}

#[tokio::test]
async fn lock_unlock_unexisting_file() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
    let url = format!("http://files1.atrium.io:{}/file3", app.port);

    let resp = lock(&app, &url).send().await?;
    assert_eq!(resp.status(), 201);

    let lock_token = resp.headers().get("lock-token").unwrap();
    let unlock_resp = unlock(&app, &url)
        .header("Lock-Token", lock_token)
        .send()
        .await?;
    assert_eq!(unlock_resp.status(), 404);

    Ok(())
}

#[tokio::test]
async fn unlock_with_invalid_token() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
    let url = format!("http://files1.atrium.io:{}/dira/file1", app.port);

    // Lock first
    lock(&app, &url).send().await?;

    // Try unlock with bad token
    let resp = unlock(&app, &url)
        .header("Lock-Token", "<invalidtoken>")
        .send()
        .await?;
    assert_eq!(resp.status(), 403); // 403 Forbidden

    Ok(())
}

#[tokio::test]
async fn lock_expiration() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
    let url = format!("http://files1.atrium.io:{}/dira/file1", app.port);

    // Lock with 1 second timeout
    let resp = lock(&app, &url)
        .header("Timeout", "Second-1")
        .send()
        .await?;
    assert_eq!(resp.status(), 200);

    // Wait for lock to expire
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Should be able to lock again
    let resp2 = lock(&app, &url).send().await?;
    assert_eq!(resp2.status(), 200);

    Ok(())
}

#[cfg(unix)]
use std::os::unix::fs::symlink as symlink_dir;
#[cfg(windows)]
use std::os::windows::fs::symlink_dir;

#[tokio::test]
async fn default_not_allow_symlinks() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
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
async fn allow_symlinks() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
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
async fn try_to_hack_symlinks() -> BoxResult<()> {
    let app = TestApp::spawn(None).await;
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
    symlink_dir(srcdir, format!("./data/{}/dirc", app.id)).expect("couldn't create symlink");
    let resp = app
        .client
        .get(format!("http://files3.atrium.io:{}/../dirc", app.port))
        .send()
        .await?;
    assert_eq!(resp.status(), 404);
    let resp = app
        .client
        .get(format!(
            "http://files3.atrium.io:{}/../dirc/file1",
            app.port
        ))
        .send()
        .await?;
    assert_eq!(resp.status(), 404);
    Ok(())
}

#[tokio::test]
async fn secured_dav_test() {
    // Arrange
    let app = TestApp::spawn(None).await;

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
    let xsrf_token = login_and_get_xsrf_token(&app, "user").await;
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
    login_and_get_xsrf_token(&app, "admin").await;
    // Act : try to access app as admin without XSRF token
    let response = app
        .client
        .get(format!("http://secured-files.atrium.io:{}", app.port))
        .send()
        .await
        .expect("failed to execute request");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    // Log as admin
    login_and_get_xsrf_token(&app, "admin").await;
    // Act : try to access app as admin with a wrong XSRF token
    let response = app
        .client
        .get(format!("http://secured-files.atrium.io:{}", app.port))
        .header("xsrf-token", "randomtoken")
        .send()
        .await
        .expect("failed to execute request");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    // Log as admin
    let xsrf_token = login_and_get_xsrf_token(&app, "admin").await;
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
    let app = TestApp::spawn(None).await;

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
    let token = token.split(';').collect::<Vec<_>>()[0]
        .split('=')
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
    // Try to access app with an empty token passed as query, must fail
    let response = client
        .get(format!("http://secured-files.atrium.io:{}?token", app.port))
        .send()
        .await
        .expect("failed to execute request");
    assert!(response.status() == 401);
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
                base64ct::Base64::encode_string(b"admin:password")
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
                base64ct::Base64::encode_string(b"admin:badpassword")
            ),
        )
        .send()
        .await
        .expect("failed to execute request");
    assert!(response.status() == StatusCode::UNAUTHORIZED);
}
