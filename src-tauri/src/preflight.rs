//! Preflight Check — a fast, local-only scan that runs right before launch
//! (or whenever the instance page is viewed) and catches the handful of
//! setup mistakes that reliably crash modded Minecraft: a mismatched Java
//! version, a RAM allocation that's obviously wrong, and duplicate mod jars.
//! Everything here is either arithmetic on data already on disk or a static
//! lookup table — no network calls, so it stays instant.

use crate::error::Result;
use crate::state::AppState;
use serde::Serialize;

/// Below this, modern Minecraft reliably runs out of memory on startup.
const ABSOLUTE_MIN_MB: u32 = 1536;
/// Mod count past which a pack is "heavy" enough to need real headroom.
const HEAVY_PACK_MOD_COUNT: u32 = 40;
const HEAVY_PACK_MIN_MB: u32 = 4096;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PreflightAction {
    IncreaseRam,
    LowerRam,
    OpenSettings,
    CleanDuplicates,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreflightWarning {
    pub title: String,
    pub detail: String,
    pub action: PreflightAction,
    /// Only set for the RAM-related actions.
    #[serde(default)]
    pub suggested_memory_mb: Option<u32>,
}

/// Take the leading run of ASCII digits off a version segment (`"2-rc1"` ->
/// `Some(2)`, `"14a"` -> `Some(14)`), so release-candidate/snapshot-flavoured
/// segments still parse instead of failing the whole lookup.
fn leading_number(s: &str) -> Option<u32> {
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// The Java major version Mojang requires for a given release, via the same
/// breakpoints the official launcher uses. Returns `None` for version strings
/// that don't look like a standard `1.x[.y]` release (e.g. snapshots), so
/// callers can skip the check rather than risk a false positive.
fn required_java_major(mc_version: &str) -> Option<u32> {
    let mut segs = mc_version.split('.');
    let major = leading_number(segs.next()?)?;
    if major != 1 {
        return None;
    }
    let minor = segs.next().and_then(leading_number).unwrap_or(0);
    let patch = segs.next().and_then(leading_number).unwrap_or(0);
    Some(if minor > 20 || (minor == 20 && patch >= 5) {
        21
    } else if minor >= 18 {
        17
    } else if minor == 17 {
        16
    } else {
        8
    })
}

/// Run the static checks for one instance. Advisory only — every finding has
/// a one-click fix the frontend can offer, but nothing here blocks launch.
pub async fn preflight_check(state: &AppState, instance_id: &str) -> Result<Vec<PreflightWarning>> {
    let instance = crate::instances::get_instance(state, instance_id)?;
    let settings = crate::instances::load_settings(state);
    let mut warnings = Vec::new();

    let system_mb: u32 = crate::system::total_memory_mb().min(u32::MAX as u64) as u32;
    let effective_memory_mb = instance.memory_mb.unwrap_or(settings.memory_mb);

    // --- Java version -------------------------------------------------------
    if let Some(required) = required_java_major(&instance.mc_version) {
        let configured_path = instance.java_path.clone().or_else(|| settings.java_path.clone());
        if let Some(path) = configured_path.filter(|p| !p.trim().is_empty()) {
            // java::detect_in shells out to candidate `java` binaries, so it
            // goes through spawn_blocking like the detect_java command does.
            let dirs = state.dirs.clone();
            let detected = tokio::task::spawn_blocking(move || crate::java::detect_in(&dirs))
                .await
                .unwrap_or_default();
            if let Some(found) = detected.iter().find(|j| j.path == path) {
                if found.major != required {
                    warnings.push(PreflightWarning {
                        title: "Java version may not match".into(),
                        detail: format!(
                            "Minecraft {} wants Java {required}, but the Java configured for this instance is version {}. Pick a matching Java in Settings.",
                            instance.mc_version, found.major
                        ),
                        action: PreflightAction::OpenSettings,
                        suggested_memory_mb: None,
                    });
                }
            }
            // A custom path outside the detected list can't be version-checked
            // without invoking it, so it's left alone rather than guessed at.
        }
        // No override at all means EZMapa auto-downloads a matching Temurin
        // runtime at launch — nothing to warn about.
    }

    // --- RAM ------------------------------------------------------------------
    let heavy_pack = instance.mod_count > HEAVY_PACK_MOD_COUNT;
    if effective_memory_mb < ABSOLUTE_MIN_MB || (heavy_pack && effective_memory_mb < HEAVY_PACK_MIN_MB) {
        let target = if instance.mod_count > 80 {
            8192
        } else if heavy_pack {
            6144
        } else {
            3072
        };
        let suggested = if system_mb > 0 {
            target.min(system_mb.saturating_sub(1024)).max(2048)
        } else {
            target
        };
        warnings.push(PreflightWarning {
            title: "Memory allocation looks low".into(),
            detail: if heavy_pack {
                format!(
                    "{} mods installed but only {effective_memory_mb} MB allocated. Packs this size usually need 4-8 GB.",
                    instance.mod_count
                )
            } else {
                format!(
                    "Only {effective_memory_mb} MB allocated — modern Minecraft usually needs at least 2-3 GB."
                )
            },
            action: PreflightAction::IncreaseRam,
            suggested_memory_mb: Some(suggested),
        });
    } else if system_mb > 0 && effective_memory_mb > system_mb {
        let suggested = system_mb.saturating_sub(1024).max(2048);
        warnings.push(PreflightWarning {
            title: "Memory allocation exceeds physical RAM".into(),
            detail: format!(
                "{effective_memory_mb} MB is allocated but this system only has {system_mb} MB. Lower it or the game may fail to start."
            ),
            action: PreflightAction::LowerRam,
            suggested_memory_mb: Some(suggested),
        });
    }

    // --- Duplicate mods ---------------------------------------------------
    let conflicts = crate::tools::scan_mod_conflicts(state, instance_id);
    if !conflicts.is_empty() {
        warnings.push(PreflightWarning {
            title: "Duplicate mod files".into(),
            detail: format!(
                "{} mod{} installed twice at different versions — this can crash on launch.",
                conflicts.len(),
                if conflicts.len() == 1 { "" } else { "s" }
            ),
            action: PreflightAction::CleanDuplicates,
            suggested_memory_mb: None,
        });
    }

    Ok(warnings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_major_pre_1_17_is_8() {
        assert_eq!(required_java_major("1.16.5"), Some(8));
        assert_eq!(required_java_major("1.7.10"), Some(8));
    }

    #[test]
    fn java_major_1_17_is_16() {
        assert_eq!(required_java_major("1.17"), Some(16));
        assert_eq!(required_java_major("1.17.1"), Some(16));
    }

    #[test]
    fn java_major_1_18_through_1_20_4_is_17() {
        assert_eq!(required_java_major("1.18"), Some(17));
        assert_eq!(required_java_major("1.20"), Some(17));
        assert_eq!(required_java_major("1.20.4"), Some(17));
    }

    #[test]
    fn java_major_1_20_5_and_later_is_21() {
        assert_eq!(required_java_major("1.20.5"), Some(21));
        assert_eq!(required_java_major("1.21"), Some(21));
        assert_eq!(required_java_major("1.21.1"), Some(21));
    }

    #[test]
    fn java_major_handles_release_candidate_suffix() {
        assert_eq!(required_java_major("1.20.2-rc1"), Some(17));
    }

    #[test]
    fn java_major_none_for_snapshots() {
        assert_eq!(required_java_major("24w14a"), None);
    }
}
