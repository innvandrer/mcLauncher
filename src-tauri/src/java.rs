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
    installs.sort_by(|a, b| b.major.cmp(&a.major));
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

async fn download(state: &AppState, major: u32) -> Result<String> {
    let url = format!(
        "https://api.adoptium.net/v3/binary/latest/{major}/ga/{}/{}/jre/hotspot/normal/eclipse",
        adoptium_os(),
        adoptium_arch()
    );
    let bytes = state
        .http
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;

    let java_root = state.dirs.java();
    std::fs::create_dir_all(&java_root)?;
    let archive_path = java_root.join(format!("temurin-{major}.zip"));
    tokio::fs::write(&archive_path, &bytes).await?;

    let dest = java_root.join(format!("temurin-{major}"));
    let dest_clone = dest.clone();
    let archive_clone = archive_path.clone();
    tokio::task::spawn_blocking(move || unzip(&archive_clone, &dest_clone))
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
