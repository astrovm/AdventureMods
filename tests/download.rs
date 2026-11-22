mod support;

use adventure_mods::external::download;
use adventure_mods::setup::common::{self, ModEntry, ModSource};

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

    assert!(
        error
            .to_string()
            .contains("Server returned HTML instead of a file")
    );
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

// XDG_CACHE_HOME moves the user's cache folder only on Linux.
#[cfg(target_os = "linux")]
#[test]
fn a_prefetched_mod_waits_in_the_app_folder_of_the_users_cache() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let _env_lock = support::env_lock();
    let server = TestServer::start(std::collections::HashMap::from([(
        "/mod.7z",
        Response::Ok {
            content_type: "application/octet-stream",
            body: "archive",
        },
    )]));
    let tmp = tempfile::tempdir().unwrap();
    let cache = tmp.path().join("cache");
    // The release build caches in the user's cache folder, which
    // XDG_CACHE_HOME moves on Linux. Each test runs in its own process under
    // nextest, and the lock keeps the other tests here out meanwhile.
    unsafe {
        std::env::remove_var("ADVENTURE_MODS_CACHE_DIR");
        std::env::set_var("XDG_CACHE_HOME", &cache);
    }
    let mod_entry = ModEntry {
        name: "Cached Mod",
        slug: "cached-mod",
        source: ModSource::DirectUrl {
            url: support::leak_str(server.url("/mod.7z")),
        },
        description: "test mod",
        full_description: None,
        pictures: &[],
        dir_name: Some("CachedMod"),
        links: &[],
    };

    let result = common::prefetch_mod_archive(&tmp.path().join("game"), &mod_entry, None);
    unsafe { std::env::remove_var("XDG_CACHE_HOME") };
    result.unwrap();

    let downloads = std::fs::read_dir(cache.join("adventure-mods/downloads"))
        .unwrap()
        .map(|entry| std::fs::read_to_string(entry.unwrap().path()).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(downloads, ["archive"]);
}
