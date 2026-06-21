//! Instance CRUD plus persistence of settings, accounts and per-instance mods.

use crate::error::{Error, Result};
use crate::models::*;
use crate::state::AppState;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

// ---------------------------------------------------------------------------
// Generic JSON helpers
// ---------------------------------------------------------------------------

fn read_json_or_default<T: DeserializeOwned + Default>(path: &Path) -> T {
    std::fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

/// Write `data` to `path` atomically: stream it into a sibling temp file and
/// rename over the destination. A crash mid-write then leaves either the old
/// file or the new one — never a truncated, unparseable JSON document.
fn atomic_write(path: &Path, data: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Append ".tmp" to the full file name (not `with_extension`) so the temp
    // path can never collide with a real sibling file.
    let mut tmp = path.as_os_str().to_os_string();
    tmp.push(".tmp");
    let tmp = std::path::PathBuf::from(tmp);
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, path)
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let data = serde_json::to_vec_pretty(value)?;
    atomic_write(path, &data)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

pub fn load_settings(state: &AppState) -> Settings {
    read_json_or_default(&state.dirs.settings_file())
}

pub fn save_settings(state: &AppState, settings: &Settings) -> Result<()> {
    write_json(&state.dirs.settings_file(), settings)
}

// ---------------------------------------------------------------------------
// Accounts
// ---------------------------------------------------------------------------

pub fn load_accounts(state: &AppState) -> AccountStore {
    read_json_or_default(&state.dirs.accounts_file())
}

pub fn save_accounts(state: &AppState, store: &AccountStore) -> Result<()> {
    write_json(&state.dirs.accounts_file(), store)
}

/// Apply `f` to the account store under a lock, persisting the result. This is
/// the only safe way to mutate accounts: the load → modify → save sequence runs
/// while holding `accounts_lock`, so concurrent callers can't lose each other's
/// writes (the classic read-modify-write race).
pub fn mutate_accounts<R>(
    state: &AppState,
    f: impl FnOnce(&mut AccountStore) -> R,
) -> Result<R> {
    let _guard = state.accounts_lock.lock().unwrap();
    let mut store = load_accounts(state);
    let result = f(&mut store);
    save_accounts(state, &store)?;
    Ok(result)
}

pub fn upsert_account(state: &AppState, account: Account) -> Result<AccountStore> {
    mutate_accounts(state, move |store| {
        let id = account.id.clone();
        if let Some(existing) = store.accounts.iter_mut().find(|a| a.id == account.id) {
            *existing = account;
        } else {
            store.accounts.push(account);
        }
        store.active = Some(id);
        store.clone()
    })
}

pub fn active_account(state: &AppState) -> Option<Account> {
    let store = load_accounts(state);
    let id = store.active.clone()?;
    store.accounts.into_iter().find(|a| a.id == id)
}

// ---------------------------------------------------------------------------
// Instances
// ---------------------------------------------------------------------------

fn slugify(name: &str) -> String {
    let mut s: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect();
    while s.contains("--") {
        s = s.replace("--", "-");
    }
    let s = s.trim_matches('-').to_string();
    if s.is_empty() {
        "instance".to_string()
    } else {
        s
    }
}

pub fn list_instances(state: &AppState) -> Vec<Instance> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(state.dirs.instances()) {
        for e in rd.flatten() {
            let manifest = e.path().join("instance.json");
            if let Ok(bytes) = std::fs::read(&manifest) {
                if let Ok(mut inst) = serde_json::from_slice::<Instance>(&bytes) {
                    let mods_dir = state.dirs.game_dir(&inst.id).join("mods");
                    inst.mod_count = std::fs::read_dir(&mods_dir)
                        .map(|rd| {
                            rd.flatten()
                                .filter(|e| {
                                    let n = e.file_name().to_string_lossy().to_string();
                                    n.ends_with(".jar") || n.ends_with(".jar.disabled")
                                })
                                .count() as u32
                        })
                        .unwrap_or(0);
                    out.push(inst);
                }
            }
        }
    }
    out.sort_by(|a, b| {
        b.last_played
            .cmp(&a.last_played)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    out
}

pub fn get_instance(state: &AppState, id: &str) -> Result<Instance> {
    let manifest = state.dirs.instance_manifest(id);
    let bytes = std::fs::read(&manifest)
        .map_err(|_| Error::NotFound(format!("instance {id}")))?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn save_instance(state: &AppState, instance: &Instance) -> Result<()> {
    write_json(&state.dirs.instance_manifest(&instance.id), instance)
}

pub fn create_instance(
    state: &AppState,
    name: &str,
    mc_version: &str,
    loader: Loader,
    loader_version: Option<String>,
    icon: Option<String>,
) -> Result<Instance> {
    let short = uuid::Uuid::new_v4().to_string();
    let id = format!("{}-{}", slugify(name), &short[0..8]);

    let instance = Instance {
        id: id.clone(),
        name: name.to_string(),
        mc_version: mc_version.to_string(),
        loader,
        loader_version,
        icon,
        group: None,
        accent: None,
        favorite: false,
        created: chrono::Utc::now().timestamp(),
        last_played: None,
        total_play_seconds: 0,
        memory_mb: None,
        java_path: None,
        jvm_args: None,
        window_width: None,
        window_height: None,
        env_vars: None,
        pre_launch: None,
        post_exit: None,
        pack_source: None,
        mod_count: 0,
    };

    // Pre-create the game directory + mods folder.
    std::fs::create_dir_all(state.dirs.game_dir(&id).join("mods"))?;
    save_instance(state, &instance)?;
    Ok(instance)
}

pub fn delete_instance(state: &AppState, id: &str) -> Result<()> {
    let dir = state.dirs.instance_dir(id);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    Ok(())
}

pub fn duplicate_instance(state: &AppState, id: &str) -> Result<Instance> {
    let src = get_instance(state, id)?;
    let new = create_instance(
        state,
        &format!("{} (copy)", src.name),
        &src.mc_version,
        src.loader,
        src.loader_version.clone(),
        src.icon.clone(),
    )?;
    // Copy the game directory contents (mods, configs, saves, ...).
    copy_dir(&state.dirs.game_dir(id), &state.dirs.game_dir(&new.id))?;
    Ok(new)
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    if !from.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)?.flatten() {
        let path = entry.path();
        let dest = to.join(entry.file_name());
        if path.is_dir() {
            copy_dir(&path, &dest)?;
        } else {
            std::fs::copy(&path, &dest)?;
        }
    }
    Ok(())
}

/// Mark an instance as played now and add to its playtime.
pub fn record_play(state: &AppState, id: &str, seconds: u64) {
    if let Ok(mut inst) = get_instance(state, id) {
        inst.last_played = Some(chrono::Utc::now().timestamp());
        inst.total_play_seconds += seconds;
        let _ = save_instance(state, &inst);
    }
}

/// Like [`record_play`] but works from a background thread that only has access
/// to the directory layout (used by the process watcher on game exit).
pub fn record_play_dirs(dirs: &crate::state::AppDirs, id: &str, seconds: u64) {
    let manifest = dirs.instance_manifest(id);
    if let Ok(bytes) = std::fs::read(&manifest) {
        if let Ok(mut inst) = serde_json::from_slice::<Instance>(&bytes) {
            inst.last_played = Some(chrono::Utc::now().timestamp());
            inst.total_play_seconds += seconds;
            if let Ok(data) = serde_json::to_vec_pretty(&inst) {
                let _ = atomic_write(&manifest, &data);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Play sessions (for the activity chart on Home)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub instance_id: String,
    /// Unix timestamp when the session started.
    pub started: i64,
    pub seconds: u64,
}

pub fn list_sessions(state: &AppState) -> Vec<Session> {
    read_json_or_default::<Vec<Session>>(&state.dirs.sessions_file())
}

/// Append a finished play session to the activity log (best-effort, capped to
/// the most recent 2000 entries). Called from the process watcher on game exit.
pub fn record_session_dirs(dirs: &crate::state::AppDirs, id: &str, started: i64, seconds: u64) {
    if seconds == 0 {
        return;
    }
    let path = dirs.sessions_file();
    let mut sessions: Vec<Session> = std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    sessions.push(Session {
        instance_id: id.to_string(),
        started,
        seconds,
    });
    let len = sessions.len();
    if len > 2000 {
        sessions.drain(0..len - 2000);
    }
    if let Ok(data) = serde_json::to_vec_pretty(&sessions) {
        let _ = atomic_write(&path, &data);
    }
}

// ---------------------------------------------------------------------------
// Export / import (zip the instance directory)
// ---------------------------------------------------------------------------

fn zip_dir_into(
    zw: &mut zip::ZipWriter<std::fs::File>,
    base: &Path,
    dir: &Path,
    opts: zip::write::SimpleFileOptions,
) -> Result<()> {
    use std::io::Write;
    for entry in std::fs::read_dir(dir)?.flatten() {
        let path = entry.path();
        let rel = path.strip_prefix(base).unwrap_or(&path);
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if path.is_dir() {
            zw.add_directory(format!("{rel_str}/"), opts)?;
            zip_dir_into(zw, base, &path, opts)?;
        } else {
            zw.start_file(rel_str, opts)?;
            let bytes = std::fs::read(&path)?;
            zw.write_all(&bytes)?;
        }
    }
    Ok(())
}

/// Zip an instance's whole directory (manifest + game files) to `dest`.
pub fn export_instance(state: &AppState, id: &str, dest: &Path) -> Result<()> {
    let dir = state.dirs.instance_dir(id);
    if !dir.exists() {
        return Err(Error::NotFound(format!("instance {id}")));
    }
    let file = std::fs::File::create(dest)?;
    let mut zw = zip::ZipWriter::new(file);
    let opts =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zip_dir_into(&mut zw, &dir, &dir, opts)?;
    zw.finish()?;
    Ok(())
}

/// Import an instance from a zip produced by [`export_instance`], assigning it a
/// fresh id so it never clobbers an existing instance.
pub fn import_instance(state: &AppState, src: &Path) -> Result<Instance> {
    let file = std::fs::File::open(src)?;
    let mut archive = zip::ZipArchive::new(file)?;

    // Read the manifest first to derive a name.
    let mut manifest: Instance = {
        let mut mf = archive
            .by_name("instance.json")
            .map_err(|_| Error::Other("not an EZMapa instance export (no instance.json)".into()))?;
        let mut buf = Vec::new();
        std::io::Read::read_to_end(&mut mf, &mut buf)?;
        serde_json::from_slice(&buf)?
    };

    let short = uuid::Uuid::new_v4().to_string();
    let new_id = format!("{}-{}", slugify(&manifest.name), &short[0..8]);
    let dest_dir = state.dirs.instance_dir(&new_id);

    for i in 0..archive.len() {
        let mut zf = archive.by_index(i)?;
        let Some(rel) = zf.enclosed_name() else {
            continue;
        };
        let out = dest_dir.join(&rel);
        if zf.is_dir() {
            std::fs::create_dir_all(&out)?;
        } else {
            if let Some(p) = out.parent() {
                std::fs::create_dir_all(p)?;
            }
            let mut w = std::fs::File::create(&out)?;
            std::io::copy(&mut zf, &mut w)?;
        }
    }

    // Rewrite the manifest with the new id (and reset runtime stats).
    manifest.id = new_id.clone();
    manifest.last_played = None;
    save_instance(state, &manifest)?;
    Ok(manifest)
}

/// The game sub-directory a given content type installs into. Single source of
/// truth shared by the Modrinth and CurseForge installers.
pub fn content_subdir(content_type: &str) -> &'static str {
    match content_type {
        "resourcepack" => "resourcepacks",
        "shader" => "shaderpacks",
        _ => "mods",
    }
}

// ---------------------------------------------------------------------------
// Content index: maps an installed file name to the project it came from, so
// the UI can tell which search results are already installed.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ContentIndex {
    /// display file name (without `.disabled`) -> project id
    #[serde(default)]
    items: HashMap<String, IndexItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IndexItem {
    project_id: String,
    #[serde(default)]
    provider: String,
}

fn index_path(state: &AppState, id: &str) -> std::path::PathBuf {
    state.dirs.instance_dir(id).join("ezmapa_index.json")
}

fn legacy_index_path(state: &AppState, id: &str) -> std::path::PathBuf {
    state.dirs.instance_dir(id).join("beacon_index.json")
}

/// Move a tracked content entry when a file is renamed during an update.
pub fn migrate_index_entry(state: &AppState, id: &str, old_name: &str, new_name: &str) {
    let mut idx = load_index(state, id);
    if let Some(item) = idx.items.remove(old_name) {
        idx.items.insert(new_name.to_string(), item);
        let _ = write_json(&index_path(state, id), &idx);
    }
}

fn load_index(state: &AppState, id: &str) -> ContentIndex {
    let path = index_path(state, id);
    if path.exists() {
        return read_json_or_default(&path);
    }
    read_json_or_default(&legacy_index_path(state, id))
}

/// Record that several `(file_name, project_id)` pairs in this instance came
/// from `provider`, in a single read-modify-write of the content index. Callers
/// installing a mod plus its dependencies previously rewrote the whole index
/// file once per entry; this collapses that to one write.
pub fn record_installs(state: &AppState, id: &str, items: &[(String, String)], provider: &str) {
    if items.is_empty() {
        return;
    }
    let mut idx = load_index(state, id);
    for (file_name, project_id) in items {
        idx.items.insert(
            file_name.clone(),
            IndexItem {
                project_id: project_id.clone(),
                provider: provider.to_string(),
            },
        );
    }
    let _ = write_json(&index_path(state, id), &idx);
}

/// Drop a file from the content index (called on delete).
fn forget_install(state: &AppState, id: &str, file_name: &str) {
    let mut idx = load_index(state, id);
    if idx.items.remove(file_name).is_some() {
        let _ = write_json(&index_path(state, id), &idx);
    }
}

// ---------------------------------------------------------------------------
// Mods within an instance
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModEntry {
    pub file_name: String,
    pub enabled: bool,
    pub size: u64,
    pub project_id: Option<String>,
}

const DISABLED_SUFFIX: &str = ".disabled";

pub fn list_mods(state: &AppState, instance_id: &str) -> Vec<ModEntry> {
    let dir = state.dirs.game_dir(instance_id).join("mods");
    let idx = load_index(state, instance_id);
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let (enabled, file_name) = if let Some(stripped) = name.strip_suffix(DISABLED_SUFFIX) {
                (false, stripped.to_string())
            } else if name.ends_with(".jar") {
                (true, name.clone())
            } else {
                continue;
            };
            let size = e.metadata().map(|m| m.len()).unwrap_or(0);
            let project_id = idx.items.get(&file_name).map(|i| i.project_id.clone());
            out.push(ModEntry {
                file_name,
                enabled,
                size,
                project_id,
            });
        }
    }
    out.sort_by(|a, b| a.file_name.to_lowercase().cmp(&b.file_name.to_lowercase()));
    out
}

pub fn set_mod_enabled(
    state: &AppState,
    instance_id: &str,
    file_name: &str,
    enabled: bool,
) -> Result<()> {
    let dir = state.dirs.game_dir(instance_id).join("mods");
    let enabled_path = dir.join(file_name);
    let disabled_path = dir.join(format!("{file_name}{DISABLED_SUFFIX}"));
    if enabled && disabled_path.exists() {
        std::fs::rename(&disabled_path, &enabled_path)?;
    } else if !enabled && enabled_path.exists() {
        std::fs::rename(&enabled_path, &disabled_path)?;
    }
    Ok(())
}

pub fn delete_mod(state: &AppState, instance_id: &str, file_name: &str) -> Result<()> {
    let dir = state.dirs.game_dir(instance_id).join("mods");
    for candidate in [dir.join(file_name), dir.join(format!("{file_name}{DISABLED_SUFFIX}"))] {
        if candidate.exists() {
            std::fs::remove_file(&candidate)?;
        }
    }
    forget_install(state, instance_id, file_name);
    Ok(())
}

// ---------------------------------------------------------------------------
// Resource packs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourcePackEntry {
    pub file_name: String,
    pub size: u64,
    pub project_id: Option<String>,
}

pub fn list_resource_packs(state: &AppState, instance_id: &str) -> Vec<ResourcePackEntry> {
    let dir = state.dirs.game_dir(instance_id).join("resourcepacks");
    let idx = load_index(state, instance_id);
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let path = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            if name.ends_with(".zip") || path.is_dir() {
                let size = e.metadata().map(|m| m.len()).unwrap_or(0);
                let project_id = idx.items.get(&name).map(|i| i.project_id.clone());
                out.push(ResourcePackEntry { file_name: name, size, project_id });
            }
        }
    }
    out.sort_by(|a, b| a.file_name.to_lowercase().cmp(&b.file_name.to_lowercase()));
    out
}

pub fn delete_resource_pack(state: &AppState, instance_id: &str, file_name: &str) -> Result<()> {
    let path = state.dirs.game_dir(instance_id).join("resourcepacks").join(file_name);
    if path.is_dir() {
        std::fs::remove_dir_all(&path)?;
    } else if path.exists() {
        std::fs::remove_file(&path)?;
    }
    forget_install(state, instance_id, file_name);
    Ok(())
}

// ---------------------------------------------------------------------------
// Shaders
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShaderEntry {
    pub file_name: String,
    pub size: u64,
    pub project_id: Option<String>,
}

pub fn list_shaders(state: &AppState, instance_id: &str) -> Vec<ShaderEntry> {
    let dir = state.dirs.game_dir(instance_id).join("shaderpacks");
    let idx = load_index(state, instance_id);
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let path = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            if name.ends_with(".zip") || path.is_dir() {
                let size = e.metadata().map(|m| m.len()).unwrap_or(0);
                let project_id = idx.items.get(&name).map(|i| i.project_id.clone());
                out.push(ShaderEntry { file_name: name, size, project_id });
            }
        }
    }
    out.sort_by(|a, b| a.file_name.to_lowercase().cmp(&b.file_name.to_lowercase()));
    out
}

pub fn delete_shader(state: &AppState, instance_id: &str, file_name: &str) -> Result<()> {
    let path = state.dirs.game_dir(instance_id).join("shaderpacks").join(file_name);
    if path.is_dir() {
        std::fs::remove_dir_all(&path)?;
    } else if path.exists() {
        std::fs::remove_file(&path)?;
    }
    forget_install(state, instance_id, file_name);
    Ok(())
}

// ---------------------------------------------------------------------------
// Worlds (saves/)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldEntry {
    pub name: String,
    pub modified: Option<i64>,
}

pub fn list_worlds(state: &AppState, instance_id: &str) -> Vec<WorldEntry> {
    let dir = state.dirs.game_dir(instance_id).join("saves");
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            if e.path().is_dir() {
                let name = e.file_name().to_string_lossy().to_string();
                let modified = e.metadata().ok().and_then(|m| m.modified().ok()).map(|t| {
                    t.duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64
                });
                out.push(WorldEntry { name, modified });
            }
        }
    }
    out.sort_by(|a, b| b.modified.cmp(&a.modified));
    out
}

pub fn delete_world(state: &AppState, instance_id: &str, name: &str) -> Result<()> {
    let path = state.dirs.game_dir(instance_id).join("saves").join(name);
    if path.is_dir() {
        std::fs::remove_dir_all(&path)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Screenshots
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotEntry {
    pub file_name: String,
    pub size: u64,
    pub taken_at: i64,
}

pub fn list_screenshots(state: &AppState, instance_id: &str) -> Vec<ScreenshotEntry> {
    let dir = state.dirs.game_dir(instance_id).join("screenshots");
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.ends_with(".png") || name.ends_with(".jpg") {
                let size = e.metadata().map(|m| m.len()).unwrap_or(0);
                let taken_at = e
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .map(|t| {
                        t.duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs() as i64
                    })
                    .unwrap_or(0);
                out.push(ScreenshotEntry { file_name: name, size, taken_at });
            }
        }
    }
    out.sort_by(|a, b| b.taken_at.cmp(&a.taken_at));
    out
}
