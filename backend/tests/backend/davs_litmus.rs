use http::Method;
use reqwest::Response;

use crate::{
    davs::{copy, mkcol, mv},
    helpers::TestApp,
};
use anyhow::Result;

fn has_header(r: &Response, header_name: &str) {
    assert!(r.headers().get(header_name).is_some());
}

fn is_header(r: &Response, header_name: &str, header_value: &str) {
    assert_eq!(
        r.headers()
            .get(header_name)
            .expect("header does not exist")
            .to_str()
            .expect("header could not be converted to str"),
        header_value
    );
}

async fn body_contains(r: Response, content: &str) {
    let body = r.text().await.expect("body could not be converted to text");
    assert!(body.contains(content));
}

async fn litmus_init() -> Result<TestApp> {
    let app = TestApp::spawn(None).await;
    let url = format!("http://secured-files.atrium.io:{}/litmus/", app.port);

    let resp = app.client.delete(&url).send().await?;
    assert_eq!(resp.status(), 401);
    is_header(&resp, "www-authenticate", "Basic realm=\"server\"");

    app.client
        .delete(&url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;

    let resp = mkcol(&app, &url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;

    assert_eq!(resp.status(), 201);
    Ok(app)
}

#[tokio::test]
async fn litmus_basic() -> Result<()> {
    // 1. BEGIN
    let app = litmus_init().await?;

    // 2. OPTIONS
    let url = format!("http://secured-files.atrium.io:{}/litmus/", app.port);
    let resp = app
        .client
        .request(Method::OPTIONS, url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 200);
    is_header(&resp, "dav", "1,2");
    is_header(
        &resp,
        "allow",
        "GET,HEAD,PUT,OPTIONS,DELETE,PROPFIND,COPY,MOVE",
    );

    // 3. PUT GET
    let url = format!("http://secured-files.atrium.io:{}/litmus/res", app.port);
    let resp = app
        .client
        .put(&url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .body(
            b"This is
a test file.
for litmus
testing.
"
            .to_vec(),
        )
        .send()
        .await?;
    assert_eq!(resp.status(), 201);
    let resp = app
        .client
        .get(url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 200);
    has_header(&resp, "last-modified");
    has_header(&resp, "etag");
    is_header(&resp, "content-type", "application/octet-stream");
    is_header(&resp, "content-disposition", "attachment; filename=\"res\"");
    is_header(&resp, "content-length", "41");
    is_header(&resp, "accept-ranges", "bytes");
    body_contains(
        resp,
        "This is
a test file.
for litmus
testing.
",
    )
    .await;

    // 4. PUT GET UTF-8 SEGMENT
    let url = format!(
        "http://secured-files.atrium.io:{}/litmus/res-%e2%82%ac",
        app.port
    );
    let resp = app
        .client
        .put(&url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .body(
            b"This is
a test file.
for litmus
testing.
"
            .to_vec(),
        )
        .send()
        .await?;
    assert_eq!(resp.status(), 201);
    let resp = app
        .client
        .get(&url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 200);
    is_header(
        &resp,
        "content-disposition",
        "attachment; filename=\"res-%E2%82%AC\"",
    );
    body_contains(
        resp,
        "This is
a test file.
for litmus
testing.
",
    )
    .await;

    // 5. PUT NO PARENT
    let url = format!(
        "http://secured-files.atrium.io:{}/litmus/409me/noparent.txt/",
        app.port
    );
    let resp = mkcol(&app, &url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 409);

    // 6. MKCOL OVER PLAIN FILE
    let url = format!(
        "http://secured-files.atrium.io:{}/litmus/res-%e2%82%ac/",
        app.port
    );
    let resp = mkcol(&app, &url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 405);
    body_contains(resp, "Method not allowed").await;

    // 7. DELETE (FILE)
    let url = format!(
        "http://secured-files.atrium.io:{}/litmus/res-%e2%82%ac",
        app.port
    );
    let resp = app
        .client
        .delete(url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 204);

    // 8. DELETE NULL
    let url = format!("http://secured-files.atrium.io:{}/litmus/404me", app.port);
    let resp = app
        .client
        .delete(url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 404);

    // 9. DELETE FRAGMENT (DIRECTORY)
    let url = format!("http://secured-files.atrium.io:{}/litmus/frag/", app.port);
    let resp = mkcol(&app, &url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 201);
    let resp = app
        .client
        .delete(url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 204);

    // 10. MKCOL
    let url = format!("http://secured-files.atrium.io:{}/litmus/coll/", app.port);
    let resp = mkcol(&app, &url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 201);

    // 11. MKCOL AGAIN
    let url = format!("http://secured-files.atrium.io:{}/litmus/coll/", app.port);
    let resp = mkcol(&app, &url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 405);
    body_contains(resp, "Method not allowed").await;

    // 12. DELETE COLL
    let resp = app
        .client
        .delete(url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 204);

    // 13. MKCOL NO PARENT
    let url = format!(
        "http://secured-files.atrium.io:{}/litmus/409me/noparent/",
        app.port
    );
    let resp = mkcol(&app, &url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 409);

    // 14. MKCOL WITH BODY
    let url = format!(
        "http://secured-files.atrium.io:{}/litmus/mkcolbody",
        app.port
    );
    let resp = mkcol(&app, &url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .header("content-type", "xzy-foo/bar-512")
        .body(b"afafafaf".to_vec())
        .send()
        .await?;
    assert_eq!(resp.status(), 415);
    body_contains(resp, "Unsupported Media Type").await;
    Ok(())
}

#[tokio::test]
async fn litmus_copymove() -> Result<()> {
    // 1. BEGIN
    let app = litmus_init().await?;

    // 2. COPY INIT
    let url = format!("http://secured-files.atrium.io:{}/litmus/copysrc", app.port);
    let resp = app
        .client
        .put(url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .body(
            b"This
is
a
test
file
called
foo

"
            .to_vec(),
        )
        .send()
        .await?;
    assert_eq!(resp.status(), 201);
    let url = format!(
        "http://secured-files.atrium.io:{}/litmus/copycoll/",
        app.port
    );
    let resp = mkcol(&app, &url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 201);

    // 3. COPY SIMPLE
    let url = format!("http://secured-files.atrium.io:{}/litmus/copysrc", app.port);
    let resp = copy(&app, &url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .header("depth", "infinity")
        .header(
            "destination",
            format!(
                "http://secured-files.atrium.io:{}/litmus/copydest",
                app.port
            ),
        )
        .header("overwrite", "F")
        .send()
        .await?;
    assert_eq!(resp.status(), 201);

    // 3. COPY OVERWRITE
    let resp = copy(&app, &url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .header("depth", "infinity")
        .header(
            "destination",
            format!(
                "http://secured-files.atrium.io:{}/litmus/copydest",
                app.port
            ),
        )
        .header("overwrite", "F")
        .send()
        .await?;
    assert_eq!(resp.status(), 412);
    let resp = copy(&app, &url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .header("depth", "infinity")
        .header(
            "destination",
            format!(
                "http://secured-files.atrium.io:{}/litmus/copydest",
                app.port
            ),
        )
        .header("overwrite", "T")
        .send()
        .await?;
    assert_eq!(resp.status(), 204);
    let url = format!("http://secured-files.atrium.io:{}/litmus/copysrc", app.port);
    let resp = copy(&app, &url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .header("depth", "infinity")
        .header(
            "destination",
            format!(
                "http://secured-files.atrium.io:{}/litmus/copycoll/",
                app.port
            ),
        )
        .header("overwrite", "T")
        .send()
        .await?;
    assert_eq!(resp.status(), 204);

    // 5. COPY NO DEST COLL
    let url = format!("http://secured-files.atrium.io:{}/litmus/copysrc", app.port);
    let resp = copy(&app, &url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .header("depth", "0")
        .header(
            "destination",
            format!(
                "http://secured-files.atrium.io:{}/litmus/nonesuch/foo",
                app.port
            ),
        )
        .header("overwrite", "F")
        .send()
        .await?;
    assert_eq!(resp.status(), 409);

    // 6. COPY CLEAN UP
    let resp = app
        .client
        .delete(url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 204);
    let url = format!(
        "http://secured-files.atrium.io:{}/litmus/copydest",
        app.port
    );
    let resp = app
        .client
        .delete(url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 204);
    let url = format!(
        "http://secured-files.atrium.io:{}/litmus/copycoll",
        app.port
    );
    let resp = app
        .client
        .delete(&url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 204);
    let resp = app
        .client
        .delete(url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 404);

    // 7. COPY COLL
    let url = format!("http://secured-files.atrium.io:{}/litmus/ccsrc/", app.port);
    let resp = mkcol(&app, &url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 201);
    for n in 0..10 {
        let url = format!(
            "http://secured-files.atrium.io:{}/litmus/ccsrc/foo.{n}",
            app.port
        );
        let resp = app
            .client
            .put(url)
            .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
            .body(
                b"This
is
a
test
file
called
foo

"
                .to_vec(),
            )
            .send()
            .await?;
        assert_eq!(resp.status(), 201);
    }
    let url = format!(
        "http://secured-files.atrium.io:{}/litmus/ccsrc/subcoll/",
        app.port
    );
    let resp = mkcol(&app, &url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 201);
    let url = format!("http://secured-files.atrium.io:{}/litmus/ccdest/", app.port);
    let resp = app
        .client
        .delete(url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 404);
    let url = format!(
        "http://secured-files.atrium.io:{}/litmus/ccdest2/",
        app.port
    );
    let resp = app
        .client
        .delete(url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 404);
    let url = format!("http://secured-files.atrium.io:{}/litmus/ccsrc/", app.port);
    let resp = copy(&app, &url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .header("depth", "infinity")
        .header(
            "destination",
            format!("http://secured-files.atrium.io:{}/litmus/ccdest/", app.port),
        )
        .header("overwrite", "F")
        .send()
        .await?;
    assert_eq!(resp.status(), 201);
    let resp = copy(&app, &url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .header("depth", "infinity")
        .header(
            "destination",
            format!(
                "http://secured-files.atrium.io:{}/litmus/ccdest2/",
                app.port
            ),
        )
        .header("overwrite", "F")
        .send()
        .await?;
    assert_eq!(resp.status(), 201);
    let url = format!("http://secured-files.atrium.io:{}/litmus/ccdest/", app.port);
    let resp = copy(&app, &url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .header("depth", "infinity")
        .header(
            "destination",
            format!(
                "http://secured-files.atrium.io:{}/litmus/ccdest2/",
                app.port
            ),
        )
        .header("overwrite", "F")
        .send()
        .await?;
    assert_eq!(resp.status(), 412);
    let url = format!(
        "http://secured-files.atrium.io:{}/litmus/ccdest2/",
        app.port
    );
    let resp = copy(&app, &url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .header("depth", "infinity")
        .header(
            "destination",
            format!("http://secured-files.atrium.io:{}/litmus/ccdest/", app.port),
        )
        .header("overwrite", "T")
        .send()
        .await?;
    assert_eq!(resp.status(), 204);
    let url = format!("http://secured-files.atrium.io:{}/litmus/ccsrc/", app.port);
    let resp = app
        .client
        .delete(url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 204);
    for n in 0..10 {
        let url = format!(
            "http://secured-files.atrium.io:{}/litmus/ccdest/foo.{n}",
            app.port
        );
        let resp = app
            .client
            .delete(url)
            .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
            .send()
            .await?;
        assert_eq!(resp.status(), 204);
    }
    let url = format!(
        "http://secured-files.atrium.io:{}/litmus/ccdest/subcoll/",
        app.port
    );
    let resp = app
        .client
        .delete(url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 204);
    let url = format!(
        "http://secured-files.atrium.io:{}/litmus/ccdest2/",
        app.port
    );
    let resp = app
        .client
        .delete(url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 204);
    let url = format!("http://secured-files.atrium.io:{}/litmus/ccdest/", app.port);
    let resp = app
        .client
        .delete(url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 204);

    // 8. COPY SHALLOW
    let url = format!("http://secured-files.atrium.io:{}/litmus/ccsrc/", app.port);
    let resp = mkcol(&app, &url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 201);
    let url = format!(
        "http://secured-files.atrium.io:{}/litmus/ccsrc/foo",
        app.port
    );
    let resp = app
        .client
        .put(url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .body(
            b"This
is
a
test
file
called
foo

"
            .to_vec(),
        )
        .send()
        .await?;
    assert_eq!(resp.status(), 201);
    let url = format!("http://secured-files.atrium.io:{}/litmus/ccdest/", app.port);
    let resp = app
        .client
        .delete(url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 404);
    let url = format!("http://secured-files.atrium.io:{}/litmus/ccsrc/", app.port);
    let resp = copy(&app, &url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .header("depth", "0")
        .header(
            "destination",
            format!("http://secured-files.atrium.io:{}/litmus/ccdest/", app.port),
        )
        .header("overwrite", "F")
        .send()
        .await?;
    assert_eq!(resp.status(), 201);
    let resp = app
        .client
        .delete(url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 204);
    let url = format!("http://secured-files.atrium.io:{}/litmus/foo", app.port);
    let resp = app
        .client
        .delete(url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 404);
    let url = format!(
        "http://secured-files.atrium.io:{}/litmus/ccdest/foo",
        app.port
    );
    let resp = app
        .client
        .delete(url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 404);
    let url = format!("http://secured-files.atrium.io:{}/litmus/ccdest/", app.port);
    let resp = app
        .client
        .delete(url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 204);

    // 9. MOVE
    let app = TestApp::spawn(None).await;
    let url = format!("http://secured-files.atrium.io:{}/litmus/move", app.port);
    let resp = app
        .client
        .put(url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .body(
            b"This
is
a
test
file
called
foo

"
            .to_vec(),
        )
        .send()
        .await?;
    assert_eq!(resp.status(), 201);

    let url = format!("http://secured-files.atrium.io:{}/litmus/move2", app.port);
    let resp = app
        .client
        .put(url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .body(
            b"This
is
a
test
file
called
foo

"
            .to_vec(),
        )
        .send()
        .await?;
    assert_eq!(resp.status(), 201);
    let url = format!(
        "http://secured-files.atrium.io:{}/litmus/movecoll/",
        app.port
    );
    let resp = mkcol(&app, &url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 201);
    let url = format!("http://secured-files.atrium.io:{}/litmus/move", app.port);
    let resp = mv(&app, &url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .header(
            "destination",
            format!(
                "http://secured-files.atrium.io:{}/litmus/movedest",
                app.port
            ),
        )
        .header("overwrite", "F")
        .send()
        .await?;
    assert_eq!(resp.status(), 201);
    let url = format!("http://secured-files.atrium.io:{}/litmus/move2", app.port);
    let resp = mv(&app, &url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .header(
            "destination",
            format!(
                "http://secured-files.atrium.io:{}/litmus/movedest",
                app.port
            ),
        )
        .header("overwrite", "F")
        .send()
        .await?;
    assert_eq!(resp.status(), 412);
    let url = format!("http://secured-files.atrium.io:{}/litmus/move2", app.port);
    let resp = mv(&app, &url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .header(
            "destination",
            format!(
                "http://secured-files.atrium.io:{}/litmus/movedest",
                app.port
            ),
        )
        .header("overwrite", "T")
        .send()
        .await?;
    assert_eq!(resp.status(), 204);
    let url = format!(
        "http://secured-files.atrium.io:{}/litmus/movedest",
        app.port
    );
    let resp = mv(&app, &url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .header(
            "destination",
            format!(
                "http://secured-files.atrium.io:{}/litmus/movecoll",
                app.port
            ),
        )
        .header("overwrite", "T")
        .send()
        .await?;
    assert_eq!(resp.status(), 204);
    let url = format!(
        "http://secured-files.atrium.io:{}/litmus/movecoll/movedest",
        app.port
    );
    let resp = app
        .client
        .delete(url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 204);
    let url = format!(
        "http://secured-files.atrium.io:{}/litmus/movecoll",
        app.port
    );
    let resp = app
        .client
        .delete(url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 204);

    // 10. MOVE COLL
    let url = format!("http://secured-files.atrium.io:{}/litmus/mvsrc/", app.port);
    let resp = mkcol(&app, &url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 201);
    for n in 0..10 {
        let url = format!(
            "http://secured-files.atrium.io:{}/litmus/mvsrc/foo.{n}",
            app.port
        );
        let resp = app
            .client
            .put(url)
            .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
            .body(
                b"This
is
a
test
file
called
foo

"
                .to_vec(),
            )
            .send()
            .await?;
        assert_eq!(resp.status(), 201);
    }
    let url = format!(
        "http://secured-files.atrium.io:{}/litmus/mvnoncoll",
        app.port
    );
    let resp = app
        .client
        .put(url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .body(
            b"This
is
a
test
file
called
foo

"
            .to_vec(),
        )
        .send()
        .await?;
    assert_eq!(resp.status(), 201);
    let url = format!(
        "http://secured-files.atrium.io:{}/litmus/mvsrc/subcoll/",
        app.port
    );
    let resp = mkcol(&app, &url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 201);
    let url = format!("http://secured-files.atrium.io:{}/litmus/mvsrc/", app.port);
    let resp = copy(&app, &url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .header("depth", "infinity")
        .header(
            "destination",
            format!(
                "http://secured-files.atrium.io:{}/litmus/mvdest2/",
                app.port
            ),
        )
        .header("overwrite", "F")
        .send()
        .await?;
    assert_eq!(resp.status(), 201);
    let url = format!("http://secured-files.atrium.io:{}/litmus/mvsrc/", app.port);
    let resp = mv(&app, &url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .header(
            "destination",
            format!("http://secured-files.atrium.io:{}/litmus/mvdest/", app.port),
        )
        .header("overwrite", "F")
        .send()
        .await?;
    assert_eq!(resp.status(), 201);
    let url = format!("http://secured-files.atrium.io:{}/litmus/mvdest/", app.port);
    let resp = mv(&app, &url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .header(
            "destination",
            format!(
                "http://secured-files.atrium.io:{}/litmus/mvdest2/",
                app.port
            ),
        )
        .header("overwrite", "F")
        .send()
        .await?;
    assert_eq!(resp.status(), 412);
    let url = format!(
        "http://secured-files.atrium.io:{}/litmus/mvdest2/",
        app.port
    );
    let resp = mv(&app, &url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .header(
            "destination",
            format!("http://secured-files.atrium.io:{}/litmus/mvdest/", app.port),
        )
        .header("overwrite", "T")
        .send()
        .await?;
    assert_eq!(resp.status(), 204);
    let url = format!("http://secured-files.atrium.io:{}/litmus/mvdest/", app.port);
    let resp = copy(&app, &url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .header("depth", "infinity")
        .header(
            "destination",
            format!(
                "http://secured-files.atrium.io:{}/litmus/mvdest2/",
                app.port
            ),
        )
        .header("overwrite", "F")
        .send()
        .await?;
    assert_eq!(resp.status(), 201);
    for n in 0..10 {
        let url = format!(
            "http://secured-files.atrium.io:{}/litmus/mvdest/foo.{n}",
            app.port
        );
        let resp = app
            .client
            .delete(url)
            .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
            .send()
            .await?;
        assert_eq!(resp.status(), 204);
    }
    let url = format!(
        "http://secured-files.atrium.io:{}/litmus/mvdest/subcoll/",
        app.port
    );
    let resp = app
        .client
        .delete(url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 204);
    let url = format!(
        "http://secured-files.atrium.io:{}/litmus/mvdest2/",
        app.port
    );
    let resp = mv(&app, &url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .header(
            "destination",
            format!(
                "http://secured-files.atrium.io:{}/litmus/mvnoncoll",
                app.port
            ),
        )
        .header("overwrite", "T")
        .send()
        .await?;
    assert_eq!(resp.status(), 204);

    // 10. MOVE CLEANUP
    let url = format!("http://secured-files.atrium.io:{}/litmus/mvdest/", app.port);
    let resp = app
        .client
        .delete(url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 204);
    let url = format!(
        "http://secured-files.atrium.io:{}/litmus/mvdest2/",
        app.port
    );
    let resp = app
        .client
        .delete(url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 204);
    let url = format!(
        "http://secured-files.atrium.io:{}/litmus/mvnoncoll",
        app.port
    );
    let resp = app
        .client
        .delete(url)
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .send()
        .await?;
    assert_eq!(resp.status(), 204);
    Ok(())
}
