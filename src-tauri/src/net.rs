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

pub async fn file_sha1(path: &Path) -> Result<String> {
    use tokio::io::AsyncReadExt;
    // Stream the file through the hasher in fixed-size chunks rather than
    // reading the whole (potentially hundreds-of-MB) file into memory at once.
    let mut file = tokio::fs::File::open(path).await?;
    let mut hasher = Sha1::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
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

/// Download a single file, verifying its checksum when provided. When `counter`
/// is supplied, every received chunk bumps it so a concurrent task can report a
/// live byte total / speed.
pub async fn download_one_counted(
    http: &reqwest::Client,
    item: &DownloadItem,
    counter: Option<&AtomicU64>,
) -> Result<()> {
    if let Some(parent) = item.path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let mut resp = http.get(&item.url).send().await?.error_for_status()?;

    let mut hasher = item.sha1.as_ref().map(|_| Sha1::new());
    let tmp = item.path.with_extension("part");
    {
        let mut f = tokio::fs::File::create(&tmp).await?;
        while let Some(chunk) = resp.chunk().await? {
            if let Some(h) = hasher.as_mut() {
                h.update(&chunk);
            }
            if let Some(c) = counter {
                c.fetch_add(chunk.len() as u64, Ordering::Relaxed);
            }
            f.write_all(&chunk).await?;
        }
        f.flush().await?;
    }

    if let (Some(expected), Some(h)) = (&item.sha1, hasher) {
        let actual = hex::encode(h.finalize());
        if !actual.eq_ignore_ascii_case(expected) {
            tokio::fs::remove_file(&tmp).await.ok();
            return Err(Error::Checksum {
                file: item.path.display().to_string(),
                expected: expected.clone(),
                actual,
            });
        }
    }

    tokio::fs::rename(&tmp, &item.path).await?;
    Ok(())
}

/// Download a single file, verifying its checksum when provided.
pub async fn download_one(http: &reqwest::Client, item: &DownloadItem) -> Result<()> {
    download_one_counted(http, item, None).await
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
    download_many_cancellable(app, http, task_id, label, items, concurrency, None).await
}

#[allow(clippy::too_many_arguments)]
pub async fn download_many_cancellable(
    app: &AppHandle,
    http: &reqwest::Client,
    task_id: &str,
    label: &str,
    items: Vec<DownloadItem>,
    concurrency: usize,
    cancel: Option<Arc<std::sync::atomic::AtomicBool>>,
) -> Result<()> {
    let total = items.len() as u64;
    if total == 0 {
        let _ = app.emit(
            "task://progress",
            TaskProgress::simple(task_id, label, "download", 1, 1, true, None),
        );
        return Ok(());
    }

    let done = Arc::new(AtomicU64::new(0));
    let bytes = Arc::new(AtomicU64::new(0));
    let current = Arc::new(std::sync::Mutex::new(String::new()));
    let start = std::time::Instant::now();

    let emit = |cur: u64, fin: bool| {
        let elapsed = start.elapsed().as_secs_f64().max(0.001);
        let b = bytes.load(Ordering::Relaxed);
        let file = current.lock().map(|g| g.clone()).unwrap_or_default();
        let _ = app.emit(
            "task://progress",
            TaskProgress {
                id: task_id.to_string(),
                label: label.to_string(),
                stage: "download".into(),
                current: cur,
                total,
                done: fin,
                error: None,
                bytes_done: Some(b),
                speed: Some((b as f64 / elapsed) as u64),
                current_file: if file.is_empty() { None } else { Some(file) },
            },
        );
    };
    emit(0, false);

    let results = stream::iter(items.into_iter().map(|item| {
        let http = http.clone();
        let done = done.clone();
        let bytes = bytes.clone();
        let current = current.clone();
        let app = app.clone();
        let task_id = task_id.to_string();
        let label = label.to_string();
        let start = start;
        let cancel = cancel.clone();
        async move {
            // Short-circuit any not-yet-started file once cancellation is asked.
            if cancel
                .as_ref()
                .map(|c| c.load(Ordering::SeqCst))
                .unwrap_or(false)
            {
                return Err(Error::Other("Download cancelled.".into()));
            }
            if let Some(name) = item.path.file_name().and_then(|n| n.to_str()) {
                if let Ok(mut g) = current.lock() {
                    *g = name.to_string();
                }
            }
            let res = if already_valid(&item).await {
                Ok(())
            } else {
                download_one_counted(&http, &item, Some(&bytes)).await
            };
            let n = done.fetch_add(1, Ordering::SeqCst) + 1;
            let elapsed = start.elapsed().as_secs_f64().max(0.001);
            let b = bytes.load(Ordering::Relaxed);
            let file = current.lock().map(|g| g.clone()).unwrap_or_default();
            let _ = app.emit(
                "task://progress",
                TaskProgress {
                    id: task_id.clone(),
                    label: label.clone(),
                    stage: "download".into(),
                    current: n,
                    total,
                    done: false,
                    error: None,
                    bytes_done: Some(b),
                    speed: Some((b as f64 / elapsed) as u64),
                    current_file: if file.is_empty() { None } else { Some(file) },
                },
            );
            res
        }
    }))
    .buffer_unordered(concurrency.max(1))
    .collect::<Vec<Result<()>>>()
    .await;

    for r in results {
        r?;
    }
    emit(total, true);
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
        TaskProgress::simple(id, label, stage, current, total, done, error),
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
