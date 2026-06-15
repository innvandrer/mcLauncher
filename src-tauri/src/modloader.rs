//! Fabric, Quilt, Forge and NeoForge loader support.

use crate::error::{Error, Result};
use crate::models::Loader;
use crate::mojang;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

const FABRIC_META: &str = "https://meta.fabricmc.net/v2";
const QUILT_META: &str = "https://meta.quiltmc.org/v3";

/// A loader version offered to the user when creating an instance.
#[derive(Debug, Clone, Serialize)]
pub struct LoaderVersion {
    pub version: String,
    pub stable: bool,
}

#[derive(Debug, Deserialize)]
struct FabricEntry {
    loader: FabricInner,
}

#[derive(Debug, Deserialize)]
struct FabricInner {
    version: String,
    #[serde(default)]
    stable: bool,
}

#[derive(Debug, Deserialize)]
struct IdOnly {
    id: String,
}

/// List available Fabric loader versions for a Minecraft version.
pub async fn list_fabric(state: &AppState, game_version: &str) -> Result<Vec<LoaderVersion>> {
    let url = format!("{FABRIC_META}/versions/loader/{game_version}");
    let entries: Vec<FabricEntry> = crate::net::get_json(&state.http, &url).await?;
    Ok(entries
        .into_iter()
        .map(|e| LoaderVersion {
            version: e.loader.version,
            stable: e.loader.stable,
        })
        .collect())
}

/// List available Quilt loader versions for a Minecraft version.
pub async fn list_quilt(state: &AppState, game_version: &str) -> Result<Vec<LoaderVersion>> {
    let url = format!("{QUILT_META}/versions/loader/{game_version}");
    let entries: Vec<FabricEntry> = crate::net::get_json(&state.http, &url).await?;
    Ok(entries
        .into_iter()
        .map(|e| LoaderVersion {
            // Quilt pre-releases contain "beta"/"pre"; mark them unstable.
            stable: !e.loader.version.contains('-'),
            version: e.loader.version,
        })
        .collect())
}

/// Ensure the Fabric profile JSON is cached and return its version id.
pub async fn install_fabric(
    state: &AppState,
    game_version: &str,
    loader_version: &str,
) -> Result<String> {
    let url = format!(
        "{FABRIC_META}/versions/loader/{game_version}/{loader_version}/profile/json"
    );
    fetch_and_cache_profile(state, &url).await
}

/// Ensure the Quilt profile JSON is cached and return its version id.
pub async fn install_quilt(
    state: &AppState,
    game_version: &str,
    loader_version: &str,
) -> Result<String> {
    let url = format!(
        "{QUILT_META}/versions/loader/{game_version}/{loader_version}/profile/json"
    );
    fetch_and_cache_profile(state, &url).await
}

async fn fetch_and_cache_profile(state: &AppState, url: &str) -> Result<String> {
    let raw = state
        .http
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    let id: IdOnly = serde_json::from_slice(&raw)?;
    mojang::save_version_json(state, &id.id, &raw).await?;
    Ok(id.id)
}

/// Resolve the concrete version id to launch for an instance, installing the
/// loader profile if necessary.
pub async fn launch_version_id(
    app: &AppHandle,
    state: &AppState,
    mc_version: &str,
    loader: Loader,
    loader_version: Option<&str>,
) -> Result<String> {
    match loader {
        Loader::Vanilla => Ok(mc_version.to_string()),
        Loader::Fabric => {
            let lv = loader_version
                .ok_or_else(|| Error::Other("Fabric loader version missing".into()))?;
            install_fabric(state, mc_version, lv).await
        }
        Loader::Quilt => {
            let lv = loader_version
                .ok_or_else(|| Error::Other("Quilt loader version missing".into()))?;
            install_quilt(state, mc_version, lv).await
        }
        Loader::Forge => {
            let lv = loader_version
                .ok_or_else(|| Error::Other("Forge loader version missing".into()))?;
            crate::forge::install_forge(app, state, mc_version, lv).await
        }
        Loader::Neoforge => {
            let lv = loader_version
                .ok_or_else(|| Error::Other("NeoForge loader version missing".into()))?;
            crate::forge::install_neoforge(app, state, mc_version, lv).await
        }
    }
}
