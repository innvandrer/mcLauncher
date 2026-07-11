use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use crate::discord::DiscordPresence;

/// Resolves the on-disk layout for all launcher data. Instances each get their
/// own folder, while assets, libraries and version metadata are shared across
/// instances (mirroring how Prism/MultiMC avoid re-downloading shared files).
#[derive(Clone)]
pub struct AppDirs {
    pub root: PathBuf,
}

impl AppDirs {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn instances(&self) -> PathBuf {
        self.root.join("instances")
    }
    pub fn libraries(&self) -> PathBuf {
        self.root.join("libraries")
    }
    pub fn assets(&self) -> PathBuf {
        self.root.join("assets")
    }
    pub fn versions(&self) -> PathBuf {
        self.root.join("versions")
    }
    pub fn java(&self) -> PathBuf {
        self.root.join("java")
    }
    pub fn natives(&self) -> PathBuf {
        self.root.join("natives")
    }

    pub fn settings_file(&self) -> PathBuf {
        self.root.join("settings.json")
    }
    /// Cross-source resolution cache (sha1 → Modrinth/CurseForge identity).
    pub fn crosssource_cache_file(&self) -> PathBuf {
        self.root.join("crosssource_cache.json")
    }
    pub fn accounts_file(&self) -> PathBuf {
        self.root.join("accounts.json")
    }
    pub fn sessions_file(&self) -> PathBuf {
        self.root.join("sessions.json")
    }

    pub fn instance_dir(&self, id: &str) -> PathBuf {
        self.instances().join(id)
    }
    pub fn instance_manifest(&self, id: &str) -> PathBuf {
        self.instance_dir(id).join("instance.json")
    }
    /// The Minecraft game directory (the equivalent of `.minecraft`).
    pub fn game_dir(&self, id: &str) -> PathBuf {
        self.instance_dir(id).join("minecraft")
    }
    pub fn version_dir(&self, version_id: &str) -> PathBuf {
        self.versions().join(version_id)
    }
    pub fn version_json(&self, version_id: &str) -> PathBuf {
        self.version_dir(version_id).join(format!("{version_id}.json"))
    }
    pub fn version_jar(&self, version_id: &str) -> PathBuf {
        self.version_dir(version_id).join(format!("{version_id}.jar"))
    }

    pub fn ensure(&self) -> std::io::Result<()> {
        for p in [
            self.instances(),
            self.libraries(),
            self.assets().join("indexes"),
            self.assets().join("objects"),
            self.versions(),
            self.java(),
            self.natives(),
        ] {
            std::fs::create_dir_all(p)?;
        }
        Ok(())
    }
}

/// Shared application state, managed by Tauri and injected into commands.
pub struct AppState {
    pub http: reqwest::Client,
    pub dirs: AppDirs,
    /// Maps an instance id to the OS pid of its running game process. Wrapped in
    /// an `Arc<Mutex<_>>` so the process-watcher threads can update it on exit.
    pub running: Arc<Mutex<HashMap<String, u32>>>,
    pub discord: DiscordPresence,
    /// Cancellation flags for in-flight download tasks, keyed by task id. Set to
    /// `true` to ask `net::download_many` to stop after the current file.
    pub cancels: Arc<Mutex<HashMap<String, Arc<std::sync::atomic::AtomicBool>>>>,
    /// Serializes read-modify-write access to `accounts.json` so concurrent
    /// commands (e.g. a token refresh racing a `set_active_account`) can't clobber
    /// each other's update.
    pub accounts_lock: Mutex<()>,
    /// In-memory cache of the Mojang version manifest with the time it was
    /// fetched, so a launch (or the create-instance dialog) doesn't re-download
    /// the multi-megabyte manifest on every call. Refreshed past the TTL.
    pub manifest: tokio::sync::RwLock<Option<(std::time::Instant, Arc<crate::mojang::VersionManifest>)>>,
    /// Cross-source (Modrinth ↔ CurseForge) identity resolver with its disk
    /// cache and per-provider rate limiters.
    pub crosssource: crate::crosssource::Resolver,
}

impl AppState {
    /// Get (creating if needed) the cancellation flag for a task.
    pub fn cancel_flag(&self, task_id: &str) -> Arc<std::sync::atomic::AtomicBool> {
        let mut map = self.cancels.lock().unwrap();
        map.entry(task_id.to_string())
            .or_insert_with(|| Arc::new(std::sync::atomic::AtomicBool::new(false)))
            .clone()
    }

    /// Signal cancellation for a task id.
    pub fn cancel_task(&self, task_id: &str) {
        if let Some(flag) = self.cancels.lock().unwrap().get(task_id) {
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }

    /// Drop the cancellation flag once a task is finished.
    pub fn clear_cancel(&self, task_id: &str) {
        self.cancels.lock().unwrap().remove(task_id);
    }
}

impl AppState {
    pub fn new(root: PathBuf) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(concat!(
                "ezmapa-launcher/",
                env!("CARGO_PKG_VERSION"),
                " (https://github.com/innvandrer/mcLauncher)"
            ))
            .build()
            .expect("failed to build HTTP client");

        let dirs = AppDirs::new(root);
        if let Err(e) = dirs.ensure() {
            eprintln!("warning: could not create data directories: {e}");
        }

        let crosssource = crate::crosssource::Resolver::new(dirs.crosssource_cache_file());
        Self {
            http,
            dirs,
            running: Arc::new(Mutex::new(HashMap::new())),
            discord: DiscordPresence::new(),
            cancels: Arc::new(Mutex::new(HashMap::new())),
            accounts_lock: Mutex::new(()),
            manifest: tokio::sync::RwLock::new(None),
            crosssource,
        }
    }
}
