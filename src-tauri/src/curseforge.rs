//! CurseForge integration: search projects and install mod/resourcepack/shader
//! files. Results are mapped onto the same [`ModHit`]/[`SearchResponse`] shapes
//! the Modrinth module uses, so the frontend can render either provider with the
//! same components.

use crate::crosssource::{CurseforgeRef, ModrinthRef};
use crate::error::{Error, Result};
use crate::modrinth::{ContentVersion, InstalledDep, ModHit, SearchResponse};
use crate::net::{self, DownloadItem};
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

const API: &str = "https://api.curseforge.com/v1";
const GAME_ID: u32 = 432; // Minecraft

// CurseForge class IDs per content type.
fn class_id(content_type: &str) -> u32 {
    match content_type {
        "resourcepack" => 12,
        "shader" => 6552,
        "modpack" => 4471,
        "world" => 17,
        _ => 6, // mod
    }
}

// CurseForge modLoaderType enum: Forge=1, Fabric=4, Quilt=5, NeoForge=6.
pub(crate) fn loader_type(loader: Option<&str>) -> Option<u8> {
    match loader {
        Some("forge") => Some(1),
        Some("fabric") => Some(4),
        Some("quilt") => Some(5),
        Some("neoforge") => Some(6),
        _ => None,
    }
}

/// Resolve the API key from settings, falling back to `EZMAPA_CF_API_KEY` or
/// legacy `BEACON_CF_API_KEY`. Returns a friendly error when neither is set.
pub(crate) fn api_key(state: &AppState) -> Result<String> {
    if let Some(k) = crate::instances::load_settings(state)
        .curseforge_api_key
        .filter(|k| !k.trim().is_empty())
    {
        return Ok(k);
    }
    for var in ["EZMAPA_CF_API_KEY", "BEACON_CF_API_KEY"] {
        if let Ok(k) = std::env::var(var) {
            if !k.trim().is_empty() {
                return Ok(k);
            }
        }
    }
    Err(Error::Other(
        "No CurseForge API key set. Add one in Settings or set EZMAPA_CF_API_KEY.".into(),
    ))
}

// ---------------------------------------------------------------------------
// API response shapes
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct CfSearchResponse {
    data: Vec<CfMod>,
    #[serde(default)]
    pagination: Option<CfPagination>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfPagination {
    #[serde(default)]
    total_count: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfMod {
    id: u32,
    name: String,
    #[serde(default)]
    slug: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    download_count: f64,
    #[serde(default)]
    logo: Option<CfLogo>,
    #[serde(default)]
    authors: Vec<CfAuthor>,
}

#[derive(Deserialize)]
struct CfLogo {
    #[serde(default)]
    url: String,
}

#[derive(Deserialize)]
struct CfAuthor {
    #[serde(default)]
    name: String,
}

#[derive(Deserialize)]
struct CfFilesResponse {
    data: Vec<CfFile>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfFile {
    id: u32,
    #[serde(default)]
    mod_id: u32,
    file_name: String,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    download_url: Option<String>,
    #[serde(default)]
    hashes: Vec<CfHash>,
    #[serde(default)]
    file_date: String,
    #[serde(default)]
    game_versions: Vec<String>,
    #[serde(default)]
    dependencies: Vec<CfDependency>,
}

impl CfFile {
    fn sha1(&self) -> Option<String> {
        self.hashes
            .iter()
            .find(|h| h.algo == 1)
            .map(|h| h.value.to_lowercase())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfDependency {
    mod_id: u32,
    relation_type: u8,
}

impl CfDependency {
    /// True when this dependency should be auto-installed.
    fn is_required(&self) -> bool {
        // 3 = RequiredDependency, 6 = Include (bundled library).
        matches!(self.relation_type, 3 | 6)
    }
}

#[derive(Deserialize)]
struct CfHash {
    value: String,
    algo: u8, // 1 = Sha1, 2 = Md5
}

// ---------------------------------------------------------------------------
// Mapping to the shared ModHit shape
// ---------------------------------------------------------------------------

impl CfMod {
    fn into_hit(self, project_type: &str) -> ModHit {
        ModHit {
            project_id: self.id.to_string(),
            slug: self.slug,
            title: self.name,
            description: self.summary,
            author: self.authors.into_iter().next().map(|a| a.name).unwrap_or_default(),
            downloads: self.download_count as u64,
            follows: 0,
            icon_url: self.logo.map(|l| l.url).filter(|u| !u.is_empty()),
            categories: Vec::new(),
            versions: Vec::new(),
            project_type: project_type.to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

/// Search CurseForge for the given content type, constrained by loader + version.
pub async fn search(
    state: &AppState,
    query: &str,
    content_type: &str,
    loader: Option<&str>,
    game_version: Option<&str>,
    limit: u32,
    offset: u32,
) -> Result<SearchResponse> {
    let key = api_key(state)?;

    let game_id = GAME_ID.to_string();
    let class = class_id(content_type).to_string();
    let limit_s = limit.to_string();
    let offset_s = offset.to_string();

    let mut params: Vec<(&str, String)> = vec![
        ("gameId", game_id),
        ("classId", class),
        ("searchFilter", query.to_string()),
        ("sortField", "2".to_string()), // Popularity
        ("sortOrder", "desc".to_string()),
        ("pageSize", limit_s),
        ("index", offset_s),
    ];
    if let Some(v) = game_version {
        if !v.is_empty() {
            params.push(("gameVersion", v.to_string()));
        }
    }
    // Loader filter only applies to mods.
    if content_type == "mod" {
        if let Some(lt) = loader_type(loader) {
            params.push(("modLoaderType", lt.to_string()));
        }
    }

    let resp: CfSearchResponse = state
        .http
        .get(format!("{API}/mods/search"))
        .header("x-api-key", &key)
        .header("Accept", "application/json")
        .query(&params)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let total = resp.pagination.map(|p| p.total_count).unwrap_or(0);
    let hits = resp.data.into_iter().map(|m| m.into_hit(content_type)).collect();
    Ok(SearchResponse {
        hits,
        total_hits: total,
    })
}

/// List files for a CurseForge project for the version picker (newest first).
pub async fn list_files(state: &AppState, project_id: &str) -> Result<Vec<ContentVersion>> {
    let key = api_key(state)?;
    let mod_id: u32 = project_id
        .parse()
        .map_err(|_| Error::Other(format!("invalid CurseForge id: {project_id}")))?;

    let resp: CfFilesResponse = state
        .http
        .get(format!("{API}/mods/{mod_id}/files?pageSize=50"))
        .header("x-api-key", &key)
        .header("Accept", "application/json")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let mut files = resp.data;
    files.sort_by(|a, b| b.file_date.cmp(&a.file_date));
    Ok(files
        .into_iter()
        .map(|f| ContentVersion {
            id: f.id.to_string(),
            name: if f.display_name.is_empty() { f.file_name.clone() } else { f.display_name },
            version_number: f.file_name,
            game_versions: f.game_versions,
            loaders: Vec::new(),
            date: f.file_date,
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Install
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Blocked downloads (`downloadUrl: null`)
//
// Authors can disable third-party distribution, in which case the API refuses
// to serve a URL. Per mod-distribution rules we never guess CDN links for such
// files; the only automated workaround is fetching the byte-identical file
// from Modrinth (matched by CF's own sha1, verified on download) — otherwise
// the user has to download it manually from the CurseForge page.
// ---------------------------------------------------------------------------

/// How a blocked file will be obtained.
enum BlockedPlan {
    /// The identical file exists on Modrinth: download it there, verifying the
    /// CurseForge sha1 byte-for-byte.
    Modrinth(ModrinthRef),
    /// Not on Modrinth (or CF served no sha1): the user must fetch it manually.
    Manual,
}

/// Try to source a blocked CurseForge file from Modrinth by exact hash.
async fn plan_blocked_file(state: &AppState, file: &CfFile) -> BlockedPlan {
    let Some(sha1) = file.sha1() else {
        return BlockedPlan::Manual;
    };
    let cf = CurseforgeRef {
        project_id: file.mod_id,
        file_id: file.id,
    };
    match state
        .crosssource
        .resolve_cf_file_to_modrinth(&state.http, cf, &sha1)
        .await
    {
        Ok(Some(mr)) => BlockedPlan::Modrinth(mr),
        _ => BlockedPlan::Manual,
    }
}

/// The CurseForge web page for a mod (for "download manually" links). Prefers
/// the API-provided site link; the `/projects/{id}` redirect is the fallback.
async fn mod_page_url(state: &AppState, mod_id: u32) -> String {
    #[derive(Deserialize)]
    struct Resp {
        data: CfModInfo,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct CfModInfo {
        #[serde(default)]
        links: CfLinks,
    }
    #[derive(Deserialize, Default)]
    #[serde(rename_all = "camelCase")]
    struct CfLinks {
        #[serde(default)]
        website_url: Option<String>,
    }

    if let Ok(key) = api_key(state) {
        let resp = state
            .http
            .get(format!("{API}/mods/{mod_id}"))
            .header("x-api-key", &key)
            .header("Accept", "application/json")
            .send()
            .await;
        if let Ok(r) = resp {
            if let Ok(r) = r.error_for_status() {
                if let Ok(body) = r.json::<Resp>().await {
                    if let Some(url) = body.data.links.website_url.filter(|u| !u.is_empty()) {
                        return url;
                    }
                }
            }
        }
    }
    format!("https://www.curseforge.com/projects/{mod_id}")
}

/// The result of installing a single CurseForge file.
pub struct CfInstall {
    pub file_name: String,
    pub deps: Vec<InstalledDep>,
    /// Set when the primary file was blocked on CurseForge and fetched from
    /// Modrinth instead (hash-verified identical file).
    pub modrinth_fallback: Option<ModrinthRef>,
}

/// Download the newest compatible file of a CurseForge project into the correct
/// sub-directory of the instance and return the installed filename plus any
/// auto-installed required dependencies.
pub async fn install_content(
    state: &AppState,
    instance_id: &str,
    project_id: &str,
    content_type: &str,
    loader: Option<&str>,
    game_version: Option<&str>,
) -> Result<CfInstall> {
    let key = api_key(state)?;
    let mod_id: u32 = project_id
        .parse()
        .map_err(|_| Error::Other(format!("invalid CurseForge id: {project_id}")))?;

    let mut params: Vec<(&str, String)> = vec![("pageSize", "50".to_string())];
    if let Some(v) = game_version {
        if !v.is_empty() {
            params.push(("gameVersion", v.to_string()));
        }
    }
    if content_type == "mod" {
        if let Some(lt) = loader_type(loader) {
            params.push(("modLoaderType", lt.to_string()));
        }
    }

    let resp: CfFilesResponse = state
        .http
        .get(format!("{API}/mods/{mod_id}/files"))
        .header("x-api-key", &key)
        .header("Accept", "application/json")
        .query(&params)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    // Newest first by file date.
    let mut files = resp.data;
    files.sort_by(|a, b| b.file_date.cmp(&a.file_date));
    let file = files
        .into_iter()
        .next()
        .ok_or_else(|| Error::NotFound(format!("compatible file for CurseForge {mod_id}")))?;

    install_cf_file(state, instance_id, content_type, loader, game_version, file).await
}

/// Install a resolved CurseForge file and its required dependencies.
async fn install_cf_file(
    state: &AppState,
    instance_id: &str,
    content_type: &str,
    loader: Option<&str>,
    game_version: Option<&str>,
    file: CfFile,
) -> Result<CfInstall> {
    let sha1 = file.sha1();
    let mut modrinth_fallback = None;
    let url = match file.download_url.clone() {
        Some(url) => url,
        None => match plan_blocked_file(state, &file).await {
            BlockedPlan::Modrinth(mr) => {
                // CF served a sha1 (resolution requires it): keep verifying
                // against CF's hash so we only ever accept the identical file.
                let url = mr.url.clone();
                modrinth_fallback = Some(mr);
                url
            }
            BlockedPlan::Manual => {
                let page = mod_page_url(state, file.mod_id).await;
                return Err(Error::Other(format!(
                    "The author of {} has disabled third-party downloads and no identical \
                     file exists on Modrinth. Download it manually from {page} and drop it \
                     into the instance folder.",
                    file.file_name
                )));
            }
        },
    };

    let folder = crate::instances::content_subdir(content_type);

    // Install required dependencies first so the primary file isn't left without
    // libraries it needs.
    let installed_deps = if content_type == "mod" {
        let mods_dir = state.dirs.game_dir(instance_id).join("mods");
        let mut visited = std::collections::HashSet::new();
        let mut out = Vec::new();
        for dep in file.dependencies.iter().filter(|d| d.is_required()) {
            out.extend(
                install_curseforge_dependency(
                    state,
                    dep.mod_id,
                    loader,
                    game_version,
                    &mods_dir,
                    &mut visited,
                )
                .await,
            );
        }
        out
    } else {
        Vec::new()
    };

    let target = state.dirs.game_dir(instance_id).join(folder).join(&file.file_name);
    net::download_one(&state.http, &DownloadItem::new(url, target, sha1)).await?;

    Ok(CfInstall {
        file_name: file.file_name,
        deps: installed_deps,
        modrinth_fallback,
    })
}

/// Recursively install a single CurseForge dependency into the mods folder.
/// `visited` prevents circular dependencies. Returns every dependency file that
/// was newly installed (including transitive ones).
fn install_curseforge_dependency<'a>(
    state: &'a AppState,
    mod_id: u32,
    loader: Option<&'a str>,
    game_version: Option<&'a str>,
    mods_dir: &'a std::path::Path,
    visited: &'a mut std::collections::HashSet<u32>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<crate::modrinth::InstalledDep>> + Send + 'a>>
{
    Box::pin(async move {
        if !visited.insert(mod_id) {
            return Vec::new();
        }

        let key = match api_key(state) {
            Ok(k) => k,
            Err(_) => return Vec::new(),
        };

        let mut params: Vec<(&str, String)> = vec![("pageSize", "50".to_string())];
        if let Some(v) = game_version {
            if !v.is_empty() {
                params.push(("gameVersion", v.to_string()));
            }
        }
        if let Some(lt) = loader_type(loader) {
            params.push(("modLoaderType", lt.to_string()));
        }

        let resp = state
            .http
            .get(format!("{API}/mods/{mod_id}/files"))
            .header("x-api-key", &key)
            .header("Accept", "application/json")
            .query(&params)
            .send()
            .await;

        let mut files = match resp {
            Ok(r) => match r.error_for_status() {
                Ok(r) => match r.json::<CfFilesResponse>().await {
                    Ok(body) => body.data,
                    Err(_) => return Vec::new(),
                },
                Err(_) => return Vec::new(),
            },
            Err(_) => return Vec::new(),
        };
        files.sort_by(|a, b| b.file_date.cmp(&a.file_date));
        let file = match files.into_iter().next() {
            Some(f) => f,
            None => return Vec::new(),
        };

        let target = mods_dir.join(&file.file_name);
        let target_dis = mods_dir.join(format!("{}.disabled", file.file_name));
        if target.exists() || target_dis.exists() {
            return Vec::new();
        }

        // Dependencies are best-effort: a blocked file that isn't mirrored on
        // Modrinth is skipped silently, like any other dependency failure.
        let mut modrinth_fallback = None;
        let url = match file.download_url.clone() {
            Some(url) => url,
            None => match plan_blocked_file(state, &file).await {
                BlockedPlan::Modrinth(mr) => {
                    let url = mr.url.clone();
                    modrinth_fallback = Some(mr);
                    url
                }
                BlockedPlan::Manual => return Vec::new(),
            },
        };
        let sha1 = file.sha1();

        if net::download_one(&state.http, &DownloadItem::new(url, target, sha1))
            .await
            .is_err()
        {
            return Vec::new();
        }

        let mut out = vec![crate::modrinth::InstalledDep {
            file_name: file.file_name.clone(),
            project_id: mod_id.to_string(),
            modrinth_fallback,
        }];

        for dep in file.dependencies.iter().filter(|d| d.is_required()) {
            out.extend(
                install_curseforge_dependency(state, dep.mod_id, loader, game_version, mods_dir, visited)
                    .await,
            );
        }

        out
    })
}

// ---------------------------------------------------------------------------
// Modpack support (CurseForge .zip with manifest.json)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct CfManifest {
    minecraft: CfManifestMc,
    #[serde(default)]
    name: String,
    #[serde(default)]
    files: Vec<CfManifestFile>,
    #[serde(default = "default_overrides")]
    overrides: String,
}

fn default_overrides() -> String {
    "overrides".to_string()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfManifestMc {
    version: String,
    #[serde(default)]
    mod_loaders: Vec<CfModLoader>,
}

#[derive(Deserialize)]
struct CfModLoader {
    id: String,
    #[serde(default)]
    primary: bool,
}

#[derive(Deserialize)]
struct CfManifestFile {
    #[serde(rename = "fileID")]
    file_id: u32,
}

/// Map a CurseForge modLoader id ("forge-47.2.0", "fabric-0.15.0", …) to a
/// (loader, version) pair.
fn parse_modloader(id: &str) -> (crate::models::Loader, Option<String>) {
    use crate::models::Loader;
    let (kind, version) = id.split_once('-').unwrap_or((id, ""));
    let loader = match kind.to_ascii_lowercase().as_str() {
        "fabric" => Loader::Fabric,
        "quilt" => Loader::Quilt,
        "neoforge" => Loader::Neoforge,
        "forge" => Loader::Forge,
        _ => Loader::Vanilla,
    };
    let v = if version.is_empty() { None } else { Some(version.to_string()) };
    (loader, v)
}

// Single-file response (`GET /v1/mods/{id}/files/{fileId}`).
#[derive(Deserialize)]
struct CfSingleFileResponse {
    data: CfFile,
}

#[derive(Deserialize)]
struct CfDescriptionResponse {
    data: String,
}

// Bulk file-resolution response (`POST /v1/mods/files`).
#[derive(Deserialize)]
struct CfBulkFiles {
    data: Vec<CfFile>,
}

/// Fetch the full HTML description of a CurseForge mod.
pub async fn get_description(state: &AppState, project_id: &str) -> Result<String> {
    let key = api_key(state)?;
    let mod_id: u32 = project_id
        .parse()
        .map_err(|_| Error::Other(format!("invalid CurseForge id: {project_id}")))?;

    let resp: CfDescriptionResponse = state
        .http
        .get(format!("{API}/mods/{mod_id}/description"))
        .header("x-api-key", &key)
        .header("Accept", "application/json")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    Ok(resp.data)
}

/// Install a specific CurseForge file by its file ID, including any required
/// dependencies when the content type is a mod.
pub async fn install_file(
    state: &AppState,
    instance_id: &str,
    project_id: &str,
    file_id: &str,
    content_type: &str,
    loader: Option<&str>,
    game_version: Option<&str>,
) -> Result<CfInstall> {
    let key = api_key(state)?;
    let mod_id: u32 = project_id
        .parse()
        .map_err(|_| Error::Other(format!("invalid CurseForge id: {project_id}")))?;
    let fid: u32 = file_id
        .parse()
        .map_err(|_| Error::Other(format!("invalid CurseForge file id: {file_id}")))?;

    let resp: CfSingleFileResponse = state
        .http
        .get(format!("{API}/mods/{mod_id}/files/{fid}"))
        .header("x-api-key", &key)
        .header("Accept", "application/json")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    install_cf_file(state, instance_id, content_type, loader, game_version, resp.data).await
}

/// Install a CurseForge modpack as a *new* instance: download the pack zip,
/// parse manifest.json, create an instance with the right loader + version,
/// extract overrides, then resolve and download the listed mod files.
pub async fn install_modpack(
    app: &AppHandle,
    state: &AppState,
    project_id: &str,
    file_id: Option<&str>,
    name_override: Option<&str>,
    icon: Option<String>,
) -> Result<crate::models::Instance> {
    let key = api_key(state)?;
    let settings = crate::instances::load_settings(state);
    let mod_id: u32 = project_id
        .parse()
        .map_err(|_| Error::Other(format!("invalid CurseForge id: {project_id}")))?;

    // Resolve the pack file (chosen or newest).
    let resp: CfFilesResponse = state
        .http
        .get(format!("{API}/mods/{mod_id}/files?pageSize=50"))
        .header("x-api-key", &key)
        .header("Accept", "application/json")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let mut files = resp.data;
    files.sort_by(|a, b| b.file_date.cmp(&a.file_date));
    let pack_file = match file_id {
        Some(fid) => {
            let fid: u32 = fid.parse().map_err(|_| Error::Other("invalid file id".into()))?;
            files.into_iter().find(|f| f.id == fid)
        }
        None => files.into_iter().next(),
    }
    .ok_or_else(|| Error::NotFound(format!("modpack file for {mod_id}")))?;

    let Some(pack_url) = pack_file.download_url.clone() else {
        let page = mod_page_url(state, mod_id).await;
        return Err(Error::Other(format!(
            "The author of this modpack has disabled third-party downloads. \
             Download the pack manually from {page} and import the zip instead."
        )));
    };
    let tmp_path = std::env::temp_dir().join(format!("ezmapa_cf_{}.zip", pack_file.id));
    net::download_one(&state.http, &DownloadItem::new(pack_url, tmp_path.clone(), None)).await?;

    // Parse manifest.json from the archive.
    let manifest: CfManifest = {
        let f = std::fs::File::open(&tmp_path)?;
        let mut archive = zip::ZipArchive::new(f)?;
        let mut mf = archive.by_name("manifest.json")?;
        let mut buf = Vec::new();
        std::io::Read::read_to_end(&mut mf, &mut buf)?;
        serde_json::from_slice(&buf)?
    };

    let mc_version = manifest.minecraft.version.clone();
    let loader_id = manifest
        .minecraft
        .mod_loaders
        .iter()
        .find(|l| l.primary)
        .or_else(|| manifest.minecraft.mod_loaders.first())
        .map(|l| l.id.clone())
        .unwrap_or_default();
    let (loader, loader_version) = parse_modloader(&loader_id);

    let pack_name = name_override
        .map(|s| s.to_string())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| if manifest.name.is_empty() { "CurseForge Pack".into() } else { manifest.name.clone() });

    let instance =
        crate::instances::create_instance(state, &pack_name, &mc_version, loader, loader_version, icon)?;
    // Surface the new instance in the grid immediately so it shows an
    // "installing" card while its content downloads.
    let _ = app.emit("instance://created", instance.clone());
    let game_dir = state.dirs.game_dir(&instance.id);

    // Extract the overrides folder into the game dir.
    {
        let prefix = format!("{}/", manifest.overrides);
        let f = std::fs::File::open(&tmp_path)?;
        let mut archive = zip::ZipArchive::new(f)?;
        for i in 0..archive.len() {
            let mut zf = archive.by_index(i)?;
            let zname = zf.name().to_string();
            if let Some(rest) = zname.strip_prefix(&prefix) {
                if rest.is_empty() {
                    continue;
                }
                // Reject entries whose name would escape the instance dir (zip slip).
                let Some(dest) = crate::archive::safe_join(&game_dir, rest) else {
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
    }
    std::fs::remove_file(&tmp_path).ok();

    // Bulk-resolve download URLs for the listed mod files. Files whose author
    // blocked third-party distribution (null downloadUrl) are re-sourced from
    // Modrinth by exact sha1 where possible; the rest are reported for manual
    // download instead of failing the install.
    let file_ids: Vec<u32> = manifest.files.iter().map(|f| f.file_id).collect();
    let mut items: Vec<DownloadItem> = Vec::new();
    let mut index_entries: Vec<(String, String)> = Vec::new();
    let mut fallbacks: Vec<(String, ModrinthRef)> = Vec::new();
    let mut blocked: Vec<CfFile> = Vec::new();
    if !file_ids.is_empty() {
        let body = serde_json::json!({ "fileIds": file_ids });
        let bulk: CfBulkFiles = state
            .http
            .post(format!("{API}/mods/files"))
            .header("x-api-key", &key)
            .header("Accept", "application/json")
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let mods_dir = game_dir.join("mods");

        // One batch Modrinth lookup for every blocked file that has a sha1.
        let blocked_hashes: Vec<String> = bulk
            .data
            .iter()
            .filter(|f| f.download_url.is_none())
            .filter_map(|f| f.sha1())
            .collect();
        let resolved = if blocked_hashes.is_empty() {
            std::collections::HashMap::new()
        } else {
            state
                .crosssource
                .resolve_hashes_modrinth(&state.http, &blocked_hashes)
                .await
                .unwrap_or_default()
        };

        for f in bulk.data {
            let sha1 = f.sha1();
            let url = match f.download_url.clone() {
                Some(url) => url,
                None => match sha1.as_ref().and_then(|h| resolved.get(h)) {
                    Some(mr) => {
                        fallbacks.push((f.file_name.clone(), mr.clone()));
                        mr.url.clone()
                    }
                    None => {
                        blocked.push(f);
                        continue;
                    }
                },
            };
            index_entries.push((f.file_name.clone(), f.mod_id.to_string()));
            items.push(DownloadItem::new(url, mods_dir.join(&f.file_name), sha1));
        }
    }

    // Tell the UI what was re-sourced and what needs manual downloading. Sent
    // before the downloads start so the progress card can show the badge.
    let report = build_install_report(state, &instance.id, fallbacks.len() as u32, &blocked).await;
    let _ = app.emit("modpack://report", report);

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

    // Remember where every mod came from (and which ones were re-sourced from
    // Modrinth) so update checks and exports know each file's identity.
    crate::instances::record_installs(state, &instance.id, &index_entries, "curseforge");
    for (file_name, mr) in &fallbacks {
        crate::instances::record_modrinth_fallback(state, &instance.id, file_name, mr);
    }

    Ok(instance)
}

// ---------------------------------------------------------------------------
// Install report (blocked-download summary for the UI)
// ---------------------------------------------------------------------------

/// Summary of blocked-file handling for one modpack install, emitted on the
/// `modpack://report` event before downloads begin.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModpackInstallReport {
    pub instance_id: String,
    /// Files whose author blocked CF downloads but that were found (and
    /// hash-verified) on Modrinth.
    pub resolved_via_modrinth: u32,
    /// Files the user must download manually from CurseForge.
    pub blocked: Vec<BlockedFileInfo>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockedFileInfo {
    pub file_name: String,
    pub mod_name: String,
    pub project_id: u32,
    pub file_id: u32,
    /// CurseForge page to download the file from.
    pub page_url: String,
}

/// Fetch names + website links for the blocked files' projects (best-effort;
/// falls back to the `/projects/{id}` redirect URL).
async fn build_install_report(
    state: &AppState,
    instance_id: &str,
    resolved_via_modrinth: u32,
    blocked: &[CfFile],
) -> ModpackInstallReport {
    #[derive(Deserialize)]
    struct Resp {
        data: Vec<CfModSummary>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct CfModSummary {
        id: u32,
        #[serde(default)]
        name: String,
        #[serde(default)]
        links: CfSummaryLinks,
    }
    #[derive(Deserialize, Default)]
    #[serde(rename_all = "camelCase")]
    struct CfSummaryLinks {
        #[serde(default)]
        website_url: Option<String>,
    }

    let mut infos: std::collections::HashMap<u32, (String, Option<String>)> =
        std::collections::HashMap::new();
    if !blocked.is_empty() {
        if let Ok(key) = api_key(state) {
            let ids: Vec<u32> = blocked.iter().map(|f| f.mod_id).collect();
            let resp = state
                .http
                .post(format!("{API}/mods"))
                .header("x-api-key", &key)
                .header("Accept", "application/json")
                .json(&serde_json::json!({ "modIds": ids }))
                .send()
                .await;
            if let Ok(r) = resp {
                if let Ok(r) = r.error_for_status() {
                    if let Ok(body) = r.json::<Resp>().await {
                        for m in body.data {
                            infos.insert(m.id, (m.name, m.links.website_url));
                        }
                    }
                }
            }
        }
    }

    let blocked = blocked
        .iter()
        .map(|f| blocked_file_info(f, infos.get(&f.mod_id)))
        .collect();

    ModpackInstallReport {
        instance_id: instance_id.to_string(),
        resolved_via_modrinth,
        blocked,
    }
}

/// Build one report row from a blocked file plus optional (name, site URL)
/// project info. Without a site link the `/projects/{id}` redirect is used,
/// and the file name stands in for the mod name.
fn blocked_file_info(f: &CfFile, info: Option<&(String, Option<String>)>) -> BlockedFileInfo {
    let (name, site) = info
        .cloned()
        .unwrap_or((f.file_name.clone(), None));
    let page_url = match site.filter(|s| !s.is_empty()) {
        Some(site) => format!("{site}/files/{}", f.id),
        None => format!("https://www.curseforge.com/projects/{}", f.mod_id),
    };
    BlockedFileInfo {
        file_name: f.file_name.clone(),
        mod_name: name,
        project_id: f.mod_id,
        file_id: f.id,
        page_url,
    }
}

// ---------------------------------------------------------------------------
// Update candidates (for the unified cross-platform update checker)
// ---------------------------------------------------------------------------

/// The newest compatible CurseForge file for one installed local file.
#[derive(Debug, Clone)]
pub struct CfUpdateCandidate {
    pub project_id: u32,
    pub file_id: u32,
    pub file_name: String,
    /// ISO-8601 upload date, comparable against Modrinth's `date_published`.
    pub file_date: String,
    /// Download URL — the API's own, or the Modrinth mirror of the identical
    /// file when the author blocked CF downloads. Candidates that can't be
    /// downloaded either way are dropped before this struct is built.
    pub url: String,
    pub sha1: Option<String>,
}

/// `POST /v1/mods` response entry (only the update-relevant fields).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfModWithIndexes {
    id: u32,
    #[serde(default)]
    latest_files_indexes: Vec<CfFileIndex>,
}

/// One entry of `latestFilesIndexes`: the newest file per (game version,
/// loader) combination, newest first.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfFileIndex {
    #[serde(default)]
    game_version: String,
    file_id: u32,
    #[serde(default)]
    release_type: u8, // 1 = release, 2 = beta, 3 = alpha
    #[serde(default)]
    mod_loader: Option<u8>,
}

/// Pick the newest compatible file id from a mod's `latestFilesIndexes`.
/// Entries are newest-first; stable releases win over beta/alpha. A `None`
/// `mod_loader` entry (non-mod content) matches any loader.
fn pick_file_index(indexes: &[CfFileIndex], game_version: &str, loader: Option<u8>) -> Option<u32> {
    let matches = |ix: &&CfFileIndex| {
        ix.game_version == game_version
            && match (ix.mod_loader, loader) {
                (Some(have), Some(want)) => have == want,
                _ => true,
            }
    };
    indexes
        .iter()
        .filter(matches)
        .find(|ix| ix.release_type == 1)
        .or_else(|| indexes.iter().find(matches))
        .map(|ix| ix.file_id)
}

/// For each local file (keyed by sha1), find the newest compatible CurseForge
/// file. Uses the fingerprint API to identify files, one `POST /v1/mods` for
/// the per-version file indexes and one `POST /v1/mods/files` for the full
/// candidate files — three requests total regardless of mod count.
///
/// Best-effort: returns an empty map when no API key is configured or any
/// step fails (the caller then simply offers Modrinth-only updates).
pub async fn update_candidates(
    state: &AppState,
    files: &[(String, std::path::PathBuf)],
    loader: Option<&str>,
    game_version: &str,
) -> std::collections::HashMap<String, CfUpdateCandidate> {
    use std::collections::HashMap;

    let mut out = HashMap::new();
    let Ok(key) = api_key(state) else {
        return out;
    };
    let Ok(refs) = state
        .crosssource
        .resolve_local_files_to_cf(&state.http, &key, files)
        .await
    else {
        return out;
    };
    if refs.is_empty() {
        return out;
    }

    // Batch-fetch every identified mod's latest-file indexes.
    let mut mod_ids: Vec<u32> = refs.values().map(|r| r.project_id).collect();
    mod_ids.sort_unstable();
    mod_ids.dedup();
    #[derive(Deserialize)]
    struct ModsResp {
        data: Vec<CfModWithIndexes>,
    }
    let resp = state
        .http
        .post(format!("{API}/mods"))
        .header("x-api-key", &key)
        .header("Accept", "application/json")
        .json(&serde_json::json!({ "modIds": mod_ids }))
        .send()
        .await;
    let mods: HashMap<u32, CfModWithIndexes> = match resp {
        Ok(r) => match r.error_for_status() {
            Ok(r) => match r.json::<ModsResp>().await {
                Ok(body) => body.data.into_iter().map(|m| (m.id, m)).collect(),
                Err(_) => return out,
            },
            Err(_) => return out,
        },
        Err(_) => return out,
    };

    // sha1 of the installed file → candidate file id (skipping "already newest").
    let lt = loader_type(loader);
    let mut wanted: Vec<(String, u32)> = Vec::new();
    for (sha1, cf) in &refs {
        let Some(m) = mods.get(&cf.project_id) else {
            continue;
        };
        if let Some(fid) = pick_file_index(&m.latest_files_indexes, game_version, lt) {
            if fid != cf.file_id {
                wanted.push((sha1.clone(), fid));
            }
        }
    }
    if wanted.is_empty() {
        return out;
    }

    // Bulk-fetch the full candidate files (dates, hashes, URLs).
    let file_ids: Vec<u32> = wanted.iter().map(|(_, fid)| *fid).collect();
    let bulk = state
        .http
        .post(format!("{API}/mods/files"))
        .header("x-api-key", &key)
        .header("Accept", "application/json")
        .json(&serde_json::json!({ "fileIds": file_ids }))
        .send()
        .await;
    let candidates: HashMap<u32, CfFile> = match bulk {
        Ok(r) => match r.error_for_status() {
            Ok(r) => match r.json::<CfBulkFiles>().await {
                Ok(body) => body.data.into_iter().map(|f| (f.id, f)).collect(),
                Err(_) => return out,
            },
            Err(_) => return out,
        },
        Err(_) => return out,
    };

    for (sha1, fid) in wanted {
        let Some(f) = candidates.get(&fid) else {
            continue;
        };
        // Blocked candidate: only offer it if the identical file is mirrored
        // on Modrinth (never guess CDN links).
        let url = match &f.download_url {
            Some(u) => u.clone(),
            None => match plan_blocked_file(state, f).await {
                BlockedPlan::Modrinth(mr) => mr.url,
                BlockedPlan::Manual => continue,
            },
        };
        out.insert(
            sha1,
            CfUpdateCandidate {
                project_id: f.mod_id,
                file_id: f.id,
                file_name: f.file_name.clone(),
                file_date: f.file_date.clone(),
                url,
                sha1: f.sha1(),
            },
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file_from_json(json: serde_json::Value) -> CfFile {
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn sha1_is_extracted_and_lowercased() {
        let f = file_from_json(serde_json::json!({
            "id": 4711,
            "modId": 238222,
            "fileName": "jei.jar",
            "hashes": [
                { "value": "9AC59C11A72CB7E7EC29ED0425E9ADD32F60A2BE", "algo": 1 },
                { "value": "ffffffffffffffffffffffffffffffff", "algo": 2 }
            ]
        }));
        assert_eq!(
            f.sha1().as_deref(),
            Some("9ac59c11a72cb7e7ec29ed0425e9add32f60a2be")
        );
    }

    #[test]
    fn missing_sha1_yields_none() {
        // Only an MD5 hash: resolution must not be attempted (no sha1 → Manual).
        let f = file_from_json(serde_json::json!({
            "id": 1,
            "fileName": "x.jar",
            "hashes": [{ "value": "ffffffffffffffffffffffffffffffff", "algo": 2 }]
        }));
        assert_eq!(f.sha1(), None);
    }

    #[test]
    fn bulk_files_with_null_download_urls_deserialize() {
        let bulk: CfBulkFiles = serde_json::from_value(serde_json::json!({
            "data": [
                {
                    "id": 100, "modId": 5, "fileName": "open.jar",
                    "downloadUrl": "https://edge.forgecdn.net/files/0/100/open.jar",
                    "hashes": [{ "value": "a".repeat(40), "algo": 1 }]
                },
                {
                    "id": 200, "modId": 6, "fileName": "blocked.jar",
                    "downloadUrl": null,
                    "hashes": [{ "value": "b".repeat(40), "algo": 1 }]
                }
            ]
        }))
        .unwrap();
        assert_eq!(bulk.data.len(), 2);
        assert!(bulk.data[0].download_url.is_some());
        assert!(bulk.data[1].download_url.is_none());
        assert_eq!(bulk.data[1].mod_id, 6);
    }

    #[test]
    fn blocked_info_uses_site_link_when_known() {
        let f = file_from_json(serde_json::json!({
            "id": 4711, "modId": 238222, "fileName": "jei.jar"
        }));
        let info = (
            "Just Enough Items".to_string(),
            Some("https://www.curseforge.com/minecraft/mc-mods/jei".to_string()),
        );
        let row = blocked_file_info(&f, Some(&info));
        assert_eq!(row.mod_name, "Just Enough Items");
        assert_eq!(
            row.page_url,
            "https://www.curseforge.com/minecraft/mc-mods/jei/files/4711"
        );
    }

    #[test]
    fn blocked_info_falls_back_to_projects_redirect() {
        let f = file_from_json(serde_json::json!({
            "id": 4711, "modId": 238222, "fileName": "jei.jar"
        }));
        // No project info at all.
        let row = blocked_file_info(&f, None);
        assert_eq!(row.mod_name, "jei.jar");
        assert_eq!(row.page_url, "https://www.curseforge.com/projects/238222");
        // Project info present but with an empty site link.
        let info = ("JEI".to_string(), Some(String::new()));
        let row = blocked_file_info(&f, Some(&info));
        assert_eq!(row.mod_name, "JEI");
        assert_eq!(row.page_url, "https://www.curseforge.com/projects/238222");
    }

    #[tokio::test]
    async fn blocked_file_without_sha1_is_manual() {
        let root = std::env::temp_dir().join(format!("ezmapa_cf_test_{}", uuid::Uuid::new_v4()));
        let state = crate::state::AppState::new(root.clone());
        let f = file_from_json(serde_json::json!({
            "id": 1, "modId": 2, "fileName": "x.jar", "downloadUrl": null
        }));
        // No sha1 → Manual without ever touching the resolver/network.
        assert!(matches!(plan_blocked_file(&state, &f).await, BlockedPlan::Manual));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn modloader_ids_parse() {
        use crate::models::Loader;
        assert!(matches!(parse_modloader("forge-47.2.0"), (Loader::Forge, Some(v)) if v == "47.2.0"));
        assert!(matches!(parse_modloader("neoforge-21.1.90"), (Loader::Neoforge, Some(v)) if v == "21.1.90"));
        assert!(matches!(parse_modloader("fabric-0.16.0"), (Loader::Fabric, Some(v)) if v == "0.16.0"));
        assert!(matches!(parse_modloader("unknown"), (Loader::Vanilla, None)));
    }
}

#[cfg(test)]
mod update_tests {
    use super::*;

    fn ix(game_version: &str, file_id: u32, release_type: u8, mod_loader: Option<u8>) -> CfFileIndex {
        CfFileIndex {
            game_version: game_version.into(),
            file_id,
            release_type,
            mod_loader,
        }
    }

    #[test]
    fn stable_release_beats_newer_beta() {
        // Newest-first list: a beta on top, a release below — release wins.
        let indexes = vec![
            ix("1.21.1", 300, 2, Some(4)),
            ix("1.21.1", 200, 1, Some(4)),
            ix("1.20.1", 100, 1, Some(4)),
        ];
        assert_eq!(pick_file_index(&indexes, "1.21.1", Some(4)), Some(200));
    }

    #[test]
    fn falls_back_to_prerelease_when_no_stable_exists() {
        let indexes = vec![ix("1.21.1", 300, 2, Some(4))];
        assert_eq!(pick_file_index(&indexes, "1.21.1", Some(4)), Some(300));
    }

    #[test]
    fn filters_by_game_version_and_loader() {
        let indexes = vec![
            ix("1.21.1", 300, 1, Some(1)), // forge
            ix("1.21.1", 200, 1, Some(4)), // fabric
            ix("1.20.1", 100, 1, Some(4)),
        ];
        assert_eq!(pick_file_index(&indexes, "1.21.1", Some(4)), Some(200));
        assert_eq!(pick_file_index(&indexes, "1.21.1", Some(1)), Some(300));
        assert_eq!(pick_file_index(&indexes, "1.19.2", Some(4)), None);
    }

    #[test]
    fn loaderless_entries_match_any_loader() {
        // Resource packs / shaders have no modLoader on their indexes.
        let indexes = vec![ix("1.21.1", 400, 1, None)];
        assert_eq!(pick_file_index(&indexes, "1.21.1", Some(4)), Some(400));
        assert_eq!(pick_file_index(&indexes, "1.21.1", None), Some(400));
    }

    #[test]
    fn mods_response_shape_parses() {
        let m: CfModWithIndexes = serde_json::from_value(serde_json::json!({
            "id": 238222,
            "name": "JEI",
            "latestFilesIndexes": [
                { "gameVersion": "1.21.1", "fileId": 42, "filename": "jei.jar",
                  "releaseType": 1, "gameVersionTypeId": 77784, "modLoader": 6 }
            ]
        }))
        .unwrap();
        assert_eq!(m.id, 238222);
        assert_eq!(m.latest_files_indexes[0].file_id, 42);
        assert_eq!(m.latest_files_indexes[0].mod_loader, Some(6));
    }
}
