use std::path::Path;

use anyhow::{Context, Result};
use futures_util::StreamExt;
use reqwest::{Client, Response, Url};
use serde_json::Value;

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

async fn download_file_async(url: &str, dest: &Path, progress: Option<ProgressFn>) -> Result<()> {
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
        && let Some(mod_id) = gamebanana_mod_id_from_url(response.url())
    {
        let resolved_url = resolve_gamebanana_download_url(client, mod_id).await?;
        let fallback_response = client
            .get(&resolved_url)
            .send()
            .await
            .with_context(|| format!("Failed to GET {resolved_url}"))?;

        if !fallback_response.status().is_success() {
            return http_error(fallback_response, &resolved_url).await;
        }

        if response_is_html(&fallback_response) {
            anyhow::bail!(
                "GameBanana returned HTML for mod {mod_id} even after resolving its latest file"
            );
        }

        return Ok(fallback_response);
    }

    if response_is_html(&response)
        && let Some(file_id) = gamebanana_file_id_from_url(response.url())
    {
        let resolved_url = resolve_gamebanana_file_url(client, file_id).await?;
        let fallback_response = client
            .get(&resolved_url)
            .send()
            .await
            .with_context(|| format!("Failed to GET {resolved_url}"))?;

        if !fallback_response.status().is_success() {
            return http_error(fallback_response, &resolved_url).await;
        }

        if response_is_html(&fallback_response) {
            anyhow::bail!(
                "GameBanana returned HTML for file {file_id} even after resolving via API"
            );
        }

        return Ok(fallback_response);
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

fn gamebanana_mod_id_from_url(url: &Url) -> Option<u64> {
    let segments: Vec<_> = url.path_segments()?.collect();
    match segments.as_slice() {
        ["mods", "download", mod_id] => mod_id.parse().ok(),
        _ => None,
    }
}

fn gamebanana_file_id_from_url(url: &Url) -> Option<u64> {
    let segments: Vec<_> = url.path_segments()?.collect();
    match segments.as_slice() {
        ["dl", file_id] => file_id.parse().ok(),
        _ => None,
    }
}

async fn resolve_gamebanana_download_url(client: &Client, mod_id: u64) -> Result<String> {
    let api_url = format!(
        "https://api.gamebanana.com/Core/Item/Data?itemtype=Mod&itemid={mod_id}&fields=Files().aFiles()"
    );
    let response = client
        .get(&api_url)
        .send()
        .await
        .with_context(|| format!("Failed to query GameBanana API at {api_url}"))?;

    if !response.status().is_success() {
        return http_error(response, &api_url).await;
    }

    let payload = response
        .text()
        .await
        .with_context(|| format!("Failed to read GameBanana API response for mod {mod_id}"))?;

    pick_gamebanana_download_url(&payload).with_context(|| {
        format!("Failed to resolve a downloadable file for GameBanana mod {mod_id}")
    })
}

async fn resolve_gamebanana_file_url(client: &Client, file_id: u64) -> Result<String> {
    let api_url = format!(
        "https://api.gamebanana.com/Core/Item/Data?itemtype=File&itemid={file_id}&fields=sDownloadUrl()"
    );
    let response = client
        .get(&api_url)
        .send()
        .await
        .with_context(|| format!("Failed to query GameBanana API at {api_url}"))?;

    if !response.status().is_success() {
        return http_error(response, &api_url).await;
    }

    let payload = response
        .text()
        .await
        .with_context(|| format!("Failed to read GameBanana API response for file {file_id}"))?;

    pick_gamebanana_file_url(&payload)
        .with_context(|| format!("Failed to resolve a downloadable file for GameBanana file {file_id}"))
}

fn pick_gamebanana_download_url(payload: &str) -> Result<String> {
    let value: Value = serde_json::from_str(payload).context("Invalid GameBanana API JSON")?;
    let files = value
        .as_array()
        .and_then(|items| items.first())
        .and_then(Value::as_object)
        .context("GameBanana API returned no files")?;

    let best = files
        .values()
        .filter_map(|file| {
            let download_url = file.get("_sDownloadUrl")?.as_str()?;
            let added = file
                .get("_tsDateAdded")
                .and_then(Value::as_i64)
                .unwrap_or(0);
            Some((added, download_url))
        })
        .max_by_key(|(added, _)| *added)
        .map(|(_, url)| url.to_string())
        .context("GameBanana API files did not include a usable download URL")?;

    Ok(best)
}

fn pick_gamebanana_file_url(payload: &str) -> Result<String> {
    let value: Value = serde_json::from_str(payload).context("Invalid GameBanana File API JSON")?;
    value
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(Value::as_str)
        .map(String::from)
        .context("GameBanana File API did not return a download URL")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gamebanana_mod_id_from_url() {
        let url = Url::parse("https://gamebanana.com/mods/download/452445").unwrap();
        assert_eq!(gamebanana_mod_id_from_url(&url), Some(452445));
    }

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
    fn test_pick_gamebanana_download_url_prefers_newest_file() {
        let payload = r#"[
            {
                "111": {
                    "_sDownloadUrl": "https://gamebanana.com/dl/111",
                    "_tsDateAdded": 100
                },
                "222": {
                    "_sDownloadUrl": "https://gamebanana.com/dl/222",
                    "_tsDateAdded": 200
                }
            }
        ]"#;

        let resolved = pick_gamebanana_download_url(payload).unwrap();
        assert_eq!(resolved, "https://gamebanana.com/dl/222");
    }

    #[test]
    fn test_resolve_gamebanana_file_url_parses_response() {
        let payload = r#"["https://files.gamebanana.com/mods/file.7z"]"#;
        let parsed = pick_gamebanana_file_url(payload).unwrap();
        assert_eq!(parsed, "https://files.gamebanana.com/mods/file.7z");
    }

    #[test]
    fn test_resolve_gamebanana_file_url_rejects_empty() {
        let payload = r#"[null]"#;
        assert!(pick_gamebanana_file_url(payload).is_err());
    }
}
