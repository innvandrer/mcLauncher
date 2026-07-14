//! Java runtime detection and (when missing) automatic download from Adoptium.

use crate::error::{Error, Result};
use crate::state::{AppDirs, AppState};
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
#[cfg(windows)]
const JAVA_EXE: &str = "java.exe";
#[cfg(not(windows))]
const JAVA_EXE: &str = "java";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaInstall {
    pub path: String,
    pub version: String,
    pub major: u32,
}

/// Take the leading run of ASCII digits off a version segment (`"2-rc1"` ->
/// `Some(2)`, `"14a"` -> `Some(14)`), so release-candidate/snapshot-flavoured
/// segments still parse instead of failing the whole lookup.
fn leading_number(s: &str) -> Option<u32> {
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// The Java major version Mojang requires for a given Minecraft release, via
/// the same breakpoints the official launcher uses (1.20.5+ → 21, 1.18+ → 17,
/// 1.17 → 16, older → 8). Returns `None` for version strings that don't look
/// like a standard `1.x[.y]` release (e.g. snapshots), so callers can skip
/// version-dependent logic rather than risk a false positive.
///
/// Single source of truth — shared by the preflight check and the JVM tuner
/// (they previously carried two diverging copies of this table).
pub fn required_major_for_mc(mc_version: &str) -> Option<u32> {
    let mut segs = mc_version.split('.');
    let major = leading_number(segs.next()?)?;
    if major != 1 {
        return None;
    }
    let minor = segs.next().and_then(leading_number).unwrap_or(0);
    let patch = segs.next().and_then(leading_number).unwrap_or(0);
    Some(if minor > 20 || (minor == 20 && patch >= 5) {
        21
    } else if minor >= 18 {
        17
    } else if minor == 17 {
        16
    } else {
        8
    })
}

fn parse_major(version: &str) -> u32 {
    let v = version.trim().trim_matches('"');
    if let Some(rest) = v.strip_prefix("1.") {
        rest.split(['.', '_']).next().and_then(|s| s.parse().ok()).unwrap_or(0)
    } else {
        v.split(['.', '+', '-', '_']).next().and_then(|s| s.parse().ok()).unwrap_or(0)
    }
}

/// Read the `release` file shipped with every modern JDK/JRE.
fn probe_release(home: &Path) -> Option<(String, u32)> {
    let content = std::fs::read_to_string(home.join("release")).ok()?;
    for line in content.lines() {
        if let Some(v) = line.strip_prefix("JAVA_VERSION=") {
            let ver = v.trim().trim_matches('"').to_string();
            let major = parse_major(&ver);
            return Some((ver, major));
        }
    }
    None
}

/// Fall back to executing `java -version` (output goes to stderr).
fn probe_exec(java: &Path) -> Option<(String, u32)> {
    use std::process::Command;
    #[cfg(windows)]
    use std::os::windows::process::CommandExt;

    let mut cmd = Command::new(java);
    cmd.arg("-version");
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let out = cmd.output().ok()?;
    let text = String::from_utf8_lossy(&out.stderr);
    let quoted = text.split('"').nth(1)?;
    Some((quoted.to_string(), parse_major(quoted)))
}

fn probe(java: &Path) -> Option<JavaInstall> {
    if !java.exists() {
        return None;
    }
    let home = java.parent().and_then(|b| b.parent());
    let (version, major) = home
        .and_then(probe_release)
        .or_else(|| probe_exec(java))?;
    if major == 0 {
        return None;
    }
    Some(JavaInstall {
        path: java.to_string_lossy().to_string(),
        version,
        major,
    })
}

/// Probe an explicit `java` executable for its major version (for JVM-flag
/// suggestions when the user pinned a runtime).
pub fn probe_major(java: &Path) -> Option<u32> {
    probe(java).map(|j| j.major)
}

fn push_java_dir(set: &mut BTreeSet<PathBuf>, dir: &Path) {
    let candidate = dir.join("bin").join(JAVA_EXE);
    if candidate.exists() {
        set.insert(candidate);
    }
}

/// Discover Java installations from JAVA_HOME, PATH, common install locations
/// and the launcher's own managed runtimes. Takes only the directory layout so
/// it can run inside `spawn_blocking` without borrowing the whole `AppState`.
pub fn detect_in(dirs: &AppDirs) -> Vec<JavaInstall> {
    let mut candidates: BTreeSet<PathBuf> = BTreeSet::new();

    if let Ok(java_home) = std::env::var("JAVA_HOME") {
        push_java_dir(&mut candidates, Path::new(&java_home));
    }

    if let Ok(path) = std::env::var("PATH") {
        for entry in std::env::split_paths(&path) {
            let c = entry.join(JAVA_EXE);
            if c.exists() {
                candidates.insert(c);
            }
        }
    }

    // Common Windows vendor directories.
    #[cfg(windows)]
    {
        let roots = [
            r"C:\Program Files\Java",
            r"C:\Program Files\Eclipse Adoptium",
            r"C:\Program Files\Microsoft",
            r"C:\Program Files\Zulu",
            r"C:\Program Files\BellSoft",
            r"C:\Program Files\Amazon Corretto",
        ];
        for root in roots {
            if let Ok(rd) = std::fs::read_dir(root) {
                for e in rd.flatten() {
                    push_java_dir(&mut candidates, &e.path());
                }
            }
        }
    }

    // Launcher-managed runtimes.
    if let Ok(rd) = std::fs::read_dir(dirs.java()) {
        for e in rd.flatten() {
            push_java_dir(&mut candidates, &e.path());
            // One level deeper (Adoptium archives contain a top-level folder).
            if let Ok(inner) = std::fs::read_dir(e.path()) {
                for i in inner.flatten() {
                    push_java_dir(&mut candidates, &i.path());
                }
            }
        }
    }

    let mut installs: Vec<JavaInstall> = candidates.iter().filter_map(|p| probe(p)).collect();
    installs.sort_by_key(|i| std::cmp::Reverse(i.major));
    installs.dedup_by(|a, b| a.path == b.path);
    installs
}

fn adoptium_os() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else {
        "linux"
    }
}

fn adoptium_arch() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "x64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x86"
    }
}

/// Return a Java executable matching `major`, downloading a runtime if needed.
pub async fn ensure(state: &AppState, major: u32) -> Result<String> {
    // Detection scans the filesystem and may exec `java -version`; run it once,
    // off the async runtime (the previous code scanned the disk twice on the
    // launch hot path and blocked the executor with subprocesses).
    let dirs = state.dirs.clone();
    let installs = tokio::task::spawn_blocking(move || detect_in(&dirs)).await?;
    if let Some(found) = installs
        .iter()
        .find(|j| j.major == major)
        // Also accept a newer runtime if one is already present (best effort).
        .or_else(|| installs.iter().find(|j| j.major >= major))
    {
        return Ok(found.path.clone());
    }

    download(state, major).await
}

/// The fields we need from one entry of Adoptium's
/// `GET /v3/assets/latest/{feature_version}/{jvm_impl}` response.
#[derive(serde::Deserialize)]
struct AdoptiumAsset {
    binary: AdoptiumBinary,
}

#[derive(serde::Deserialize)]
struct AdoptiumBinary {
    package: AdoptiumPackage,
}

#[derive(serde::Deserialize)]
struct AdoptiumPackage {
    /// Archive file name, e.g. `OpenJDK21U-jre_x64_linux_hotspot_21.0.5_11.tar.gz`.
    name: String,
    /// Download URL.
    link: String,
    /// SHA-256 of the archive, hex-encoded.
    checksum: String,
}

async fn download(state: &AppState, major: u32) -> Result<String> {
    // The assets API (unlike the bare binary redirect) tells us the archive's
    // file name and sha256 up front, so the download can be verified before
    // anything gets extracted — and the name tells us the archive format:
    // Adoptium ships .zip on Windows but .tar.gz on Linux/macOS.
    let url = format!(
        "https://api.adoptium.net/v3/assets/latest/{major}/hotspot\
         ?architecture={}&image_type=jre&os={}&vendor=eclipse",
        adoptium_arch(),
        adoptium_os()
    );
    let assets: Vec<AdoptiumAsset> = crate::net::get_json(&state.http, &url).await?;
    let pkg = assets
        .into_iter()
        .next()
        .ok_or_else(|| {
            Error::Other(format!(
                "No Temurin {major} JRE is available for {}/{}",
                adoptium_os(),
                adoptium_arch()
            ))
        })?
        .binary
        .package;

    let java_root = state.dirs.java();
    std::fs::create_dir_all(&java_root)?;
    let archive_path = java_root.join(&pkg.name);

    // Stream the (tens of MB) archive to disk while hashing, then verify the
    // published checksum before extracting.
    use sha2::{Digest, Sha256};
    let mut resp = state.http.get(&pkg.link).send().await?.error_for_status()?;
    let mut hasher = Sha256::new();
    {
        use tokio::io::AsyncWriteExt;
        let mut f = tokio::fs::File::create(&archive_path).await?;
        while let Some(chunk) = resp.chunk().await? {
            hasher.update(&chunk);
            f.write_all(&chunk).await?;
        }
        f.flush().await?;
    }
    let actual = hex::encode(hasher.finalize());
    if !actual.eq_ignore_ascii_case(pkg.checksum.trim()) {
        let _ = tokio::fs::remove_file(&archive_path).await;
        return Err(Error::Checksum {
            file: pkg.name.clone(),
            expected: pkg.checksum.clone(),
            actual,
        });
    }

    let dest = java_root.join(format!("temurin-{major}"));
    let dest_clone = dest.clone();
    let archive_clone = archive_path.clone();
    let is_tar_gz = pkg.name.ends_with(".tar.gz") || pkg.name.ends_with(".tgz");
    tokio::task::spawn_blocking(move || {
        if is_tar_gz {
            untar_gz(&archive_clone, &dest_clone)
        } else {
            unzip(&archive_clone, &dest_clone)
        }
    })
    .await
    .map_err(|e| Error::Other(format!("extraction task failed: {e}")))??;
    let _ = tokio::fs::remove_file(&archive_path).await;

    // Locate java(.exe) within the extracted tree.
    find_java_in(&dest)
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| Error::Other(format!("Java {major} downloaded but executable not found")))
}

fn find_java_in(root: &Path) -> Option<PathBuf> {
    let direct = root.join("bin").join(JAVA_EXE);
    if direct.exists() {
        return Some(direct);
    }
    let rd = std::fs::read_dir(root).ok()?;
    for e in rd.flatten() {
        let p = e.path().join("bin").join(JAVA_EXE);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

#[cfg(test)]
mod required_major_tests {
    use super::*;

    #[test]
    fn java_major_pre_1_17_is_8() {
        assert_eq!(required_major_for_mc("1.16.5"), Some(8));
        assert_eq!(required_major_for_mc("1.7.10"), Some(8));
    }

    #[test]
    fn java_major_1_17_is_16() {
        assert_eq!(required_major_for_mc("1.17"), Some(16));
        assert_eq!(required_major_for_mc("1.17.1"), Some(16));
    }

    #[test]
    fn java_major_1_18_through_1_20_4_is_17() {
        assert_eq!(required_major_for_mc("1.18"), Some(17));
        assert_eq!(required_major_for_mc("1.20"), Some(17));
        assert_eq!(required_major_for_mc("1.20.4"), Some(17));
    }

    #[test]
    fn java_major_1_20_5_and_later_is_21() {
        assert_eq!(required_major_for_mc("1.20.5"), Some(21));
        assert_eq!(required_major_for_mc("1.21"), Some(21));
        assert_eq!(required_major_for_mc("1.21.1"), Some(21));
    }

    #[test]
    fn java_major_handles_release_candidate_suffix() {
        assert_eq!(required_major_for_mc("1.20.2-rc1"), Some(17));
    }

    #[test]
    fn java_major_none_for_snapshots() {
        assert_eq!(required_major_for_mc("24w14a"), None);
    }
}

/// Extract a `.tar.gz` runtime archive (the format Adoptium ships for Linux
/// and macOS). `tar::Archive::unpack` refuses entries that would escape
/// `dest` and preserves the executable bits `bin/java` needs.
fn untar_gz(archive: &Path, dest: &Path) -> Result<()> {
    let file = std::fs::File::open(archive)?;
    let gz = flate2::read::GzDecoder::new(file);
    let mut tar = tar::Archive::new(gz);
    std::fs::create_dir_all(dest)?;
    tar.unpack(dest)?;
    Ok(())
}

#[cfg(test)]
mod archive_tests {
    use super::*;

    #[test]
    fn untar_gz_extracts_a_runtime_layout_find_java_can_locate() {
        let root = std::env::temp_dir().join(format!("ezmapa_tar_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let archive = root.join("jre.tar.gz");

        // Build a minimal Temurin-shaped tar.gz: <release>/bin/java(.exe).
        {
            let f = std::fs::File::create(&archive).unwrap();
            let gz = flate2::write::GzEncoder::new(f, flate2::Compression::default());
            let mut tar = tar::Builder::new(gz);
            let data = b"not really a jvm";
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            tar.append_data(&mut header, format!("jdk-21.0.5+11-jre/bin/{JAVA_EXE}"), &data[..])
                .unwrap();
            tar.into_inner().unwrap().finish().unwrap();
        }

        let dest = root.join("out");
        untar_gz(&archive, &dest).unwrap();
        assert!(dest.join("jdk-21.0.5+11-jre").join("bin").join(JAVA_EXE).is_file());
        // The one-level-deep search used after extraction must find it.
        assert!(find_java_in(&dest).is_some());

        std::fs::remove_dir_all(&root).ok();
    }
}

fn unzip(archive: &Path, dest: &Path) -> Result<()> {
    let file = std::fs::File::open(archive)?;
    let mut zip = zip::ZipArchive::new(file)?;
    std::fs::create_dir_all(dest)?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let Some(rel) = entry.enclosed_name() else {
            continue;
        };
        let out = dest.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out)?;
        } else {
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut w = std::fs::File::create(&out)?;
            std::io::copy(&mut entry, &mut w)?;
        }
    }
    Ok(())
}
