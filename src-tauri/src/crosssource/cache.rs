//! On-disk cache of cross-source resolution results, keyed by a file's sha1.
//! Positive results (the file exists on a platform) are immutable facts and
//! never expire; negative results ("not on Modrinth") carry a timestamp so a
//! later re-check can pick up files that get mirrored after the fact.

use super::{CurseforgeRef, ModrinthRef};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

/// How long a "not found" answer stays trusted before we ask the API again.
pub const NEGATIVE_TTL_SECS: i64 = 7 * 24 * 3600;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheEntry {
    #[serde(default)]
    pub modrinth: Option<ModrinthRef>,
    #[serde(default)]
    pub curseforge: Option<CurseforgeRef>,
    /// Unix time of the last Modrinth lookup (set on hits and misses alike).
    #[serde(default)]
    pub modrinth_checked_at: Option<i64>,
    /// Unix time of the last CurseForge fingerprint lookup.
    #[serde(default)]
    pub curseforge_checked_at: Option<i64>,
}

impl CacheEntry {
    /// True when the cached Modrinth answer (hit or miss) is still usable.
    pub fn modrinth_fresh(&self, now: i64) -> bool {
        answer_fresh(self.modrinth.is_some(), self.modrinth_checked_at, now)
    }

    /// True when the cached CurseForge answer (hit or miss) is still usable.
    pub fn curseforge_fresh(&self, now: i64) -> bool {
        answer_fresh(self.curseforge.is_some(), self.curseforge_checked_at, now)
    }
}

fn answer_fresh(positive: bool, checked_at: Option<i64>, now: i64) -> bool {
    match checked_at {
        None => false,
        Some(_) if positive => true,
        Some(t) => now - t < NEGATIVE_TTL_SECS,
    }
}

/// Lazily-loaded JSON cache. All mutation happens under the mutex and is
/// flushed to disk atomically before the lock is released, so concurrent
/// resolvers can't lose each other's writes.
pub struct DiskCache {
    path: PathBuf,
    entries: Mutex<Option<HashMap<String, CacheEntry>>>,
}

impl DiskCache {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            entries: Mutex::new(None),
        }
    }

    fn ensure_loaded(map: &mut Option<HashMap<String, CacheEntry>>, path: &PathBuf) {
        if map.is_none() {
            let loaded = std::fs::read(path)
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
                .unwrap_or_default();
            *map = Some(loaded);
        }
    }

    pub fn get(&self, sha1: &str) -> Option<CacheEntry> {
        let mut guard = self.entries.lock().unwrap();
        Self::ensure_loaded(&mut guard, &self.path);
        guard.as_ref().and_then(|m| m.get(sha1).cloned())
    }

    /// Mutate any number of entries in one lock + one disk write. Entries are
    /// created on first touch.
    pub fn record(&self, f: impl FnOnce(&mut HashMap<String, CacheEntry>)) {
        let mut guard = self.entries.lock().unwrap();
        Self::ensure_loaded(&mut guard, &self.path);
        let map = guard.as_mut().expect("cache loaded above");
        f(map);
        match serde_json::to_vec(map) {
            Ok(data) => {
                if let Err(e) = crate::instances::atomic_write(&self.path, &data) {
                    eprintln!("warning: could not persist cross-source cache: {e}");
                }
            }
            Err(e) => eprintln!("warning: could not serialize cross-source cache: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_cache_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("ezmapa_test_cache_{name}_{}.json", uuid::Uuid::new_v4()))
    }

    fn sample_modrinth() -> ModrinthRef {
        ModrinthRef {
            project_id: "AANobbMI".into(),
            version_id: "abc123".into(),
            url: "https://cdn.modrinth.com/data/AANobbMI/versions/abc123/sodium.jar".into(),
            filename: "sodium.jar".into(),
            sha1: "d".repeat(40),
            sha512: Some("e".repeat(128)),
            size: 1234,
        }
    }

    #[test]
    fn round_trips_through_disk() {
        let path = temp_cache_path("roundtrip");
        let sha1 = "a".repeat(40);
        {
            let cache = DiskCache::new(path.clone());
            cache.record(|map| {
                let e = map.entry(sha1.clone()).or_default();
                e.modrinth = Some(sample_modrinth());
                e.modrinth_checked_at = Some(1_700_000_000);
                e.curseforge = Some(CurseforgeRef {
                    project_id: 238222,
                    file_id: 42,
                });
                e.curseforge_checked_at = Some(1_700_000_000);
            });
        }
        // A fresh instance reads the same data back from disk.
        let cache = DiskCache::new(path.clone());
        let entry = cache.get(&sha1).expect("entry persisted");
        assert_eq!(entry.modrinth.as_ref().unwrap().project_id, "AANobbMI");
        assert_eq!(entry.curseforge.as_ref().unwrap().project_id, 238222);
        assert!(cache.get(&"b".repeat(40)).is_none());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn positive_answers_never_expire_negative_ones_do() {
        let now = 1_700_000_000;
        let old = now - NEGATIVE_TTL_SECS - 1;
        let recent = now - 60;

        let positive = CacheEntry {
            modrinth: Some(sample_modrinth()),
            modrinth_checked_at: Some(old),
            ..Default::default()
        };
        assert!(positive.modrinth_fresh(now));

        let recent_negative = CacheEntry {
            modrinth_checked_at: Some(recent),
            ..Default::default()
        };
        assert!(recent_negative.modrinth_fresh(now));

        let stale_negative = CacheEntry {
            modrinth_checked_at: Some(old),
            ..Default::default()
        };
        assert!(!stale_negative.modrinth_fresh(now));

        let never_checked = CacheEntry::default();
        assert!(!never_checked.modrinth_fresh(now));
        assert!(!never_checked.curseforge_fresh(now));
    }

    #[test]
    fn survives_corrupt_cache_file() {
        let path = temp_cache_path("corrupt");
        std::fs::write(&path, b"{not json").unwrap();
        let cache = DiskCache::new(path.clone());
        assert!(cache.get("anything").is_none());
        // Recording over a corrupt file replaces it with valid JSON.
        cache.record(|map| {
            map.entry("x".into()).or_default().modrinth_checked_at = Some(1);
        });
        let cache2 = DiskCache::new(path.clone());
        assert!(cache2.get("x").is_some());
        std::fs::remove_file(&path).ok();
    }
}
