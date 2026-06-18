//! Modrinth integration: search projects and install mod/modpack files.

use crate::error::{Error, Result};
use crate::net::{self, DownloadItem};
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::{AppHandle, Emitter};

const API: &str = "https://api.modrinth.com/v2";

// Field names mirror Modrinth's snake_case JSON and are passed through to the
// frontend unchanged, so the TS types use snake_case for these too.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModHit {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub follows: u64,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub versions: Vec<String>,
    #[serde(default)]
    pub project_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub hits: Vec<ModHit>,
    #[serde(default)]
    pub total_hits: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectVersion {
    pub id: String,
    pub version_number: String,
    #[serde(default)]
    pub name: String,
    pub files: Vec<VersionFile>,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
    #[serde(default)]
    pub date_published: String,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionFile {
    pub url: String,
    pub filename: String,
    #[serde(default)]
    pub primary: bool,
    #[serde(default)]
    pub hashes: Hashes,
    #[serde(default)]
    pub size: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Hashes {
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub sha512: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub dependency_type: String,
}

/// A dependency that was auto-installed alongside a mod.
#[derive(Debug, Clone, Serialize)]
pub struct InstalledDep {
    pub file_name: String,
    pub project_id: String,
}

/// Search Modrinth, optionally constrained by loader and game version.
#[allow(clippy::too_many_arguments)]
pub async fn search(
    state: &AppState,
    query: &str,
    project_type: &str,
    loader: Option<&str>,
    game_version: Option<&str>,
    index: &str,
    limit: u32,
    offset: u32,
) -> Result<SearchResponse> {
    let mut facets: Vec<Vec<String>> = vec![vec![format!("project_type:{project_type}")]];
    if let Some(l) = loader {
        if !l.is_empty() && l != "vanilla" {
            facets.push(vec![format!("categories:{l}")]);
        }
    }
    if let Some(v) = game_version {
        if !v.is_empty() {
            facets.push(vec![format!("versions:{v}")]);
        }
    }
    let facets_json = serde_json::to_string(&facets)?;
    let url = format!("{API}/search");
    let limit_s = limit.to_string();
    let offset_s = offset.to_string();
    let params: Vec<(&str, &str)> = vec![
        ("query", query),
        ("limit", &limit_s),
        ("offset", &offset_s),
        ("index", index),
        ("facets", &facets_json),
    ];
    let resp = state
        .http
        .get(&url)
        .query(&params)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(resp)
}

/// A version of a project, in a provider-neutral shape for the version picker.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentVersion {
    /// Modrinth version id or CurseForge file id (as a string).
    pub id: String,
    pub name: String,
    pub version_number: String,
    pub game_versions: Vec<String>,
    /// Mod loaders this version targets (Modrinth). Empty for CurseForge, where
    /// loader names appear inside `game_versions` instead.
    #[serde(default)]
    pub loaders: Vec<String>,
    pub date: String,
}

/// List all versions of a project for the version picker (newest first).
pub async fn list_versions(state: &AppState, project_id: &str) -> Result<Vec<ContentVersion>> {
    let versions = project_versions(state, project_id, None, None).await?;
    Ok(versions
        .into_iter()
        .map(|v| ContentVersion {
            id: v.id,
            name: if v.name.is_empty() { v.version_number.clone() } else { v.name },
            version_number: v.version_number,
            game_versions: v.game_versions,
            loaders: v.loaders,
            date: v.date_published,
        })
        .collect())
}

/// Fetch the versions of a project compatible with the given loader + version,
/// newest first.
pub async fn project_versions(
    state: &AppState,
    project_id: &str,
    loader: Option<&str>,
    game_version: Option<&str>,
) -> Result<Vec<ProjectVersion>> {
    let url = format!("{API}/project/{project_id}/version");
    let mut req = state.http.get(&url);
    if let Some(l) = loader {
        if !l.is_empty() && l != "vanilla" {
            req = req.query(&[("loaders", format!("[\"{l}\"]"))]);
        }
    }
    if let Some(v) = game_version {
        if !v.is_empty() {
            req = req.query(&[("game_versions", format!("[\"{v}\"]"))]);
        }
    }
    let versions = req.send().await?.error_for_status()?.json().await?;
    Ok(versions)
}

/// Download the best matching file of a project into an instance's `mods` folder
/// and return the installed filename plus any auto-installed dependencies.
pub async fn install_mod(
    state: &AppState,
    instance_id: &str,
    project_id: &str,
    loader: Option<&str>,
    game_version: Option<&str>,
) -> Result<(String, Vec<InstalledDep>)> {
    install_content(state, instance_id, project_id, "mod", loader, game_version).await
}

/// Install a Modrinth project (mod / resourcepack / shader) into the correct
/// sub-directory of the instance and return the installed filename plus any
/// auto-installed dependencies. For mods, required dependencies are installed
/// recursively and constrained to the instance's loader + game version.
pub async fn install_content(
    state: &AppState,
    instance_id: &str,
    project_id: &str,
    content_type: &str,
    loader: Option<&str>,
    game_version: Option<&str>,
) -> Result<(String, Vec<InstalledDep>)> {
    let versions = project_versions(state, project_id, loader, game_version).await?;
    let version = versions
        .into_iter()
        .next()
        .ok_or_else(|| Error::NotFound(format!("compatible version for {project_id}")))?;

    // Install required dependencies first so the primary mod isn't left without
    // libraries it needs.
    let installed_deps = if content_type == "mod" {
        let mods_dir = state.dirs.game_dir(instance_id).join("mods");
        let mut visited = std::collections::HashSet::new();
        let mut out = Vec::new();
        for dep in &version.dependencies {
            if dep.dependency_type == "required" {
                if let Some(dep_id) = &dep.project_id {
                    out.extend(
                        install_dependency(state, dep_id, loader, game_version, &mods_dir, &mut visited)
                            .await,
                    );
                }
            }
        }
        out
    } else {
        Vec::new()
    };

    let file = version
        .files
        .iter()
        .find(|f| f.primary)
        .or_else(|| version.files.first())
        .cloned()
        .ok_or_else(|| Error::NotFound("download file".into()))?;

    let folder = match content_type {
        "resourcepack" => "resourcepacks",
        "shader" => "shaderpacks",
        _ => "mods",
    };

    let dest_dir = state.dirs.game_dir(instance_id).join(folder);
    let target = dest_dir.join(&file.filename);
    net::download_one(
        &state.http,
        &DownloadItem::new(file.url.clone(), target, file.hashes.sha1.clone()),
    )
    .await?;

    Ok((file.filename, installed_deps))
}

/// Recursively install a single required dependency into the mods folder.
/// `visited` prevents circular dependencies. Returns every dependency file that
/// was newly installed (including transitive ones), constrained to the given
/// loader and game version.
fn install_dependency<'a>(
    state: &'a AppState,
    project_id: &'a str,
    loader: Option<&'a str>,
    game_version: Option<&'a str>,
    mods_dir: &'a std::path::Path,
    visited: &'a mut std::collections::HashSet<String>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<InstalledDep>> + Send + 'a>> {
    Box::pin(async move {
        if !visited.insert(project_id.to_string()) {
            return Vec::new();
        }

        let versions = match project_versions(state, project_id, loader, game_version).await {
            Ok(v) => v,
            Err(_) => return Vec::new(),
        };
        let version = match versions.into_iter().next() {
            Some(v) => v,
            None => return Vec::new(),
        };
        let file = match version
            .files
            .iter()
            .find(|f| f.primary)
            .or_else(|| version.files.first())
        {
            Some(f) => f.clone(),
            None => return Vec::new(),
        };

        let target = mods_dir.join(&file.filename);
        let target_dis = mods_dir.join(format!("{}.disabled", file.filename));
        if target.exists() || target_dis.exists() {
            return Vec::new();
        }

        if net::download_one(
            &state.http,
            &DownloadItem::new(file.url.clone(), target, file.hashes.sha1.clone()),
        )
        .await
        .is_err()
        {
            return Vec::new();
        }

        let mut out = vec![InstalledDep {
            file_name: file.filename,
            project_id: project_id.to_string(),
        }];

        // Recurse into this dependency's own required dependencies.
        for dep in &version.dependencies {
            if dep.dependency_type == "required" {
                if let Some(dep_id) = &dep.project_id {
                    out.extend(
                        install_dependency(state, dep_id.as_str(), loader, game_version, mods_dir, visited)
                            .await,
                    );
                }
            }
        }

        out
    })
}

/// Install a specific Modrinth version by its version ID directly. Returns the
/// installed filename plus the dependencies that were auto-installed alongside
/// it (for mods).
pub async fn install_version(
    state: &AppState,
    instance_id: &str,
    version_id: &str,
    content_type: &str,
    loader: Option<&str>,
    game_version: Option<&str>,
) -> Result<(String, Vec<InstalledDep>)> {
    let version: ProjectVersion = state
        .http
        .get(format!("{API}/version/{version_id}"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    // Auto-install required dependencies for mods, constrained to the same
    // loader + game version as the selected version.
    let installed_deps = if content_type == "mod" {
        let mods_dir = state.dirs.game_dir(instance_id).join("mods");
        let mut visited = std::collections::HashSet::new();
        let mut out = Vec::new();
        for dep in &version.dependencies {
            if dep.dependency_type == "required" {
                if let Some(dep_id) = &dep.project_id {
                    out.extend(
                        install_dependency(state, dep_id, loader, game_version, &mods_dir, &mut visited)
                            .await,
                    );
                }
            }
        }
        out
    } else {
        Vec::new()
    };

    let file = version
        .files
        .iter()
        .find(|f| f.primary)
        .or_else(|| version.files.first())
        .cloned()
        .ok_or_else(|| Error::NotFound("download file".into()))?;

    let folder = match content_type {
        "resourcepack" => "resourcepacks",
        "shader" => "shaderpacks",
        _ => "mods",
    };
    let dest_dir = state.dirs.game_dir(instance_id).join(folder);
    net::download_one(
        &state.http,
        &DownloadItem::new(file.url.clone(), dest_dir.join(&file.filename), file.hashes.sha1.clone()),
    )
    .await?;

    Ok((file.filename, installed_deps))
}

/// Fetch the full project body (GitHub-flavored markdown string) from Modrinth.
pub async fn get_project_body(state: &AppState, project_id: &str) -> Result<String> {
    #[derive(Deserialize)]
    struct Proj {
        #[serde(default)]
        body: String,
    }
    let p: Proj = state
        .http
        .get(format!("{API}/project/{project_id}"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(p.body)
}

// ---------------------------------------------------------------------------
// Modrinth modpack (.mrpack) support
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MrpackIndex {
    #[serde(default)]
    name: String,
    #[serde(default, rename = "versionId")]
    version_id: String,
    #[serde(default)]
    files: Vec<MrpackEntry>,
    #[serde(default)]
    dependencies: MrpackDeps,
}

#[derive(Deserialize, Default)]
struct MrpackDeps {
    #[serde(default)]
    minecraft: Option<String>,
    #[serde(default, rename = "fabric-loader")]
    fabric_loader: Option<String>,
    #[serde(default, rename = "quilt-loader")]
    quilt_loader: Option<String>,
    #[serde(default)]
    forge: Option<String>,
    #[serde(default)]
    neoforge: Option<String>,
}

#[derive(Deserialize)]
struct MrpackEntry {
    path: String,
    downloads: Vec<String>,
    #[serde(default)]
    hashes: MrpackHashes,
    #[serde(default)]
    env: Option<MrpackEnv>,
}

#[derive(Deserialize, Default)]
struct MrpackHashes {
    #[serde(default)]
    sha1: Option<String>,
}

#[derive(Deserialize)]
struct MrpackEnv {
    client: String,
}

impl MrpackDeps {
    /// Resolve the dependency block into a (loader, loader_version) pair.
    fn loader(&self) -> (crate::models::Loader, Option<String>) {
        use crate::models::Loader;
        if let Some(v) = &self.fabric_loader {
            (Loader::Fabric, Some(v.clone()))
        } else if let Some(v) = &self.quilt_loader {
            (Loader::Quilt, Some(v.clone()))
        } else if let Some(v) = &self.neoforge {
            (Loader::Neoforge, Some(v.clone()))
        } else if let Some(v) = &self.forge {
            (Loader::Forge, Some(v.clone()))
        } else {
            (Loader::Vanilla, None)
        }
    }
}

/// Resolve the chosen (or latest) modpack version and download its `.mrpack`
/// archive to a temp file, returning (temp_path, fallback_pack_name).
async fn fetch_mrpack_archive(
    state: &AppState,
    project_id: &str,
    version_id: Option<&str>,
) -> Result<(std::path::PathBuf, String)> {
    let versions = project_versions(state, project_id, None, None).await?;
    let version = if let Some(vid) = version_id {
        versions
            .into_iter()
            .find(|v| v.id == vid)
            .ok_or_else(|| Error::NotFound(format!("modpack version {vid}")))?
    } else {
        versions
            .into_iter()
            .next()
            .ok_or_else(|| Error::NotFound(format!("any version for {project_id}")))?
    };

    let mrpack_file = version
        .files
        .iter()
        .find(|f| f.filename.ends_with(".mrpack"))
        .or_else(|| version.files.first())
        .cloned()
        .ok_or_else(|| Error::NotFound("mrpack file".into()))?;

    let tmp_path = std::env::temp_dir()
        .join(format!("beacon_{}.mrpack", &version.id[..version.id.len().min(8)]));
    net::download_one(
        &state.http,
        &DownloadItem::new(mrpack_file.url.clone(), tmp_path.clone(), mrpack_file.hashes.sha1.clone()),
    )
    .await?;

    Ok((tmp_path, version.name))
}

/// Parse a `.mrpack` archive: extract `overrides/` (and `client-overrides/`)
/// into `game_dir` and return (parsed index, download items for `files`).
fn unpack_mrpack(
    archive_path: &std::path::Path,
    game_dir: &std::path::Path,
) -> Result<(MrpackIndex, Vec<DownloadItem>)> {
    let file = std::fs::File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    let index: MrpackIndex = {
        let mut idx = archive.by_name("modrinth.index.json")?;
        let mut buf = Vec::new();
        std::io::Read::read_to_end(&mut idx, &mut buf)?;
        serde_json::from_slice(&buf)?
    };

    for i in 0..archive.len() {
        let mut zf = archive.by_index(i)?;
        let zname = zf.name().to_string();
        let rest = zname
            .strip_prefix("overrides/")
            .or_else(|| zname.strip_prefix("client-overrides/"));
        if let Some(rest) = rest {
            if rest.is_empty() {
                continue;
            }
            let dest = game_dir.join(rest);
            if zname.ends_with('/') {
                std::fs::create_dir_all(&dest)?;
            } else {
                if let Some(p) = dest.parent() {
                    std::fs::create_dir_all(p)?;
                }
                let mut out = std::fs::File::create(&dest)?;
                std::io::copy(&mut zf, &mut out)?;
            }
        }
    }

    let items = index
        .files
        .iter()
        .filter(|e| {
            e.env
                .as_ref()
                .map(|env| env.client != "unsupported")
                .unwrap_or(true)
        })
        .filter_map(|e| {
            e.downloads
                .first()
                .map(|url| DownloadItem::new(url.clone(), game_dir.join(&e.path), e.hashes.sha1.clone()))
        })
        .collect::<Vec<_>>();

    Ok((index, items))
}

/// Download and install a Modrinth modpack (.mrpack) into an existing instance.
/// Returns the pack name/version string.
pub async fn install_mrpack(
    app: &AppHandle,
    state: &AppState,
    instance_id: &str,
    project_id: &str,
    version_id: Option<&str>,
) -> Result<String> {
    let settings = crate::instances::load_settings(state);
    let (tmp_path, fallback_name) = fetch_mrpack_archive(state, project_id, version_id).await?;
    let game_dir = state.dirs.game_dir(instance_id);

    let (index, items) = unpack_mrpack(&tmp_path, &game_dir)?;
    std::fs::remove_file(&tmp_path).ok();

    let pack_name = if index.name.is_empty() { fallback_name } else { index.name };
    net::download_many(
        app,
        &state.http,
        &format!("mrpack:{instance_id}"),
        &format!("Installing {pack_name}"),
        items,
        settings.max_concurrent_downloads,
    )
    .await?;

    Ok(pack_name)
}

// ---------------------------------------------------------------------------
// Modpack updates (diff + apply)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModpackUpdate {
    pub version_id: String,
    pub version_name: String,
    pub current_version: Option<String>,
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub updated: Vec<String>,
}

/// Read the mod file names (basenames under `mods/`) from a target mrpack.
async fn target_mod_files(
    state: &AppState,
    project_id: &str,
    version_id: &str,
) -> Result<(String, Vec<String>)> {
    let (tmp_path, _) = fetch_mrpack_archive(state, project_id, Some(version_id)).await?;
    let meta: MrpackIndex = {
        let file = std::fs::File::open(&tmp_path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        let mut idx = archive.by_name("modrinth.index.json")?;
        let mut buf = Vec::new();
        std::io::Read::read_to_end(&mut idx, &mut buf)?;
        serde_json::from_slice(&buf)?
    };
    std::fs::remove_file(&tmp_path).ok();
    let files = meta
        .files
        .iter()
        .filter(|e| e.path.starts_with("mods/"))
        .filter_map(|e| e.path.rsplit('/').next().map(|s| s.to_string()))
        .collect();
    Ok((meta.name, files))
}

/// Check whether a newer version of a Modrinth modpack exists for this instance
/// and, if so, compute the mod-level diff against what's currently installed.
pub async fn check_modpack_update(
    state: &AppState,
    instance_id: &str,
) -> Result<Option<ModpackUpdate>> {
    let instance = crate::instances::get_instance(state, instance_id)?;
    let src = match instance.pack_source {
        Some(s) if s.provider == "modrinth" => s,
        _ => return Ok(None),
    };

    let versions = list_versions(state, &src.project_id).await?;
    let latest = match versions.first() {
        Some(v) => v.clone(),
        None => return Ok(None),
    };
    if Some(&latest.id) == src.version_id.as_ref() {
        return Ok(None); // already newest
    }

    let (_, target) = target_mod_files(state, &src.project_id, &latest.id).await?;
    let installed: Vec<String> = crate::instances::list_mods(state, instance_id)
        .into_iter()
        .map(|m| m.file_name.trim_end_matches(".disabled").to_string())
        .collect();

    use std::collections::HashMap;
    let target_keys: HashMap<String, String> = target
        .iter()
        .map(|f| (crate::tools::mod_base_key(f), f.clone()))
        .collect();
    let installed_keys: HashMap<String, String> = installed
        .iter()
        .map(|f| (crate::tools::mod_base_key(f), f.clone()))
        .collect();

    let mut added = Vec::new();
    let mut updated = Vec::new();
    for (k, f) in &target_keys {
        match installed_keys.get(k) {
            None => added.push(f.clone()),
            Some(cur) if cur != f => updated.push(f.clone()),
            _ => {}
        }
    }
    let mut removed: Vec<String> = installed_keys
        .iter()
        .filter(|(k, _)| !target_keys.contains_key(*k))
        .map(|(_, f)| f.clone())
        .collect();

    added.sort();
    removed.sort();
    updated.sort();

    Ok(Some(ModpackUpdate {
        version_id: latest.id,
        version_name: latest.version_number,
        current_version: src.version_name,
        added,
        removed,
        updated,
    }))
}

/// Apply a modpack update: remove mods no longer in the pack, install the target
/// version, and record the new version on the instance.
pub async fn apply_modpack_update(
    app: &AppHandle,
    state: &AppState,
    instance_id: &str,
    version_id: &str,
) -> Result<()> {
    let mut instance = crate::instances::get_instance(state, instance_id)?;
    let src = match instance.pack_source.clone() {
        Some(s) if s.provider == "modrinth" => s,
        _ => return Err(Error::Other("This instance isn't a Modrinth modpack.".into())),
    };

    // Remove mods that the target version drops, so stale jars don't linger.
    let (_, target) = target_mod_files(state, &src.project_id, version_id).await?;
    use std::collections::HashSet;
    let target_keys: HashSet<String> =
        target.iter().map(|f| crate::tools::mod_base_key(f)).collect();
    let mods_dir = state.dirs.game_dir(instance_id).join("mods");
    for m in crate::instances::list_mods(state, instance_id) {
        let base = m.file_name.trim_end_matches(".disabled");
        if !target_keys.contains(&crate::tools::mod_base_key(base)) {
            std::fs::remove_file(mods_dir.join(&m.file_name)).ok();
        }
    }

    // Install the target version's files into the existing instance.
    let name = install_mrpack(app, state, instance_id, &src.project_id, Some(version_id)).await?;

    // Record the new version so future checks compare correctly.
    instance.pack_source = Some(crate::models::PackSource {
        provider: "modrinth".into(),
        project_id: src.project_id,
        version_id: Some(version_id.to_string()),
        version_name: Some(name),
    });
    crate::instances::save_instance(state, &instance)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Mod updates (via Modrinth's version_files/update endpoint)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModUpdate {
    pub old_file_name: String,
    pub new_file_name: String,
    pub version_number: String,
    pub url: String,
    pub sha1: Option<String>,
    pub enabled: bool,
    /// "modrinth" (default) or "curseforge".
    #[serde(default = "default_mod_provider")]
    pub provider: String,
}

fn default_mod_provider() -> String {
    "modrinth".to_string()
}

/// Check installed mods against Modrinth and return those with a newer version
/// available for the instance's loader + game version.
pub async fn check_updates(
    state: &AppState,
    instance_id: &str,
    loader: Option<&str>,
    game_version: Option<&str>,
) -> Result<Vec<ModUpdate>> {
    let mods_dir = state.dirs.game_dir(instance_id).join("mods");

    // Map sha1 -> (display jar name, enabled).
    let mut by_hash: HashMap<String, (String, bool)> = HashMap::new();
    if let Ok(rd) = std::fs::read_dir(&mods_dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let (enabled, jar) = if let Some(s) = name.strip_suffix(".disabled") {
                (false, s.to_string())
            } else if name.ends_with(".jar") {
                (true, name.clone())
            } else {
                continue;
            };
            if let Ok(h) = net::file_sha1(&e.path()).await {
                by_hash.insert(h, (jar, enabled));
            }
        }
    }
    if by_hash.is_empty() {
        return Ok(Vec::new());
    }

    let hashes: Vec<String> = by_hash.keys().cloned().collect();
    let mut body = serde_json::json!({ "hashes": hashes, "algorithm": "sha1" });
    if let Some(l) = loader {
        if !l.is_empty() && l != "vanilla" {
            body["loaders"] = serde_json::json!([l]);
        }
    }
    if let Some(v) = game_version {
        if !v.is_empty() {
            body["game_versions"] = serde_json::json!([v]);
        }
    }

    let resp: HashMap<String, ProjectVersion> = state
        .http
        .post(format!("{API}/version_files/update"))
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let mut updates = Vec::new();
    for (old_hash, version) in resp {
        let Some((old_name, enabled)) = by_hash.get(&old_hash) else {
            continue;
        };
        let Some(file) = version.files.iter().find(|f| f.primary).or_else(|| version.files.first())
        else {
            continue;
        };
        // Already up to date if the returned file is the same one we have.
        if file.hashes.sha1.as_deref() == Some(old_hash.as_str()) || &file.filename == old_name {
            continue;
        }
        updates.push(ModUpdate {
            old_file_name: old_name.clone(),
            new_file_name: file.filename.clone(),
            version_number: version.version_number.clone(),
            url: file.url.clone(),
            sha1: file.hashes.sha1.clone(),
            enabled: *enabled,
        });
    }
    Ok(updates)
}

/// Apply a single mod update: download the new jar (preserving enabled state)
/// and remove the old one.
pub async fn apply_update(state: &AppState, instance_id: &str, update: ModUpdate) -> Result<()> {
    let mods_dir = state.dirs.game_dir(instance_id).join("mods");
    let target_name = if update.enabled {
        update.new_file_name.clone()
    } else {
        format!("{}.disabled", update.new_file_name)
    };
    net::download_one(
        &state.http,
        &DownloadItem::new(update.url.clone(), mods_dir.join(&target_name), update.sha1.clone()),
    )
    .await?;

    // Remove the old jar (unless it shares the new name).
    if update.old_file_name != update.new_file_name {
        for cand in [
            mods_dir.join(&update.old_file_name),
            mods_dir.join(format!("{}.disabled", update.old_file_name)),
        ] {
            if cand.exists() {
                std::fs::remove_file(&cand)?;
            }
        }
        crate::instances::rename_install(
            state,
            instance_id,
            &update.old_file_name,
            &update.new_file_name,
        );
    }
    Ok(())
}

/// Install a Modrinth modpack as a *new* instance: parse the pack's
/// dependencies to pick the right Minecraft version + loader, create the
/// instance, then extract overrides and download its files.
pub async fn install_modpack(
    app: &AppHandle,
    state: &AppState,
    project_id: &str,
    version_id: Option<&str>,
    name_override: Option<&str>,
    icon: Option<String>,
) -> Result<crate::models::Instance> {
    let settings = crate::instances::load_settings(state);
    let (tmp_path, fallback_name) = fetch_mrpack_archive(state, project_id, version_id).await?;

    // Parse the index once up front (without a game dir) to read dependencies.
    let meta: MrpackIndex = {
        let file = std::fs::File::open(&tmp_path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        let mut idx = archive.by_name("modrinth.index.json")?;
        let mut buf = Vec::new();
        std::io::Read::read_to_end(&mut idx, &mut buf)?;
        serde_json::from_slice(&buf)?
    };

    let mc_version = meta
        .dependencies
        .minecraft
        .clone()
        .ok_or_else(|| Error::Other("modpack is missing a Minecraft version".into()))?;
    let (loader, loader_version) = meta.dependencies.loader();
    let pack_name = name_override
        .map(|s| s.to_string())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| if meta.name.is_empty() { fallback_name } else { meta.name.clone() });

    let mut instance =
        crate::instances::create_instance(state, &pack_name, &mc_version, loader, loader_version, icon)?;
    // Remember where this pack came from so we can offer diff-based updates.
    instance.pack_source = Some(crate::models::PackSource {
        provider: "modrinth".into(),
        project_id: project_id.to_string(),
        version_id: version_id.map(|s| s.to_string()),
        version_name: Some(meta.version_id.clone()),
    });
    crate::instances::save_instance(state, &instance)?;
    // Surface the new instance in the grid immediately so it shows an
    // "installing" card while its content downloads.
    let _ = app.emit("instance://created", instance.clone());

    let game_dir = state.dirs.game_dir(&instance.id);
    let (_index, items) = unpack_mrpack(&tmp_path, &game_dir)?;
    std::fs::remove_file(&tmp_path).ok();

    let task_id = format!("modpack:{}", instance.id);
    let cancel = state.cancel_flag(&task_id);
    let res = net::download_many_cancellable(
        app,
        &state.http,
        &task_id,
        &format!("Installing {pack_name}"),
        items,
        settings.max_concurrent_downloads,
        Some(cancel),
    )
    .await;
    state.clear_cancel(&task_id);

    if let Err(e) = res {
        // Roll back the half-installed instance and pull its card from the grid.
        let _ = crate::instances::delete_instance(state, &instance.id);
        let _ = app.emit("instance://removed", instance.id.clone());
        return Err(e);
    }

    Ok(instance)
}

// ---------------------------------------------------------------------------
// Export an instance as a Modrinth modpack (.mrpack)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct ExportIndex {
    #[serde(rename = "formatVersion")]
    format_version: u32,
    game: String,
    #[serde(rename = "versionId")]
    version_id: String,
    name: String,
    files: Vec<ExportFile>,
    dependencies: HashMap<String, String>,
}

#[derive(Serialize)]
struct ExportFile {
    path: String,
    hashes: ExportHashes,
    downloads: Vec<String>,
    #[serde(rename = "fileSize")]
    file_size: u64,
}

#[derive(Serialize)]
struct ExportHashes {
    sha1: String,
    sha512: String,
}

fn collect_overrides(
    dir: &std::path::Path,
    game_dir: &std::path::Path,
    out: &mut Vec<(String, std::path::PathBuf)>,
) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                collect_overrides(&p, game_dir, out);
            } else if p.is_file() {
                if let Ok(rel) = p.strip_prefix(game_dir) {
                    let zip_path = format!("overrides/{}", rel.to_string_lossy().replace('\\', "/"));
                    out.push((zip_path, p.clone()));
                }
            }
        }
    }
}

/// Export an instance as a `.mrpack`: mods that exist on Modrinth become
/// download references (resolved by hash), everything else (CurseForge/local
/// mods + `config/`) is bundled as overrides.
pub async fn export_mrpack(
    state: &AppState,
    instance_id: &str,
    dest: &std::path::Path,
) -> Result<()> {
    use std::io::Write;
    let instance = crate::instances::get_instance(state, instance_id)?;
    let game_dir = state.dirs.game_dir(instance_id);
    let mods_dir = game_dir.join("mods");

    // Hash every enabled jar (skip .disabled), keyed sha1 -> filename.
    let mut by_hash: HashMap<String, String> = HashMap::new();
    if let Ok(rd) = std::fs::read_dir(&mods_dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !name.ends_with(".jar") {
                continue;
            }
            if let Ok(h) = net::file_sha1(&e.path()).await {
                by_hash.insert(h, name);
            }
        }
    }

    // Resolve all hashes against Modrinth in one request.
    let mut resolved: HashMap<String, ProjectVersion> = HashMap::new();
    if !by_hash.is_empty() {
        let hashes: Vec<String> = by_hash.keys().cloned().collect();
        let body = serde_json::json!({ "hashes": hashes, "algorithm": "sha1" });
        if let Ok(r) = state.http.post(format!("{API}/version_files")).json(&body).send().await {
            if let Ok(ok) = r.error_for_status() {
                resolved = ok.json().await.unwrap_or_default();
            }
        }
    }

    let mut files = Vec::new();
    let mut bundled: Vec<(String, std::path::PathBuf)> = Vec::new();

    for (hash, filename) in &by_hash {
        let file = resolved.get(hash).and_then(|v| {
            v.files
                .iter()
                .find(|f| f.hashes.sha1.as_deref() == Some(hash.as_str()))
                .or_else(|| v.files.iter().find(|f| f.primary))
                .or_else(|| v.files.first())
        });
        if let Some(f) = file {
            if let (Some(sha1), Some(sha512)) = (f.hashes.sha1.clone(), f.hashes.sha512.clone()) {
                files.push(ExportFile {
                    path: format!("mods/{filename}"),
                    hashes: ExportHashes { sha1, sha512 },
                    downloads: vec![f.url.clone()],
                    file_size: f.size,
                });
                continue;
            }
        }
        // Couldn't resolve on Modrinth — bundle the jar directly.
        bundled.push((format!("overrides/mods/{filename}"), mods_dir.join(filename)));
    }

    // Bundle config/ as overrides so the pack is playable out of the box.
    let config_dir = game_dir.join("config");
    if config_dir.is_dir() {
        collect_overrides(&config_dir, &game_dir, &mut bundled);
    }

    let mut deps = HashMap::new();
    deps.insert("minecraft".to_string(), instance.mc_version.clone());
    let loader_key = match instance.loader {
        crate::models::Loader::Fabric => Some("fabric-loader"),
        crate::models::Loader::Quilt => Some("quilt-loader"),
        crate::models::Loader::Forge => Some("forge"),
        crate::models::Loader::Neoforge => Some("neoforge"),
        crate::models::Loader::Vanilla => None,
    };
    if let (Some(k), Some(v)) = (loader_key, instance.loader_version.clone()) {
        deps.insert(k.to_string(), v);
    }

    let index = ExportIndex {
        format_version: 1,
        game: "minecraft".to_string(),
        version_id: "1.0.0".to_string(),
        name: instance.name.clone(),
        files,
        dependencies: deps,
    };

    let file = std::fs::File::create(dest)?;
    let mut zip = zip::ZipWriter::new(file);
    let opts: zip::write::FileOptions<()> =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("modrinth.index.json", opts)?;
    zip.write_all(serde_json::to_string_pretty(&index)?.as_bytes())?;
    for (zip_path, src) in &bundled {
        if let Ok(bytes) = std::fs::read(src) {
            zip.start_file(zip_path.clone(), opts)?;
            zip.write_all(&bytes)?;
        }
    }
    zip.finish()?;
    Ok(())
}
