//! Forge and NeoForge loader support.
//!
//! Install strategy: download the official installer JAR and run it with
//! `--installClient <staging_dir>`, then copy the produced version JSON and
//! libraries into our managed directories.

use crate::error::{Error, Result};
use crate::models::TaskProgress;
use crate::state::AppState;
use crate::java;
use serde::Deserialize;
use std::path::Path;
use tauri::{AppHandle, Emitter};

const FORGE_MAVEN: &str = "https://maven.minecraftforge.net/net/minecraftforge/forge";
/// The promotions JSON lives on the files host, not the maven repo.
const FORGE_FILES: &str = "https://files.minecraftforge.net/net/minecraftforge/forge";
const NEOFORGE_MAVEN: &str = "https://maven.neoforged.net/releases/net/neoforged/neoforge";

/// Matches the frontend `LoaderVersion` type (`{ version, stable }`).
#[derive(Debug, Clone, serde::Serialize)]
pub struct ForgeVersion {
    pub version: String,
    pub stable: bool,
}

// ── Version listing ─────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct Promotions {
    promos: std::collections::HashMap<String, String>,
}

/// List Forge versions for a given Minecraft version.
pub async fn list_forge(state: &AppState, mc_version: &str) -> Result<Vec<ForgeVersion>> {
    let promo_url = format!("{FORGE_FILES}/promotions_slim.json");
    let promo: Promotions = crate::net::get_json(&state.http, &promo_url).await?;

    let rec_key = format!("{mc_version}-recommended");
    let lat_key = format!("{mc_version}-latest");
    let recommended = promo.promos.get(&rec_key).cloned();
    let latest = promo.promos.get(&lat_key).cloned();

    let meta_url = format!("{FORGE_MAVEN}/maven-metadata.xml");
    let xml = state
        .http
        .get(&meta_url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    let prefix = format!("{mc_version}-");
    let mut versions: Vec<ForgeVersion> = parse_xml_versions(&xml)
        .into_iter()
        .filter(|v| v.starts_with(&prefix))
        .map(|v| {
            let forge_ver = v[prefix.len()..].to_string();
            let is_stable = recommended.as_deref() == Some(forge_ver.as_str());
            ForgeVersion { version: forge_ver, stable: is_stable }
        })
        .collect();

    // Recommended first, then latest, then rest (newest first via reverse).
    versions.sort_by(|a, b| {
        let rank = |v: &ForgeVersion| {
            if recommended.as_deref() == Some(v.version.as_str()) { 0 }
            else if latest.as_deref() == Some(v.version.as_str()) { 1 }
            else { 2 }
        };
        rank(a).cmp(&rank(b))
    });

    Ok(versions)
}

/// List NeoForge versions for a given Minecraft version.
pub async fn list_neoforge(state: &AppState, mc_version: &str) -> Result<Vec<ForgeVersion>> {
    let meta_url = format!("{NEOFORGE_MAVEN}/maven-metadata.xml");
    let xml = state
        .http
        .get(&meta_url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    // MC "1.21.1" → NeoForge prefix "21.1."
    let nf_prefix = mc_to_neoforge_prefix(mc_version);

    let mut versions: Vec<ForgeVersion> = parse_xml_versions(&xml)
        .into_iter()
        .filter(|v| v.starts_with(&nf_prefix))
        .filter(|v| !v.contains("beta") && !v.contains("rc") && !v.contains("-pre"))
        .map(|v| ForgeVersion { version: v, stable: false })
        .collect();

    versions.reverse(); // newest first
    if let Some(first) = versions.first_mut() {
        first.stable = true; // mark latest as "recommended"
    }

    Ok(versions)
}

/// Map a Minecraft version to the NeoForge version prefix.
/// NeoForge uses `{mc_minor}.{mc_patch}.{build}`, where a patchless MC release
/// (e.g. `1.21`) maps to patch `0` (`21.0.x`).
/// `1.21`   → `21.0.`   `1.21.1` → `21.1.`   `1.20.4` → `20.4.`
fn mc_to_neoforge_prefix(mc_version: &str) -> String {
    let stripped = mc_version.strip_prefix("1.").unwrap_or(mc_version);
    if stripped.contains('.') {
        format!("{stripped}.")
    } else {
        format!("{stripped}.0.")
    }
}

fn parse_xml_versions(xml: &str) -> Vec<String> {
    xml.lines()
        .filter_map(|line| {
            let t = line.trim();
            let inner = t.strip_prefix("<version>")?.strip_suffix("</version>")?;
            Some(inner.to_string())
        })
        .collect()
}

// ── Installer-based install ─────────────────────────────────────────────────

fn emit_progress(app: &AppHandle, id: &str, label: &str, stage: &str, cur: u64, total: u64) {
    let _ = app.emit(
        "task://progress",
        TaskProgress::simple(id, label, stage, cur, total, false, None),
    );
}

fn emit_done(app: &AppHandle, id: &str, label: &str) {
    let _ = app.emit(
        "task://progress",
        TaskProgress::simple(id, label, "", 1, 1, true, None),
    );
}

/// Install Forge and return the version ID to use for launching.
pub async fn install_forge(
    app: &AppHandle,
    state: &AppState,
    mc_version: &str,
    forge_version: &str,
) -> Result<String> {
    let version_str = format!("{mc_version}-{forge_version}");
    let task_id = format!("forge-{version_str}");
    let installer_url = format!(
        "{FORGE_MAVEN}/{version_str}/forge-{version_str}-installer.jar"
    );
    // The installer names the version `{mc}-forge-{forge}`, e.g. `1.20.1-forge-47.4.0`.
    let expected_id = format!("{mc_version}-forge-{forge_version}");

    run_installer(app, state, &task_id, "Forge", &installer_url, &expected_id, "forge").await
}

/// Install NeoForge and return the version ID to use for launching.
pub async fn install_neoforge(
    app: &AppHandle,
    state: &AppState,
    _mc_version: &str,
    neo_version: &str,
) -> Result<String> {
    let task_id = format!("neoforge-{neo_version}");
    let installer_url = format!(
        "{NEOFORGE_MAVEN}/{neo_version}/neoforge-{neo_version}-installer.jar"
    );
    let expected_id = format!("neoforge-{neo_version}");

    run_installer(app, state, &task_id, "NeoForge", &installer_url, &expected_id, "forge").await
}

async fn run_installer(
    app: &AppHandle,
    state: &AppState,
    task_id: &str,
    label: &str,
    installer_url: &str,
    expected_id: &str,
    marker: &str,
) -> Result<String> {
    // Return early if already installed.
    if state.dirs.version_json(expected_id).exists() {
        return Ok(expected_id.to_string());
    }

    emit_progress(app, task_id, &format!("Installing {label}"), "Downloading installer", 0, 4);

    let installer_bytes = state
        .http
        .get(installer_url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;

    let temp_base = std::env::temp_dir().join(format!("beacon-{task_id}"));
    let _ = std::fs::remove_dir_all(&temp_base); // clear any stale partial install
    std::fs::create_dir_all(&temp_base)?;
    let installer_path = temp_base.join("installer.jar");
    std::fs::write(&installer_path, &installer_bytes)?;

    emit_progress(app, task_id, &format!("Installing {label}"), "Resolving Java", 1, 4);

    let java_path = java::ensure(state, 17).await?;

    let staging = temp_base.join("staging");
    std::fs::create_dir_all(&staging)?;
    // The Forge/NeoForge installer refuses to run against a directory that has
    // no `launcher_profiles.json` ("no minecraft directory set up"). Provide a
    // stub so it treats `staging` as a valid `.minecraft` folder.
    std::fs::write(staging.join("launcher_profiles.json"), "{\"profiles\":{}}")?;

    emit_progress(
        app,
        task_id,
        &format!("Installing {label}"),
        "Running installer (this may take a minute…)",
        2,
        4,
    );

    let mut cmd = std::process::Command::new(&java_path);
    cmd.args([
        "-jar",
        installer_path.to_str().unwrap(),
        "--installClient",
        staging.to_str().unwrap(),
    ]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }

    let output = cmd
        .output()
        .map_err(|e| Error::Other(format!("Failed to launch {label} installer: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let _ = std::fs::remove_dir_all(&temp_base);
        return Err(Error::Other(format!("{label} installer failed: {stderr}")));
    }

    emit_progress(app, task_id, &format!("Installing {label}"), "Copying artifacts", 3, 4);

    let staging_libs = staging.join("libraries");
    if staging_libs.exists() {
        copy_dir_all(&staging_libs, &state.dirs.libraries())?;
    }

    let staging_versions = staging.join("versions");
    // Prefer the exact id we expect; fall back to any version dir whose name
    // contains the loader marker (handles naming drift across versions).
    let version_id = find_installed_version(&staging_versions, expected_id, marker)?;

    let src_json = staging_versions
        .join(&version_id)
        .join(format!("{version_id}.json"));
    let dst_dir = state.dirs.versions().join(&version_id);
    std::fs::create_dir_all(&dst_dir)?;
    std::fs::copy(&src_json, dst_dir.join(format!("{version_id}.json")))?;

    let _ = std::fs::remove_dir_all(&temp_base);

    emit_done(app, task_id, &format!("{label} installed"));
    Ok(version_id)
}

/// Find the modded version directory the installer created.
fn find_installed_version(versions_dir: &Path, expected_id: &str, marker: &str) -> Result<String> {
    if !versions_dir.exists() {
        return Err(Error::Other(
            "Installer did not create a versions directory.".into(),
        ));
    }
    // Exact match first.
    if versions_dir.join(expected_id).is_dir() {
        return Ok(expected_id.to_string());
    }
    // Fall back to the dir whose name contains the loader marker (e.g. "forge").
    for entry in std::fs::read_dir(versions_dir)? {
        let name = entry?.file_name().to_string_lossy().to_string();
        if name.to_lowercase().contains(marker) {
            return Ok(name);
        }
    }
    Err(Error::Other(format!(
        "Could not find the installed '{expected_id}' version in the staging directory."
    )))
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else if !dst_path.exists() {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neoforge_prefix_patchless_release_maps_to_zero() {
        // MC 1.21 (no patch) → NeoForge 21.0.x
        assert_eq!(mc_to_neoforge_prefix("1.21"), "21.0.");
    }

    #[test]
    fn neoforge_prefix_with_patch() {
        assert_eq!(mc_to_neoforge_prefix("1.21.1"), "21.1.");
        assert_eq!(mc_to_neoforge_prefix("1.20.4"), "20.4.");
    }

    #[test]
    fn neoforge_prefix_does_not_cross_match() {
        // The 1.21.1 prefix must not match 21.10.x builds.
        let p = mc_to_neoforge_prefix("1.21.1");
        assert!("21.1.169".starts_with(&p));
        assert!(!"21.10.5".starts_with(&p));
        // And the patchless 1.21 prefix must not catch 21.1.x.
        let p0 = mc_to_neoforge_prefix("1.21");
        assert!("21.0.167".starts_with(&p0));
        assert!(!"21.1.231".starts_with(&p0));
    }

    #[test]
    fn parses_versions_from_maven_metadata() {
        let xml = r#"<metadata>
  <versioning>
    <versions>
      <version>20.2.59-beta</version>
      <version>21.1.169</version>
    </versions>
  </versioning>
</metadata>"#;
        let v = parse_xml_versions(xml);
        assert_eq!(v, vec!["20.2.59-beta", "21.1.169"]);
    }
}
