use std::path::Path;

use anyhow::{Context, Result};
use futures_util::StreamExt;
use reqwest::{Client, Response, Url};

/// Progress callback: (downloaded_bytes, total_bytes_if_known)
pub type ProgressFn = Box<dyn Fn(u64, Option<u64>) + Send>;

/// Download a file from a URL with progress reporting.
///
/// This function creates its own tokio runtime internally,
/// so it must be called from a blocking thread (e.g. `gio::spawn_blocking`),
/// NOT from an async context.
pub fn download_file(url: &str, dest: &Path, progress: Option<ProgressFn>) -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("Failed to create tokio runtime")?;

    let progress_ref: Option<&dyn Fn(u64, Option<u64>)> =
        progress.as_deref().map(|f| f as &dyn Fn(u64, Option<u64>));
    rt.block_on(download_file_async(url, dest, progress_ref))
}

/// Like `download_file` but accepts any `FnMut` without `Send` or `'static` bounds.
/// Use when the callback captures non-Send state on the current blocking thread.
pub fn download_file_with(
    url: &str,
    dest: &Path,
    progress: Option<&mut dyn FnMut(u64, Option<u64>) -> Result<()>>,
) -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("Failed to create tokio runtime")?;

    rt.block_on(download_file_async_mut(url, dest, progress))
}

async fn download_file_async_mut(
    url: &str,
    dest: &Path,
    mut progress: Option<&mut dyn FnMut(u64, Option<u64>) -> Result<()>>,
) -> Result<()> {
    let client = Client::new();
    let response = fetch_download_response(&client, url).await?;

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

async fn download_file_async(
    url: &str,
    dest: &Path,
    progress: Option<&dyn Fn(u64, Option<u64>)>,
) -> Result<()> {
    let client = Client::new();
    let response = fetch_download_response(&client, url).await?;

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

        if let Some(ref progress) = progress {
            progress(downloaded, total);
        }
    }

    file.flush()?;
    Ok(())
}

async fn fetch_download_response(client: &Client, url: &str) -> Result<Response> {
    let response = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("Failed to GET {url}"))?;

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

    #[test]
    fn test_gamebanana_file_id_from_url() {
        let url = Url::parse("https://gamebanana.com/dl/1388911").unwrap();
        assert_eq!(gamebanana_file_id_from_url(&url), Some(1388911));
    }

    #[test]
    fn test_gamebanana_file_id_from_url_rejects_non_dl() {
        let url = Url::parse("https://gamebanana.com/mods/452445").unwrap();
        assert_eq!(gamebanana_file_id_from_url(&url), None);
    }

    #[test]
    fn test_gamebanana_mod_id_from_url_is_not_supported() {
        let url = Url::parse("https://gamebanana.com/mods/download/452445").unwrap();
        assert_eq!(gamebanana_file_id_from_url(&url), None);
    }

    #[test]
    fn test_gamebanana_file_download_is_unsupported() {
        let err = unsupported_gamebanana_file_download::<()>(1388911)
            .unwrap_err()
            .to_string();

        assert!(err.contains("supported headless download path"));
    }
}
