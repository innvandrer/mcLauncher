//! Modpack export: one shared "resolved pack model" (every exportable file
//! with its hashes and its identity on each platform) consumed by two format
//! writers — Modrinth `.mrpack` and CurseForge manifest zip.
//!
//! Platform-exclusive files can't be referenced by the other format (mrpack
//! only allows downloads from allowlisted domains; the CF manifest can only
//! reference CF project/file ids), so callers pass an explicit embed list
//! decided by the user in the pre-export review dialog. Files neither
//! referenced nor embedded are excluded from the archive.

use crate::crosssource::{CurseforgeRef, ModrinthRef};
use crate::error::{Error, Result};
use crate::models::{Instance, Loader};
use crate::state::AppState;
use serde::Serialize;
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Resolved pack model
// ---------------------------------------------------------------------------

/// One exportable file (mod / resource pack / shader) with everything both
/// format writers need.
pub struct ResolvedEntry {
    /// Game sub-directory: `mods`, `resourcepacks`, or `shaderpacks`.
    pub subdir: &'static str,
    pub filename: String,
    pub path: PathBuf,
    pub sha1: String,
    pub sha512: String,
    pub size: u64,
    pub modrinth: Option<ModrinthRef>,
    pub curseforge: Option<CurseforgeRef>,
}

impl ResolvedEntry {
    /// Which platforms this file exists on: "both", "modrinth", "curseforge"
    /// or "none".
    pub fn availability(&self) -> &'static str {
        match (&self.modrinth, &self.curseforge) {
            (Some(_), Some(_)) => "both",
            (Some(_), None) => "modrinth",
            (None, Some(_)) => "curseforge",
            (None, None) => "none",
        }
    }
}

/// Collect enabled mods (.jar), resource packs (.zip), and shaders (.zip).
fn collect_exportable_files(game_dir: &Path) -> Vec<(&'static str, String, PathBuf)> {
    let mut out = Vec::new();

    if let Ok(rd) = std::fs::read_dir(game_dir.join("mods")) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.ends_with(".disabled") || !name.ends_with(".jar") {
                continue;
            }
            out.push(("mods", name, e.path()));
        }
    }
    for subdir in ["resourcepacks", "shaderpacks"] {
        if let Ok(rd) = std::fs::read_dir(game_dir.join(subdir)) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if !name.ends_with(".zip") {
                    continue;
                }
                let dir: &'static str = if subdir == "resourcepacks" {
                    "resourcepacks"
                } else {
                    "shaderpacks"
                };
                out.push((dir, name, e.path()));
            }
        }
    }
    out
}

/// Build the shared model: hash every exportable file and resolve it on both
/// platforms (Modrinth by sha1 batch, CurseForge by fingerprint). Resolution
/// is best-effort — offline or key-less runs simply mark files unavailable.
pub async fn build_resolved_pack(
    state: &AppState,
    instance_id: &str,
) -> Result<(Instance, Vec<ResolvedEntry>)> {
    let instance = crate::instances::get_instance(state, instance_id)?;
    let game_dir = state.dirs.game_dir(instance_id);

    let mut entries: Vec<ResolvedEntry> = Vec::new();
    for (subdir, filename, path) in collect_exportable_files(&game_dir) {
        let Ok((sha1, sha512)) = crate::crosssource::file_sha1_sha512(&path).await else {
            continue;
        };
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        entries.push(ResolvedEntry {
            subdir,
            filename,
            path,
            sha1,
            sha512,
            size,
            modrinth: None,
            curseforge: None,
        });
    }

    let hashes: Vec<String> = entries.iter().map(|e| e.sha1.clone()).collect();
    let modrinth = state
        .crosssource
        .resolve_hashes_modrinth(&state.http, &hashes)
        .await
        .unwrap_or_default();

    let curseforge = match crate::curseforge::api_key(state) {
        Ok(key) => {
            let files: Vec<(String, PathBuf)> = entries
                .iter()
                .map(|e| (e.sha1.clone(), e.path.clone()))
                .collect();
            state
                .crosssource
                .resolve_local_files_to_cf(&state.http, &key, &files)
                .await
                .unwrap_or_default()
        }
        Err(_) => HashMap::new(),
    };

    for e in &mut entries {
        e.modrinth = modrinth.get(&e.sha1).cloned();
        e.curseforge = curseforge.get(&e.sha1).copied();
    }

    Ok((instance, entries))
}

// ---------------------------------------------------------------------------
// Pre-export preview (for the review dialog)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackExportPreview {
    pub entries: Vec<PreviewEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewEntry {
    pub subdir: String,
    pub file_name: String,
    /// "both" | "modrinth" | "curseforge" | "none"
    pub availability: String,
}

pub async fn prepare_pack_export(state: &AppState, instance_id: &str) -> Result<PackExportPreview> {
    let (_, entries) = build_resolved_pack(state, instance_id).await?;
    Ok(PackExportPreview {
        entries: entries
            .iter()
            .map(|e| PreviewEntry {
                subdir: e.subdir.to_string(),
                file_name: e.filename.clone(),
                availability: e.availability().to_string(),
            })
            .collect(),
    })
}

/// Whether a file that isn't referenceable by the target format should be
/// bundled into `overrides/`. `None` (no review ran — legacy callers) embeds
/// everything so nothing is silently dropped; with a review the file must
/// have been explicitly opted in.
fn should_embed(embed: Option<&[String]>, filename: &str) -> bool {
    match embed {
        None => true,
        Some(list) => list.iter().any(|f| f == filename),
    }
}

// ---------------------------------------------------------------------------
// Shared override collection (config/, servers.dat, folder-based packs)
// ---------------------------------------------------------------------------

fn collect_overrides(dir: &Path, game_dir: &Path, out: &mut Vec<(String, PathBuf)>) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                collect_overrides(&p, game_dir, out);
            } else if p.is_file() {
                if let Ok(rel) = p.strip_prefix(game_dir) {
                    let zip_path = format!("overrides/{}", rel.to_string_lossy().replace('\\', "/"));
                    out.push((zip_path, p.clone()));
                }
            }
        }
    }
}

/// Overrides every exported pack ships regardless of format: `config/`, the
/// saved server list, and folder-based resource/shader packs.
fn common_overrides(game_dir: &Path) -> Vec<(String, PathBuf)> {
    let mut bundled = Vec::new();
    for subdir in ["resourcepacks", "shaderpacks"] {
        let dir = game_dir.join(subdir);
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                if e.path().is_dir() {
                    collect_overrides(&e.path(), game_dir, &mut bundled);
                }
            }
        }
    }
    let config_dir = game_dir.join("config");
    if config_dir.is_dir() {
        collect_overrides(&config_dir, game_dir, &mut bundled);
    }
    // Bundle the saved multiplayer server list too, so a shared pack brings a
    // friend straight to the right server instead of just the right mods.
    let servers_dat = game_dir.join("servers.dat");
    if servers_dat.is_file() {
        bundled.push(("overrides/servers.dat".to_string(), servers_dat));
    }
    bundled
}

fn export_version_id(instance: &Instance) -> String {
    if let Some(ps) = &instance.pack_source {
        if let Some(name) = &ps.version_name {
            if !name.is_empty() {
                return name.clone();
            }
        }
    }
    let date = chrono::Local::now().format("%Y.%m.%d").to_string();
    let short = uuid::Uuid::new_v4().to_string();
    format!("{date}-{}", &short[..8])
}

/// Write a zip: `texts` are generated in-memory files, `bundled` are copied
/// from disk (best-effort per file, matching the previous export behavior).
/// Blocking I/O — call via [`write_zip`].
fn write_zip_sync(
    dest: &Path,
    texts: &[(String, String)],
    bundled: &[(String, PathBuf)],
) -> Result<()> {
    let file = std::fs::File::create(dest)?;
    let mut zip = zip::ZipWriter::new(file);
    let opts: zip::write::FileOptions<()> =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (name, content) in texts {
        zip.start_file(name.clone(), opts)?;
        zip.write_all(content.as_bytes())?;
    }
    for (name, src) in bundled {
        if let Ok(bytes) = std::fs::read(src) {
            zip.start_file(name.clone(), opts)?;
            zip.write_all(&bytes)?;
        }
    }
    zip.finish()?;
    Ok(())
}

/// Async wrapper for [`write_zip_sync`] — bundled overrides can be large, so
/// the archive is written off the async runtime.
async fn write_zip(
    dest: PathBuf,
    texts: Vec<(String, String)>,
    bundled: Vec<(String, PathBuf)>,
) -> Result<()> {
    tokio::task::spawn_blocking(move || write_zip_sync(&dest, &texts, &bundled)).await?
}

// ---------------------------------------------------------------------------
// Modrinth .mrpack writer
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct MrIndex {
    #[serde(rename = "formatVersion")]
    format_version: u32,
    game: String,
    #[serde(rename = "versionId")]
    version_id: String,
    name: String,
    files: Vec<MrIndexFile>,
    dependencies: HashMap<String, String>,
}

#[derive(Serialize)]
struct MrIndexFile {
    path: String,
    hashes: MrIndexHashes,
    downloads: Vec<String>,
    #[serde(rename = "fileSize")]
    file_size: u64,
}

#[derive(Serialize)]
struct MrIndexHashes {
    sha1: String,
    sha512: String,
}

fn mrpack_dependencies(instance: &Instance) -> HashMap<String, String> {
    let mut deps = HashMap::new();
    deps.insert("minecraft".to_string(), instance.mc_version.clone());
    let loader_key = match instance.loader {
        Loader::Fabric => Some("fabric-loader"),
        Loader::Quilt => Some("quilt-loader"),
        Loader::Forge => Some("forge"),
        Loader::Neoforge => Some("neoforge"),
        Loader::Vanilla => None,
    };
    if let (Some(k), Some(v)) = (loader_key, instance.loader_version.clone()) {
        deps.insert(k.to_string(), v);
    }
    deps
}

/// Build the mrpack index plus the list of files to bundle as overrides.
/// Entries on Modrinth become hash-verified download references; the rest are
/// embedded or excluded per the review decisions.
fn build_mrpack_parts(
    instance: &Instance,
    entries: &[ResolvedEntry],
    embed: Option<&[String]>,
) -> (MrIndex, Vec<(String, PathBuf)>) {
    let mut files = Vec::new();
    let mut bundled = Vec::new();
    for e in entries {
        let pack_path = format!("{}/{}", e.subdir, e.filename);
        if let Some(mr) = &e.modrinth {
            files.push(MrIndexFile {
                path: pack_path,
                hashes: MrIndexHashes {
                    sha1: e.sha1.clone(),
                    sha512: e.sha512.clone(),
                },
                downloads: vec![mr.url.clone()],
                file_size: e.size,
            });
        } else if should_embed(embed, &e.filename) {
            bundled.push((format!("overrides/{pack_path}"), e.path.clone()));
        }
    }
    let index = MrIndex {
        format_version: 1,
        game: "minecraft".to_string(),
        version_id: export_version_id(instance),
        name: instance.name.clone(),
        files,
        dependencies: mrpack_dependencies(instance),
    };
    (index, bundled)
}

/// Export an instance as a `.mrpack`. `embed` is the review-dialog decision
/// list for files that only exist on CurseForge (or nowhere).
pub async fn export_mrpack(
    state: &AppState,
    instance_id: &str,
    dest: &Path,
    embed: Option<&[String]>,
) -> Result<()> {
    let (instance, entries) = build_resolved_pack(state, instance_id).await?;
    let (index, mut bundled) = build_mrpack_parts(&instance, &entries, embed);
    bundled.extend(common_overrides(&state.dirs.game_dir(instance_id)));

    let index_json = serde_json::to_string_pretty(&index)?;
    write_zip(
        dest.to_path_buf(),
        vec![("modrinth.index.json".to_string(), index_json)],
        bundled,
    )
    .await
}

// ---------------------------------------------------------------------------
// CurseForge manifest writer
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CfManifest {
    minecraft: CfManifestMc,
    manifest_type: &'static str,
    manifest_version: u32,
    name: String,
    version: String,
    author: String,
    files: Vec<CfManifestFile>,
    overrides: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CfManifestMc {
    version: String,
    mod_loaders: Vec<CfManifestLoader>,
}

#[derive(Serialize)]
struct CfManifestLoader {
    id: String,
    primary: bool,
}

#[derive(Serialize)]
struct CfManifestFile {
    #[serde(rename = "projectID")]
    project_id: u32,
    #[serde(rename = "fileID")]
    file_id: u32,
    required: bool,
}

/// The CF manifest modLoader id ("forge-47.2.0" style). None for vanilla.
fn cf_loader_id(instance: &Instance) -> Option<String> {
    let kind = match instance.loader {
        Loader::Forge => "forge",
        Loader::Neoforge => "neoforge",
        Loader::Fabric => "fabric",
        Loader::Quilt => "quilt",
        Loader::Vanilla => return None,
    };
    instance
        .loader_version
        .as_ref()
        .map(|v| format!("{kind}-{v}"))
}

/// Build manifest.json content plus the files to bundle as overrides.
fn build_cf_manifest_parts(
    instance: &Instance,
    entries: &[ResolvedEntry],
    embed: Option<&[String]>,
) -> (CfManifest, Vec<(String, PathBuf)>) {
    let mut files = Vec::new();
    let mut bundled = Vec::new();
    for e in entries {
        if let Some(cf) = &e.curseforge {
            files.push(CfManifestFile {
                project_id: cf.project_id,
                file_id: cf.file_id,
                required: true,
            });
        } else if should_embed(embed, &e.filename) {
            bundled.push((
                format!("overrides/{}/{}", e.subdir, e.filename),
                e.path.clone(),
            ));
        }
    }
    let manifest = CfManifest {
        minecraft: CfManifestMc {
            version: instance.mc_version.clone(),
            mod_loaders: cf_loader_id(instance)
                .map(|id| vec![CfManifestLoader { id, primary: true }])
                .unwrap_or_default(),
        },
        manifest_type: "minecraftModpack",
        manifest_version: 1,
        name: instance.name.clone(),
        version: export_version_id(instance),
        author: String::new(),
        files,
        overrides: "overrides",
    };
    (manifest, bundled)
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The modlist.html CurseForge launchers show next to the manifest: one link
/// per referenced project.
fn build_modlist(entries: &[ResolvedEntry]) -> String {
    let mut out = String::from("<ul>\n");
    for e in entries {
        if let Some(cf) = &e.curseforge {
            out.push_str(&format!(
                "<li><a href=\"https://www.curseforge.com/projects/{}\">{}</a></li>\n",
                cf.project_id,
                html_escape(&e.filename),
            ));
        }
    }
    out.push_str("</ul>\n");
    out
}

/// Export an instance as a CurseForge modpack zip (manifest.json +
/// modlist.html + overrides/). `embed` mirrors the mrpack review decisions
/// for files that only exist on Modrinth (or nowhere).
pub async fn export_curseforge_pack(
    state: &AppState,
    instance_id: &str,
    dest: &Path,
    embed: Option<&[String]>,
) -> Result<()> {
    let (instance, entries) = build_resolved_pack(state, instance_id).await?;
    if entries.iter().any(|e| e.curseforge.is_none()) && embed.is_none() {
        // The CF format can't hash-verify overrides, so never bundle silently:
        // this writer requires an explicit decision list when anything is
        // missing from CurseForge (the UI always sends one).
        return Err(Error::Other(
            "Some files aren't on CurseForge — export needs per-file embed/exclude decisions."
                .into(),
        ));
    }
    let (manifest, mut bundled) = build_cf_manifest_parts(&instance, &entries, embed);
    bundled.extend(common_overrides(&state.dirs.game_dir(instance_id)));

    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    let modlist = build_modlist(&entries);
    write_zip(
        dest.to_path_buf(),
        vec![
            ("manifest.json".to_string(), manifest_json),
            ("modlist.html".to_string(), modlist),
        ],
        bundled,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn instance(loader: Loader, loader_version: Option<&str>) -> Instance {
        Instance {
            id: "test".into(),
            name: "My <Pack> & Friends".into(),
            mc_version: "1.21.1".into(),
            loader,
            loader_version: loader_version.map(String::from),
            icon: None,
            group: None,
            accent: None,
            favorite: false,
            created: 0,
            last_played: None,
            total_play_seconds: 0,
            memory_mb: None,
            java_path: None,
            jvm_args: None,
            window_width: None,
            window_height: None,
            env_vars: None,
            pre_launch: None,
            post_exit: None,
            pack_source: None,
            loadouts: Vec::new(),
            mod_count: 0,
        }
    }

    fn entry(
        filename: &str,
        modrinth: Option<&str>,
        curseforge: Option<(u32, u32)>,
    ) -> ResolvedEntry {
        ResolvedEntry {
            subdir: "mods",
            filename: filename.into(),
            path: PathBuf::from(format!("/tmp/{filename}")),
            sha1: "a".repeat(40),
            sha512: "b".repeat(128),
            size: 123,
            modrinth: modrinth.map(|url| ModrinthRef {
                project_id: "P".into(),
                version_id: "V".into(),
                url: url.into(),
                filename: filename.into(),
                sha1: "a".repeat(40),
                sha512: Some("b".repeat(128)),
                size: 123,
            }),
            curseforge: curseforge.map(|(p, f)| CurseforgeRef {
                project_id: p,
                file_id: f,
            }),
        }
    }

    #[test]
    fn availability_classification() {
        assert_eq!(entry("a.jar", Some("u"), Some((1, 2))).availability(), "both");
        assert_eq!(entry("a.jar", Some("u"), None).availability(), "modrinth");
        assert_eq!(entry("a.jar", None, Some((1, 2))).availability(), "curseforge");
        assert_eq!(entry("a.jar", None, None).availability(), "none");
    }

    #[test]
    fn embed_decisions() {
        // No review ran (legacy path): embed everything.
        assert!(should_embed(None, "x.jar"));
        // With a review, only explicitly opted-in files are embedded.
        let list = vec!["yes.jar".to_string()];
        assert!(should_embed(Some(&list), "yes.jar"));
        assert!(!should_embed(Some(&list), "no.jar"));
        assert!(!should_embed(Some(&[]), "no.jar"));
    }

    #[test]
    fn cf_manifest_shape() {
        let inst = instance(Loader::Neoforge, Some("21.1.90"));
        let entries = vec![
            entry("on-cf.jar", None, Some((238222, 4711))),
            entry("mr-only.jar", Some("https://cdn.modrinth.com/x.jar"), None),
        ];
        let embed: Vec<String> = vec!["mr-only.jar".into()];
        let (manifest, bundled) = build_cf_manifest_parts(&inst, &entries, Some(&embed));
        let json = serde_json::to_value(&manifest).unwrap();

        assert_eq!(json["manifestType"], "minecraftModpack");
        assert_eq!(json["manifestVersion"], 1);
        assert_eq!(json["minecraft"]["version"], "1.21.1");
        assert_eq!(json["minecraft"]["modLoaders"][0]["id"], "neoforge-21.1.90");
        assert_eq!(json["minecraft"]["modLoaders"][0]["primary"], true);
        assert_eq!(json["files"][0]["projectID"], 238222);
        assert_eq!(json["files"][0]["fileID"], 4711);
        assert_eq!(json["files"][0]["required"], true);
        assert_eq!(json["files"].as_array().unwrap().len(), 1);
        assert_eq!(json["overrides"], "overrides");
        // The Modrinth-only file was embedded, not silently referenced.
        assert_eq!(bundled, vec![(
            "overrides/mods/mr-only.jar".to_string(),
            PathBuf::from("/tmp/mr-only.jar"),
        )]);
    }

    #[test]
    fn cf_manifest_excludes_undecided_files() {
        let inst = instance(Loader::Forge, Some("47.2.0"));
        let entries = vec![entry("mr-only.jar", Some("u"), None)];
        let (manifest, bundled) = build_cf_manifest_parts(&inst, &entries, Some(&[]));
        assert!(manifest.files.is_empty());
        assert!(bundled.is_empty());
    }

    #[test]
    fn vanilla_has_no_modloader_entry() {
        let inst = instance(Loader::Vanilla, None);
        let (manifest, _) = build_cf_manifest_parts(&inst, &[], None);
        assert!(manifest.minecraft.mod_loaders.is_empty());
    }

    #[test]
    fn modlist_escapes_html() {
        let entries = vec![entry("evil <b>&\"name\".jar", None, Some((7, 8)))];
        let html = build_modlist(&entries);
        assert!(html.contains("https://www.curseforge.com/projects/7"));
        assert!(html.contains("evil &lt;b&gt;&amp;&quot;name&quot;.jar"));
        assert!(!html.contains("<b>"));
    }

    #[test]
    fn mrpack_index_references_modrinth_files_and_embeds_the_rest() {
        let inst = instance(Loader::Fabric, Some("0.16.0"));
        let entries = vec![
            entry("on-mr.jar", Some("https://cdn.modrinth.com/on-mr.jar"), None),
            entry("cf-only.jar", None, Some((1, 2))),
            entry("nowhere.jar", None, None),
        ];
        let embed = vec!["cf-only.jar".to_string()];
        let (index, bundled) = build_mrpack_parts(&inst, &entries, Some(&embed));

        assert_eq!(index.files.len(), 1);
        assert_eq!(index.files[0].path, "mods/on-mr.jar");
        assert_eq!(index.files[0].downloads, vec!["https://cdn.modrinth.com/on-mr.jar"]);
        assert_eq!(index.files[0].hashes.sha1, "a".repeat(40));
        assert_eq!(index.dependencies.get("minecraft").unwrap(), "1.21.1");
        assert_eq!(index.dependencies.get("fabric-loader").unwrap(), "0.16.0");
        // cf-only embedded (opted in), nowhere.jar excluded (not opted in).
        assert_eq!(bundled.len(), 1);
        assert_eq!(bundled[0].0, "overrides/mods/cf-only.jar");
    }
}
