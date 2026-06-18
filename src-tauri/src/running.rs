//! Tracks which instances have a live game process and persists that map to
//! disk so a Beacon restart can still see games launched before the crash.

use crate::error::{Error, Result};
use crate::state::AppDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct RunningFile {
    #[serde(default)]
    instances: HashMap<String, u32>,
}

fn running_file(dirs: &AppDirs) -> std::path::PathBuf {
    dirs.root.join("running.json")
}

fn read_file(path: &Path) -> RunningFile {
    std::fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn write_file(path: &Path, data: &RunningFile) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_vec_pretty(data)?)?;
    Ok(())
}

/// True when the OS reports `pid` as a live process.
pub fn pid_alive(pid: u32) -> bool {
  if pid == 0 {
    return false;
  }
  #[cfg(windows)]
  {
    #[link(name = "kernel32")]
    extern "system" {
      fn OpenProcess(dw_desired_access: u32, b_inherit_handle: i32, dw_process_id: u32) -> isize;
      fn CloseHandle(h: isize) -> i32;
    }
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    unsafe {
      let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
      if h == 0 {
        return false;
      }
      CloseHandle(h);
      true
    }
  }
  #[cfg(unix)]
  {
    unsafe { libc::kill(pid as i32, 0) == 0 }
  }
}

/// Load persisted running instances, dropping entries whose process has exited.
pub fn load_reconciled(dirs: &AppDirs) -> HashMap<String, u32> {
    let path = running_file(dirs);
    let file = read_file(&path);
    let alive: HashMap<String, u32> = file
        .instances
        .into_iter()
        .filter(|(_, pid)| pid_alive(*pid))
        .collect();
    let _ = write_file(&path, &RunningFile { instances: alive.clone() });
    alive
}

/// Mark an instance as running and flush to disk.
pub fn mark_running(
    dirs: &AppDirs,
    memory: &Arc<Mutex<HashMap<String, u32>>>,
    instance_id: &str,
    pid: u32,
) {
    if let Ok(mut map) = memory.lock() {
        map.insert(instance_id.to_string(), pid);
        let _ = write_file(&running_file(dirs), &RunningFile { instances: map.clone() });
    }
}

/// Mark an instance as stopped and flush to disk.
pub fn mark_stopped(dirs: &AppDirs, memory: &Arc<Mutex<HashMap<String, u32>>>, instance_id: &str) {
    if let Ok(mut map) = memory.lock() {
        map.remove(instance_id);
        let _ = write_file(&running_file(dirs), &RunningFile { instances: map.clone() });
    }
}

/// Refuse deletion when the instance still has a live game process.
pub fn ensure_not_running(
    memory: &Arc<Mutex<HashMap<String, u32>>>,
    instance_id: &str,
) -> Result<()> {
    let map = memory.lock().map_err(|_| Error::Other("state lock poisoned".into()))?;
    if map.contains_key(instance_id) {
        return Err(Error::Other(
            "This instance is still running. Stop it before deleting.".into(),
        ));
    }
    Ok(())
}
