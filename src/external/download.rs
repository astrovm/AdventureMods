use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{Context, Result};
use futures_util::StreamExt;
use reqwest::{Client, Response, Url};

/// Progress callback: (downloaded_bytes, total_bytes_if_known)
pub type ProgressFn = Box<dyn Fn(u64, Option<u64>) + Send>;

/// Metadata checks are optional, so they give up quickly on a slow server.
const HEAD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

/// Run HTTP work to completion from a blocking thread.
///
/// Every download shares one small runtime and one [`Client`], so parallel mod
/// installs reuse pooled connections and TLS sessions instead of paying a new
/// handshake (and a new runtime) per request. Must be called from a blocking
/// thread (e.g. `gio::spawn_blocking`), NOT from an async context.
pub(crate) fn block_on<F: Future>(future: F) -> Result<F::Output> {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

    let runtime = match RUNTIME.get() {
        Some(runtime) => runtime,
        None => {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .thread_name("adventure-mods-http")
                .enable_all()
                .build()
                .context("Failed to create tokio runtime")?;
            RUNTIME.get_or_init(|| runtime)
        }
    };

    Ok(runtime.block_on(future))
}

/// The shared HTTP client. Only use it inside [`block_on`].
pub(crate) fn client() -> &'static Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        Client::builder()
            .connect_timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default()
    })
}

/// Download a file from a URL with progress reporting.
///
/// Must be called from a blocking thread (e.g. `gio::spawn_blocking`),
/// NOT from an async context.
pub fn download_file(url: &str, dest: &Path, progress: Option<ProgressFn>) -> Result<()> {
    let mut cb = progress.map(|f| {
        move |downloaded: u64, total: Option<u64>| -> Result<()> {
            f(downloaded, total);
            Ok(())
        }
    });
    download_file_with(
        url,
        dest,
        cb.as_mut()
            .map(|f| f as &mut dyn FnMut(u64, Option<u64>) -> Result<()>),
    )
}

/// Like `download_file` but accepts any `FnMut` without `Send` or `'static` bounds.
/// Use when the callback captures non-Send state on the current blocking thread.
pub fn download_file_with(
    url: &str,
    dest: &Path,
    progress: Option<&mut dyn FnMut(u64, Option<u64>) -> Result<()>>,
) -> Result<()> {
    block_on(download_file_async(url, dest, progress))?
}

async fn download_file_async(
    url: &str,
    dest: &Path,
    mut progress: Option<&mut dyn FnMut(u64, Option<u64>) -> Result<()>>,
) -> Result<()> {
    let response = fetch_download_response(client(), url).await?;

    let total = response.content_length();

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut file = std::fs::File::create(dest)
        .with_context(|| format!("Failed to create {}", dest.display()))?;

    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;

    use std::io::Write;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("Error reading response body")?;
        file.write_all(&chunk)?;
        downloaded += chunk.len() as u64;

        if let Some(ref mut progress) = progress {
            progress(downloaded, total)?;
        }
    }

    file.flush()?;
    Ok(())
}

/// Where an interrupted download keeps its bytes, and the validator that
/// proves those bytes still belong to the same remote file.
fn partial_paths(dest: &Path) -> (PathBuf, PathBuf) {
    let mut part = dest.as_os_str().to_owned();
    part.push(".part");
    let mut validator = part.clone();
    validator.push(".validator");
    (PathBuf::from(part), PathBuf::from(validator))
}

/// A header value that identifies one version of a remote file: a strong
/// ETag, else Last-Modified. Both are valid `If-Range` values.
pub(crate) fn response_validator(response: &Response) -> Option<String> {
    let header = |name| {
        response
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };
    header(reqwest::header::ETAG)
        .filter(|etag| !etag.starts_with("W/"))
        .or_else(|| header(reqwest::header::LAST_MODIFIED))
}

/// Download `url` to `dest`, resuming a previous interrupted attempt when the
/// server supports ranges and the file has not changed since.
///
/// Bytes are written to `dest.part` and renamed to `dest` once complete, so an
/// existing `dest` is always a whole file. Returns the file's validator (see
/// [`response_validator`]) when the server sent one.
///
/// Must be called from a blocking thread (e.g. `gio::spawn_blocking`).
pub fn download_file_resumable(
    url: &str,
    dest: &Path,
    progress: Option<&mut dyn FnMut(u64, Option<u64>) -> Result<()>>,
) -> Result<Option<String>> {
    block_on(download_file_resumable_async(url, dest, progress))?
}

async fn download_file_resumable_async(
    url: &str,
    dest: &Path,
    mut progress: Option<&mut dyn FnMut(u64, Option<u64>) -> Result<()>>,
) -> Result<Option<String>> {
    use std::io::Write;

    let (part, validator_path) = partial_paths(dest);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let resume_from = std::fs::metadata(&part).map(|meta| meta.len()).unwrap_or(0);
    let saved_validator = std::fs::read_to_string(&validator_path).ok();
    let resume = match saved_validator.as_deref() {
        Some(validator) if resume_from > 0 => Some((resume_from, validator.trim())),
        _ => None,
    };

    let response = fetch_download_response_from(client(), url, resume).await?;
    let validator = response_validator(&response);
    let resumed = resume.is_some() && response.status() == reqwest::StatusCode::PARTIAL_CONTENT;

    let (mut file, mut downloaded) = if resumed {
        tracing::info!("Resuming download of {url} at {resume_from} bytes");
        let file = std::fs::OpenOptions::new()
            .append(true)
            .open(&part)
            .with_context(|| format!("Failed to open {}", part.display()))?;
        (file, resume_from)
    } else {
        let file = std::fs::File::create(&part)
            .with_context(|| format!("Failed to create {}", part.display()))?;
        match &validator {
            Some(validator) => std::fs::write(&validator_path, validator)?,
            None => {
                let _ = std::fs::remove_file(&validator_path);
            }
        }
        (file, 0)
    };
    let total = response.content_length().map(|len| len + downloaded);

    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("Error reading response body")?;
        file.write_all(&chunk)?;
        downloaded += chunk.len() as u64;

        if let Some(ref mut progress) = progress {
            progress(downloaded, total)?;
        }
    }
    file.flush()?;
    drop(file);

    std::fs::rename(&part, dest)
        .with_context(|| format!("Failed to finish download to {}", dest.display()))?;
    let _ = std::fs::remove_file(&validator_path);
    Ok(validator)
}

/// Ask the server which version of `url` it currently serves (see
/// [`response_validator`]) without downloading it. Returns `None` when the
/// server does not say, or answers with a web page instead of the file.
pub fn remote_validator(url: &str) -> Result<Option<String>> {
    block_on(async {
        let response = client()
            .head(url)
            .timeout(HEAD_TIMEOUT)
            .send()
            .await
            .with_context(|| format!("Failed to HEAD {url}"))?;
        if !response.status().is_success() {
            anyhow::bail!("HTTP error {} for {url}", response.status());
        }
        if response_is_html(&response) {
            return Ok(None);
        }
        Ok(response_validator(&response))
    })?
}

/// Ask the server how large the file at `url` is without downloading it.
pub fn remote_size(url: &str) -> Result<Option<u64>> {
    block_on(async {
        let response = client()
            .head(url)
            .timeout(HEAD_TIMEOUT)
            .send()
            .await
            .with_context(|| format!("Failed to HEAD {url}"))?;
        if !response.status().is_success() {
            anyhow::bail!("HTTP error {} for {url}", response.status());
        }
        if response_is_html(&response) {
            return Ok(None);
        }
        Ok(response
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok()?.parse().ok())
            .filter(|&len: &u64| len > 0))
    })?
}

/// Remove a finished download together with any partial leftovers.
pub fn remove_download(dest: &Path) {
    let (part, validator) = partial_paths(dest);
    for path in [dest, part.as_path(), validator.as_path()] {
        let _ = std::fs::remove_file(path);
    }
}

async fn fetch_download_response(client: &Client, url: &str) -> Result<Response> {
    fetch_download_response_from(client, url, None).await
}

/// GET `url`, asking to continue at `resume.0` if the file still matches the
/// validator `resume.1` (the server answers 206 to resume, or 200 with the
/// whole file when it changed or ranges are unsupported).
async fn fetch_download_response_from(
    client: &Client,
    url: &str,
    resume: Option<(u64, &str)>,
) -> Result<Response> {
    let mut request = client.get(url);
    if let Some((offset, validator)) = resume {
        request = request
            .header(reqwest::header::RANGE, format!("bytes={offset}-"))
            .header(reqwest::header::IF_RANGE, validator);
    }
    let response = request
        .send()
        .await
        .with_context(|| format!("Failed to GET {url}"))?;

    if resume.is_some() && response.status() == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
        // The partial file is not a prefix of the remote file; start over.
        return Box::pin(fetch_download_response_from(client, url, None)).await;
    }

    if !response.status().is_success() {
        return http_error(response, url).await;
    }

    if response_is_html(&response)
        && let Some(file_id) = gamebanana_file_id_from_url(response.url())
    {
        unsupported_gamebanana_file_download::<Response>(file_id)?;
    }

    if response_is_html(&response) {
        anyhow::bail!(
            "Server returned HTML instead of a file for {url}. The download link may be broken"
        );
    }

    Ok(response)
}

async fn http_error<T>(response: Response, url: &str) -> Result<T> {
    let status = response.status();
    let body = response
        .text()
        .await
        .unwrap_or_else(|_| String::from("<response body unavailable>"));
    let body = body.trim();
    let snippet = if body.is_empty() {
        String::from("<empty response body>")
    } else {
        body.chars().take(200).collect()
    };
    anyhow::bail!("HTTP error {} for {url}: {snippet}", status)
}

fn response_is_html(response: &Response) -> bool {
    response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|content_type| content_type.starts_with("text/html"))
}

fn gamebanana_file_id_from_url(url: &Url) -> Option<u64> {
    let segments: Vec<_> = url.path_segments()?.collect();
    match segments.as_slice() {
        ["dl", file_id] => file_id.parse().ok(),
        _ => None,
    }
}

fn unsupported_gamebanana_file_download<T>(file_id: u64) -> Result<T> {
    anyhow::bail!(
        "GameBanana file pages do not expose a supported headless download path for file {file_id}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::external::test_http::{Reply, Request, serve};

    const BODY: &[u8] = b"0123456789";

    fn init() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }

    /// Serves BODY with a strong ETag and honors `Range` + `If-Range`.
    fn ranged_file(request: &Request, etag: &'static str) -> Reply {
        let range_start = request
            .header("Range")
            .and_then(|range| range.strip_prefix("bytes="))
            .and_then(|range| range.trim_end_matches('-').parse::<usize>().ok());
        let if_range_ok = request.header("If-Range").is_none_or(|value| value == etag);
        match range_start {
            Some(start) if start >= BODY.len() => {
                Reply::ok(Vec::new()).status("416 Range Not Satisfiable")
            }
            Some(start) if if_range_ok => Reply::ok(&BODY[start..])
                .status("206 Partial Content")
                .header("ETag", etag),
            _ => Reply::ok(BODY).header("ETag", etag),
        }
    }

    fn seed_partial(dest: &Path, bytes: &[u8], validator: Option<&str>) {
        let (part, validator_path) = partial_paths(dest);
        std::fs::write(part, bytes).unwrap();
        if let Some(validator) = validator {
            std::fs::write(validator_path, validator).unwrap();
        }
    }

    #[test]
    fn resumable_download_continues_a_matching_partial_file() {
        init();
        let (base, log) = serve(|request| ranged_file(request, "\"v1\""));
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("file.7z");
        seed_partial(&dest, b"01234", Some("\"v1\""));

        let mut first_progress = None;
        let mut progress = |downloaded: u64, total: Option<u64>| {
            first_progress.get_or_insert((downloaded, total));
            Ok(())
        };
        let validator =
            download_file_resumable(&format!("{base}/file"), &dest, Some(&mut progress)).unwrap();

        assert_eq!(std::fs::read(&dest).unwrap(), BODY);
        assert_eq!(validator.as_deref(), Some("\"v1\""));
        assert_eq!(first_progress, Some((10, Some(10))));
        let (part, validator_path) = partial_paths(&dest);
        assert!(!part.exists() && !validator_path.exists());
        assert_eq!(log.lock().unwrap().len(), 1);
    }

    #[test]
    fn resumable_download_restarts_when_the_remote_file_changed() {
        init();
        let (base, _) = serve(|request| ranged_file(request, "\"v2\""));
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("file.7z");
        seed_partial(&dest, b"xxxxx", Some("\"v1\""));

        download_file_resumable(&format!("{base}/file"), &dest, None).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), BODY);
    }

    #[test]
    fn resumable_download_restarts_without_a_validator_or_after_416() {
        init();
        let (base, log) = serve(|request| ranged_file(request, "\"v1\""));
        let tmp = tempfile::tempdir().unwrap();

        let no_validator = tmp.path().join("a.7z");
        seed_partial(&no_validator, b"xxxxx", None);
        download_file_resumable(&format!("{base}/a"), &no_validator, None).unwrap();
        assert_eq!(std::fs::read(&no_validator).unwrap(), BODY);

        let oversized = tmp.path().join("b.7z");
        seed_partial(&oversized, b"012345678901", Some("\"v1\""));
        download_file_resumable(&format!("{base}/b"), &oversized, None).unwrap();
        assert_eq!(std::fs::read(&oversized).unwrap(), BODY);
        // One request for the first file; a 416 and a full retry for the second.
        assert_eq!(log.lock().unwrap().len(), 3);

        remove_download(&oversized);
        assert!(!oversized.exists());
    }

    #[test]
    fn head_checks_report_validators_and_sizes() {
        init();
        let (base, _) = serve(|request| match request.path.as_str() {
            "/etag" => Reply::ok(BODY).header("ETag", "\"abc\""),
            "/weak" => Reply::ok(BODY)
                .header("ETag", "W/\"weak\"")
                .header("Last-Modified", "Sun, 20 Sep 2026 09:27:45 GMT"),
            "/page" => Reply::ok("<html></html>").header("Content-Type", "text/html"),
            _ => Reply::ok(Vec::new()).status("404 Not Found"),
        });

        assert_eq!(
            remote_validator(&format!("{base}/etag"))
                .unwrap()
                .as_deref(),
            Some("\"abc\"")
        );
        assert_eq!(
            remote_validator(&format!("{base}/weak"))
                .unwrap()
                .as_deref(),
            Some("Sun, 20 Sep 2026 09:27:45 GMT")
        );
        assert_eq!(remote_validator(&format!("{base}/page")).unwrap(), None);
        assert!(remote_validator(&format!("{base}/missing")).is_err());

        assert_eq!(remote_size(&format!("{base}/etag")).unwrap(), Some(10));
        assert_eq!(remote_size(&format!("{base}/page")).unwrap(), None);
        assert!(remote_size(&format!("{base}/missing")).is_err());
    }

    #[test]
    fn gamebanana_file_id_from_url_parses_dl_link() {
        let url = Url::parse("https://gamebanana.com/dl/1388911").unwrap();
        assert_eq!(gamebanana_file_id_from_url(&url), Some(1388911));
    }

    #[test]
    fn gamebanana_file_id_from_url_rejects_non_dl() {
        let url = Url::parse("https://gamebanana.com/mods/452445").unwrap();
        assert_eq!(gamebanana_file_id_from_url(&url), None);
    }

    #[test]
    fn gamebanana_mod_id_from_url_is_not_supported() {
        let url = Url::parse("https://gamebanana.com/mods/download/452445").unwrap();
        assert_eq!(gamebanana_file_id_from_url(&url), None);
    }

    #[test]
    fn gamebanana_file_download_is_unsupported() {
        let err = unsupported_gamebanana_file_download::<()>(1388911)
            .unwrap_err()
            .to_string();

        assert!(err.contains("supported headless download path"));
    }
}
