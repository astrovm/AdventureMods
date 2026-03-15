use std::path::Path;

use anyhow::{Context, Result};
use futures_util::StreamExt;

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

    rt.block_on(download_file_async(url, dest, progress))
}

/// Resolve a GameBanana file ID to its download URL using the v11 API.
///
/// This function creates its own tokio runtime internally,
/// so it must be called from a blocking thread (e.g. `gio::spawn_blocking`),
/// NOT from an async context.
pub fn resolve_gamebanana_url(file_id: u64) -> Result<(String, String)> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("Failed to create tokio runtime")?;

    rt.block_on(resolve_gamebanana_url_async(file_id))
}

async fn download_file_async(
    url: &str,
    dest: &Path,
    progress: Option<ProgressFn>,
) -> Result<()> {
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("Failed to GET {url}"))?
        .error_for_status()
        .with_context(|| format!("HTTP error for {url}"))?;

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

async fn resolve_gamebanana_url_async(file_id: u64) -> Result<(String, String)> {
    let api_url = format!(
        "https://gamebanana.com/apiv11/File/{file_id}?_csvProperties=_sDownloadUrl,_sFile"
    );

    let client = reqwest::Client::new();
    let response = client
        .get(&api_url)
        .send()
        .await
        .with_context(|| format!("Failed to query GameBanana API for file {file_id}"))?
        .error_for_status()?;

    let response: serde_json::Value = response.json().await?;

    let download_url = response["_sDownloadUrl"]
        .as_str()
        .context("Missing _sDownloadUrl in GameBanana response")?
        .to_string();

    let filename = response["_sFile"]
        .as_str()
        .context("Missing _sFile in GameBanana response")?
        .to_string();

    Ok((download_url, filename))
}
