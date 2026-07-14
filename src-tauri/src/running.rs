//! Persistence for "which instance is running", so a launcher restart while
//! Minecraft is still open doesn't lose track of the game process. Without
//! this, `state.running` (purely in-memory) goes empty on every restart: the
//! UI shows the instance as not-running even though the JVM is alive, Stop
//! stops working for it, and nothing stops the user from launching a second
//! copy on top of it.
//!
//! We can't `Child::wait()` on a pid we didn't spawn ourselves (only the
//! process's real parent can) — a launcher restart orphans the JVM to the
//! OS's reaper — so reconciled instances are watched by polling liveness on
//! a background thread instead, the same tradeoff any "adopt an external
//! pid" tool makes.

use crate::models::InstanceState;
use crate::state::{AppDirs, AppState};
use std::collections::HashMap;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct RunningFile {
    #[serde(default)]
    instances: HashMap<String, u32>,
}

fn running_file(dirs: &AppDirs) -> std::path::PathBuf {
    dirs.root.join("running.json")
}

/// Persist the current running map to disk (best-effort — a failed write
/// just means the next restart won't reconcile this particular change).
pub fn save(dirs: &AppDirs, running: &HashMap<String, u32>) {
    let data = RunningFile {
        instances: running.clone(),
    };
    if let Ok(bytes) = serde_json::to_vec_pretty(&data) {
        let _ = crate::instances::atomic_write(&running_file(dirs), &bytes);
    }
}

/// The OS-reported process name for `pid` (lowercased), or empty if the pid
/// doesn't exist.
#[cfg(windows)]
fn process_name(pid: u32) -> String {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .next()
                .map(|s| s.to_lowercase())
        })
        .unwrap_or_default()
}

#[cfg(not(windows))]
fn process_name(pid: u32) -> String {
    std::process::Command::new("ps")
        .args(["-o", "comm=", "-p", &pid.to_string()])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_lowercase())
        .unwrap_or_default()
}

/// True when a process with this pid currently exists.
pub fn pid_alive(pid: u32) -> bool {
    !process_name(pid).is_empty()
}

/// True when `pid` is alive and looks like a Java process. Guards against a
/// pid-reuse collision after a restart: trusting bare liveness alone could
/// reconcile onto a totally unrelated process the OS reassigned the number
/// to, and `stop()` would end up signalling it. Only used at startup
/// reconciliation — once an entry is trusted, later polling only needs
/// [`pid_alive`].
fn looks_like_reconcilable_jvm(pid: u32) -> bool {
    process_name(pid).contains("java")
}

/// On startup, load the last-known running map and re-arm bookkeeping for
/// every pid that's still alive and still looks like a JVM; anything else is
/// dropped. Each survivor gets a lightweight poll thread so the UI and
/// Discord presence still learn about the eventual exit — playtime for the
/// pre-restart portion of the session isn't recoverable across a restart, so
/// it isn't counted for reconciled sessions.
pub fn reconcile_on_startup(app: &AppHandle, state: &AppState) {
    let dirs = state.dirs.clone();
    let Ok(bytes) = std::fs::read(running_file(&dirs)) else {
        return;
    };
    let Ok(saved) = serde_json::from_slice::<RunningFile>(&bytes) else {
        return;
    };
    if saved.instances.is_empty() {
        return;
    }

    let alive: HashMap<String, u32> = saved
        .instances
        .into_iter()
        .filter(|(_, pid)| looks_like_reconcilable_jvm(*pid))
        .collect();

    *state.running.lock().unwrap() = alive.clone();
    save(&dirs, &alive);

    for (id, pid) in alive {
        let app = app.clone();
        let running_map = state.running.clone();
        let discord = state.discord.clone();
        let dirs = dirs.clone();
        std::thread::spawn(move || {
            while pid_alive(pid) {
                std::thread::sleep(Duration::from_secs(3));
            }
            discord.set_idle();
            let remaining = {
                let mut m = running_map.lock().unwrap();
                m.remove(&id);
                m.clone()
            };
            save(&dirs, &remaining);
            let _ = app.emit(
                "instance://state",
                InstanceState {
                    instance_id: id,
                    running: false,
                    exit_code: None,
                },
            );
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_disk() {
        let dirs = AppDirs::new(std::env::temp_dir().join(format!(
            "ezmapa_running_test_{}",
            uuid::Uuid::new_v4()
        )));
        std::fs::create_dir_all(&dirs.root).unwrap();

        let mut map = HashMap::new();
        map.insert("survival-abc123".to_string(), 4242u32);
        save(&dirs, &map);

        let bytes = std::fs::read(running_file(&dirs)).unwrap();
        let loaded: RunningFile = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(loaded.instances.get("survival-abc123"), Some(&4242));

        std::fs::remove_dir_all(&dirs.root).ok();
    }

    #[test]
    fn our_own_pid_is_alive() {
        // The current test process is a real, checkable pid on every OS.
        assert!(pid_alive(std::process::id()));
    }

    #[test]
    fn a_pid_that_cannot_exist_is_not_alive() {
        // 4,294,967,295 (u32::MAX) is never a valid OS pid.
        assert!(!pid_alive(u32::MAX));
    }

    #[test]
    fn empty_or_missing_file_reconciles_to_nothing() {
        let dirs = AppDirs::new(std::env::temp_dir().join(format!(
            "ezmapa_running_test_{}",
            uuid::Uuid::new_v4()
        )));
        // No file at all, and an explicit empty one — both must be silent no-ops
        // (reconcile_on_startup needs an AppHandle it can't get in a unit test,
        // so this only exercises the file-reading half directly).
        assert!(std::fs::read(running_file(&dirs)).is_err());

        std::fs::create_dir_all(&dirs.root).unwrap();
        save(&dirs, &HashMap::new());
        let bytes = std::fs::read(running_file(&dirs)).unwrap();
        let loaded: RunningFile = serde_json::from_slice(&bytes).unwrap();
        assert!(loaded.instances.is_empty());

        std::fs::remove_dir_all(&dirs.root).ok();
    }
}
