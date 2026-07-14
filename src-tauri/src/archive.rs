//! Safe extraction helpers for untrusted zip archives (modpacks, snapshots).
//!
//! Archive entry names are attacker-controlled, so joining them straight onto a
//! destination directory enables "zip slip": an entry named `../../evil` (or an
//! absolute path) escapes the intended folder and can overwrite arbitrary files.
//! [`safe_join`] rejects any entry that would resolve outside `base`.

use std::path::{Component, Path, PathBuf};

/// Join an untrusted archive-relative path onto `base`, returning `None` when
/// the entry would escape `base` via a parent (`..`) segment, an absolute root,
/// or a Windows drive prefix. Only plain file/directory name components are
/// kept; a leading `.` is ignored.
///
/// The returned path is always inside `base`, so callers can extract to it
/// without a further containment check.
pub fn safe_join(base: &Path, relative: &str) -> Option<PathBuf> {
    let mut out = base.to_path_buf();
    for comp in Path::new(relative).components() {
        match comp {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(out)
}

/// True when `name` is a single plain path component — no `..`, no separators,
/// no absolute root or Windows drive prefix, not empty.
///
/// Used to validate webview-supplied identifiers (instance ids, world names,
/// mod/pack/snapshot file names) before they are joined onto data directories,
/// so no Tauri command can be steered outside the folder it operates in.
pub fn is_safe_name(name: &str) -> bool {
    let mut comps = Path::new(name).components();
    matches!(
        (comps.next(), comps.next()),
        (Some(Component::Normal(_)), None)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_names_are_single_plain_components() {
        assert!(is_safe_name("survival-abc12345"));
        assert!(is_safe_name("sodium-0.5.8.jar"));
        assert!(is_safe_name("My World"));
        assert!(is_safe_name("..jar")); // odd but harmless: one normal component
    }

    #[test]
    fn unsafe_names_are_rejected() {
        assert!(!is_safe_name(""));
        assert!(!is_safe_name("."));
        assert!(!is_safe_name(".."));
        assert!(!is_safe_name("../evil"));
        assert!(!is_safe_name("a/b"));
        assert!(!is_safe_name("/etc/passwd"));
        assert!(!is_safe_name("saves/.."));
        #[cfg(windows)]
        {
            assert!(!is_safe_name("a\\b"));
            assert!(!is_safe_name("C:\\evil"));
        }
    }

    #[test]
    fn keeps_normal_nested_paths() {
        let base = Path::new("base/inst");
        assert_eq!(
            safe_join(base, "config/mod.toml"),
            Some(Path::new("base/inst/config/mod.toml").to_path_buf())
        );
    }

    #[test]
    fn ignores_current_dir_segments() {
        let base = Path::new("base/inst");
        assert_eq!(
            safe_join(base, "./mods/a.jar"),
            Some(Path::new("base/inst/mods/a.jar").to_path_buf())
        );
    }

    #[test]
    fn rejects_parent_traversal() {
        let base = Path::new("base/inst");
        assert_eq!(safe_join(base, "../evil.txt"), None);
        assert_eq!(safe_join(base, "mods/../../evil.txt"), None);
    }

    #[test]
    fn rejects_absolute_paths() {
        let base = Path::new("base/inst");
        assert_eq!(safe_join(base, "/etc/passwd"), None);
    }
}
