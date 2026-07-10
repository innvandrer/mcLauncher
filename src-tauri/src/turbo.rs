//! Turbo Button — one click installs a small, well-established performance
//! mod stack matched to the instance's loader ("my FPS is bad" is the second
//! most common newbie pain point after crashes). Deliberately a short,
//! long-lived set of foundational projects rather than a scrape of
//! Modrinth's "optimization" category — that category routinely contains
//! mods that duplicate or conflict with each other, which is exactly the
//! failure mode Preflight's duplicate-mod check exists to catch. Mods
//! already present (by project id or file-name key) are skipped, not
//! reinstalled; mods with no build for this loader/MC version are skipped
//! too, not treated as an error.

use crate::error::Result;
use crate::models::Loader;
use crate::state::AppState;
use serde::Serialize;
use std::collections::HashSet;

fn stack_for(loader: Loader) -> &'static [(&'static str, &'static str)] {
    match loader {
        Loader::Fabric | Loader::Quilt => &[
            ("sodium", "Sodium"),
            ("lithium", "Lithium"),
            ("ferritecore", "FerriteCore"),
        ],
        Loader::Forge | Loader::Neoforge => &[
            ("embeddium", "Embeddium"),
            ("ferritecore", "FerriteCore"),
        ],
        Loader::Vanilla => &[],
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TurboSkip {
    pub label: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TurboResult {
    /// File names installed this run (primary mods + their dependencies) —
    /// exactly what "Undo" needs to delete to restore the previous state.
    pub installed: Vec<String>,
    pub skipped: Vec<TurboSkip>,
}

/// Install the performance stack for this instance's loader.
pub async fn apply(state: &AppState, instance_id: &str) -> Result<TurboResult> {
    let instance = crate::instances::get_instance(state, instance_id)?;
    let stack = stack_for(instance.loader);
    if stack.is_empty() {
        return Ok(TurboResult {
            installed: Vec::new(),
            skipped: Vec::new(),
        });
    }

    let existing = crate::instances::list_mods(state, instance_id);
    let existing_project_ids: HashSet<String> =
        existing.iter().filter_map(|m| m.project_id.clone()).collect();
    let existing_keys: HashSet<String> = existing
        .iter()
        .map(|m| crate::tools::mod_base_key(m.file_name.trim_end_matches(".disabled")))
        .collect();

    let mut installed = Vec::new();
    let mut skipped = Vec::new();

    for &(slug, label) in stack {
        let project_id = match crate::modrinth::resolve_project_id(state, slug).await {
            Ok(id) => id,
            Err(_) => {
                skipped.push(TurboSkip {
                    label: label.to_string(),
                    reason: "not found on Modrinth".into(),
                });
                continue;
            }
        };

        if existing_project_ids.contains(&project_id)
            || existing_keys.contains(&crate::tools::mod_base_key(slug))
        {
            skipped.push(TurboSkip {
                label: label.to_string(),
                reason: "already installed".into(),
            });
            continue;
        }

        match crate::modrinth::install_mod(
            state,
            instance_id,
            &project_id,
            Some(instance.loader.as_str()),
            Some(&instance.mc_version),
        )
        .await
        {
            Ok((file, deps)) => {
                installed.push(file);
                installed.extend(deps.into_iter().map(|d| d.file_name));
            }
            Err(_) => {
                skipped.push(TurboSkip {
                    label: label.to_string(),
                    reason: format!(
                        "no build for {} {}",
                        instance.loader.as_str(),
                        instance.mc_version
                    ),
                });
            }
        }
    }

    Ok(TurboResult { installed, skipped })
}
