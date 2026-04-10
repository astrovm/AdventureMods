mod support;

use adventure_mods::external::download;

use support::http_server::{Response, TestServer};

#[test]
fn download_rejects_html_responses() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let server = TestServer::start(std::collections::HashMap::from([(
        "/download",
        Response::Ok {
            content_type: "text/html",
            body: "<html>broken</html>",
        },
    )]));
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join("file.bin");

    let error = download::download_file(&server.url("/download"), &dest, None).unwrap_err();

    assert!(error
        .to_string()
        .contains("Server returned HTML instead of a file"));
}

#[test]
fn download_truncates_http_error_body_snippet() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let body = "x".repeat(300);
    let body: &'static str = Box::leak(body.into_boxed_str());
    let server = TestServer::start(std::collections::HashMap::from([(
        "/download",
        Response::Status {
            status_line: "500 Internal Server Error",
            content_type: "text/plain",
            body,
        },
    )]));
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join("file.bin");

    let error = download::download_file(&server.url("/download"), &dest, None).unwrap_err();
    let message = error.to_string();

    assert!(message.contains("HTTP error 500 Internal Server Error"));
    assert!(message.contains(&"x".repeat(200)));
    assert!(!message.contains(&"x".repeat(250)));
}
