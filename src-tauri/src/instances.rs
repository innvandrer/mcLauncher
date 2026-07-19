//! Instance CRUD plus persistence of settings, accounts and per-instance mods.

use crate::error::{Error, Result};
use crate::models::*;
use crate::state::AppState;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

// ---------------------------------------------------------------------------
// Input validation
// ---------------------------------------------------------------------------

/// Reject webview-supplied ids / file names that aren't a single plain path
/// component (`..`, separators, absolute paths). Commands join these strings
/// onto data directories, so anything else could escape the intended folder —
/// the delete/rename commands especially must never traverse.
pub(crate) fn require_safe_name(kind: &str, value: &str) -> Result<()> {
    if crate::archive::is_safe_name(value) {
        Ok(())
    } else {
        Err(Error::Other(format!("Invalid {kind}: {value:?}")))
    }
}

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
pub(crate) fn atomic_write(path: &Path, data: &[u8]) -> std::io::Result<()> {
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

/// On-disk shape before tokens were moved to the OS keyring. Used only for
/// one-time migration when loading `accounts.json`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyAccount {
    pub id: String,
    pub username: String,
    #[serde(default)]
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub expires_at: i64,
    #[serde(default)]
    pub xuid: Option<String>,
    #[serde(rename = "type", default = "default_legacy_account_type")]
    pub kind: String,
}

fn default_legacy_account_type() -> String {
    "microsoft".to_string()
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyAccountStore {
    #[serde(default)]
    accounts: Vec<LegacyAccount>,
    #[serde(default)]
    active: Option<String>,
}

fn legacy_has_inline_tokens(legacy: &LegacyAccountStore) -> bool {
    legacy.accounts.iter().any(|a| {
        !a.access_token.is_empty()
            || a
                .refresh_token
                .as_ref()
                .is_some_and(|r| !r.is_empty())
    })
}

fn legacy_to_stored(leg: &LegacyAccount) -> StoredAccount {
    StoredAccount {
        id: leg.id.clone(),
        username: leg.username.clone(),
        expires_at: leg.expires_at,
        xuid: leg.xuid.clone(),
        kind: leg.kind.clone(),
    }
}

fn inline_tokens_from_file(path: &Path, account_id: &str) -> Option<(String, Option<String>)> {
    let bytes = std::fs::read(path).ok()?;
    let legacy: LegacyAccountStore = serde_json::from_slice(&bytes).ok()?;
    let leg = legacy.accounts.iter().find(|a| a.id == account_id)?;
    let has_tokens = !leg.access_token.is_empty()
        || leg
            .refresh_token
            .as_ref()
            .is_some_and(|r| !r.is_empty());
    if !has_tokens {
        return None;
    }
    Some((leg.access_token.clone(), leg.refresh_token.clone()))
}

fn try_migrate_inline_tokens(path: &Path, legacy: &LegacyAccountStore) -> bool {
    if !legacy_has_inline_tokens(legacy) {
        return false;
    }

    let mut all_migrated = true;
    for leg in &legacy.accounts {
        let has_tokens = !leg.access_token.is_empty()
            || leg
                .refresh_token
                .as_ref()
                .is_some_and(|r| !r.is_empty());
        if has_tokens && leg.kind == "microsoft" {
            if let Err(e) = crate::account_tokens::store_tokens(
                &leg.id,
                &leg.access_token,
                leg.refresh_token.as_deref(),
            ) {
                eprintln!(
                    "warning: could not migrate tokens for account {} to secure storage: {e}",
                    leg.id
                );
                all_migrated = false;
            }
        }
    }

    if all_migrated {
        let store = AccountStore {
            accounts: legacy.accounts.iter().map(legacy_to_stored).collect(),
            active: legacy.active.clone(),
        };
        if let Err(e) = write_json(path, &store) {
            eprintln!("warning: could not rewrite accounts.json without tokens: {e}");
            return false;
        }
    }
    all_migrated
}

fn read_account_store(path: &Path) -> AccountStore {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(_) => return AccountStore::default(),
    };

    let legacy: LegacyAccountStore = match serde_json::from_slice(&bytes) {
        Ok(s) => s,
        Err(_) => return AccountStore::default(),
    };

    let _ = try_migrate_inline_tokens(path, &legacy);

    AccountStore {
        accounts: legacy.accounts.iter().map(legacy_to_stored).collect(),
        active: legacy.active,
    }
}

fn hydrate_account(stored: &StoredAccount, accounts_path: &Path) -> Result<Account> {
    let (mut access_token, mut refresh_token) = if stored.kind == "microsoft" {
        crate::account_tokens::load_tokens(&stored.id)?
    } else {
        (String::new(), None)
    };

    // Fallback for accounts.json that still has inline tokens (e.g. keyring
    // migration failed on a previous launch).
    if stored.kind == "microsoft"
        && access_token.is_empty()
        && refresh_token.as_ref().is_none_or(|r| r.is_empty())
    {
        if let Some((a, r)) = inline_tokens_from_file(accounts_path, &stored.id) {
            access_token = a;
            refresh_token = r;
            let _ = crate::account_tokens::store_tokens(
                &stored.id,
                &access_token,
                refresh_token.as_deref(),
            );
            if let Ok(bytes) = std::fs::read(accounts_path) {
                if let Ok(legacy) = serde_json::from_slice::<LegacyAccountStore>(&bytes) {
                    let _ = try_migrate_inline_tokens(accounts_path, &legacy);
                }
            }
        }
    }

    Ok(Account::from_stored(
        stored.clone(),
        access_token,
        refresh_token,
    ))
}

pub fn load_accounts(state: &AppState) -> AccountStore {
    read_account_store(&state.dirs.accounts_file())
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
    if account.kind == "microsoft" {
        crate::account_tokens::store_tokens(
            &account.id,
            &account.access_token,
            account.refresh_token.as_deref(),
        )?;
    }
    let stored = account.into_stored();
    mutate_accounts(state, move |store| {
        let id = stored.id.clone();
        if let Some(existing) = store.accounts.iter_mut().find(|a| a.id == stored.id) {
            *existing = stored;
        } else {
            store.accounts.push(stored);
        }
        store.active = Some(id);
        store.clone()
    })
}

pub fn remove_account(state: &AppState, id: &str) -> Result<AccountStore> {
    let _ = crate::account_tokens::delete_tokens(id);
    mutate_accounts(state, |store| {
        store.accounts.retain(|a| a.id != id);
        if store.active.as_deref() == Some(id) {
            store.active = store.accounts.first().map(|a| a.id.clone());
        }
        store.clone()
    })
}

pub fn active_account(state: &AppState) -> Result<Account> {
    let path = state.dirs.accounts_file();
    let store = load_accounts(state);
    let id = store.active.clone().ok_or_else(|| {
        crate::error::Error::Auth("No account selected. Add an account first.".into())
    })?;
    let stored = store
        .accounts
        .iter()
        .find(|a| a.id == id)
        .ok_or_else(|| crate::error::Error::Auth("Active account not found.".into()))?
        .clone();
    hydrate_account(&stored, &path)
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
    require_safe_name("instance id", id)?;
    let manifest = state.dirs.instance_manifest(id);
    let bytes = std::fs::read(&manifest)
        .map_err(|_| Error::NotFound(format!("instance {id}")))?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn save_instance(state: &AppState, instance: &Instance) -> Result<()> {
    require_safe_name("instance id", &instance.id)?;
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
        archived: false,
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
        loadouts: Vec::new(),
        tags: Vec::new(),
        launch_profiles: Vec::new(),
        mod_count: 0,
    };

    // Pre-create the game directory + mods folder.
    std::fs::create_dir_all(state.dirs.game_dir(&id).join("mods"))?;
    save_instance(state, &instance)?;
    Ok(instance)
}

pub fn delete_instance(state: &AppState, id: &str) -> Result<()> {
    require_safe_name("instance id", id)?;
    let dir = state.dirs.instance_dir(id);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    Ok(())
}

pub async fn duplicate_instance(state: &AppState, id: &str) -> Result<Instance> {
    let src = get_instance(state, id)?;
    let new = create_instance(
        state,
        &format!("{} (copy)", src.name),
        &src.mc_version,
        src.loader,
        src.loader_version.clone(),
        src.icon.clone(),
    )?;
    // Copy the game directory contents (mods, configs, saves, ...). Saves can
    // run to gigabytes, so keep the copy off the async runtime.
    let from = state.dirs.game_dir(id);
    let to = state.dirs.game_dir(&new.id);
    tokio::task::spawn_blocking(move || copy_dir(&from, &to)).await??;
    Ok(new)
}

/// Recursive overwrite-copy of a directory tree. Shared with the legacy
/// data-dir migration in `lib.rs`; `forge::copy_dir_all` stays separate on
/// purpose (it skips existing destination files to avoid re-copying shared
/// libraries).
pub(crate) fn copy_dir(from: &Path, to: &Path) -> Result<()> {
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

/// Mark an instance as played now and add to its playtime. Works from a
/// background thread that only has access to the directory layout (used by
/// the process watcher on game exit).
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
    // Watcher threads for different instances can exit at the same moment;
    // serialize the read-modify-write so one session isn't lost to the race.
    static SESSIONS_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = SESSIONS_LOCK.lock().unwrap_or_else(|p| p.into_inner());
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
    for entry in std::fs::read_dir(dir)?.flatten() {
        let path = entry.path();
        let rel = path.strip_prefix(base).unwrap_or(&path);
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if path.is_dir() {
            zw.add_directory(format!("{rel_str}/"), opts)?;
            zip_dir_into(zw, base, &path, opts)?;
        } else {
            // Stream instead of fs::read — region files can be hundreds of MB.
            zw.start_file(rel_str, opts)?;
            let mut f = std::fs::File::open(&path)?;
            std::io::copy(&mut f, zw)?;
        }
    }
    Ok(())
}

/// Zip an instance's whole directory (manifest + game files) to `dest`.
/// The zip work runs off the async runtime — instances can be gigabytes.
pub async fn export_instance(state: &AppState, id: &str, dest: &Path) -> Result<()> {
    require_safe_name("instance id", id)?;
    let dir = state.dirs.instance_dir(id);
    if !dir.exists() {
        return Err(Error::NotFound(format!("instance {id}")));
    }
    let dest = dest.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::create(&dest)?;
        let mut zw = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip_dir_into(&mut zw, &dir, &dir, opts)?;
        zw.finish()?;
        Ok(())
    })
    .await?
}

/// Import an instance from a zip produced by [`export_instance`], assigning it a
/// fresh id so it never clobbers an existing instance. Extraction runs off the
/// async runtime.
pub async fn import_instance(state: &AppState, src: &Path) -> Result<Instance> {
    let src = src.to_path_buf();
    let instances_root = state.dirs.instances();
    let mut manifest =
        tokio::task::spawn_blocking(move || extract_instance_import(&src, &instances_root))
            .await??;
    // Reset runtime stats — an import is a fresh instance.
    manifest.last_played = None;
    manifest.total_play_seconds = 0;
    manifest.created = chrono::Utc::now().timestamp();
    save_instance(state, &manifest)?;
    Ok(manifest)
}

/// Blocking core of [`import_instance`]: read the manifest, mint a fresh id,
/// extract everything under it. Returns the manifest with the new id set.
fn extract_instance_import(src: &Path, instances_root: &Path) -> Result<Instance> {
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
    let dest_dir = instances_root.join(&new_id);

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

    manifest.id = new_id;
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
    /// Set when the file's bytes were fetched from Modrinth because the
    /// CurseForge author blocked third-party downloads. The primary identity
    /// (`provider`/`project_id`) stays CurseForge; this records where the
    /// hash-verified identical file actually came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fallback: Option<FallbackSource>,
    /// Primary project ids that required this file when it was installed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    required_by: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FallbackSource {
    provider: String,
    project_id: String,
    version_id: String,
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
        let required_by = idx
            .items
            .get(file_name)
            .map(|item| item.required_by.clone())
            .unwrap_or_default();
        idx.items.insert(
            file_name.clone(),
            IndexItem {
                project_id: project_id.clone(),
                provider: provider.to_string(),
                fallback: None,
                required_by,
            },
        );
    }
    let _ = write_json(&index_path(state, id), &idx);
}

/// Mark a tracked file as having been sourced from Modrinth as a fallback for
/// a blocked CurseForge download. Creates the entry when the file isn't
/// tracked yet (keeping the CurseForge identity empty in that case).
pub fn record_modrinth_fallback(
    state: &AppState,
    id: &str,
    file_name: &str,
    mr: &crate::crosssource::ModrinthRef,
) {
    let mut idx = load_index(state, id);
    let entry = idx.items.entry(file_name.to_string()).or_insert(IndexItem {
        project_id: String::new(),
        provider: "curseforge".into(),
        fallback: None,
        required_by: Vec::new(),
    });
    entry.fallback = Some(FallbackSource {
        provider: "modrinth".into(),
        project_id: mr.project_id.clone(),
        version_id: mr.version_id.clone(),
    });
    let _ = write_json(&index_path(state, id), &idx);
}

/// Persist which dependency files were installed for a primary project. This
/// enables safe-removal warnings without another network request.
pub fn record_install_dependencies(
    state: &AppState,
    id: &str,
    primary_project_id: &str,
    dependency_files: &[String],
) {
    if dependency_files.is_empty() {
        return;
    }
    let mut idx = load_index(state, id);
    for file_name in dependency_files {
        if let Some(item) = idx.items.get_mut(file_name) {
            if !item.required_by.iter().any(|project| project == primary_project_id) {
                item.required_by.push(primary_project_id.to_string());
            }
        }
    }
    let _ = write_json(&index_path(state, id), &idx);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovalImpact {
    pub file_name: String,
    pub project_id: Option<String>,
    pub required_by: Vec<String>,
    pub safe: bool,
}

pub fn mod_removal_impact(state: &AppState, instance_id: &str, file_name: &str) -> Result<RemovalImpact> {
    require_safe_name("instance id", instance_id)?;
    require_safe_name("mod file name", file_name)?;
    let idx = load_index(state, instance_id);
    let item = idx.items.get(file_name);
    let required_by = item.map(|entry| entry.required_by.clone()).unwrap_or_default();
    Ok(RemovalImpact {
        file_name: file_name.to_string(),
        project_id: item.map(|entry| entry.project_id.clone()).filter(|id| !id.is_empty()),
        safe: required_by.is_empty(),
        required_by,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareContentEntry {
    pub file_name: String,
    pub provider: String,
    pub project_id: String,
    pub content_type: String,
    pub enabled: bool,
}

pub fn share_content_entries(state: &AppState, instance_id: &str) -> Vec<ShareContentEntry> {
    let game = state.dirs.game_dir(instance_id);
    load_index(state, instance_id)
        .items
        .into_iter()
        .filter_map(|(file_name, item)| {
            if item.project_id.is_empty() || item.provider.is_empty() {
                return None;
            }
            let disabled = game.join("mods").join(format!("{file_name}.disabled"));
            let (content_type, enabled) = if game.join("mods").join(&file_name).is_file() {
                ("mod", true)
            } else if disabled.is_file() {
                ("mod", false)
            } else if game.join("resourcepacks").join(&file_name).exists() {
                ("resourcepack", true)
            } else if game.join("shaderpacks").join(&file_name).exists() {
                ("shader", true)
            } else {
                return None;
            };
            Some(ShareContentEntry {
                file_name,
                provider: item.provider,
                project_id: item.project_id,
                content_type: content_type.to_string(),
                enabled,
            })
        })
        .collect()
}

/// Map of tracked file name → provider ("modrinth"/"curseforge"), used as the
/// per-file source-of-truth pin by the update checker.
pub fn content_provider_map(state: &AppState, id: &str) -> HashMap<String, String> {
    load_index(state, id)
        .items
        .into_iter()
        .filter(|(_, item)| !item.provider.is_empty())
        .map(|(name, item)| (name, item.provider))
        .collect()
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
    out.sort_by_key(|a| a.file_name.to_lowercase());
    out
}

pub fn set_mod_enabled(
    state: &AppState,
    instance_id: &str,
    file_name: &str,
    enabled: bool,
) -> Result<()> {
    require_safe_name("instance id", instance_id)?;
    require_safe_name("mod file name", file_name)?;
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
    require_safe_name("instance id", instance_id)?;
    require_safe_name("mod file name", file_name)?;
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
// Loadouts (named enable/disable mod sets)
// ---------------------------------------------------------------------------

/// Stable identity for a mod across version updates: the tracked project id
/// when the content index knows the file, otherwise the versionless part of
/// the file name.
fn loadout_key(entry: &ModEntry) -> String {
    entry
        .project_id
        .clone()
        .unwrap_or_else(|| crate::tools::mod_base_key(&entry.file_name))
}

/// Snapshot the instance's current enabled/disabled mod state into a named
/// loadout, creating it or overwriting an existing one. Returns the updated
/// instance.
pub fn save_loadout(state: &AppState, instance_id: &str, name: &str) -> Result<Instance> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::Other("Loadout name can't be empty.".into()));
    }
    let disabled: Vec<String> = list_mods(state, instance_id)
        .iter()
        .filter(|m| !m.enabled)
        .map(loadout_key)
        .collect();
    let mut inst = get_instance(state, instance_id)?;
    if let Some(existing) = inst.loadouts.iter_mut().find(|l| l.name == name) {
        existing.disabled = disabled;
    } else {
        inst.loadouts.push(Loadout { name: name.to_string(), disabled });
    }
    save_instance(state, &inst)?;
    Ok(inst)
}

/// Enable/disable every mod to match the named loadout. Mods added after the
/// loadout was saved aren't in its `disabled` list, so they end up enabled.
/// Returns how many mods actually changed state.
pub fn apply_loadout(state: &AppState, instance_id: &str, name: &str) -> Result<u32> {
    let inst = get_instance(state, instance_id)?;
    let loadout = inst
        .loadouts
        .iter()
        .find(|l| l.name == name)
        .ok_or_else(|| Error::NotFound(format!("loadout {name}")))?;
    let disabled: std::collections::HashSet<&str> =
        loadout.disabled.iter().map(|s| s.as_str()).collect();

    let mut changed = 0;
    for m in list_mods(state, instance_id) {
        let want_enabled = !disabled.contains(loadout_key(&m).as_str());
        if m.enabled != want_enabled {
            set_mod_enabled(state, instance_id, &m.file_name, want_enabled)?;
            changed += 1;
        }
    }
    Ok(changed)
}

/// Remove a named loadout. Mods on disk are left exactly as they are.
pub fn delete_loadout(state: &AppState, instance_id: &str, name: &str) -> Result<Instance> {
    let mut inst = get_instance(state, instance_id)?;
    inst.loadouts.retain(|l| l.name != name);
    save_instance(state, &inst)?;
    Ok(inst)
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
    out.sort_by_key(|a| a.file_name.to_lowercase());
    out
}

pub fn delete_resource_pack(state: &AppState, instance_id: &str, file_name: &str) -> Result<()> {
    require_safe_name("instance id", instance_id)?;
    require_safe_name("resource pack file name", file_name)?;
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
    out.sort_by_key(|a| a.file_name.to_lowercase());
    out
}

pub fn delete_shader(state: &AppState, instance_id: &str, file_name: &str) -> Result<()> {
    require_safe_name("instance id", instance_id)?;
    require_safe_name("shader file name", file_name)?;
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
    pub size: u64,
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
                let size = crate::tools::dir_size(&e.path());
                out.push(WorldEntry { name, modified, size });
            }
        }
    }
    out.sort_by_key(|w| std::cmp::Reverse(w.modified));
    out
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
    out.sort_by_key(|s| std::cmp::Reverse(s.taken_at));
    out
}

#[cfg(test)]
mod account_migration_tests {
    use super::*;

    #[test]
    fn detects_inline_tokens_in_legacy_json() {
        let json = r#"{
            "accounts":[{
                "id":"abc",
                "username":"Steve",
                "accessToken":"secret",
                "refreshToken":"refresh",
                "expiresAt":123,
                "type":"microsoft"
            }],
            "active":"abc"
        }"#;
        let legacy: LegacyAccountStore = serde_json::from_str(json).unwrap();
        assert!(legacy_has_inline_tokens(&legacy));
    }

    #[test]
    fn metadata_only_json_has_no_inline_tokens() {
        let json = r#"{
            "accounts":[{
                "id":"abc",
                "username":"Steve",
                "expiresAt":123,
                "type":"microsoft"
            }],
            "active":"abc"
        }"#;
        let legacy: LegacyAccountStore = serde_json::from_str(json).unwrap();
        assert!(!legacy_has_inline_tokens(&legacy));
    }

    #[test]
    fn loadout_key_prefers_project_id() {
        let m = ModEntry {
            file_name: "sodium-fabric-0.5.8.jar".into(),
            enabled: true,
            size: 0,
            project_id: Some("AANobbMI".into()),
        };
        assert_eq!(loadout_key(&m), "AANobbMI");
    }

    #[test]
    fn loadout_key_falls_back_to_versionless_file_key() {
        let m = ModEntry {
            file_name: "sodium-fabric-0.5.8.jar".into(),
            enabled: false,
            size: 0,
            project_id: None,
        };
        assert_eq!(loadout_key(&m), "sodium");
    }

    #[test]
    fn legacy_to_stored_omits_secrets() {
        let leg = LegacyAccount {
            id: "x".into(),
            username: "Steve".into(),
            access_token: "secret".into(),
            refresh_token: Some("refresh".into()),
            expires_at: 1,
            xuid: None,
            kind: "microsoft".into(),
        };
        let stored = legacy_to_stored(&leg);
        assert_eq!(stored.id, "x");
        assert_eq!(stored.username, "Steve");
        assert_eq!(stored.kind, "microsoft");
    }
}

#[cfg(test)]
mod index_tests {
    use super::*;
    use crate::state::AppState;

    fn temp_state() -> (AppState, std::path::PathBuf) {
        let root = std::env::temp_dir().join(format!("ezmapa_idx_test_{}", uuid::Uuid::new_v4()));
        (AppState::new(root.clone()), root)
    }

    fn sample_modrinth_ref() -> crate::crosssource::ModrinthRef {
        crate::crosssource::ModrinthRef {
            project_id: "AANobbMI".into(),
            version_id: "ver1".into(),
            url: "https://cdn.modrinth.com/x.jar".into(),
            filename: "x.jar".into(),
            sha1: "a".repeat(40),
            sha512: None,
            size: 1,
        }
    }

    #[test]
    fn fallback_survives_reload_and_keeps_cf_identity() {
        let (state, root) = temp_state();
        let id = "inst1";
        std::fs::create_dir_all(state.dirs.instance_dir(id)).unwrap();

        record_installs(&state, id, &[("blocked.jar".into(), "238222".into())], "curseforge");
        record_modrinth_fallback(&state, id, "blocked.jar", &sample_modrinth_ref());

        let idx = load_index(&state, id);
        let item = idx.items.get("blocked.jar").expect("tracked");
        // The primary identity stays CurseForge...
        assert_eq!(item.provider, "curseforge");
        assert_eq!(item.project_id, "238222");
        // ...while the fallback records where the bytes came from.
        let fb = item.fallback.as_ref().expect("fallback recorded");
        assert_eq!(fb.provider, "modrinth");
        assert_eq!(fb.project_id, "AANobbMI");
        assert_eq!(fb.version_id, "ver1");

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn fallback_on_untracked_file_creates_entry() {
        let (state, root) = temp_state();
        let id = "inst2";
        std::fs::create_dir_all(state.dirs.instance_dir(id)).unwrap();

        record_modrinth_fallback(&state, id, "orphan.jar", &sample_modrinth_ref());
        let idx = load_index(&state, id);
        let item = idx.items.get("orphan.jar").expect("created");
        assert_eq!(item.provider, "curseforge");
        assert!(item.project_id.is_empty());
        assert!(item.fallback.is_some());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn legacy_index_entries_without_fallback_still_parse() {
        let json = r#"{
            "items": {
                "sodium.jar": { "project_id": "AANobbMI", "provider": "modrinth" }
            }
        }"#;
        let idx: ContentIndex = serde_json::from_str(json).unwrap();
        let item = idx.items.get("sodium.jar").unwrap();
        assert_eq!(item.project_id, "AANobbMI");
        assert!(item.fallback.is_none());
    }

    #[test]
    fn record_installs_resets_fallback_on_reinstall() {
        // Re-installing a file through the normal CF path replaces the entry,
        // clearing a stale fallback marker from a previous blocked install.
        let (state, root) = temp_state();
        let id = "inst3";
        std::fs::create_dir_all(state.dirs.instance_dir(id)).unwrap();

        record_modrinth_fallback(&state, id, "m.jar", &sample_modrinth_ref());
        record_installs(&state, id, &[("m.jar".into(), "42".into())], "curseforge");
        let idx = load_index(&state, id);
        assert!(idx.items.get("m.jar").unwrap().fallback.is_none());

        std::fs::remove_dir_all(&root).ok();
    }
}
