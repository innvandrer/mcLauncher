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

/// A signed-in account. Tokens are persisted locally but never sent to the
/// frontend (see [`PublicAccount`]).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
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
    #[serde(rename = "type", default = "default_account_type")]
    pub kind: String,
}

fn default_account_type() -> String {
    "microsoft".to_string()
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

impl Account {
    pub fn to_public(&self, active: bool) -> PublicAccount {
        PublicAccount {
            id: self.id.clone(),
            username: self.username.clone(),
            kind: self.kind.clone(),
            active,
        }
    }
}

/// Stored on disk in `accounts.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountStore {
    #[serde(default)]
    pub accounts: Vec<Account>,
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
