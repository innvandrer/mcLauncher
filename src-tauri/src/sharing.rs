//! Tiny, backend-free instance manifests. A `.ezmapa` file contains provider
//! project identities and instance metadata; recipients reconstruct content
//! through the same Modrinth/CurseForge installers used by the normal UI.

use crate::error::{Error, Result};
use crate::models::{Instance, Loader};
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareManifest {
    format_version: u32,
    name: String,
    mc_version: String,
    loader: Loader,
    loader_version: Option<String>,
    icon: Option<String>,
    content: Vec<crate::instances::ShareContentEntry>,
}

pub fn export_share_manifest(state: &AppState, instance_id: &str, dest: &Path) -> Result<()> {
    let manifest = build_share_manifest(state, instance_id)?;
    std::fs::write(dest, serde_json::to_vec_pretty(&manifest)?)?;
    Ok(())
}

fn build_share_manifest(state: &AppState, instance_id: &str) -> Result<ShareManifest> {
    let instance = crate::instances::get_instance(state, instance_id)?;
    Ok(ShareManifest {
        format_version: 1,
        name: instance.name,
        mc_version: instance.mc_version,
        loader: instance.loader,
        loader_version: instance.loader_version,
        icon: instance.icon,
        content: crate::instances::share_content_entries(state, instance_id),
    })
}

pub fn share_code(state: &AppState, instance_id: &str) -> Result<String> {
    Ok(serde_json::to_string(&build_share_manifest(state, instance_id)?)?)
}

pub async fn import_share_manifest(state: &AppState, src: &Path) -> Result<Instance> {
    let manifest: ShareManifest = serde_json::from_slice(&std::fs::read(src)?)?;
    import_manifest(state, manifest).await
}

pub async fn import_share_code(state: &AppState, code: &str) -> Result<Instance> {
    let manifest: ShareManifest = serde_json::from_str(code)?;
    import_manifest(state, manifest).await
}

async fn import_manifest(state: &AppState, manifest: ShareManifest) -> Result<Instance> {
    if manifest.format_version != 1 {
        return Err(Error::Other(format!(
            "Unsupported EZMapa share format {}.",
            manifest.format_version
        )));
    }
    let instance = crate::instances::create_instance(
        state,
        &manifest.name,
        &manifest.mc_version,
        manifest.loader,
        manifest.loader_version,
        manifest.icon,
    )?;

    let result = async {
        for entry in manifest.content {
            let installed = match entry.provider.as_str() {
                "modrinth" => {
                    let (file, deps) = crate::modrinth::install_content(
                        state,
                        &instance.id,
                        &entry.project_id,
                        &entry.content_type,
                        Some(instance.loader.as_str()),
                        Some(&instance.mc_version),
                    )
                    .await?;
                    let mut index = vec![(file.clone(), entry.project_id.clone())];
                    index.extend(
                        deps.iter()
                            .map(|dep| (dep.file_name.clone(), dep.project_id.clone())),
                    );
                    crate::instances::record_installs(state, &instance.id, &index, "modrinth");
                    crate::instances::record_install_dependencies(
                        state,
                        &instance.id,
                        &entry.project_id,
                        &deps.iter()
                            .map(|dep| dep.file_name.clone())
                            .collect::<Vec<_>>(),
                    );
                    file
                }
                "curseforge" => {
                    let installed = crate::curseforge::install_content(
                        state,
                        &instance.id,
                        &entry.project_id,
                        &entry.content_type,
                        Some(instance.loader.as_str()),
                        Some(&instance.mc_version),
                    )
                    .await?;
                    let mut index =
                        vec![(installed.file_name.clone(), entry.project_id.clone())];
                    index.extend(
                        installed
                            .deps
                            .iter()
                            .map(|dep| (dep.file_name.clone(), dep.project_id.clone())),
                    );
                    crate::instances::record_installs(state, &instance.id, &index, "curseforge");
                    crate::instances::record_install_dependencies(
                        state,
                        &instance.id,
                        &entry.project_id,
                        &installed
                            .deps
                            .iter()
                            .map(|dep| dep.file_name.clone())
                            .collect::<Vec<_>>(),
                    );
                    installed.file_name
                }
                provider => {
                    return Err(Error::Other(format!(
                        "Unsupported content provider: {provider}"
                    )))
                }
            };
            if entry.content_type == "mod" && !entry.enabled {
                crate::instances::set_mod_enabled(state, &instance.id, &installed, false)?;
            }
        }
        Ok::<(), Error>(())
    }
    .await;

    if let Err(error) = result {
        let _ = crate::instances::delete_instance(state, &instance.id);
        return Err(error);
    }
    Ok(instance)
}
