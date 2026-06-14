//! Instance CRUD plus persistence of settings, accounts and per-instance mods.

use crate::error::{Error, Result};
use crate::models::*;
use crate::state::AppState;
use serde::de::DeserializeOwned;
use serde::Serialize;
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

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_vec_pretty(value)?;
    std::fs::write(path, data)?;
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

pub fn upsert_account(state: &AppState, account: Account) -> Result<AccountStore> {
    let mut store = load_accounts(state);
    if let Some(existing) = store.accounts.iter_mut().find(|a| a.id == account.id) {
        *existing = account.clone();
    } else {
        store.accounts.push(account.clone());
    }
    store.active = Some(account.id);
    save_accounts(state, &store)?;
    Ok(store)
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
                if let Ok(inst) = serde_json::from_slice::<Instance>(&bytes) {
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
        created: chrono::Utc::now().timestamp(),
        last_played: None,
        total_play_seconds: 0,
        memory_mb: None,
        java_path: None,
        jvm_args: None,
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
                let _ = std::fs::write(&manifest, data);
            }
        }
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
}

const DISABLED_SUFFIX: &str = ".disabled";

pub fn list_mods(state: &AppState, instance_id: &str) -> Vec<ModEntry> {
    let dir = state.dirs.game_dir(instance_id).join("mods");
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
            out.push(ModEntry {
                file_name,
                enabled,
                size,
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
    Ok(())
}
