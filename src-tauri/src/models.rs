use serde::{Deserialize, Serialize};

/// A mod loader supported by the launcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Loader {
    Vanilla,
    Fabric,
    Quilt,
    Forge,
    Neoforge,
}

impl Loader {
    pub fn as_str(&self) -> &'static str {
        match self {
            Loader::Vanilla => "vanilla",
            Loader::Fabric => "fabric",
            Loader::Quilt => "quilt",
            Loader::Forge => "forge",
            Loader::Neoforge => "neoforge",
        }
    }
    /// The identifier Modrinth uses for this loader in its facets.
    pub fn modrinth_id(&self) -> &'static str {
        self.as_str()
    }
}

impl Default for Loader {
    fn default() -> Self {
        Loader::Vanilla
    }
}

/// A user-created instance: an isolated Minecraft installation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub mc_version: String,
    #[serde(default)]
    pub loader: Loader,
    #[serde(default)]
    pub loader_version: Option<String>,
    /// Emoji or short string used as the card icon.
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub group: Option<String>,
    /// Accent colour key for this instance's card (falls back to the loader
    /// theme when `None`). One of the keys in the frontend ACCENTS map.
    #[serde(default)]
    pub accent: Option<String>,
    /// Pinned to the top of the instances list.
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub created: i64,
    #[serde(default)]
    pub last_played: Option<i64>,
    #[serde(default)]
    pub total_play_seconds: u64,
    /// Per-instance overrides; fall back to global settings when `None`.
    #[serde(default)]
    pub memory_mb: Option<u32>,
    #[serde(default)]
    pub java_path: Option<String>,
    #[serde(default)]
    pub jvm_args: Option<String>,
    /// Game window size — both must be set to pass `--width`/`--height`.
    #[serde(default)]
    pub window_width: Option<u32>,
    #[serde(default)]
    pub window_height: Option<u32>,
    /// Newline-separated `KEY=VALUE` pairs set on the game process environment.
    #[serde(default)]
    pub env_vars: Option<String>,
    /// Shell command run before launch (blocking).
    #[serde(default)]
    pub pre_launch: Option<String>,
    /// Shell command run after the game exits.
    #[serde(default)]
    pub post_exit: Option<String>,
    /// Origin of a modpack-derived instance, used to offer updates with a diff.
    #[serde(default)]
    pub pack_source: Option<PackSource>,
    /// Number of .jar files in the mods/ folder. Computed at list time, never read from disk.
    #[serde(default, skip_deserializing)]
    pub mod_count: u32,
}

/// Where a modpack instance came from, so we can check for newer versions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackSource {
    /// "modrinth" or "curseforge".
    pub provider: String,
    pub project_id: String,
    #[serde(default)]
    pub version_id: Option<String>,
    #[serde(default)]
    pub version_name: Option<String>,
}

/// Global launcher settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default = "default_memory")]
    pub memory_mb: u32,
    #[serde(default)]
    pub java_path: Option<String>,
    #[serde(default = "default_jvm_args")]
    pub jvm_args: String,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_accent")]
    pub accent: String,
    #[serde(default = "default_concurrency")]
    pub max_concurrent_downloads: usize,
    #[serde(default)]
    pub close_on_launch: bool,
    /// When enabled, check Modrinth for newer mods/resource packs/shaders before
    /// launch and when opening an instance.
    #[serde(default)]
    pub auto_update_content: bool,
    /// Optional CurseForge Core API key. Falls back to `EZMAPA_CF_API_KEY` (or
    /// legacy `BEACON_CF_API_KEY`) when empty. Required for CurseForge search/installs.
    #[serde(default)]
    pub curseforge_api_key: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            memory_mb: default_memory(),
            java_path: None,
            jvm_args: default_jvm_args(),
            theme: default_theme(),
            accent: default_accent(),
            max_concurrent_downloads: default_concurrency(),
            close_on_launch: false,
            auto_update_content: false,
            curseforge_api_key: None,
        }
    }
}

fn default_memory() -> u32 {
    4096
}
fn default_jvm_args() -> String {
    "-XX:+UnlockExperimentalVMOptions -XX:+UseG1GC -XX:G1NewSizePercent=20 -XX:G1ReservePercent=20 -XX:MaxGCPauseMillis=50 -XX:G1HeapRegionSize=32M".to_string()
}
fn default_theme() -> String {
    "dark".to_string()
}
fn default_accent() -> String {
    "violet".to_string()
}
fn default_concurrency() -> usize {
    8
}

fn default_account_type() -> String {
    "microsoft".to_string()
}

/// Non-secret account metadata persisted in `accounts.json`. Tokens live in the
/// OS keyring (see `account_tokens`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredAccount {
    pub id: String,
    pub username: String,
    #[serde(default)]
    pub expires_at: i64,
    #[serde(default)]
    pub xuid: Option<String>,
    #[serde(rename = "type", default = "default_account_type")]
    pub kind: String,
}

/// Full in-memory account including tokens loaded from secure storage. Never
/// serialized to disk or sent to the frontend (see [`PublicAccount`]).
#[derive(Debug, Clone)]
pub struct Account {
    pub id: String,
    pub username: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: i64,
    pub xuid: Option<String>,
    pub kind: String,
}

impl StoredAccount {
    pub fn to_public(&self, active: bool) -> PublicAccount {
        PublicAccount {
            id: self.id.clone(),
            username: self.username.clone(),
            kind: self.kind.clone(),
            active,
        }
    }
}

impl Account {
    pub fn from_stored(
        stored: StoredAccount,
        access_token: String,
        refresh_token: Option<String>,
    ) -> Self {
        Self {
            id: stored.id,
            username: stored.username,
            access_token,
            refresh_token,
            expires_at: stored.expires_at,
            xuid: stored.xuid,
            kind: stored.kind,
        }
    }

    pub fn into_stored(self) -> StoredAccount {
        StoredAccount {
            id: self.id,
            username: self.username,
            expires_at: self.expires_at,
            xuid: self.xuid,
            kind: self.kind,
        }
    }
}

/// Account info safe to send to the frontend (no tokens).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicAccount {
    pub id: String,
    pub username: String,
    pub kind: String,
    pub active: bool,
}

/// Stored on disk in `accounts.json` (metadata only — no tokens).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountStore {
    #[serde(default)]
    pub accounts: Vec<StoredAccount>,
    #[serde(default)]
    pub active: Option<String>,
}

/// A progress update emitted on the `task://progress` event.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskProgress {
    pub id: String,
    pub label: String,
    pub stage: String,
    pub current: u64,
    pub total: u64,
    pub done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Total bytes downloaded so far across the task (when known).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_done: Option<u64>,
    /// Current download speed in bytes/second (rolling).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<u64>,
    /// Name of the file currently being fetched (for detail panels).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_file: Option<String>,
}

impl TaskProgress {
    /// A bare progress update without byte-level detail (used by staged tasks
    /// like Forge install and version resolution).
    pub fn simple(
        id: &str,
        label: &str,
        stage: &str,
        current: u64,
        total: u64,
        done: bool,
        error: Option<String>,
    ) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            stage: stage.to_string(),
            current,
            total,
            done,
            error,
            bytes_done: None,
            speed: None,
            current_file: None,
        }
    }
}

/// A line of output emitted on the `instance://log` event.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogLine {
    pub instance_id: String,
    pub line: String,
    pub is_err: bool,
}

/// Emitted on `instance://state` when an instance starts or stops.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceState {
    pub instance_id: String,
    pub running: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
}
