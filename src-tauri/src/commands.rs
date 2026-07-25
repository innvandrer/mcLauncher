//! Tauri command surface — the bridge between the React UI and the backend.

use crate::error::{Error, Result};
use crate::instances::{ResourcePackEntry, ScreenshotEntry, ShaderEntry, WorldEntry};
use crate::models::*;
use crate::state::AppState;
use crate::{
    auth, curseforge, developer, forge, instances, java, launch, modloader, modrinth, mojang,
    preflight, servers, sharing, skin, tools, turbo,
};
use serde::Serialize;
use std::path::Path;
use tauri::{AppHandle, State};

// ---------------------------------------------------------------------------
// Developer Hub
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn developer_hub_enabled() -> bool {
    developer::is_enabled()
}

#[tauri::command]
pub async fn discover_developer_projects() -> Result<Vec<developer::DeveloperProject>> {
    developer::require_enabled()?;
    Ok(tokio::task::spawn_blocking(developer::discover_projects).await?)
}

#[tauri::command]
pub async fn inspect_developer_project(path: String) -> Result<developer::DeveloperProject> {
    developer::require_enabled()?;
    tokio::task::spawn_blocking(move || developer::inspect_project(Path::new(&path))).await?
}

#[tauri::command]
pub async fn run_developer_task(
    path: String,
    task: String,
) -> Result<developer::DeveloperTaskResult> {
    developer::require_enabled()?;
    developer::run_task(Path::new(&path), &task).await
}

#[tauri::command]
pub async fn install_developer_artifact(
    state: State<'_, AppState>,
    path: String,
    instance_id: String,
) -> Result<developer::DeveloperInstallResult> {
    developer::require_enabled()?;
    developer::install_artifact(state.inner(), Path::new(&path), &instance_id)
}

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
        latest_release: manifest.latest.release.clone(),
        latest_snapshot: manifest.latest.snapshot.clone(),
        versions: manifest.versions.clone(),
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
    let store = instances::mutate_accounts(state.inner(), |store| {
        if store.accounts.iter().any(|a| a.id == id) {
            store.active = Some(id.clone());
        }
        store.clone()
    })?;
    Ok(public_accounts(&store))
}

#[tauri::command]
pub async fn remove_account(state: State<'_, AppState>, id: String) -> Result<Vec<PublicAccount>> {
    let store = instances::remove_account(state.inner(), &id)?;
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
    instances::duplicate_instance(state.inner(), &id).await
}

#[tauri::command]
pub async fn open_instance_folder(state: State<'_, AppState>, id: String) -> Result<()> {
    instances::require_safe_name("instance id", &id)?;
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

/// Create a Desktop shortcut that relaunches this instance straight into a
/// world or server (or just opens it, if neither is given). Returns the
/// created shortcut's path.
#[tauri::command]
pub async fn create_shortcut(
    instance_id: String,
    instance_name: String,
    world: Option<String>,
    server: Option<String>,
) -> Result<String> {
    crate::shortcuts::create_desktop_shortcut(
        &instance_id,
        &instance_name,
        world.as_deref(),
        server.as_deref(),
    )
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
    let mut deps_idx: Vec<(String, String)> = vec![(file.clone(), project_id.clone())];
    deps_idx.extend(deps
        .iter()
        .map(|d| (d.file_name.clone(), d.project_id.clone()))
    );
    instances::record_installs(state.inner(), &instance_id, &deps_idx, "modrinth");
    instances::record_install_dependencies(
        state.inner(),
        &instance_id,
        &project_id,
        &deps.iter().map(|d| d.file_name.clone()).collect::<Vec<_>>(),
    );
    Ok(InstallOutcome {
        file,
        dependencies: deps.into_iter().map(|d| d.file_name).collect(),
        via_modrinth_fallback: false,
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

/// Save the instance's current mod enable/disable state as a named loadout.
#[tauri::command]
pub async fn save_loadout(
    state: State<'_, AppState>,
    instance_id: String,
    name: String,
) -> Result<Instance> {
    instances::save_loadout(state.inner(), &instance_id, &name)
}

/// Toggle mods to match a named loadout; returns how many changed state.
#[tauri::command]
pub async fn apply_loadout(
    state: State<'_, AppState>,
    instance_id: String,
    name: String,
) -> Result<u32> {
    instances::apply_loadout(state.inner(), &instance_id, &name)
}

/// Remove a named loadout (mods on disk are untouched).
#[tauri::command]
pub async fn delete_loadout(
    state: State<'_, AppState>,
    instance_id: String,
    name: String,
) -> Result<Instance> {
    instances::delete_loadout(state.inner(), &instance_id, &name)
}

#[tauri::command]
pub async fn delete_mod(
    state: State<'_, AppState>,
    instance_id: String,
    file_name: String,
) -> Result<()> {
    instances::delete_mod(state.inner(), &instance_id, &file_name)
}

#[tauri::command]
pub async fn mod_removal_impact(
    state: State<'_, AppState>,
    instance_id: String,
    file_name: String,
) -> Result<instances::RemovalImpact> {
    instances::mod_removal_impact(state.inner(), &instance_id, &file_name)
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
    let installed = curseforge::install_content(
        state.inner(),
        &instance_id,
        &project_id,
        &content_type,
        loader.as_deref(),
        game_version.as_deref(),
    )
    .await?;
    record_cf_install(state.inner(), &instance_id, &project_id, &installed);
    Ok(InstallOutcome {
        via_modrinth_fallback: installed.modrinth_fallback.is_some(),
        file: installed.file_name,
        dependencies: installed.deps.into_iter().map(|d| d.file_name).collect(),
    })
}

/// Record a CurseForge install (primary file + dependencies) in the content
/// index, including which files were re-sourced from Modrinth because the
/// author blocked CF downloads.
fn record_cf_install(
    state: &AppState,
    instance_id: &str,
    project_id: &str,
    installed: &curseforge::CfInstall,
) {
    let mut idx = vec![(installed.file_name.clone(), project_id.to_string())];
    idx.extend(
        installed
            .deps
            .iter()
            .map(|d| (d.file_name.clone(), d.project_id.clone())),
    );
    instances::record_installs(state, instance_id, &idx, "curseforge");
    instances::record_install_dependencies(
        state,
        instance_id,
        project_id,
        &installed.deps.iter().map(|d| d.file_name.clone()).collect::<Vec<_>>(),
    );
    if let Some(mr) = &installed.modrinth_fallback {
        instances::record_modrinth_fallback(state, instance_id, &installed.file_name, mr);
    }
    for dep in &installed.deps {
        if let Some(mr) = &dep.modrinth_fallback {
            instances::record_modrinth_fallback(state, instance_id, &dep.file_name, mr);
        }
    }
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
    let mut idx = vec![(file.clone(), project_id.clone())];
    idx.extend(deps.iter().map(|d| (d.file_name.clone(), d.project_id.clone())));
    instances::record_installs(state.inner(), &instance_id, &idx, "modrinth");
    instances::record_install_dependencies(
        state.inner(),
        &instance_id,
        &project_id,
        &deps.iter().map(|d| d.file_name.clone()).collect::<Vec<_>>(),
    );
    Ok(InstallOutcome {
        file,
        dependencies: deps.into_iter().map(|d| d.file_name).collect(),
        via_modrinth_fallback: false,
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

/// Import a local .mrpack file as a new instance (derives loader + version
/// from the pack's index).
#[tauri::command]
pub async fn import_mrpack(
    app: AppHandle,
    state: State<'_, AppState>,
    src: String,
) -> Result<Instance> {
    modrinth::import_mrpack_file(&app, state.inner(), Path::new(&src)).await
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
) -> Result<tools::Snapshot> {
    let dirs = state.inner().dirs.clone();
    tokio::task::spawn_blocking(move || {
        tools::backup_and_delete_world(&dirs, &instance_id, &name)
    })
    .await?
}

#[tauri::command]
pub async fn open_world_folder(state: State<'_, AppState>, instance_id: String, name: String) -> Result<()> {
    instances::require_safe_name("instance id", &instance_id)?;
    instances::require_safe_name("world name", &name)?;
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
    instances::require_safe_name("instance id", &instance_id)?;
    instances::require_safe_name("screenshot file name", &file_name)?;
    let path = state.inner().dirs.game_dir(&instance_id).join("screenshots").join(&file_name);
    open_path(&path);
    Ok(())
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineReadiness {
    ready: bool,
    missing: Vec<String>,
}

#[tauri::command]
pub async fn offline_readiness(state: State<'_, AppState>, instance_id: String) -> Result<OfflineReadiness> {
    let instance = instances::get_instance(state.inner(), &instance_id)?;
    let dirs = &state.inner().dirs;
    let mut missing = Vec::new();
    if !dirs.version_json(&instance.mc_version).is_file() || !dirs.version_jar(&instance.mc_version).is_file() { missing.push("Minecraft version files".to_string()); }
    if std::fs::read_dir(dirs.assets().join("indexes")).map(|mut entries| entries.next().is_none()).unwrap_or(true) { missing.push("asset index".to_string()); }
    if std::fs::read_dir(dirs.libraries()).map(|mut entries| entries.next().is_none()).unwrap_or(true) { missing.push("game libraries".to_string()); }
    let java_ready = instance.java_path.as_ref().map(|path| std::path::Path::new(path).is_file()).unwrap_or(false)
        || std::fs::read_dir(dirs.java()).map(|mut entries| entries.next().is_some()).unwrap_or(false);
    if !java_ready { missing.push("Java runtime".to_string()); }
    Ok(OfflineReadiness { ready: missing.is_empty(), missing })
}

#[tauri::command]
pub async fn read_screenshot(state: State<'_, AppState>, instance_id: String, file_name: String) -> Result<Vec<u8>> {
    instances::require_safe_name("instance id", &instance_id)?;
    instances::require_safe_name("screenshot file name", &file_name)?;
    let path = state.inner().dirs.game_dir(&instance_id).join("screenshots").join(&file_name);
    Ok(std::fs::read(path)?)
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
    modrinth::apply_updates_transaction(state.inner(), &instance_id, vec![update])
        .await
        .map(|_| ())
}

#[tauri::command]
pub async fn apply_mod_updates(
    state: State<'_, AppState>,
    instance_id: String,
    updates: Vec<modrinth::ModUpdate>,
) -> Result<u32> {
    modrinth::apply_updates_transaction(state.inner(), &instance_id, updates).await
}

#[tauri::command]
pub async fn rollback_last_content_update(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<u32> {
    modrinth::rollback_last_update(state.inner(), &instance_id)
}

/// Re-pin an installed mod to the other platform ("switch source of truth"):
/// the file's bytes stay untouched, but its content-index identity — which the
/// update checker uses as the preferred source — moves to `provider`. Returns
/// the project id on the new provider.
#[tauri::command]
pub async fn set_mod_source(
    state: State<'_, AppState>,
    instance_id: String,
    file_name: String,
    provider: String,
) -> Result<String> {
    let state = state.inner();
    instances::require_safe_name("instance id", &instance_id)?;
    instances::require_safe_name("mod file name", &file_name)?;
    let dir = state.dirs.game_dir(&instance_id).join("mods");
    let path = [
        dir.join(&file_name),
        dir.join(format!("{file_name}.disabled")),
    ]
    .into_iter()
    .find(|p| p.exists())
    .ok_or_else(|| Error::NotFound(format!("mod file {file_name}")))?;
    let sha1 = crate::net::file_sha1(&path).await?;

    let project_id = match provider.as_str() {
        "modrinth" => state
            .crosssource
            .resolve_by_hash(&state.http, &sha1)
            .await?
            .map(|mr| mr.project_id)
            .ok_or_else(|| {
                Error::NotFound(format!("{file_name} on Modrinth (no identical file)"))
            })?,
        "curseforge" => {
            let key = curseforge::api_key(state)?;
            state
                .crosssource
                .resolve_local_file_to_cf(&state.http, &key, &sha1, &path)
                .await?
                .map(|cf| cf.project_id.to_string())
                .ok_or_else(|| {
                    Error::NotFound(format!("{file_name} on CurseForge (no identical file)"))
                })?
        }
        other => return Err(Error::Other(format!("unknown provider: {other}"))),
    };

    instances::record_installs(
        state,
        &instance_id,
        &[(file_name, project_id.clone())],
        &provider,
    );
    Ok(project_id)
}

#[tauri::command]
pub async fn auto_update_instance_content(
    state: State<'_, AppState>,
    instance_id: String,
    loader: Option<String>,
    game_version: Option<String>,
) -> Result<u32> {
    modrinth::auto_update_all(
        state.inner(),
        &instance_id,
        loader.as_deref(),
        game_version.as_deref(),
    )
    .await
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
    instances::export_instance(state.inner(), &id, Path::new(&dest)).await
}

#[tauri::command]
pub async fn import_instance(state: State<'_, AppState>, src: String) -> Result<Instance> {
    instances::import_instance(state.inner(), Path::new(&src)).await
}

#[tauri::command]
pub async fn export_share_manifest(
    state: State<'_, AppState>,
    id: String,
    dest: String,
) -> Result<()> {
    sharing::export_share_manifest(state.inner(), &id, Path::new(&dest))
}

#[tauri::command]
pub async fn import_share_manifest(
    state: State<'_, AppState>,
    src: String,
) -> Result<Instance> {
    sharing::import_share_manifest(state.inner(), Path::new(&src)).await
}

#[tauri::command]
pub async fn get_share_code(state: State<'_, AppState>, id: String) -> Result<String> {
    sharing::share_code(state.inner(), &id)
}

#[tauri::command]
pub async fn import_share_code(state: State<'_, AppState>, code: String) -> Result<Instance> {
    sharing::import_share_code(state.inner(), &code).await
}

/// Export an instance as a Modrinth modpack (`.mrpack`). `embed` lists the
/// files (not available on Modrinth) the user chose to bundle into overrides;
/// omitted = legacy behavior (bundle everything unresolved).
#[tauri::command]
pub async fn export_mrpack(
    state: State<'_, AppState>,
    id: String,
    dest: String,
    embed: Option<Vec<String>>,
) -> Result<()> {
    crate::export::export_mrpack(state.inner(), &id, Path::new(&dest), embed.as_deref()).await
}

/// Export an instance as a CurseForge modpack zip (manifest.json +
/// modlist.html + overrides/). `embed` lists the files (not available on
/// CurseForge) the user chose to bundle into overrides.
#[tauri::command]
pub async fn export_curseforge_pack(
    state: State<'_, AppState>,
    id: String,
    dest: String,
    embed: Option<Vec<String>>,
) -> Result<()> {
    crate::export::export_curseforge_pack(state.inner(), &id, Path::new(&dest), embed.as_deref())
        .await
}

/// Resolve every exportable file on both platforms for the pre-export review
/// dialog (which files are Modrinth-only / CurseForge-only / unresolved).
#[tauri::command]
pub async fn prepare_pack_export(
    state: State<'_, AppState>,
    id: String,
) -> Result<crate::export::PackExportPreview> {
    crate::export::prepare_pack_export(state.inner(), &id).await
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
    /// True when the file was blocked on CurseForge and fetched from Modrinth
    /// instead (hash-verified identical file).
    #[serde(default)]
    pub via_modrinth_fallback: bool,
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
    let mut idx = vec![(file.clone(), project_id.clone())];
    idx.extend(deps.iter().map(|d| (d.file_name.clone(), d.project_id.clone())));
    instances::record_installs(state.inner(), &instance_id, &idx, "modrinth");
    Ok(InstallOutcome {
        file,
        dependencies: deps.into_iter().map(|d| d.file_name).collect(),
        via_modrinth_fallback: false,
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
    let installed = curseforge::install_file(
        state.inner(),
        &instance_id,
        &project_id,
        &file_id,
        &content_type,
        loader.as_deref(),
        game_version.as_deref(),
    )
    .await?;
    record_cf_install(state.inner(), &instance_id, &project_id, &installed);
    Ok(InstallOutcome {
        via_modrinth_fallback: installed.modrinth_fallback.is_some(),
        file: installed.file_name,
        dependencies: installed.deps.into_iter().map(|d| d.file_name).collect(),
    })
}

// ---------------------------------------------------------------------------
// Open URL in system browser
// ---------------------------------------------------------------------------

fn validate_http_url(url: &str) -> Result<String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err(Error::Other("URL cannot be empty.".into()));
    }
    if trimmed.chars().any(|c| c.is_control()) {
        return Err(Error::Other("URL contains invalid characters.".into()));
    }
    let lower = trimmed.to_ascii_lowercase();
    let rest = if let Some(r) = lower.strip_prefix("https://") {
        r
    } else if let Some(r) = lower.strip_prefix("http://") {
        r
    } else {
        return Err(Error::Other("Only http:// and https:// URLs are allowed.".into()));
    };
    if rest.is_empty() || rest.starts_with('/') {
        return Err(Error::Other("Invalid URL.".into()));
    }
    Ok(trimmed.to_string())
}

#[tauri::command]
pub async fn open_url(url: String) -> Result<()> {
    let url = validate_http_url(&url)?;
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
    // Walking a many-GB instance is blocking filesystem work.
    let dirs = state.inner().dirs.clone();
    Ok(tokio::task::spawn_blocking(move || tools::instance_disk_usage(&dirs, &instance_id)).await?)
}

#[tauri::command]
pub async fn list_snapshots(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<Vec<tools::Snapshot>> {
    Ok(tools::list_snapshots(&state.inner().dirs, &instance_id))
}

#[tauri::command]
pub async fn create_snapshot(
    state: State<'_, AppState>,
    instance_id: String,
    world: String,
) -> Result<tools::Snapshot> {
    // Zipping a world can take a while — keep it off the async runtime.
    let dirs = state.inner().dirs.clone();
    tokio::task::spawn_blocking(move || tools::create_snapshot(&dirs, &instance_id, &world)).await?
}

#[tauri::command]
pub async fn restore_snapshot(
    state: State<'_, AppState>,
    instance_id: String,
    file_name: String,
) -> Result<()> {
    let dirs = state.inner().dirs.clone();
    tokio::task::spawn_blocking(move || tools::restore_snapshot(&dirs, &instance_id, &file_name))
        .await?
}

#[tauri::command]
pub async fn delete_snapshot(
    state: State<'_, AppState>,
    instance_id: String,
    file_name: String,
) -> Result<()> {
    tools::delete_snapshot(&state.inner().dirs, &instance_id, &file_name)
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

/// Preflight Check — fast local scan for the setup mistakes that reliably
/// crash modded Minecraft (Java mismatch, bad RAM allocation, duplicate mods).
#[tauri::command]
pub async fn preflight_check(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<Vec<preflight::PreflightWarning>> {
    preflight::preflight_check(state.inner(), &instance_id).await
}

/// Turbo Button — install the curated performance mod stack for this
/// instance's loader in one click.
#[tauri::command]
pub async fn apply_turbo(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<turbo::TurboResult> {
    turbo::apply(state.inner(), &instance_id).await
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
    let dirs = state.dirs.clone();
    Ok(tokio::task::spawn_blocking(move || java::detect_in(&dirs)).await?)
}

// ---------------------------------------------------------------------------
// JVM tuning + startup measurement
// ---------------------------------------------------------------------------

/// Recommended JVM settings for an instance (heap from pack size vs. system
/// RAM, GC flags from the Java major). Read-only — applying is a normal
/// `update_instance` after the user confirms the diff.
#[tauri::command]
pub async fn suggest_jvm_args(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<crate::jvmtune::JvmSuggestion> {
    crate::jvmtune::suggest(state.inner(), &instance_id).await
}

/// Average startup time under the instance's current JVM settings vs. the
/// previous settings (for the before/after readout).
#[tauri::command]
pub async fn startup_stats(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<crate::startup::StartupStats> {
    crate::startup::startup_stats(state.inner(), &instance_id)
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

/// Decode a modded (Forge/NeoForge) server's mod list and map every mod to a
/// downloadable file — the plan behind "Create matching instance".
#[tauri::command]
pub async fn analyze_server_mods(
    app: AppHandle,
    state: State<'_, AppState>,
    address: String,
) -> Result<crate::server_mods::ServerModPlan> {
    crate::server_mods::analyze_server(&app, state.inner(), &address).await
}

/// Build an instance matching a modded server: right loader + MC version,
/// all resolvable mods installed, plus a report of what needs manual work.
#[tauri::command]
pub async fn create_instance_from_server(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    address: String,
) -> Result<crate::server_mods::ServerInstanceOutcome> {
    crate::server_mods::create_instance_from_server(&app, state.inner(), &name, &address).await
}

// ---------------------------------------------------------------------------
// Play sessions (activity stats)
// ---------------------------------------------------------------------------

/// The recorded play sessions across all instances (for the activity chart).
#[tauri::command]
pub async fn list_sessions(state: State<'_, AppState>) -> Result<Vec<instances::Session>> {
    Ok(instances::list_sessions(state.inner()))
}

/// Write PNG bytes to a user-chosen destination (from the save dialog).
/// Used by the Wrapped share card, which is rendered to a canvas in the UI.
#[tauri::command]
pub async fn save_png(dest: String, data: Vec<u8>) -> Result<()> {
    if !dest.to_lowercase().ends_with(".png") {
        return Err(Error::Other("Destination must be a .png file.".into()));
    }
    std::fs::write(Path::new(&dest), &data)?;
    Ok(())
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
