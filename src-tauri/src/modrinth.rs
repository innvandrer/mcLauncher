//! Modrinth integration: search projects and install mod/modpack files.

use crate::error::{Error, Result};
use crate::net::{self, DownloadItem};
use crate::state::AppState;
use serde::{Deserialize, Serialize};

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

/// Search Modrinth, optionally constrained by loader and game version.
pub async fn search(
    state: &AppState,
    query: &str,
    project_type: &str,
    loader: Option<&str>,
    game_version: Option<&str>,
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
        ("index", "relevance"),
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
/// and return the installed filename.
pub async fn install_mod(
    state: &AppState,
    instance_id: &str,
    project_id: &str,
    loader: Option<&str>,
    game_version: Option<&str>,
) -> Result<String> {
    let versions = project_versions(state, project_id, loader, game_version).await?;
    let version = versions
        .into_iter()
        .next()
        .ok_or_else(|| Error::NotFound(format!("compatible version for {project_id}")))?;

    let file = version
        .files
        .iter()
        .find(|f| f.primary)
        .or_else(|| version.files.first())
        .cloned()
        .ok_or_else(|| Error::NotFound("download file".into()))?;

    let mods_dir = state.dirs.game_dir(instance_id).join("mods");
    let target = mods_dir.join(&file.filename);
    net::download_one(
        &state.http,
        &DownloadItem::new(file.url.clone(), target, file.hashes.sha1.clone()),
    )
    .await?;

    Ok(file.filename)
}
