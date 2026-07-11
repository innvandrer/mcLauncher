//! Per-instance JVM tuning suggestions: pick a heap size from pack size and
//! system RAM, and GC flags from the Java major version (generational ZGC on
//! 21+, Aikar-style client G1 otherwise). Suggestions are shown as a diff and
//! applied only after the user confirms — nothing here writes settings.

use crate::error::Result;
use crate::state::AppState;
use serde::Serialize;

/// Generational ZGC — Java 21–23 need the flag; from 24 generational is the
/// only ZGC, and the flag is obsolete.
const ZGC_GENERATIONAL: &str = "-XX:+UseZGC -XX:+ZGenerational";
const ZGC_PLAIN: &str = "-XX:+UseZGC";

/// Aikar-style G1 trimmed for clients (no AlwaysPreTouch — it slows startup
/// for no client benefit).
const G1_CLIENT: &str = "-XX:+UnlockExperimentalVMOptions -XX:+UseG1GC \
-XX:G1NewSizePercent=30 -XX:G1MaxNewSizePercent=40 -XX:G1HeapRegionSize=8M \
-XX:G1ReservePercent=20 -XX:MaxGCPauseMillis=37 -XX:G1HeapWastePercent=5 \
-XX:G1MixedGCCountTarget=4 -XX:InitiatingHeapOccupancyPercent=15 \
-XX:+ParallelRefProcEnabled -XX:+PerfDisableSharedMem -XX:MaxTenuringThreshold=1";

/// Conservative G1 for old runtimes (the launcher's long-standing default).
const G1_LEGACY: &str = "-XX:+UnlockExperimentalVMOptions -XX:+UseG1GC \
-XX:G1NewSizePercent=20 -XX:G1ReservePercent=20 -XX:MaxGCPauseMillis=50 \
-XX:G1HeapRegionSize=32M";

/// Modpacks known to need a large heap regardless of raw mod count
/// (CurseForge/Modrinth project ids — extend as reports come in).
const HEAVY_PACK_IDS: &[&str] = &[
    "925200", // All the Mods 10 (CF)
    "520914", // All the Mods 9 (CF)
    "285109", // RLCraft (CF)
    "875245", // Prominence II (CF)
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JvmSuggestion {
    pub current_args: String,
    pub suggested_args: String,
    pub current_xmx_mb: u32,
    pub suggested_xmx_mb: u32,
    pub java_major: u32,
    pub system_ram_mb: u64,
    pub mod_count: u32,
    /// True when the instance has its own custom args (the UI shows a merge
    /// view instead of a plain replace diff).
    pub has_custom_args: bool,
    /// Human-readable notes on how the numbers were chosen.
    pub reasons: Vec<String>,
}

/// GC flags for a Java major version.
fn gc_flags(java_major: u32) -> &'static str {
    match java_major {
        0..=16 => G1_LEGACY,
        17..=20 => G1_CLIENT,
        21..=23 => ZGC_GENERATIONAL,
        _ => ZGC_PLAIN,
    }
}

/// The Java major Mojang requires for a Minecraft version (used when no
/// explicit runtime is pinned): 21 from 1.20.5, 17 from 1.17, 8 before.
fn required_major_for_mc(mc_version: &str) -> u32 {
    let mut parts = mc_version.split('.');
    let major: u32 = parts.next().and_then(|p| p.parse().ok()).unwrap_or(1);
    let minor: u32 = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let patch: u32 = parts
        .next()
        .and_then(|p| p.split(['-', '_']).next().unwrap_or("").parse().ok())
        .unwrap_or(0);
    if major != 1 {
        return 21;
    }
    if minor > 20 || (minor == 20 && patch >= 5) {
        21
    } else if minor >= 17 {
        17
    } else {
        8
    }
}

/// Heap suggestion from pack size, capped against system RAM: never more than
/// half the machine and always leaving ≥ 2 GiB for the OS.
fn suggest_xmx_mb(mod_count: u32, heavy_pack: bool, system_ram_mb: u64) -> u32 {
    let base: u32 = match mod_count {
        0..=20 => 2048,
        21..=80 => 4096,
        81..=150 => 6144,
        _ => 8192,
    };
    let base = if heavy_pack { base.max(8192) } else { base };
    if system_ram_mb == 0 {
        return base;
    }
    let half = (system_ram_mb / 2) as u32;
    let leave_os = (system_ram_mb.saturating_sub(2048)) as u32;
    base.min(half.max(1024)).min(leave_os.max(1024)).max(1024)
}

/// Tokens the tuner owns: existing GC/heap-shaping flags are replaced by the
/// suggestion; everything else the user wrote (system properties,
/// --add-opens, agent flags, …) is preserved.
fn is_gc_token(token: &str) -> bool {
    const GC_PREFIXES: &[&str] = &[
        "-XX:+Use",
        "-XX:-Use",
        "-XX:+ZGenerational",
        "-XX:-ZGenerational",
        "-XX:G1",
        "-XX:MaxGCPauseMillis",
        "-XX:InitiatingHeapOccupancy",
        "-XX:+ParallelRefProcEnabled",
        "-XX:-ParallelRefProcEnabled",
        "-XX:+PerfDisableSharedMem",
        "-XX:MaxTenuringThreshold",
        "-XX:SurvivorRatio",
        "-XX:+UnlockExperimentalVMOptions",
        "-XX:+AlwaysPreTouch",
        "-XX:-AlwaysPreTouch",
        "-XX:+DisableExplicitGC",
        "-XX:ConcGCThreads",
        "-XX:ParallelGCThreads",
        "-Xmx",
        "-Xms",
        "-Xmn",
    ];
    GC_PREFIXES.iter().any(|p| token.starts_with(p))
}

/// Merge: suggested GC flags first, then every non-GC token the user had, in
/// their original order and deduplicated.
fn merge_args(current: &str, suggested: &str) -> String {
    let mut out: Vec<&str> = suggested.split_whitespace().collect();
    for token in current.split_whitespace() {
        if !is_gc_token(token) && !out.contains(&token) {
            out.push(token);
        }
    }
    out.join(" ")
}

/// Build the suggestion for an instance. Pure decision logic lives in the
/// helpers above (tested); this gathers the inputs.
pub async fn suggest(state: &AppState, instance_id: &str) -> Result<JvmSuggestion> {
    let instance = crate::instances::get_instance(state, instance_id)?;
    let settings = crate::instances::load_settings(state);

    let current_args = instance
        .jvm_args
        .clone()
        .unwrap_or_else(|| settings.jvm_args.clone());
    let has_custom_args = instance.jvm_args.is_some();
    let current_xmx_mb = instance.memory_mb.unwrap_or(settings.memory_mb);

    // Java major: a pinned runtime wins; otherwise what Mojang requires for
    // this MC version (java::ensure picks exactly that at launch).
    let pinned_path = instance
        .java_path
        .clone()
        .or_else(|| settings.java_path.clone())
        .filter(|p| !p.trim().is_empty());
    let mut reasons = Vec::new();
    let java_major = match &pinned_path {
        Some(p) => {
            let path = std::path::PathBuf::from(p);
            let probed =
                tokio::task::spawn_blocking(move || crate::java::probe_major(&path)).await?;
            match probed {
                Some(major) => {
                    reasons.push(format!("Java {major} (from the configured Java path)"));
                    major
                }
                None => {
                    let major = required_major_for_mc(&instance.mc_version);
                    reasons.push(format!(
                        "Configured Java path couldn't be probed — assuming Java {major} \
                         (required for Minecraft {})",
                        instance.mc_version
                    ));
                    major
                }
            }
        }
        None => {
            let major = required_major_for_mc(&instance.mc_version);
            reasons.push(format!(
                "Java {major} (what Minecraft {} launches with)",
                instance.mc_version
            ));
            major
        }
    };

    let mod_count = crate::instances::list_mods(state, instance_id).len() as u32;
    let heavy_pack = instance
        .pack_source
        .as_ref()
        .is_some_and(|ps| HEAVY_PACK_IDS.contains(&ps.project_id.as_str()));
    let system_ram_mb = crate::system::total_memory_mb();

    let suggested_xmx_mb = suggest_xmx_mb(mod_count, heavy_pack, system_ram_mb);
    reasons.push(match (mod_count, heavy_pack) {
        (_, true) => format!("Known heavy modpack + {mod_count} mods → large heap"),
        (n, false) => format!("{n} mods installed"),
    });
    if system_ram_mb > 0 {
        reasons.push(format!(
            "Capped against {} MB system RAM (≤ half, ≥ 2 GB left for the OS)",
            system_ram_mb
        ));
    } else {
        reasons.push("System RAM unknown — using the uncapped pack-size suggestion".into());
    }
    reasons.push(match java_major {
        0..=16 => "Java ≤ 16: conservative G1 flags".into(),
        17..=20 => "Java 17–20: Aikar-style client G1 flags".into(),
        21..=23 => "Java 21+: generational ZGC (-XX:+UseZGC -XX:+ZGenerational)".into(),
        _ => "Java 24+: ZGC (generational by default)".into(),
    });

    let suggested_args = merge_args(&current_args, gc_flags(java_major));

    Ok(JvmSuggestion {
        current_args,
        suggested_args,
        current_xmx_mb,
        suggested_xmx_mb,
        java_major,
        system_ram_mb,
        mod_count,
        has_custom_args,
        reasons,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_java_majors() {
        assert_eq!(required_major_for_mc("1.16.5"), 8);
        assert_eq!(required_major_for_mc("1.17"), 17);
        assert_eq!(required_major_for_mc("1.20.4"), 17);
        assert_eq!(required_major_for_mc("1.20.5"), 21);
        assert_eq!(required_major_for_mc("1.21.1"), 21);
        assert_eq!(required_major_for_mc("1.12.2"), 8);
    }

    #[test]
    fn gc_flag_selection_by_major() {
        assert!(gc_flags(8).contains("G1HeapRegionSize=32M"));
        assert!(gc_flags(17).contains("MaxGCPauseMillis=37"));
        assert!(gc_flags(21).contains("+ZGenerational"));
        assert!(gc_flags(23).contains("+ZGenerational"));
        // From 24 generational ZGC is the default and the flag is obsolete.
        assert!(!gc_flags(24).contains("ZGenerational"));
        assert!(gc_flags(24).contains("+UseZGC"));
    }

    #[test]
    fn heap_scales_with_pack_and_caps_at_ram() {
        assert_eq!(suggest_xmx_mb(10, false, 16384), 2048);
        assert_eq!(suggest_xmx_mb(60, false, 16384), 4096);
        assert_eq!(suggest_xmx_mb(120, false, 16384), 6144);
        assert_eq!(suggest_xmx_mb(300, false, 16384), 8192);
        // Heavy pack floor.
        assert_eq!(suggest_xmx_mb(60, true, 32768), 8192);
        // Half-of-RAM cap on an 8 GB machine.
        assert_eq!(suggest_xmx_mb(300, false, 8192), 4096);
        // Tiny machine: leave the OS 2 GB but never suggest below 1 GB.
        assert_eq!(suggest_xmx_mb(300, false, 2048), 1024);
        // Unknown RAM: uncapped pack-size suggestion.
        assert_eq!(suggest_xmx_mb(300, false, 0), 8192);
    }

    #[test]
    fn merge_replaces_gc_flags_but_keeps_user_tokens() {
        let current = "-XX:+UseG1GC -XX:MaxGCPauseMillis=50 -Dfml.earlyprogresswindow=false \
                       --add-opens java.base/java.lang=ALL-UNNAMED -Xmx4G";
        let merged = merge_args(current, ZGC_GENERATIONAL);
        assert!(merged.starts_with("-XX:+UseZGC -XX:+ZGenerational"));
        assert!(merged.contains("-Dfml.earlyprogresswindow=false"));
        assert!(merged.contains("--add-opens"));
        assert!(merged.contains("java.base/java.lang=ALL-UNNAMED"));
        // Old GC/heap tokens are gone.
        assert!(!merged.contains("UseG1GC"));
        assert!(!merged.contains("MaxGCPauseMillis=50"));
        assert!(!merged.contains("-Xmx4G"));
    }

    #[test]
    fn merge_deduplicates_shared_tokens() {
        let merged = merge_args("-XX:+UseZGC -Dx=1", ZGC_GENERATIONAL);
        assert_eq!(merged, format!("{ZGC_GENERATIONAL} -Dx=1"));
    }
}
