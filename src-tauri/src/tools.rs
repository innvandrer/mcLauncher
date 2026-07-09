//! Instance maintenance tools: disk-usage breakdown, world snapshots (zip
//! backups of `saves/`), and a lightweight mod-conflict scan. Each is exposed
//! as a Tauri command in `commands.rs`.

use crate::error::{Error, Result};
use crate::state::AppState;
use serde::Serialize;
use std::io::{Read, Write};
use std::path::Path;

// ---------------------------------------------------------------------------
// Disk usage
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskUsage {
    pub total: u64,
    pub mods: u64,
    pub saves: u64,
    pub resourcepacks: u64,
    pub shaders: u64,
    pub other: u64,
}

/// Recursively sum the byte size of every file under `path`.
fn dir_size(path: &Path) -> u64 {
    let mut total = 0;
    if let Ok(rd) = std::fs::read_dir(path) {
        for entry in rd.flatten() {
            let p = entry.path();
            match entry.file_type() {
                Ok(ft) if ft.is_dir() => total += dir_size(&p),
                Ok(ft) if ft.is_file() => {
                    total += entry.metadata().map(|m| m.len()).unwrap_or(0);
                }
                _ => {}
            }
        }
    }
    total
}

pub fn instance_disk_usage(state: &AppState, instance_id: &str) -> DiskUsage {
    let game = state.dirs.game_dir(instance_id);
    // Walk the game directory once: size each top-level entry, attribute the
    // known content folders, and fold everything else into `other`. (The old
    // version walked mods/saves/resourcepacks/shaders and then the whole tree
    // again for the total.)
    let mut usage = DiskUsage::default();
    if let Ok(rd) = std::fs::read_dir(&game) {
        for entry in rd.flatten() {
            let path = entry.path();
            let size = if path.is_dir() {
                dir_size(&path)
            } else {
                entry.metadata().map(|m| m.len()).unwrap_or(0)
            };
            usage.total += size;
            match entry.file_name().to_string_lossy().as_ref() {
                "mods" => usage.mods = size,
                "saves" => usage.saves = size,
                "resourcepacks" => usage.resourcepacks = size,
                "shaderpacks" => usage.shaders = size,
                _ => usage.other += size,
            }
        }
    }
    usage
}

// ---------------------------------------------------------------------------
// World snapshots
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub file_name: String,
    pub world: String,
    pub size: u64,
    pub created: i64,
}

fn snapshots_dir(state: &AppState, instance_id: &str) -> std::path::PathBuf {
    state.dirs.instance_dir(instance_id).join("snapshots")
}

pub fn list_snapshots(state: &AppState, instance_id: &str) -> Vec<Snapshot> {
    let dir = snapshots_dir(state, instance_id);
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !name.ends_with(".zip") {
                continue;
            }
            let meta = e.metadata().ok();
            let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
            let created = meta
                .and_then(|m| m.modified().ok())
                .map(|t| {
                    t.duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64
                })
                .unwrap_or(0);
            // File name is "<world>__<timestamp>.zip" — recover the world label.
            let world = name
                .rsplit_once("__")
                .map(|(w, _)| w.to_string())
                .unwrap_or_else(|| name.trim_end_matches(".zip").to_string());
            out.push(Snapshot { file_name: name, world, size, created });
        }
    }
    out.sort_by(|a, b| b.created.cmp(&a.created));
    out
}

/// Zip up a single world folder under `saves/` into the snapshots directory.
pub fn create_snapshot(state: &AppState, instance_id: &str, world: &str) -> Result<Snapshot> {
    let world_dir = state.dirs.game_dir(instance_id).join("saves").join(world);
    if !world_dir.is_dir() {
        return Err(Error::Other(format!("World “{world}” not found.")));
    }
    let dir = snapshots_dir(state, instance_id);
    std::fs::create_dir_all(&dir)?;
    let stamp = chrono::Utc::now().timestamp();
    // Sanitize the world name for use in a file name.
    let safe: String = world
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let file_name = format!("{safe}__{stamp}.zip");
    let zip_path = dir.join(&file_name);

    let file = std::fs::File::create(&zip_path)?;
    let mut zip = zip::ZipWriter::new(file);
    let opts: zip::write::FileOptions<()> =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    zip_dir_recursive(&mut zip, &world_dir, &world_dir, world, &opts)?;
    zip.finish()?;

    let size = std::fs::metadata(&zip_path).map(|m| m.len()).unwrap_or(0);
    Ok(Snapshot { file_name, world: world.to_string(), size, created: stamp })
}

fn zip_dir_recursive<W: Write + std::io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    base: &Path,
    dir: &Path,
    prefix: &str,
    opts: &zip::write::FileOptions<()>,
) -> Result<()> {
    for entry in std::fs::read_dir(dir)?.flatten() {
        let path = entry.path();
        let rel = path.strip_prefix(base).unwrap_or(&path);
        let name = format!("{prefix}/{}", rel.to_string_lossy().replace('\\', "/"));
        if path.is_dir() {
            zip_dir_recursive(zip, base, &path, prefix, opts)?;
        } else if path.is_file() {
            zip.start_file(name, *opts)?;
            let mut f = std::fs::File::open(&path)?;
            let mut buf = Vec::new();
            f.read_to_end(&mut buf)?;
            zip.write_all(&buf)?;
        }
    }
    Ok(())
}

/// Restore a snapshot, overwriting the current world folder of the same name.
pub fn restore_snapshot(state: &AppState, instance_id: &str, file_name: &str) -> Result<()> {
    let zip_path = snapshots_dir(state, instance_id).join(file_name);
    if !zip_path.is_file() {
        return Err(Error::Other("Snapshot not found.".into()));
    }
    let saves = state.dirs.game_dir(instance_id).join("saves");

    let file = std::fs::File::open(&zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    // The first path component is the world folder — wipe it before extracting
    // so removed files don't linger. `safe_join` keeps the target inside `saves`
    // even if the snapshot's first entry is a `..` path, and the non-equal check
    // stops a `./`-prefixed entry from wiping the whole saves folder.
    if let Some(world) = archive.by_index(0).ok().and_then(|f| {
        f.name().split('/').next().map(|s| s.to_string())
    }) {
        if let Some(target) = crate::archive::safe_join(&saves, &world) {
            if target != saves && target.is_dir() {
                std::fs::remove_dir_all(&target).ok();
            }
        }
    }

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        // Reject entries whose name would escape the saves dir (zip slip).
        let Some(out) = crate::archive::safe_join(&saves, entry.name()) else {
            continue;
        };
        if entry.is_dir() {
            std::fs::create_dir_all(&out)?;
        } else {
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf)?;
            std::fs::write(&out, buf)?;
        }
    }
    Ok(())
}

pub fn delete_snapshot(state: &AppState, instance_id: &str, file_name: &str) -> Result<()> {
    let zip_path = snapshots_dir(state, instance_id).join(file_name);
    if zip_path.is_file() {
        std::fs::remove_file(&zip_path)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Mod conflict scan
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModConflict {
    /// Shared base name (e.g. "sodium") with multiple installed jars.
    pub name: String,
    pub files: Vec<String>,
}

/// Strip a version-ish suffix from a mod jar's file name so two builds of the
/// same mod collapse to the same key. Heuristic, but catches the common case of
/// installing a mod twice at different versions.
pub fn mod_base_key(file: &str) -> String {
    let stem = file
        .trim_end_matches(".jar")
        .trim_end_matches(".disabled")
        .trim_end_matches(".jar");
    // Cut at the first segment that starts with a digit (usually the version).
    let mut key = String::new();
    for (i, part) in stem.split(['-', '_', '+']).enumerate() {
        let starts_digit = part.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false);
        if i > 0 && (starts_digit || part.eq_ignore_ascii_case("fabric") || part.eq_ignore_ascii_case("forge")) {
            break;
        }
        if !key.is_empty() {
            key.push('-');
        }
        key.push_str(&part.to_lowercase());
    }
    if key.is_empty() {
        stem.to_lowercase()
    } else {
        key
    }
}

pub fn scan_mod_conflicts(state: &AppState, instance_id: &str) -> Vec<ModConflict> {
    let dir = state.dirs.game_dir(instance_id).join("mods");
    let mut groups: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !(name.ends_with(".jar") || name.ends_with(".jar.disabled")) {
                continue;
            }
            groups.entry(mod_base_key(&name)).or_default().push(name);
        }
    }
    let mut out: Vec<ModConflict> = groups
        .into_iter()
        .filter(|(_, files)| files.len() > 1)
        .map(|(name, mut files)| {
            files.sort();
            ModConflict { name, files }
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Version-aware ("natural") filename comparison so `mod-1.10.jar` sorts after
/// `mod-1.9.jar`. Compares digit runs numerically, everything else lexically.
fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let av: Vec<char> = a.to_lowercase().chars().collect();
    let bv: Vec<char> = b.to_lowercase().chars().collect();
    let (mut i, mut j) = (0usize, 0usize);
    while i < av.len() && j < bv.len() {
        let (ca, cb) = (av[i], bv[j]);
        if ca.is_ascii_digit() && cb.is_ascii_digit() {
            let si = i;
            while i < av.len() && av[i].is_ascii_digit() {
                i += 1;
            }
            let sj = j;
            while j < bv.len() && bv[j].is_ascii_digit() {
                j += 1;
            }
            let na: String = av[si..i].iter().collect();
            let nb: String = bv[sj..j].iter().collect();
            let na_t = na.trim_start_matches('0');
            let nb_t = nb.trim_start_matches('0');
            let ord = na_t.len().cmp(&nb_t.len()).then_with(|| na_t.cmp(nb_t));
            if ord != Ordering::Equal {
                return ord;
            }
        } else {
            if ca != cb {
                return ca.cmp(&cb);
            }
            i += 1;
            j += 1;
        }
    }
    av.len().cmp(&bv.len())
}

/// For every duplicate group, keep the newest-versioned jar and delete the rest.
/// Returns the file names that were removed. Triggered explicitly by the user.
pub fn resolve_mod_conflicts(state: &AppState, instance_id: &str) -> Vec<String> {
    let mut removed = Vec::new();
    for c in scan_mod_conflicts(state, instance_id) {
        let mut files = c.files.clone();
        files.sort_by(|a, b| natural_cmp(a, b));
        // The greatest by natural order is the newest version — keep it.
        let keep = match files.last() {
            Some(k) => k.clone(),
            None => continue,
        };
        let keep_display = keep.trim_end_matches(".disabled").to_string();
        let mut seen = std::collections::HashSet::new();
        for f in &files {
            if *f == keep {
                continue;
            }
            let display = f.trim_end_matches(".disabled").to_string();
            // Never delete via a name that also matches the kept file.
            if display == keep_display || !seen.insert(display.clone()) {
                continue;
            }
            if crate::instances::delete_mod(state, instance_id, &display).is_ok() {
                removed.push(f.clone());
            }
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_ordering() {
        assert!(natural_cmp("mod-1.9.jar", "mod-1.10.jar").is_lt());
        assert!(natural_cmp("cc-1.117.1.jar", "cc-1.113.1.jar").is_gt());
        assert!(natural_cmp("a-1.0.jar", "a-1.0.jar").is_eq());
    }

    #[test]
    fn base_key_collapses_versions() {
        assert_eq!(
            mod_base_key("sodium-fabric-0.5.3.jar"),
            mod_base_key("sodium-fabric-0.6.0.jar"),
        );
        assert_eq!(
            mod_base_key("cc-tweaked-1.21.1-forge-1.113.1.jar"),
            mod_base_key("cc-tweaked-1.21.1-forge-1.117.1.jar"),
        );
    }
}
