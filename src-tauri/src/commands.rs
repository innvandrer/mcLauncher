//! Tauri command surface — the bridge between the React UI and the backend.

use crate::error::{Error, Result};
use crate::instances::{ResourcePackEntry, ScreenshotEntry, ShaderEntry, WorldEntry};
use crate::models::*;
use crate::state::AppState;
use crate::{auth, curseforge, instances, java, launch, modloader, modrinth, mojang};
use serde::Serialize;
use std::path::Path;
use tauri::{AppHandle, State};

// ---------------------------------------------------------------------------
// Versions / loaders
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionList {
    latest_release: String,
    latest_snapshot: String,
    versions: Vec<mojang::VersionStub>,
}

#[tauri::command]
pub async fn list_minecraft_versions(state: State<'_, AppState>) -> Result<VersionList> {
    let manifest = mojang::fetch_manifest(state.inner()).await?;
    Ok(VersionList {
        latest_release: manifest.latest.release,
        latest_snapshot: manifest.latest.snapshot,
        versions: manifest.versions,
    })
}

#[tauri::command]
pub async fn list_fabric_versions(
    state: State<'_, AppState>,
    mc_version: String,
) -> Result<Vec<modloader::LoaderVersion>> {
    modloader::list_fabric(state.inner(), &mc_version).await
}

#[tauri::command]
pub async fn list_quilt_versions(
    state: State<'_, AppState>,
    mc_version: String,
) -> Result<Vec<modloader::LoaderVersion>> {
    modloader::list_quilt(state.inner(), &mc_version).await
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Settings> {
    Ok(instances::load_settings(state.inner()))
}

#[tauri::command]
pub async fn save_settings(state: State<'_, AppState>, settings: Settings) -> Result<()> {
    instances::save_settings(state.inner(), &settings)
}

// ---------------------------------------------------------------------------
// Accounts
// ---------------------------------------------------------------------------

fn public_accounts(store: &AccountStore) -> Vec<PublicAccount> {
    store
        .accounts
        .iter()
        .map(|a| a.to_public(store.active.as_deref() == Some(a.id.as_str())))
        .collect()
}

#[tauri::command]
pub async fn list_accounts(state: State<'_, AppState>) -> Result<Vec<PublicAccount>> {
    Ok(public_accounts(&instances::load_accounts(state.inner())))
}

#[tauri::command]
pub async fn login_microsoft(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<PublicAccount>> {
    let account = auth::login_redirect(&app, state.inner()).await?;
    let store = instances::upsert_account(state.inner(), account)?;
    Ok(public_accounts(&store))
}

#[tauri::command]
pub async fn add_offline_account(
    state: State<'_, AppState>,
    username: String,
) -> Result<Vec<PublicAccount>> {
    let username = username.trim().to_string();
    if username.is_empty() {
        return Err(Error::Other("Username cannot be empty.".into()));
    }
    let account = Account {
        id: auth::offline_uuid(&username),
        username,
        access_token: String::new(),
        refresh_token: None,
        expires_at: 0,
        xuid: None,
        kind: "offline".into(),
    };
    let store = instances::upsert_account(state.inner(), account)?;
    Ok(public_accounts(&store))
}

#[tauri::command]
pub async fn set_active_account(
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<PublicAccount>> {
    let state = state.inner();
    let mut store = instances::load_accounts(state);
    if store.accounts.iter().any(|a| a.id == id) {
        store.active = Some(id);
        instances::save_accounts(state, &store)?;
    }
    Ok(public_accounts(&store))
}

#[tauri::command]
pub async fn remove_account(state: State<'_, AppState>, id: String) -> Result<Vec<PublicAccount>> {
    let state = state.inner();
    let mut store = instances::load_accounts(state);
    store.accounts.retain(|a| a.id != id);
    if store.active.as_deref() == Some(id.as_str()) {
        store.active = store.accounts.first().map(|a| a.id.clone());
    }
    instances::save_accounts(state, &store)?;
    Ok(public_accounts(&store))
}

// ---------------------------------------------------------------------------
// Instances
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_instances(state: State<'_, AppState>) -> Result<Vec<Instance>> {
    Ok(instances::list_instances(state.inner()))
}

#[tauri::command]
pub async fn get_instance(state: State<'_, AppState>, id: String) -> Result<Instance> {
    instances::get_instance(state.inner(), &id)
}

#[tauri::command]
pub async fn create_instance(
    state: State<'_, AppState>,
    name: String,
    mc_version: String,
    loader: Loader,
    loader_version: Option<String>,
    icon: Option<String>,
) -> Result<Instance> {
    instances::create_instance(state.inner(), &name, &mc_version, loader, loader_version, icon)
}

#[tauri::command]
pub async fn update_instance(state: State<'_, AppState>, instance: Instance) -> Result<Instance> {
    instances::save_instance(state.inner(), &instance)?;
    Ok(instance)
}

#[tauri::command]
pub async fn delete_instance(state: State<'_, AppState>, id: String) -> Result<()> {
    instances::delete_instance(state.inner(), &id)
}

#[tauri::command]
pub async fn duplicate_instance(state: State<'_, AppState>, id: String) -> Result<Instance> {
    instances::duplicate_instance(state.inner(), &id)
}

#[tauri::command]
pub async fn open_instance_folder(state: State<'_, AppState>, id: String) -> Result<()> {
    let dir = state.inner().dirs.game_dir(&id);
    std::fs::create_dir_all(&dir)?;
    open_path(&dir);
    Ok(())
}

fn open_path(path: &Path) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("explorer").arg(path).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(path).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    }
}

// ---------------------------------------------------------------------------
// Launch
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn launch_instance(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<()> {
    launch::launch(&app, state.inner(), &id).await
}

#[tauri::command]
pub async fn stop_instance(state: State<'_, AppState>, id: String) -> Result<()> {
    launch::stop(state.inner(), &id)
}

#[tauri::command]
pub async fn running_instances(state: State<'_, AppState>) -> Result<Vec<String>> {
    Ok(launch::running_ids(state.inner()))
}

// ---------------------------------------------------------------------------
// Modrinth + mods
// ---------------------------------------------------------------------------

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn search_modrinth(
    state: State<'_, AppState>,
    query: String,
    project_type: String,
    loader: Option<String>,
    game_version: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<modrinth::SearchResponse> {
    modrinth::search(
        state.inner(),
        &query,
        &project_type,
        loader.as_deref(),
        game_version.as_deref(),
        limit.unwrap_or(30),
        offset.unwrap_or(0),
    )
    .await
}

#[tauri::command]
pub async fn install_mod(
    state: State<'_, AppState>,
    instance_id: String,
    project_id: String,
    loader: Option<String>,
    game_version: Option<String>,
) -> Result<String> {
    modrinth::install_mod(
        state.inner(),
        &instance_id,
        &project_id,
        loader.as_deref(),
        game_version.as_deref(),
    )
    .await
}

#[tauri::command]
pub async fn list_mods(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<Vec<instances::ModEntry>> {
    Ok(instances::list_mods(state.inner(), &instance_id))
}

#[tauri::command]
pub async fn set_mod_enabled(
    state: State<'_, AppState>,
    instance_id: String,
    file_name: String,
    enabled: bool,
) -> Result<()> {
    instances::set_mod_enabled(state.inner(), &instance_id, &file_name, enabled)
}

#[tauri::command]
pub async fn delete_mod(
    state: State<'_, AppState>,
    instance_id: String,
    file_name: String,
) -> Result<()> {
    instances::delete_mod(state.inner(), &instance_id, &file_name)
}

// ---------------------------------------------------------------------------
// CurseForge
// ---------------------------------------------------------------------------

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn search_curseforge(
    state: State<'_, AppState>,
    query: String,
    content_type: String,
    loader: Option<String>,
    game_version: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<modrinth::SearchResponse> {
    curseforge::search(
        state.inner(),
        &query,
        &content_type,
        loader.as_deref(),
        game_version.as_deref(),
        limit.unwrap_or(30),
        offset.unwrap_or(0),
    )
    .await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn install_curseforge_content(
    state: State<'_, AppState>,
    instance_id: String,
    project_id: String,
    content_type: String,
    loader: Option<String>,
    game_version: Option<String>,
) -> Result<String> {
    let file = curseforge::install_content(
        state.inner(),
        &instance_id,
        &project_id,
        &content_type,
        loader.as_deref(),
        game_version.as_deref(),
    )
    .await?;
    instances::record_install(state.inner(), &instance_id, &file, &project_id, "curseforge");
    Ok(file)
}

/// Install a CurseForge modpack as a new instance.
#[tauri::command]
pub async fn install_curseforge_modpack(
    app: AppHandle,
    state: State<'_, AppState>,
    project_id: String,
    file_id: Option<String>,
    name: Option<String>,
    icon: Option<String>,
) -> Result<Instance> {
    curseforge::install_modpack(
        &app,
        state.inner(),
        &project_id,
        file_id.as_deref(),
        name.as_deref(),
        icon,
    )
    .await
}

// ---------------------------------------------------------------------------
// Modrinth — unified content installer + mrpack
// ---------------------------------------------------------------------------

/// Install any Modrinth project type (mod / resourcepack / shader) into the
/// correct sub-directory of the instance.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn install_content(
    state: State<'_, AppState>,
    instance_id: String,
    project_id: String,
    content_type: String,
    loader: Option<String>,
    game_version: Option<String>,
) -> Result<String> {
    let file = modrinth::install_content(
        state.inner(),
        &instance_id,
        &project_id,
        &content_type,
        loader.as_deref(),
        game_version.as_deref(),
    )
    .await?;
    instances::record_install(state.inner(), &instance_id, &file, &project_id, "modrinth");
    Ok(file)
}

/// Download and install a Modrinth modpack (.mrpack) into an existing instance.
#[tauri::command]
pub async fn install_mrpack(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
    project_id: String,
    version_id: Option<String>,
) -> Result<String> {
    modrinth::install_mrpack(
        &app,
        state.inner(),
        &instance_id,
        &project_id,
        version_id.as_deref(),
    )
    .await
}

/// List all versions of a Modrinth project (for the version picker).
#[tauri::command]
pub async fn list_modrinth_versions(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<modrinth::ContentVersion>> {
    modrinth::list_versions(state.inner(), &project_id).await
}

/// List all files of a CurseForge project (for the version picker).
#[tauri::command]
pub async fn list_curseforge_files(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<modrinth::ContentVersion>> {
    curseforge::list_files(state.inner(), &project_id).await
}

/// Install a Modrinth modpack as a new instance (derives loader + version).
#[tauri::command]
pub async fn install_modrinth_modpack(
    app: AppHandle,
    state: State<'_, AppState>,
    project_id: String,
    version_id: Option<String>,
    name: Option<String>,
    icon: Option<String>,
) -> Result<Instance> {
    modrinth::install_modpack(
        &app,
        state.inner(),
        &project_id,
        version_id.as_deref(),
        name.as_deref(),
        icon,
    )
    .await
}

// ---------------------------------------------------------------------------
// Resource packs
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_resource_packs(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<Vec<ResourcePackEntry>> {
    Ok(instances::list_resource_packs(state.inner(), &instance_id))
}

#[tauri::command]
pub async fn delete_resource_pack(
    state: State<'_, AppState>,
    instance_id: String,
    file_name: String,
) -> Result<()> {
    instances::delete_resource_pack(state.inner(), &instance_id, &file_name)
}

// ---------------------------------------------------------------------------
// Shaders
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_shaders(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<Vec<ShaderEntry>> {
    Ok(instances::list_shaders(state.inner(), &instance_id))
}

#[tauri::command]
pub async fn delete_shader(
    state: State<'_, AppState>,
    instance_id: String,
    file_name: String,
) -> Result<()> {
    instances::delete_shader(state.inner(), &instance_id, &file_name)
}

// ---------------------------------------------------------------------------
// Worlds
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_worlds(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<Vec<WorldEntry>> {
    Ok(instances::list_worlds(state.inner(), &instance_id))
}

#[tauri::command]
pub async fn delete_world(
    state: State<'_, AppState>,
    instance_id: String,
    name: String,
) -> Result<()> {
    instances::delete_world(state.inner(), &instance_id, &name)
}

#[tauri::command]
pub async fn open_world_folder(state: State<'_, AppState>, instance_id: String, name: String) -> Result<()> {
    let path = state.inner().dirs.game_dir(&instance_id).join("saves").join(&name);
    std::fs::create_dir_all(&path)?;
    open_path(&path);
    Ok(())
}

// ---------------------------------------------------------------------------
// Screenshots
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_screenshots(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<Vec<ScreenshotEntry>> {
    Ok(instances::list_screenshots(state.inner(), &instance_id))
}

#[tauri::command]
pub async fn open_screenshot(state: State<'_, AppState>, instance_id: String, file_name: String) -> Result<()> {
    let path = state.inner().dirs.game_dir(&instance_id).join("screenshots").join(&file_name);
    open_path(&path);
    Ok(())
}

// ---------------------------------------------------------------------------
// Mod updates
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn check_mod_updates(
    state: State<'_, AppState>,
    instance_id: String,
    loader: Option<String>,
    game_version: Option<String>,
) -> Result<Vec<modrinth::ModUpdate>> {
    modrinth::check_updates(
        state.inner(),
        &instance_id,
        loader.as_deref(),
        game_version.as_deref(),
    )
    .await
}

#[tauri::command]
pub async fn apply_mod_update(
    state: State<'_, AppState>,
    instance_id: String,
    update: modrinth::ModUpdate,
) -> Result<()> {
    modrinth::apply_update(state.inner(), &instance_id, update).await
}

// ---------------------------------------------------------------------------
// Export / import
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn export_instance(
    state: State<'_, AppState>,
    id: String,
    dest: String,
) -> Result<()> {
    instances::export_instance(state.inner(), &id, Path::new(&dest))
}

#[tauri::command]
pub async fn import_instance(state: State<'_, AppState>, src: String) -> Result<Instance> {
    instances::import_instance(state.inner(), Path::new(&src))
}

// ---------------------------------------------------------------------------
// Project info (for description pages)
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_modrinth_project_body(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<String> {
    modrinth::get_project_body(state.inner(), &project_id).await
}

#[tauri::command]
pub async fn get_curseforge_description(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<String> {
    curseforge::get_description(state.inner(), &project_id).await
}

// ---------------------------------------------------------------------------
// Version-specific installs (for the version picker)
// ---------------------------------------------------------------------------

/// Install a specific Modrinth version (from the version picker).
#[tauri::command]
pub async fn install_content_version(
    state: State<'_, AppState>,
    instance_id: String,
    project_id: String,
    version_id: String,
    content_type: String,
) -> Result<String> {
    let file = modrinth::install_version(
        state.inner(),
        &instance_id,
        &version_id,
        &content_type,
    )
    .await?;
    instances::record_install(state.inner(), &instance_id, &file, &project_id, "modrinth");
    Ok(file)
}

/// Install a specific CurseForge file (from the version picker).
#[tauri::command]
pub async fn install_curseforge_file(
    state: State<'_, AppState>,
    instance_id: String,
    project_id: String,
    file_id: String,
    content_type: String,
) -> Result<String> {
    let file = curseforge::install_file(
        state.inner(),
        &instance_id,
        &project_id,
        &file_id,
        &content_type,
    )
    .await?;
    instances::record_install(state.inner(), &instance_id, &file, &project_id, "curseforge");
    Ok(file)
}

// ---------------------------------------------------------------------------
// Open URL in system browser
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn open_url(url: String) -> Result<()> {
    #[cfg(windows)]
    { let _ = std::process::Command::new("cmd").args(["/C", "start", "", &url]).spawn(); }
    #[cfg(target_os = "macos")]
    { let _ = std::process::Command::new("open").arg(&url).spawn(); }
    #[cfg(all(unix, not(target_os = "macos")))]
    { let _ = std::process::Command::new("xdg-open").arg(&url).spawn(); }
    Ok(())
}

// ---------------------------------------------------------------------------
// Java
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn detect_java(state: State<'_, AppState>) -> Result<Vec<java::JavaInstall>> {
    Ok(java::detect(state.inner()))
}
