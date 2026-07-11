//! Cross-source mod identity: given a mod file (or its hashes), find the same
//! file on the *other* platform. Modrinth lookups go through the batch
//! `POST /v2/version_files` endpoint (sha1); CurseForge lookups go through the
//! `POST /v1/fingerprints/{game}` endpoint (murmur2 over whitespace-stripped
//! bytes — see [`murmur2`]). Results are cached on disk so repeat lookups are
//! free, and every API call passes a per-provider token-bucket rate limiter.
//!
//! Direction support: CurseForge → Modrinth works from metadata alone (CF
//! serves a sha1 for most files). Modrinth → CurseForge requires the actual
//! file bytes to fingerprint, so it is only supported for files we hold
//! locally (which covers update checking and pack export). Resolving a
//! remote-only Modrinth version to CurseForge is intentionally unsupported.

// The module lands one phase ahead of its consumers (blocked-download
// fallback, unified updates, dual export). Remove once Phase 1 calls into it.
#![allow(dead_code)]

pub mod murmur2;

mod cache;
mod limiter;

use crate::error::Result;
use cache::DiskCache;
use limiter::RateLimiter;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const MODRINTH_API: &str = "https://api.modrinth.com/v2";
const CURSEFORGE_API: &str = "https://api.curseforge.com/v1";
const CURSEFORGE_GAME_ID: u32 = 432; // Minecraft

/// Max hashes/fingerprints per batch request, kept well under API limits.
const BATCH_SIZE: usize = 800;

/// The Modrinth identity of a file, resolved by exact sha1 match.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModrinthRef {
    pub project_id: String,
    pub version_id: String,
    pub url: String,
    pub filename: String,
    pub sha1: String,
    #[serde(default)]
    pub sha512: Option<String>,
    #[serde(default)]
    pub size: u64,
}

/// The CurseForge identity of a file.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseforgeRef {
    pub project_id: u32,
    pub file_id: u32,
}

/// Both identities of one file, when known.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrossRef {
    pub modrinth: Option<ModrinthRef>,
    pub curseforge: Option<CurseforgeRef>,
}

/// Compute sha1 + sha512 of a file in one streaming pass (the pair Modrinth
/// uses to identify files).
pub async fn file_sha1_sha512(path: &Path) -> Result<(String, String)> {
    use sha1::{Digest, Sha1};
    use sha2::Sha512;
    use tokio::io::AsyncReadExt;

    let mut file = tokio::fs::File::open(path).await?;
    let mut sha1 = Sha1::new();
    let mut sha512 = Sha512::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        sha1.update(&buf[..n]);
        sha512.update(&buf[..n]);
    }
    Ok((hex::encode(sha1.finalize()), hex::encode(sha512.finalize())))
}

// ---------------------------------------------------------------------------
// API response shapes
// ---------------------------------------------------------------------------

/// `POST /v1/fingerprints/{game}` response (only the fields we need).
#[derive(Deserialize)]
struct CfFingerprintsResponse {
    data: CfFingerprintsData,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfFingerprintsData {
    #[serde(default)]
    exact_matches: Vec<CfFingerprintMatch>,
}

#[derive(Deserialize)]
struct CfFingerprintMatch {
    file: CfMatchedFile,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfMatchedFile {
    id: u32,
    mod_id: u32,
    file_fingerprint: u64,
}

/// Pick the file inside a Modrinth version whose sha1 is exactly `sha1`.
fn modrinth_ref_from_version(sha1: &str, v: &crate::modrinth::ProjectVersion) -> Option<ModrinthRef> {
    let file = v
        .files
        .iter()
        .find(|f| f.hashes.sha1.as_deref() == Some(sha1))?;
    if v.project_id.is_empty() {
        return None;
    }
    Some(ModrinthRef {
        project_id: v.project_id.clone(),
        version_id: v.id.clone(),
        url: file.url.clone(),
        filename: file.filename.clone(),
        sha1: sha1.to_string(),
        sha512: file.hashes.sha512.clone(),
        size: file.size,
    })
}

// ---------------------------------------------------------------------------
// Resolver
// ---------------------------------------------------------------------------

/// Shared resolver held in [`crate::state::AppState`]: disk cache plus one
/// rate limiter per provider.
pub struct Resolver {
    cache: DiskCache,
    modrinth_limiter: RateLimiter,
    curseforge_limiter: RateLimiter,
}

impl Resolver {
    pub fn new(cache_path: PathBuf) -> Self {
        Self {
            cache: DiskCache::new(cache_path),
            // Modrinth documents 300 req/min; stay under it with headroom.
            modrinth_limiter: RateLimiter::new(20, 240),
            // CurseForge publishes no number; be conservative.
            curseforge_limiter: RateLimiter::new(10, 60),
        }
    }

    /// Resolve many sha1 hashes against Modrinth in as few requests as
    /// possible. Cached answers (including still-fresh misses) never hit the
    /// network. The returned map only contains hashes that resolved.
    pub async fn resolve_hashes_modrinth(
        &self,
        http: &reqwest::Client,
        hashes: &[String],
    ) -> Result<HashMap<String, ModrinthRef>> {
        let now = chrono::Utc::now().timestamp();
        let mut out = HashMap::new();
        let mut to_query: Vec<String> = Vec::new();

        for h in hashes {
            let h = h.to_lowercase();
            match self.cache.get(&h) {
                Some(entry) if entry.modrinth_fresh(now) => {
                    if let Some(m) = entry.modrinth {
                        out.insert(h, m);
                    }
                }
                _ => to_query.push(h),
            }
        }
        to_query.sort();
        to_query.dedup();

        for chunk in to_query.chunks(BATCH_SIZE) {
            self.modrinth_limiter.acquire().await;
            let body = serde_json::json!({ "hashes": chunk, "algorithm": "sha1" });
            let resp: HashMap<String, crate::modrinth::ProjectVersion> = http
                .post(format!("{MODRINTH_API}/version_files"))
                .json(&body)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;

            let mut resolved: HashMap<String, ModrinthRef> = HashMap::new();
            for (hash, version) in &resp {
                if let Some(r) = modrinth_ref_from_version(hash, version) {
                    resolved.insert(hash.to_lowercase(), r);
                }
            }
            self.cache.record(|map| {
                for h in chunk {
                    let entry = map.entry(h.clone()).or_default();
                    entry.modrinth = resolved.get(h).cloned();
                    entry.modrinth_checked_at = Some(now);
                }
            });
            out.extend(resolved);
        }

        Ok(out)
    }

    /// Resolve a single sha1 to its Modrinth identity, if the file exists there.
    pub async fn resolve_by_hash(
        &self,
        http: &reqwest::Client,
        sha1: &str,
    ) -> Result<Option<ModrinthRef>> {
        let map = self
            .resolve_hashes_modrinth(http, std::slice::from_ref(&sha1.to_string()))
            .await?;
        Ok(map.into_values().next())
    }

    /// Resolve a CurseForge file to its Modrinth equivalent using the sha1
    /// from CF's file metadata, recording the CF identity in the cache so the
    /// reverse direction is answered for free later.
    pub async fn resolve_cf_file_to_modrinth(
        &self,
        http: &reqwest::Client,
        cf: CurseforgeRef,
        sha1: &str,
    ) -> Result<Option<ModrinthRef>> {
        let sha1 = sha1.to_lowercase();
        let now = chrono::Utc::now().timestamp();
        self.cache.record(|map| {
            let entry = map.entry(sha1.clone()).or_default();
            entry.curseforge = Some(cf);
            entry.curseforge_checked_at = Some(now);
        });
        self.resolve_by_hash(http, &sha1).await
    }

    /// Resolve local files to their CurseForge identities via the fingerprint
    /// API. `files` pairs each file's sha1 (the cache key) with its path; the
    /// returned map is keyed by sha1 and only contains files CF recognised.
    ///
    /// This is the supported "Modrinth → CurseForge" direction: it needs the
    /// file bytes, so it only works for files already on disk.
    pub async fn resolve_local_files_to_cf(
        &self,
        http: &reqwest::Client,
        api_key: &str,
        files: &[(String, PathBuf)],
    ) -> Result<HashMap<String, CurseforgeRef>> {
        let now = chrono::Utc::now().timestamp();
        let mut out = HashMap::new();
        // sha1 → fingerprint for files whose cached answer is stale/missing.
        let mut to_query: Vec<(String, u32)> = Vec::new();

        for (sha1, path) in files {
            let sha1 = sha1.to_lowercase();
            match self.cache.get(&sha1) {
                Some(entry) if entry.curseforge_fresh(now) => {
                    if let Some(c) = entry.curseforge {
                        out.insert(sha1, c);
                    }
                }
                _ => {
                    let fp = murmur2::cf_fingerprint_file(path).await?;
                    to_query.push((sha1, fp));
                }
            }
        }

        for chunk in to_query.chunks(BATCH_SIZE) {
            let fingerprints: Vec<u32> = chunk.iter().map(|(_, fp)| *fp).collect();
            self.curseforge_limiter.acquire().await;
            let body = serde_json::json!({ "fingerprints": fingerprints });
            let resp: CfFingerprintsResponse = http
                .post(format!("{CURSEFORGE_API}/fingerprints/{CURSEFORGE_GAME_ID}"))
                .header("x-api-key", api_key)
                .header("Accept", "application/json")
                .json(&body)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;

            let by_fingerprint: HashMap<u64, CurseforgeRef> = resp
                .data
                .exact_matches
                .iter()
                .map(|m| {
                    (
                        m.file.file_fingerprint,
                        CurseforgeRef {
                            project_id: m.file.mod_id,
                            file_id: m.file.id,
                        },
                    )
                })
                .collect();

            let mut resolved: Vec<(String, Option<CurseforgeRef>)> = Vec::new();
            for (sha1, fp) in chunk {
                resolved.push((sha1.clone(), by_fingerprint.get(&(*fp as u64)).copied()));
            }
            self.cache.record(|map| {
                for (sha1, cf) in &resolved {
                    let entry = map.entry(sha1.clone()).or_default();
                    entry.curseforge = *cf;
                    entry.curseforge_checked_at = Some(now);
                }
            });
            for (sha1, cf) in resolved {
                if let Some(c) = cf {
                    out.insert(sha1, c);
                }
            }
        }

        Ok(out)
    }

    /// Resolve one local file to its CurseForge identity (see
    /// [`Self::resolve_local_files_to_cf`] for the direction caveat).
    pub async fn resolve_local_file_to_cf(
        &self,
        http: &reqwest::Client,
        api_key: &str,
        sha1: &str,
        path: &Path,
    ) -> Result<Option<CurseforgeRef>> {
        let map = self
            .resolve_local_files_to_cf(
                http,
                api_key,
                &[(sha1.to_string(), path.to_path_buf())],
            )
            .await?;
        Ok(map.into_values().next())
    }

    /// Wait for a Modrinth request slot. For callers that talk to the API
    /// directly (search, project lookups) but should share this limiter.
    pub async fn throttle_modrinth(&self) {
        self.modrinth_limiter.acquire().await;
    }

    /// Wait for a CurseForge request slot (see [`Self::throttle_modrinth`]).
    pub async fn throttle_curseforge(&self) {
        self.curseforge_limiter.acquire().await;
    }

    /// Everything we know about a file from the cache alone (no network).
    pub fn cached(&self, sha1: &str) -> CrossRef {
        match self.cache.get(&sha1.to_lowercase()) {
            Some(entry) => CrossRef {
                modrinth: entry.modrinth,
                curseforge: entry.curseforge,
            },
            None => CrossRef::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version_json(project_id: &str, sha1: &str) -> serde_json::Value {
        serde_json::json!({
            "id": "vErSiOn1",
            "project_id": project_id,
            "version_number": "1.2.3",
            "name": "Release 1.2.3",
            "date_published": "2025-06-01T12:00:00Z",
            "game_versions": ["1.21.1"],
            "loaders": ["fabric"],
            "files": [
                {
                    "url": "https://cdn.modrinth.com/data/AANobbMI/versions/vErSiOn1/other.jar",
                    "filename": "other.jar",
                    "primary": false,
                    "size": 10,
                    "hashes": { "sha1": "f".repeat(40), "sha512": "a".repeat(128) }
                },
                {
                    "url": "https://cdn.modrinth.com/data/AANobbMI/versions/vErSiOn1/sodium.jar",
                    "filename": "sodium.jar",
                    "primary": true,
                    "size": 42,
                    "hashes": { "sha1": sha1, "sha512": "b".repeat(128) }
                }
            ],
            "dependencies": []
        })
    }

    #[test]
    fn picks_file_matching_requested_hash() {
        let sha1 = "c".repeat(40);
        let v: crate::modrinth::ProjectVersion =
            serde_json::from_value(version_json("AANobbMI", &sha1)).unwrap();
        let r = modrinth_ref_from_version(&sha1, &v).expect("resolved");
        assert_eq!(r.project_id, "AANobbMI");
        assert_eq!(r.version_id, "vErSiOn1");
        assert_eq!(r.filename, "sodium.jar");
        assert_eq!(r.size, 42);
        assert_eq!(r.sha512.as_deref(), Some("b".repeat(128).as_str()));

        // A hash that matches no file in the version resolves to nothing —
        // never fall back to a *different* file's URL.
        assert!(modrinth_ref_from_version(&"9".repeat(40), &v).is_none());
    }

    #[test]
    fn missing_project_id_is_not_resolved() {
        let sha1 = "c".repeat(40);
        let mut json = version_json("", &sha1);
        json.as_object_mut().unwrap().remove("project_id");
        let v: crate::modrinth::ProjectVersion = serde_json::from_value(json).unwrap();
        assert!(modrinth_ref_from_version(&sha1, &v).is_none());
    }

    #[test]
    fn parses_fingerprint_response() {
        let json = serde_json::json!({
            "data": {
                "isCacheBuilt": true,
                "exactMatches": [
                    {
                        "id": 238222,
                        "file": {
                            "id": 4711,
                            "modId": 238222,
                            "fileName": "jei-1.21.1.jar",
                            "fileFingerprint": 3376380438u32,
                            "displayName": "JEI"
                        },
                        "latestFiles": []
                    }
                ],
                "exactFingerprints": [3376380438u32],
                "partialMatches": [],
                "unmatchedFingerprints": [111]
            }
        });
        let resp: CfFingerprintsResponse = serde_json::from_value(json).unwrap();
        assert_eq!(resp.data.exact_matches.len(), 1);
        let f = &resp.data.exact_matches[0].file;
        assert_eq!(f.mod_id, 238222);
        assert_eq!(f.id, 4711);
        assert_eq!(f.file_fingerprint, 3_376_380_438);
    }

    #[tokio::test]
    async fn hashes_sha1_and_sha512_in_one_pass() {
        let path = std::env::temp_dir()
            .join(format!("ezmapa_test_hash_{}.bin", uuid::Uuid::new_v4()));
        tokio::fs::write(&path, b"abc").await.unwrap();
        let (sha1, sha512) = file_sha1_sha512(&path).await.unwrap();
        // FIPS-180 test vectors for "abc".
        assert_eq!(sha1, "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(
            sha512,
            "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a\
             2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"
        );
        tokio::fs::remove_file(&path).await.ok();
    }
}
