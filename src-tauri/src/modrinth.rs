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
    /// The owning project's id. Always present in API responses; defaulted so
    /// older cached/serialized shapes without it still deserialize.
    #[serde(default)]
    pub project_id: String,
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
    /// Set when a blocked CurseForge dependency was fetched from Modrinth
    /// instead (hash-verified identical file).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modrinth_fallback: Option<crate::crosssource::ModrinthRef>,
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

#[derive(Debug, Deserialize)]
struct ProjectLookup {
    id: String,
}

/// Resolve a Modrinth slug (or project id — the endpoint accepts either) to
/// its canonical project id.
pub async fn resolve_project_id(state: &AppState, slug_or_id: &str) -> Result<String> {
    let resp: ProjectLookup = state
        .http
        .get(format!("{API}/project/{slug_or_id}"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(resp.id)
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

    let folder = crate::instances::content_subdir(content_type);

    let dest_dir = state.dirs.game_dir(instance_id).join(folder);
    let target = dest_dir.join(&file.filename);
    net::download_one(
        &state.http,
        &DownloadItem::new(file.url.clone(), target, file.hashes.sha1.clone()),
    )
    .await?;

    Ok((file.filename, installed_deps))
}

/// EuphoriaPatcher (bundled by many modpacks, e.g. All the Mods 10) patches a
/// base Complementary shader at runtime, but it does NOT ship that base pack —
/// it expects the user to drop e.g. `ComplementaryShaders r5.7.1` into the
/// `shaderpacks` folder, otherwise it prints a "SHADER NOT FOUND" warning and
/// no shaders are available. This downloads the exact required Complementary
/// pack from Modrinth into the instance so the patcher just works.
///
/// `required_version` is the version token EuphoriaPatcher logs (e.g. `r5.7.1`).
/// EuphoriaPatcher accepts either Complementary Reimagined or Unbound as the
/// base, so we try Reimagined first (the more common default) then Unbound.
/// Returns the installed filename, or `None` if a matching pack is already
/// present.
pub async fn ensure_complementary_shader(
    state: &AppState,
    instance_id: &str,
    required_version: &str,
) -> Result<Option<String>> {
    let want = required_version.trim().trim_start_matches('v').to_lowercase();
    let dir = state.dirs.game_dir(instance_id).join("shaderpacks");
    std::fs::create_dir_all(&dir).ok();

    // Already have a matching Complementary pack? Then there's nothing to do.
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if name.contains("complementary") && name.contains(&want) {
                return Ok(None);
            }
        }
    }

    // Shader packs aren't tied to a mod loader, so don't filter by the
    // instance's loader (Complementary lists `iris`/`optifine`, never the
    // modpack's loader) — match purely on the version number.
    for slug in ["complementary-reimagined", "complementary-unbound"] {
        let versions = match project_versions(state, slug, None, None).await {
            Ok(v) => v,
            Err(_) => continue,
        };
        let Some(version) = versions.iter().find(|v| {
            let vn = v.version_number.to_lowercase();
            vn == want || vn.trim_start_matches('r') == want.trim_start_matches('r')
        }) else {
            continue;
        };
        let Some(file) = version
            .files
            .iter()
            .find(|f| f.primary)
            .or_else(|| version.files.first())
        else {
            continue;
        };
        let target = dir.join(&file.filename);
        net::download_one(
            &state.http,
            &DownloadItem::new(file.url.clone(), target, file.hashes.sha1.clone()),
        )
        .await?;
        return Ok(Some(file.filename.clone()));
    }

    Err(Error::NotFound(format!(
        "Complementary shader {required_version} on Modrinth"
    )))
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
            modrinth_fallback: None,
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

    let folder = crate::instances::content_subdir(content_type);
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

    // Stage under our own cache dir, not the system temp dir: `%TEMP%` can
    // resolve to a protected location when the app runs elevated (the same
    // AccessDenied class the 0.3.1 installer-staging fix addressed), and a
    // uuid suffix keeps concurrent installs of the same pack from colliding.
    let cache_dir = state.dirs.cache();
    std::fs::create_dir_all(&cache_dir)?;
    let tmp_path = cache_dir.join(format!(
        "mrpack-{}-{}.mrpack",
        &version.id[..version.id.len().min(8)],
        uuid::Uuid::new_v4()
    ));
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
            // Reject entries whose name would escape the instance dir (zip slip).
            let Some(dest) = crate::archive::safe_join(game_dir, rest) else {
                continue;
            };
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
    #[serde(default = "default_content_type")]
    pub content_type: String,
    pub old_file_name: String,
    pub new_file_name: String,
    pub version_number: String,
    pub url: String,
    pub sha1: Option<String>,
    pub enabled: bool,
    /// Which platform the update comes from ("modrinth" / "curseforge").
    #[serde(default = "default_source")]
    pub source: String,
    /// The project's id on `source` (Modrinth project id or CF mod id).
    #[serde(default)]
    pub source_project_id: Option<String>,
    /// The Modrinth version id or CurseForge file id being offered.
    #[serde(default)]
    pub source_version_id: Option<String>,
    /// ISO-8601 release date of the offered version (used for cross-platform
    /// comparison; kept for the UI).
    #[serde(default)]
    pub date: Option<String>,
    /// The file's current source-of-truth pin from the content index, so the
    /// UI can offer "switch to the other platform".
    #[serde(default)]
    pub pinned_provider: Option<String>,
}

fn default_content_type() -> String {
    "mod".into()
}

fn default_source() -> String {
    "modrinth".into()
}

fn content_dir(game_dir: &std::path::Path, content_type: &str) -> std::path::PathBuf {
    game_dir.join(crate::instances::content_subdir(content_type))
}

/// One installed file eligible for update checking.
struct LocalContent {
    content_type: String,
    /// Display name (without any `.disabled` suffix).
    file_name: String,
    enabled: bool,
    path: std::path::PathBuf,
}

/// Map sha1 -> installed file info.
async fn collect_updatable_hashes(
    state: &AppState,
    instance_id: &str,
) -> Result<HashMap<String, LocalContent>> {
    let game = state.dirs.game_dir(instance_id);
    let mut by_hash: HashMap<String, LocalContent> = HashMap::new();

    if let Ok(rd) = std::fs::read_dir(game.join("mods")) {
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
                by_hash.insert(
                    h,
                    LocalContent {
                        content_type: "mod".into(),
                        file_name: jar,
                        enabled,
                        path: e.path(),
                    },
                );
            }
        }
    }

    for (content_type, subdir) in [("resourcepack", "resourcepacks"), ("shader", "shaderpacks")] {
        if let Ok(rd) = std::fs::read_dir(game.join(subdir)) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if !name.ends_with(".zip") {
                    continue;
                }
                if let Ok(h) = net::file_sha1(&e.path()).await {
                    by_hash.insert(
                        h,
                        LocalContent {
                            content_type: content_type.into(),
                            file_name: name,
                            enabled: true,
                            path: e.path(),
                        },
                    );
                }
            }
        }
    }

    Ok(by_hash)
}

/// Order two ISO-8601 timestamps. Falls back to a plain string comparison for
/// anything chrono can't parse (still correct for same-format timestamps).
fn newer_date(a: &str, b: &str) -> std::cmp::Ordering {
    use chrono::DateTime;
    match (
        DateTime::parse_from_rfc3339(a),
        DateTime::parse_from_rfc3339(b),
    ) {
        (Ok(a), Ok(b)) => a.cmp(&b),
        _ => a.cmp(b),
    }
}

/// Given at most one candidate per platform, pick the one to offer: newest
/// release date wins; the file's pinned provider (its content-index source of
/// truth) breaks ties.
fn choose_update(
    modrinth: Option<ModUpdate>,
    curseforge: Option<ModUpdate>,
    pinned: Option<&str>,
) -> Option<ModUpdate> {
    match (modrinth, curseforge) {
        (None, None) => None,
        (Some(m), None) => Some(m),
        (None, Some(c)) => Some(c),
        (Some(m), Some(c)) => {
            let ord = newer_date(
                m.date.as_deref().unwrap_or(""),
                c.date.as_deref().unwrap_or(""),
            );
            Some(match ord {
                std::cmp::Ordering::Greater => m,
                std::cmp::Ordering::Less => c,
                std::cmp::Ordering::Equal => {
                    if pinned == Some("curseforge") {
                        c
                    } else {
                        m
                    }
                }
            })
        }
    }
}

/// One `version_files/update` batch, optionally constrained by loader and
/// game version. Empty hash lists skip the request entirely.
async fn modrinth_update_batch(
    state: &AppState,
    hashes: Vec<String>,
    loader: Option<&str>,
    game_version: Option<&str>,
) -> Result<HashMap<String, ProjectVersion>> {
    if hashes.is_empty() {
        return Ok(HashMap::new());
    }
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
    Ok(state
        .http
        .post(format!("{API}/version_files/update"))
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?)
}

/// Check installed mods, resource packs, and shaders against BOTH Modrinth and
/// CurseForge and return, per file, the newest version available for the
/// instance's loader + game version (with its source platform).
///
/// CurseForge checking is best-effort: without an API key (or when the
/// fingerprint lookup fails) the result is Modrinth-only, as before.
pub async fn check_updates(
    state: &AppState,
    instance_id: &str,
    loader: Option<&str>,
    game_version: Option<&str>,
) -> Result<Vec<ModUpdate>> {
    let by_hash = collect_updatable_hashes(state, instance_id).await?;
    if by_hash.is_empty() {
        return Ok(Vec::new());
    }

    // Two Modrinth batches: the mod-loader filter only applies to mods.
    // Resource-pack and shader versions are tagged `minecraft`/`iris`/
    // `optifine` on Modrinth — never `fabric`/`forge` — so filtering their
    // hashes by the instance loader would hide every update they have.
    let mut mod_hashes = Vec::new();
    let mut pack_hashes = Vec::new();
    for (hash, local) in &by_hash {
        if local.content_type == "mod" {
            mod_hashes.push(hash.clone());
        } else {
            pack_hashes.push(hash.clone());
        }
    }
    let mut resp = modrinth_update_batch(state, mod_hashes, loader, game_version).await?;
    resp.extend(modrinth_update_batch(state, pack_hashes, None, game_version).await?);

    // CurseForge candidates for the same files (empty without an API key).
    let cf_candidates = match game_version.filter(|v| !v.is_empty()) {
        Some(gv) => {
            let files: Vec<(String, std::path::PathBuf)> = by_hash
                .iter()
                .map(|(h, c)| (h.clone(), c.path.clone()))
                .collect();
            crate::curseforge::update_candidates(state, &files, loader, gv).await
        }
        None => std::collections::HashMap::new(),
    };

    // The pinned provider per file (where it was installed from / switched to).
    let providers = crate::instances::content_provider_map(state, instance_id);

    let mut updates = Vec::new();
    for (old_hash, local) in &by_hash {
        let from_modrinth = resp.get(old_hash).and_then(|version| {
            let file = version.files.iter().find(|f| f.primary).or_else(|| version.files.first())?;
            if file.hashes.sha1.as_deref() == Some(old_hash.as_str())
                || file.filename == local.file_name
            {
                return None;
            }
            Some(ModUpdate {
                content_type: local.content_type.clone(),
                old_file_name: local.file_name.clone(),
                new_file_name: file.filename.clone(),
                version_number: version.version_number.clone(),
                url: file.url.clone(),
                sha1: file.hashes.sha1.clone(),
                enabled: local.enabled,
                source: "modrinth".into(),
                source_project_id: Some(version.project_id.clone()).filter(|p| !p.is_empty()),
                source_version_id: Some(version.id.clone()),
                date: Some(version.date_published.clone()),
                pinned_provider: None,
            })
        });

        let from_curseforge = cf_candidates.get(old_hash).and_then(|c| {
            if c.sha1.as_deref() == Some(old_hash.as_str()) || c.file_name == local.file_name {
                return None;
            }
            Some(ModUpdate {
                content_type: local.content_type.clone(),
                old_file_name: local.file_name.clone(),
                new_file_name: c.file_name.clone(),
                version_number: c.file_name.trim_end_matches(".jar").to_string(),
                url: c.url.clone(),
                sha1: c.sha1.clone(),
                enabled: local.enabled,
                source: "curseforge".into(),
                source_project_id: Some(c.project_id.to_string()),
                source_version_id: Some(c.file_id.to_string()),
                date: Some(c.file_date.clone()),
                pinned_provider: None,
            })
        });

        let pinned = providers.get(&local.file_name).map(|s| s.as_str());
        if let Some(mut chosen) = choose_update(from_modrinth, from_curseforge, pinned) {
            chosen.pinned_provider = pinned.map(|s| s.to_string());
            updates.push(chosen);
        }
    }
    Ok(updates)
}

/// Apply a single content update (mod / resource pack / shader).
pub async fn apply_update(state: &AppState, instance_id: &str, update: ModUpdate) -> Result<()> {
    let dir = content_dir(&state.dirs.game_dir(instance_id), &update.content_type);
    let target_name = if update.content_type == "mod" && !update.enabled {
        format!("{}.disabled", update.new_file_name)
    } else {
        update.new_file_name.clone()
    };
    net::download_one(
        &state.http,
        &DownloadItem::new(update.url.clone(), dir.join(&target_name), update.sha1.clone()),
    )
    .await?;

    if update.old_file_name != update.new_file_name {
        let old_paths: Vec<std::path::PathBuf> = if update.content_type == "mod" {
            vec![
                dir.join(&update.old_file_name),
                dir.join(format!("{}.disabled", update.old_file_name)),
            ]
        } else {
            vec![dir.join(&update.old_file_name)]
        };
        for cand in old_paths {
            if cand.exists() {
                std::fs::remove_file(&cand)?;
            }
        }
    }

    // Keep the content index in sync when the file name changes.
    crate::instances::migrate_index_entry(state, instance_id, &update.old_file_name, &update.new_file_name);

    // The installed bytes now come from `update.source`: re-anchor the file's
    // identity there so future checks and exports use the right project.
    if let Some(project_id) = update.source_project_id.as_deref().filter(|p| !p.is_empty()) {
        crate::instances::record_installs(
            state,
            instance_id,
            &[(update.new_file_name.clone(), project_id.to_string())],
            &update.source,
        );
    }

    Ok(())
}

/// Check for updates and apply them all. Returns how many items were updated.
pub async fn auto_update_all(
    state: &AppState,
    instance_id: &str,
    loader: Option<&str>,
    game_version: Option<&str>,
) -> Result<u32> {
    let instance = crate::instances::get_instance(state, instance_id)?;
    // Modpack instances are pinned to a tested set — per-mod updates break them.
    if instance.pack_source.is_some() {
        return Ok(0);
    }

    let updates = check_updates(state, instance_id, loader, game_version).await?;
    let count = updates.len() as u32;
    for update in updates {
        apply_update(state, instance_id, update).await?;
    }
    Ok(count)
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
    let (tmp_path, fallback_name) = fetch_mrpack_archive(state, project_id, version_id).await?;
    let source = Some((project_id.to_string(), version_id.map(|s| s.to_string())));
    let res =
        instance_from_mrpack_archive(app, state, &tmp_path, fallback_name, name_override, icon, source)
            .await;
    std::fs::remove_file(&tmp_path).ok();
    res
}

/// Import a local `.mrpack` file as a new instance. The file itself is left in
/// place; without a Modrinth project id the instance can't be diff-updated.
pub async fn import_mrpack_file(
    app: &AppHandle,
    state: &AppState,
    path: &std::path::Path,
) -> Result<crate::models::Instance> {
    let fallback_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Imported pack")
        .to_string();
    instance_from_mrpack_archive(app, state, path, fallback_name, None, None, None).await
}

/// Shared core of modpack install/import: parse the index of an `.mrpack`
/// already on disk, create the instance, extract overrides, download files.
/// `source` is the Modrinth (project_id, version_id) when known.
async fn instance_from_mrpack_archive(
    app: &AppHandle,
    state: &AppState,
    archive_path: &std::path::Path,
    fallback_name: String,
    name_override: Option<&str>,
    icon: Option<String>,
    source: Option<(String, Option<String>)>,
) -> Result<crate::models::Instance> {
    let settings = crate::instances::load_settings(state);

    // Parse the index once up front (without a game dir) to read dependencies.
    let meta: MrpackIndex = {
        let file = std::fs::File::open(archive_path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        let mut idx = archive.by_name("modrinth.index.json").map_err(|_| {
            Error::Other("Not a Modrinth modpack: missing modrinth.index.json.".into())
        })?;
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
    if let Some((project_id, version_id)) = source {
        instance.pack_source = Some(crate::models::PackSource {
            provider: "modrinth".into(),
            project_id,
            version_id,
            version_name: Some(meta.version_id.clone()),
        });
        crate::instances::save_instance(state, &instance)?;
    }
    // Surface the new instance in the grid immediately so it shows an
    // "installing" card while its content downloads.
    let _ = app.emit("instance://created", instance.clone());

    let game_dir = state.dirs.game_dir(&instance.id);
    let (_index, items) = match unpack_mrpack(archive_path, &game_dir) {
        Ok(v) => v,
        Err(e) => {
            // Roll back the half-created instance so a bad archive doesn't
            // leave a broken card in the grid.
            let _ = crate::instances::delete_instance(state, &instance.id);
            let _ = app.emit("instance://removed", instance.id.clone());
            return Err(e);
        }
    };

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

// (Modpack export now lives in `crate::export`, which resolves files on both
// platforms and writes .mrpack and CurseForge manifest zips from one model.)

#[cfg(test)]
mod tests {
    use super::*;

    fn update(source: &str, date: &str) -> ModUpdate {
        ModUpdate {
            content_type: "mod".into(),
            old_file_name: "old.jar".into(),
            new_file_name: format!("{source}.jar"),
            version_number: "1.1".into(),
            url: format!("https://example.invalid/{source}.jar"),
            sha1: None,
            enabled: true,
            source: source.into(),
            source_project_id: Some("p".into()),
            source_version_id: Some("v".into()),
            date: Some(date.into()),
            pinned_provider: None,
        }
    }

    #[test]
    fn newest_release_date_wins_across_platforms() {
        let m = update("modrinth", "2025-06-01T12:00:00+00:00");
        // CurseForge dates carry fractional seconds — must still compare right.
        let c = update("curseforge", "2025-06-02T09:30:00.123Z");
        let chosen = choose_update(Some(m.clone()), Some(c.clone()), None).unwrap();
        assert_eq!(chosen.source, "curseforge");

        let chosen = choose_update(Some(m), Some(update("curseforge", "2025-05-01T00:00:00Z")), None)
            .unwrap();
        assert_eq!(chosen.source, "modrinth");
    }

    #[test]
    fn pinned_provider_breaks_date_ties() {
        let same = "2025-06-01T12:00:00Z";
        let m = update("modrinth", same);
        let c = update("curseforge", same);
        let chosen = choose_update(Some(m.clone()), Some(c.clone()), Some("curseforge")).unwrap();
        assert_eq!(chosen.source, "curseforge");
        let chosen = choose_update(Some(m.clone()), Some(c.clone()), Some("modrinth")).unwrap();
        assert_eq!(chosen.source, "modrinth");
        // No pin: Modrinth is the default tiebreak.
        let chosen = choose_update(Some(m), Some(c), None).unwrap();
        assert_eq!(chosen.source, "modrinth");
    }

    #[test]
    fn single_platform_candidates_pass_through() {
        let m = update("modrinth", "2025-06-01T12:00:00Z");
        assert_eq!(choose_update(Some(m), None, Some("curseforge")).unwrap().source, "modrinth");
        let c = update("curseforge", "2025-06-01T12:00:00Z");
        assert_eq!(choose_update(None, Some(c), Some("modrinth")).unwrap().source, "curseforge");
        assert!(choose_update(None, None, None).is_none());
    }

    #[test]
    fn unparseable_dates_fall_back_to_string_order() {
        assert_eq!(newer_date("zzz", "aaa"), std::cmp::Ordering::Greater);
        assert_eq!(
            newer_date("2025-06-01T00:00:00Z", "2025-06-01T00:00:00+00:00"),
            std::cmp::Ordering::Equal
        );
    }

    #[test]
    fn legacy_mod_update_payloads_deserialize() {
        // Payloads from a frontend built before the cross-source fields
        // existed must still round-trip through apply_mod_update.
        let json = serde_json::json!({
            "contentType": "mod",
            "oldFileName": "a.jar",
            "newFileName": "b.jar",
            "versionNumber": "2.0",
            "url": "https://cdn.modrinth.com/b.jar",
            "sha1": null,
            "enabled": true
        });
        let u: ModUpdate = serde_json::from_value(json).unwrap();
        assert_eq!(u.source, "modrinth");
        assert!(u.source_project_id.is_none());
        assert!(u.date.is_none());
    }
}
