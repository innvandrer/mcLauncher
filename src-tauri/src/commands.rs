//! Tauri command surface — the bridge between the React UI and the backend.

use crate::error::{Error, Result};
use crate::instances::{ResourcePackEntry, ScreenshotEntry, ShaderEntry, WorldEntry};
use crate::models::*;
use crate::state::AppState;
use crate::{
    auth, curseforge, forge, instances, java, launch, modloader, modrinth, mojang, servers, skin,
    tools,
};
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

#[tauri::command]
pub async fn list_forge_versions(
    state: State<'_, AppState>,
    mc_version: String,
) -> Result<Vec<forge::ForgeVersion>> {
    forge::list_forge(state.inner(), &mc_version).await
}

#[tauri::command]
pub async fn list_neoforge_versions(
    state: State<'_, AppState>,
    mc_version: String,
) -> Result<Vec<forge::ForgeVersion>> {
    forge::list_neoforge(state.inner(), &mc_version).await
}

// ---------------------------------------------------------------------------
// Skin
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_skin(state: State<'_, AppState>) -> Result<skin::SkinInfo> {
    skin::get_skin(state.inner()).await
}

#[tauri::command]
pub async fn set_skin_url(
    state: State<'_, AppState>,
    url: String,
    variant: String,
) -> Result<()> {
    skin::set_skin_url(state.inner(), &url, &variant).await
}

#[tauri::command]
pub async fn set_skin_file(
    state: State<'_, AppState>,
    file_path: String,
    variant: String,
) -> Result<()> {
    skin::set_skin_file(state.inner(), &file_path, &variant).await
}

// ---------------------------------------------------------------------------
// System
// ---------------------------------------------------------------------------

/// Total physical RAM in megabytes (0 if unknown). Used to suggest a heap size.
#[tauri::command]
pub fn system_memory_mb() -> u64 {
    crate::system::total_memory_mb()
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
    quick_world: Option<String>,
    quick_server: Option<String>,
) -> Result<()> {
    launch::launch(
        &app,
        state.inner(),
        &id,
        quick_world.as_deref(),
        quick_server.as_deref(),
    )
    .await
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
    index: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<modrinth::SearchResponse> {
    modrinth::search(
        state.inner(),
        &query,
        &project_type,
        loader.as_deref(),
        game_version.as_deref(),
        index.as_deref().unwrap_or("relevance"),
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
) -> Result<InstallOutcome> {
    let (file, deps) = modrinth::install_mod(
        state.inner(),
        &instance_id,
        &project_id,
        loader.as_deref(),
        game_version.as_deref(),
    )
    .await?;
    for dep in &deps {
        instances::record_install(state.inner(), &instance_id, &dep.file_name, &dep.project_id, "modrinth");
    }
    Ok(InstallOutcome {
        file,
        dependencies: deps.into_iter().map(|d| d.file_name).collect(),
    })
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
) -> Result<InstallOutcome> {
    let (file, deps) = curseforge::install_content(
        state.inner(),
        &instance_id,
        &project_id,
        &content_type,
        loader.as_deref(),
        game_version.as_deref(),
    )
    .await?;
    instances::record_install(state.inner(), &instance_id, &file, &project_id, "curseforge");
    for dep in &deps {
        instances::record_install(state.inner(), &instance_id, &dep.file_name, &dep.project_id, "curseforge");
    }
    Ok(InstallOutcome {
        file,
        dependencies: deps.into_iter().map(|d| d.file_name).collect(),
    })
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
) -> Result<InstallOutcome> {
    let (file, deps) = modrinth::install_content(
        state.inner(),
        &instance_id,
        &project_id,
        &content_type,
        loader.as_deref(),
        game_version.as_deref(),
    )
    .await?;
    instances::record_install(state.inner(), &instance_id, &file, &project_id, "modrinth");
    for dep in &deps {
        instances::record_install(state.inner(), &instance_id, &dep.file_name, &dep.project_id, "modrinth");
    }
    Ok(InstallOutcome {
        file,
        dependencies: deps.into_iter().map(|d| d.file_name).collect(),
    })
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

/// Export an instance as a Modrinth modpack (`.mrpack`).
#[tauri::command]
pub async fn export_mrpack(state: State<'_, AppState>, id: String, dest: String) -> Result<()> {
    modrinth::export_mrpack(state.inner(), &id, Path::new(&dest)).await
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

/// The result of installing content: the primary file plus any required
/// dependencies that were auto-installed alongside it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallOutcome {
    pub file: String,
    pub dependencies: Vec<String>,
}

/// Install a specific Modrinth version (from the version picker).
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn install_content_version(
    state: State<'_, AppState>,
    instance_id: String,
    project_id: String,
    version_id: String,
    content_type: String,
    loader: Option<String>,
    game_version: Option<String>,
) -> Result<InstallOutcome> {
    let (file, deps) = modrinth::install_version(
        state.inner(),
        &instance_id,
        &version_id,
        &content_type,
        loader.as_deref(),
        game_version.as_deref(),
    )
    .await?;
    instances::record_install(state.inner(), &instance_id, &file, &project_id, "modrinth");
    for dep in &deps {
        instances::record_install(state.inner(), &instance_id, &dep.file_name, &dep.project_id, "modrinth");
    }
    Ok(InstallOutcome {
        file,
        dependencies: deps.into_iter().map(|d| d.file_name).collect(),
    })
}

/// Install a specific CurseForge file (from the version picker).
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn install_curseforge_file(
    state: State<'_, AppState>,
    instance_id: String,
    project_id: String,
    file_id: String,
    content_type: String,
    loader: Option<String>,
    game_version: Option<String>,
) -> Result<InstallOutcome> {
    let (file, deps) = curseforge::install_file(
        state.inner(),
        &instance_id,
        &project_id,
        &file_id,
        &content_type,
        loader.as_deref(),
        game_version.as_deref(),
    )
    .await?;
    instances::record_install(state.inner(), &instance_id, &file, &project_id, "curseforge");
    for dep in &deps {
        instances::record_install(state.inner(), &instance_id, &dep.file_name, &dep.project_id, "curseforge");
    }
    Ok(InstallOutcome {
        file,
        dependencies: deps.into_iter().map(|d| d.file_name).collect(),
    })
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

/// Upload game-log text to mclo.gs and return the shareable URL (for sharing
/// crash logs without copy-pasting walls of text).
#[tauri::command]
pub async fn upload_log(state: State<'_, AppState>, content: String) -> Result<String> {
    #[derive(serde::Deserialize)]
    struct Resp {
        success: bool,
        #[serde(default)]
        url: Option<String>,
        #[serde(default)]
        error: Option<String>,
    }
    let resp: Resp = state
        .inner()
        .http
        .post("https://api.mclo.gs/1/log")
        .form(&[("content", content.as_str())])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    if resp.success {
        resp.url.ok_or_else(|| Error::Other("mclo.gs returned no URL".into()))
    } else {
        Err(Error::Other(resp.error.unwrap_or_else(|| "log upload failed".into())))
    }
}

// ---------------------------------------------------------------------------
// Tools — disk usage, world snapshots, mod conflicts
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn instance_disk_usage(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<tools::DiskUsage> {
    Ok(tools::instance_disk_usage(state.inner(), &instance_id))
}

#[tauri::command]
pub async fn list_snapshots(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<Vec<tools::Snapshot>> {
    Ok(tools::list_snapshots(state.inner(), &instance_id))
}

#[tauri::command]
pub async fn create_snapshot(
    state: State<'_, AppState>,
    instance_id: String,
    world: String,
) -> Result<tools::Snapshot> {
    tools::create_snapshot(state.inner(), &instance_id, &world)
}

#[tauri::command]
pub async fn restore_snapshot(
    state: State<'_, AppState>,
    instance_id: String,
    file_name: String,
) -> Result<()> {
    tools::restore_snapshot(state.inner(), &instance_id, &file_name)
}

#[tauri::command]
pub async fn delete_snapshot(
    state: State<'_, AppState>,
    instance_id: String,
    file_name: String,
) -> Result<()> {
    tools::delete_snapshot(state.inner(), &instance_id, &file_name)
}

#[tauri::command]
pub async fn scan_mod_conflicts(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<Vec<tools::ModConflict>> {
    Ok(tools::scan_mod_conflicts(state.inner(), &instance_id))
}

/// Remove duplicate mods, keeping the newest version of each. Returns the
/// removed file names.
#[tauri::command]
pub async fn resolve_mod_conflicts(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<Vec<String>> {
    Ok(tools::resolve_mod_conflicts(state.inner(), &instance_id))
}

/// Signal an in-flight download task (e.g. "modpack:<id>") to stop.
#[tauri::command]
pub async fn cancel_task(state: State<'_, AppState>, task_id: String) -> Result<()> {
    state.cancel_task(&task_id);
    Ok(())
}

/// Check whether a newer version of this instance's modpack exists, with a diff.
#[tauri::command]
pub async fn check_modpack_update(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<Option<modrinth::ModpackUpdate>> {
    modrinth::check_modpack_update(state.inner(), &instance_id).await
}

/// Apply a modpack update to the given target version.
#[tauri::command]
pub async fn apply_modpack_update(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
    version_id: String,
) -> Result<()> {
    modrinth::apply_modpack_update(&app, state.inner(), &instance_id, &version_id).await
}

// ---------------------------------------------------------------------------
// Java
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn detect_java(state: State<'_, AppState>) -> Result<Vec<java::JavaInstall>> {
    Ok(java::detect(state.inner()))
}

// ---------------------------------------------------------------------------
// Servers — saved multiplayer list + live Server List Ping
// ---------------------------------------------------------------------------

/// Read the saved servers (`servers.dat`) for an instance. Empty until the user
/// has added servers in-game.
#[tauri::command]
pub async fn list_servers(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<Vec<servers::SavedServer>> {
    Ok(servers::list_servers(state.inner(), &instance_id))
}

/// Live-ping a `host` / `host:port` address for MOTD, player count and latency.
#[tauri::command]
pub async fn ping_server(address: String) -> Result<servers::ServerStatus> {
    Ok(servers::ping_server(&address).await)
}

// ---------------------------------------------------------------------------
// Play sessions (activity stats)
// ---------------------------------------------------------------------------

/// The recorded play sessions across all instances (for the activity chart).
#[tauri::command]
pub async fn list_sessions(state: State<'_, AppState>) -> Result<Vec<instances::Session>> {
    Ok(instances::list_sessions(state.inner()))
}

// ---------------------------------------------------------------------------
// Skin wardrobe
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_saved_skins(state: State<'_, AppState>) -> Result<Vec<skin::SavedSkin>> {
    Ok(skin::list_saved_skins(state.inner()))
}

#[tauri::command]
pub async fn save_skin(
    state: State<'_, AppState>,
    name: String,
    url: String,
    variant: String,
) -> Result<Vec<skin::SavedSkin>> {
    skin::save_skin(state.inner(), &name, &url, &variant)
}

/// Import a local PNG into the skin wardrobe (apply later without a URL).
#[tauri::command]
pub async fn save_skin_file(
    state: State<'_, AppState>,
    name: String,
    file_path: String,
    variant: String,
) -> Result<Vec<skin::SavedSkin>> {
    skin::save_skin_file(state.inner(), &name, &file_path, &variant)
}

#[tauri::command]
pub async fn delete_saved_skin(
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<skin::SavedSkin>> {
    skin::delete_saved_skin(state.inner(), &id)
}

/// Look up any player's current skin by username / UUID / NameMC link.
#[tauri::command]
pub async fn fetch_player_skin(
    state: State<'_, AppState>,
    query: String,
) -> Result<skin::PlayerSkin> {
    skin::fetch_player_skin(state.inner(), &query).await
}
