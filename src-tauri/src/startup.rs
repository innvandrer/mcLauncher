//! Startup-time measurement: how long from process spawn until the game is
//! actually usable (window/title screen ready or world joined), detected from
//! the client log. Samples are stored per instance and grouped by a
//! fingerprint of the effective JVM settings, so the instance page can show
//! "avg startup before/after" around a flag change. Fully local — nothing
//! leaves the machine.

use crate::state::AppDirs;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupSample {
    /// Unix time the session started.
    pub started: i64,
    /// Milliseconds from process spawn to the first readiness marker.
    pub startup_ms: u64,
    /// Fingerprint of the JVM settings this session launched with.
    pub args_fingerprint: String,
}

/// Fingerprint of the effective JVM settings (heap + args, whitespace
/// normalized) so samples group correctly across cosmetic edits.
pub fn args_fingerprint(memory_mb: u32, jvm_args: &str) -> String {
    use sha1::{Digest, Sha1};
    let normalized = jvm_args.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut h = Sha1::new();
    h.update(format!("{memory_mb}|{normalized}").as_bytes());
    hex::encode(h.finalize())[..12].to_string()
}

/// Log lines that mean "the game is up": sound-engine start fires when the
/// title screen is ready (1.14+); the world markers cover shortcuts that jump
/// straight into a world. First match wins.
const READY_MARKERS: &[&str] = &[
    "Sound engine started",
    "Preparing spawn area",
    "Joining world",
];

pub fn is_ready_marker(line: &str) -> bool {
    READY_MARKERS.iter().any(|m| line.contains(m))
}

/// Watches a session's log lines and records one sample on the first
/// readiness marker. Shared by the stdout and stderr reader threads.
pub struct StartupTracker {
    dirs: AppDirs,
    instance_id: String,
    fingerprint: String,
    start: std::time::Instant,
    started_at: i64,
    fired: AtomicBool,
}

impl StartupTracker {
    pub fn new(dirs: AppDirs, instance_id: String, fingerprint: String) -> Arc<Self> {
        Arc::new(Self {
            dirs,
            instance_id,
            fingerprint,
            start: std::time::Instant::now(),
            started_at: chrono::Utc::now().timestamp(),
            fired: AtomicBool::new(false),
        })
    }

    pub fn observe_line(&self, line: &str) {
        if !is_ready_marker(line) {
            return;
        }
        if self.fired.swap(true, Ordering::SeqCst) {
            return;
        }
        let sample = StartupSample {
            started: self.started_at,
            startup_ms: self.start.elapsed().as_millis().min(u64::MAX as u128) as u64,
            args_fingerprint: self.fingerprint.clone(),
        };
        record_sample_dirs(&self.dirs, &self.instance_id, sample);
    }
}

fn stats_path(dirs: &AppDirs, instance_id: &str) -> std::path::PathBuf {
    dirs.instance_dir(instance_id).join("startup_stats.json")
}

/// Append a sample (best-effort, capped to the most recent 200).
pub fn record_sample_dirs(dirs: &AppDirs, instance_id: &str, sample: StartupSample) {
    let path = stats_path(dirs, instance_id);
    let mut samples: Vec<StartupSample> = std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    samples.push(sample);
    let len = samples.len();
    if len > 200 {
        samples.drain(0..len - 200);
    }
    if let Ok(data) = serde_json::to_vec_pretty(&samples) {
        let _ = crate::instances::atomic_write(&path, &data);
    }
}

pub fn list_samples_dirs(dirs: &AppDirs, instance_id: &str) -> Vec<StartupSample> {
    std::fs::read(stats_path(dirs, instance_id))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupStat {
    pub fingerprint: String,
    pub avg_ms: u64,
    pub count: u32,
}

/// Startup averages for the instance page: sessions launched with the
/// current JVM settings vs. the most recently used different settings
/// ("before the flag change").
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupStats {
    pub current: Option<GroupStat>,
    pub previous: Option<GroupStat>,
}

fn group_stat(samples: &[StartupSample], fingerprint: &str) -> Option<GroupStat> {
    let times: Vec<u64> = samples
        .iter()
        .filter(|s| s.args_fingerprint == fingerprint)
        .map(|s| s.startup_ms)
        .collect();
    if times.is_empty() {
        return None;
    }
    Some(GroupStat {
        fingerprint: fingerprint.to_string(),
        avg_ms: times.iter().sum::<u64>() / times.len() as u64,
        count: times.len() as u32,
    })
}

/// Pure aggregation: `current_fingerprint` is the instance's settings now;
/// "previous" is the newest sample recorded under different settings.
pub fn compute_stats(samples: &[StartupSample], current_fingerprint: &str) -> StartupStats {
    let previous_fp = samples
        .iter()
        .rev()
        .find(|s| s.args_fingerprint != current_fingerprint)
        .map(|s| s.args_fingerprint.clone());
    StartupStats {
        current: group_stat(samples, current_fingerprint),
        previous: previous_fp.and_then(|fp| group_stat(samples, &fp)),
    }
}

/// Stats for an instance, keyed to its current effective JVM settings.
pub fn startup_stats(
    state: &crate::state::AppState,
    instance_id: &str,
) -> crate::error::Result<StartupStats> {
    let instance = crate::instances::get_instance(state, instance_id)?;
    let settings = crate::instances::load_settings(state);
    let mem = instance.memory_mb.unwrap_or(settings.memory_mb);
    let args = instance.jvm_args.unwrap_or(settings.jvm_args);
    let fp = args_fingerprint(mem, &args);
    let samples = list_samples_dirs(&state.dirs, instance_id);
    Ok(compute_stats(&samples, &fp))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(started: i64, ms: u64, fp: &str) -> StartupSample {
        StartupSample {
            started,
            startup_ms: ms,
            args_fingerprint: fp.into(),
        }
    }

    #[test]
    fn fingerprint_normalizes_whitespace_but_not_order() {
        let a = args_fingerprint(4096, "-XX:+UseZGC  -XX:+ZGenerational");
        let b = args_fingerprint(4096, "-XX:+UseZGC -XX:+ZGenerational ");
        let c = args_fingerprint(4096, "-XX:+ZGenerational -XX:+UseZGC");
        let d = args_fingerprint(6144, "-XX:+UseZGC -XX:+ZGenerational");
        assert_eq!(a, b);
        assert_ne!(a, c); // order changes the effective command line
        assert_ne!(a, d); // heap size is part of the settings
        assert_eq!(a.len(), 12);
    }

    #[test]
    fn ready_markers_match_real_log_lines() {
        assert!(is_ready_marker(
            "[Render thread/INFO]: Sound engine started"
        ));
        assert!(is_ready_marker(
            "[Server thread/INFO]: Preparing spawn area: 0%"
        ));
        assert!(!is_ready_marker("[main/INFO]: Loading Minecraft 1.21.1"));
        assert!(!is_ready_marker("[Render thread/INFO]: Created: 1024x1024 textures-atlas"));
    }

    #[test]
    fn stats_split_before_and_after_a_flag_change() {
        let samples = vec![
            sample(100, 40_000, "old-flags"),
            sample(200, 44_000, "old-flags"),
            sample(300, 30_000, "new-flags"),
            sample(400, 34_000, "new-flags"),
        ];
        let stats = compute_stats(&samples, "new-flags");
        let cur = stats.current.unwrap();
        assert_eq!(cur.avg_ms, 32_000);
        assert_eq!(cur.count, 2);
        let prev = stats.previous.unwrap();
        assert_eq!(prev.fingerprint, "old-flags");
        assert_eq!(prev.avg_ms, 42_000);
        assert_eq!(prev.count, 2);
    }

    #[test]
    fn stats_handle_missing_groups() {
        let stats = compute_stats(&[], "whatever");
        assert!(stats.current.is_none());
        assert!(stats.previous.is_none());

        // Only current settings ever measured: no "before".
        let samples = vec![sample(1, 10_000, "fp")];
        let stats = compute_stats(&samples, "fp");
        assert_eq!(stats.current.unwrap().count, 1);
        assert!(stats.previous.is_none());

        // Only old settings measured (fresh flag change, no run yet).
        let samples = vec![sample(1, 10_000, "old")];
        let stats = compute_stats(&samples, "new");
        assert!(stats.current.is_none());
        assert_eq!(stats.previous.unwrap().count, 1);
    }
}
