use crate::error::{Error, Result};
use crate::models::TaskProgress;
use futures::stream::{self, StreamExt};
use sha1::{Digest, Sha1};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;

/// A single file to download.
#[derive(Clone)]
pub struct DownloadItem {
    pub url: String,
    pub path: PathBuf,
    pub sha1: Option<String>,
}

impl DownloadItem {
    pub fn new(url: impl Into<String>, path: PathBuf, sha1: Option<String>) -> Self {
        Self {
            url: url.into(),
            path,
            sha1,
        }
    }
}

pub fn sha1_hex(bytes: &[u8]) -> String {
    let mut h = Sha1::new();
    h.update(bytes);
    hex::encode(h.finalize())
}

pub async fn file_sha1(path: &Path) -> Result<String> {
    let bytes = tokio::fs::read(path).await?;
    Ok(sha1_hex(&bytes))
}

/// True when the file already exists and (if a hash is given) matches it.
async fn already_valid(item: &DownloadItem) -> bool {
    if !item.path.exists() {
        return false;
    }
    match &item.sha1 {
        Some(expected) => file_sha1(&item.path)
            .await
            .map(|h| h.eq_ignore_ascii_case(expected))
            .unwrap_or(false),
        None => true,
    }
}

/// Download a single file, verifying its checksum when provided.
pub async fn download_one(http: &reqwest::Client, item: &DownloadItem) -> Result<()> {
    if let Some(parent) = item.path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let bytes = http
        .get(&item.url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;

    if let Some(expected) = &item.sha1 {
        let actual = sha1_hex(&bytes);
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(Error::Checksum {
                file: item.path.display().to_string(),
                expected: expected.clone(),
                actual,
            });
        }
    }

    let tmp = item.path.with_extension("part");
    {
        let mut f = tokio::fs::File::create(&tmp).await?;
        f.write_all(&bytes).await?;
        f.flush().await?;
    }
    tokio::fs::rename(&tmp, &item.path).await?;
    Ok(())
}

/// Download many files concurrently, emitting `task://progress` events keyed by
/// `task_id`. Files that already exist with a matching hash are skipped.
pub async fn download_many(
    app: &AppHandle,
    http: &reqwest::Client,
    task_id: &str,
    label: &str,
    items: Vec<DownloadItem>,
    concurrency: usize,
) -> Result<()> {
    let total = items.len() as u64;
    if total == 0 {
        emit_progress(app, task_id, label, "download", 1, 1, true, None);
        return Ok(());
    }
    let done = Arc::new(AtomicU64::new(0));
    emit_progress(app, task_id, label, "download", 0, total, false, None);

    let results = stream::iter(items.into_iter().map(|item| {
        let http = http.clone();
        let done = done.clone();
        let app = app.clone();
        let task_id = task_id.to_string();
        let label = label.to_string();
        async move {
            let res = if already_valid(&item).await {
                Ok(())
            } else {
                download_one(&http, &item).await
            };
            let n = done.fetch_add(1, Ordering::SeqCst) + 1;
            emit_progress(&app, &task_id, &label, "download", n, total, false, None);
            res
        }
    }))
    .buffer_unordered(concurrency.max(1))
    .collect::<Vec<Result<()>>>()
    .await;

    for r in results {
        r?;
    }
    emit_progress(app, task_id, label, "download", total, total, true, None);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn emit_progress(
    app: &AppHandle,
    id: &str,
    label: &str,
    stage: &str,
    current: u64,
    total: u64,
    done: bool,
    error: Option<String>,
) {
    let _ = app.emit(
        "task://progress",
        TaskProgress {
            id: id.to_string(),
            label: label.to_string(),
            stage: stage.to_string(),
            current,
            total,
            done,
            error,
        },
    );
}

/// Fetch and deserialize JSON from a URL.
pub async fn get_json<T: serde::de::DeserializeOwned>(
    http: &reqwest::Client,
    url: &str,
) -> Result<T> {
    let v = http
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json::<T>()
        .await?;
    Ok(v)
}
