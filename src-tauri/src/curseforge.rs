//! CurseForge integration: search projects and install mod/resourcepack/shader
//! files. Results are mapped onto the same [`ModHit`]/[`SearchResponse`] shapes
//! the Modrinth module uses, so the frontend can render either provider with the
//! same components.

use crate::error::{Error, Result};
use crate::modrinth::{ContentVersion, ModHit, SearchResponse};
use crate::net::{self, DownloadItem};
use crate::state::AppState;
use serde::Deserialize;
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
fn loader_type(loader: Option<&str>) -> Option<u8> {
    match loader {
        Some("forge") => Some(1),
        Some("fabric") => Some(4),
        Some("quilt") => Some(5),
        Some("neoforge") => Some(6),
        _ => None,
    }
}

/// Resolve the API key from settings, falling back to the `BEACON_CF_API_KEY`
/// env var. Returns a friendly error when neither is set.
fn api_key(state: &AppState) -> Result<String> {
    if let Some(k) = crate::instances::load_settings(state)
        .curseforge_api_key
        .filter(|k| !k.trim().is_empty())
    {
        return Ok(k);
    }
    if let Ok(k) = std::env::var("BEACON_CF_API_KEY") {
        if !k.trim().is_empty() {
            return Ok(k);
        }
    }
    Err(Error::Other(
        "No CurseForge API key set. Add one in Settings or set BEACON_CF_API_KEY.".into(),
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

/// Build the public forgecdn URL for a file whose `downloadUrl` is null.
fn fallback_url(file_id: u32, file_name: &str) -> String {
    let s = file_id.to_string();
    let split = s.len().saturating_sub(3);
    let prefix = &s[..split];
    let suffix: u32 = s[split..].parse().unwrap_or(0);
    let encoded = file_name.replace(' ', "%20");
    format!("https://edge.forgecdn.net/files/{prefix}/{suffix}/{encoded}")
}

/// Download the newest compatible file of a CurseForge project into the correct
/// sub-directory of the instance and return the installed filename.
pub async fn install_content(
    state: &AppState,
    instance_id: &str,
    project_id: &str,
    content_type: &str,
    loader: Option<&str>,
    game_version: Option<&str>,
) -> Result<String> {
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

    let url = file.download_url.clone().unwrap_or_else(|| fallback_url(file.id, &file.file_name));
    let sha1 = file
        .hashes
        .iter()
        .find(|h| h.algo == 1)
        .map(|h| h.value.clone());

    let folder = match content_type {
        "resourcepack" => "resourcepacks",
        "shader" => "shaderpacks",
        _ => "mods",
    };
    let target = state.dirs.game_dir(instance_id).join(folder).join(&file.file_name);
    net::download_one(&state.http, &DownloadItem::new(url, target, sha1)).await?;

    Ok(file.file_name)
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

/// Install a specific CurseForge file by its file ID.
pub async fn install_file(
    state: &AppState,
    instance_id: &str,
    project_id: &str,
    file_id: &str,
    content_type: &str,
) -> Result<String> {
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

    let file = resp.data;
    let url = file.download_url.clone().unwrap_or_else(|| fallback_url(file.id, &file.file_name));
    let sha1 = file.hashes.iter().find(|h| h.algo == 1).map(|h| h.value.clone());

    let folder = match content_type {
        "resourcepack" => "resourcepacks",
        "shader" => "shaderpacks",
        _ => "mods",
    };
    let target = state.dirs.game_dir(instance_id).join(folder).join(&file.file_name);
    net::download_one(&state.http, &DownloadItem::new(url, target, sha1)).await?;

    Ok(file.file_name)
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

    let pack_url = pack_file
        .download_url
        .clone()
        .unwrap_or_else(|| fallback_url(pack_file.id, &pack_file.file_name));
    let tmp_path = std::env::temp_dir().join(format!("beacon_cf_{}.zip", pack_file.id));
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
    }
    std::fs::remove_file(&tmp_path).ok();

    // Bulk-resolve download URLs for the listed mod files.
    let file_ids: Vec<u32> = manifest.files.iter().map(|f| f.file_id).collect();
    let mut items: Vec<DownloadItem> = Vec::new();
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
        for f in bulk.data {
            let url = f
                .download_url
                .clone()
                .unwrap_or_else(|| fallback_url(f.id, &f.file_name));
            let sha1 = f.hashes.iter().find(|h| h.algo == 1).map(|h| h.value.clone());
            items.push(DownloadItem::new(url, mods_dir.join(&f.file_name), sha1));
        }
    }

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
