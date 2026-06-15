//! Vanilla Minecraft metadata + asset/library installation, plus resolution of
//! modded (Fabric/Quilt) version JSONs that `inheritsFrom` a vanilla version.

use crate::error::{Error, Result};
use crate::net::{self, DownloadItem};
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tauri::AppHandle;

const VERSION_MANIFEST: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const RESOURCES_BASE: &str = "https://resources.download.minecraft.net";

// ---------------------------------------------------------------------------
// Manifest
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct VersionManifest {
    pub latest: Latest,
    pub versions: Vec<VersionStub>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Latest {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionStub {
    pub id: String,
    #[serde(rename(serialize = "kind", deserialize = "type"))]
    pub kind: String,
    pub url: String,
    #[serde(default)]
    pub release_time: String,
    #[serde(default)]
    pub sha1: String,
}

pub async fn fetch_manifest(state: &AppState) -> Result<VersionManifest> {
    net::get_json(&state.http, VERSION_MANIFEST).await
}

// ---------------------------------------------------------------------------
// Version JSON
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct VersionJson {
    pub id: String,
    #[serde(default, rename = "mainClass")]
    pub main_class: String,
    #[serde(default, rename = "assetIndex")]
    pub asset_index: Option<AssetIndexRef>,
    #[serde(default)]
    pub assets: Option<String>,
    #[serde(default, rename = "javaVersion")]
    pub java_version: Option<JavaVersion>,
    #[serde(default)]
    pub downloads: Option<Downloads>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(default)]
    pub arguments: Option<Arguments>,
    #[serde(default, rename = "minecraftArguments")]
    pub minecraft_arguments: Option<String>,
    #[serde(default, rename = "inheritsFrom")]
    pub inherits_from: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetIndexRef {
    pub id: String,
    pub sha1: String,
    #[serde(default)]
    pub size: u64,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JavaVersion {
    #[serde(default)]
    pub component: String,
    #[serde(rename = "majorVersion")]
    pub major_version: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Downloads {
    pub client: Option<DownloadRef>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DownloadRef {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Library {
    pub name: String,
    #[serde(default)]
    pub downloads: Option<LibDownloads>,
    #[serde(default)]
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub natives: Option<HashMap<String, String>>,
    /// Fabric/Quilt libraries omit `downloads` and instead give a maven `url`.
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LibDownloads {
    #[serde(default)]
    pub artifact: Option<DownloadRef>,
    #[serde(default)]
    pub classifiers: Option<HashMap<String, DownloadRef>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Rule {
    pub action: String,
    #[serde(default)]
    pub os: Option<OsRule>,
    #[serde(default)]
    pub features: Option<HashMap<String, bool>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OsRule {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub arch: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<Argument>,
    #[serde(default)]
    pub jvm: Vec<Argument>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Argument {
    Plain(String),
    Conditional { rules: Vec<Rule>, value: ArgValue },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ArgValue {
    One(String),
    Many(Vec<String>),
}

// ---------------------------------------------------------------------------
// Resolved version (after merging inheritance) — what `launch` consumes.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Resolved {
    pub id: String,
    pub main_class: String,
    pub asset_index: Option<AssetIndexRef>,
    pub assets_id: String,
    pub java_major: u32,
    pub client: Option<DownloadRef>,
    /// The version id the client jar is stored under (the vanilla root).
    pub client_id: String,
    pub libraries: Vec<Library>,
    pub game_args: Vec<Argument>,
    pub jvm_args: Vec<Argument>,
    pub legacy_args: Option<String>,
}

// ---------------------------------------------------------------------------
// OS / arch helpers
// ---------------------------------------------------------------------------

pub fn os_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}

pub fn os_arch() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "x64"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else if cfg!(target_arch = "x86") {
        "x86"
    } else {
        "x64"
    }
}

/// Evaluate a list of rules for the current OS and the given feature flags.
pub fn rules_allow(rules: &[Rule], features: &HashMap<String, bool>) -> bool {
    if rules.is_empty() {
        return true;
    }
    let mut allowed = false;
    for rule in rules {
        let mut applies = true;
        if let Some(os) = &rule.os {
            if let Some(name) = &os.name {
                if name != os_name() {
                    applies = false;
                }
            }
            if let Some(arch) = &os.arch {
                if arch != os_arch() {
                    applies = false;
                }
            }
        }
        if let Some(feats) = &rule.features {
            for (k, v) in feats {
                if features.get(k).copied().unwrap_or(false) != *v {
                    applies = false;
                }
            }
        }
        if applies {
            allowed = rule.action == "allow";
        }
    }
    allowed
}

// ---------------------------------------------------------------------------
// Maven coordinate helpers ("group:artifact:version[:classifier]")
// ---------------------------------------------------------------------------

pub fn maven_to_path(name: &str) -> String {
    let parts: Vec<&str> = name.split(':').collect();
    if parts.len() < 3 {
        return name.replace(':', "/");
    }
    let group = parts[0].replace('.', "/");
    let artifact = parts[1];
    let version = parts[2];
    let classifier = if parts.len() >= 4 {
        format!("-{}", parts[3])
    } else {
        String::new()
    };
    format!("{group}/{artifact}/{version}/{artifact}-{version}{classifier}.jar")
}

/// True when a maven coordinate carries a `natives-*` classifier (new format).
fn is_native_coord(name: &str) -> bool {
    name.split(':')
        .nth(3)
        .map(|c| c.starts_with("natives"))
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Loading + resolving version JSON
// ---------------------------------------------------------------------------

/// Load a version JSON from cache, downloading it from the manifest if missing.
pub async fn load_version_json(state: &AppState, version_id: &str) -> Result<VersionJson> {
    let path = state.dirs.version_json(version_id);
    if path.exists() {
        let raw = tokio::fs::read(&path).await?;
        return Ok(serde_json::from_slice(&raw)?);
    }
    // Not cached: find it in the manifest and download.
    let manifest = fetch_manifest(state).await?;
    let stub = manifest
        .versions
        .iter()
        .find(|v| v.id == version_id)
        .ok_or_else(|| Error::NotFound(format!("Minecraft version {version_id}")))?;
    let raw = state
        .http
        .get(&stub.url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&path, &raw).await?;
    Ok(serde_json::from_slice(&raw)?)
}

/// Persist an externally produced version JSON (e.g. a Fabric profile) to cache.
pub async fn save_version_json(state: &AppState, version_id: &str, raw: &[u8]) -> Result<()> {
    let path = state.dirs.version_json(version_id);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&path, raw).await?;
    Ok(())
}

/// Resolve a version into a single merged [`Resolved`], following `inheritsFrom`.
pub async fn resolve_version(state: &AppState, version_id: &str) -> Result<Resolved> {
    let v = load_version_json(state, version_id).await?;

    if let Some(parent_id) = v.inherits_from.clone() {
        let parent = Box::pin(resolve_version(state, &parent_id)).await?;
        Ok(merge(parent, v))
    } else {
        let args = v.arguments.unwrap_or_default();
        Ok(Resolved {
            id: v.id.clone(),
            main_class: v.main_class,
            asset_index: v.asset_index,
            assets_id: v.assets.unwrap_or_else(|| "legacy".to_string()),
            java_major: v.java_version.map(|j| j.major_version).unwrap_or(8),
            client: v.downloads.and_then(|d| d.client),
            client_id: v.id,
            libraries: v.libraries,
            game_args: args.game,
            jvm_args: args.jvm,
            legacy_args: v.minecraft_arguments,
        })
    }
}

/// Merge a child (modded) version JSON onto an already-resolved parent.
fn merge(parent: Resolved, child: VersionJson) -> Resolved {
    let child_args = child.arguments.unwrap_or_default();

    // Child libraries take precedence over parent ones with the same
    // group:artifact, so loaders can override vanilla libraries.
    let mut libraries = child.libraries;
    let have: Vec<String> = libraries.iter().map(|l| coord_key(&l.name)).collect();
    for lib in parent.libraries {
        if !have.contains(&coord_key(&lib.name)) {
            libraries.push(lib);
        }
    }

    let mut game_args = parent.game_args;
    game_args.extend(child_args.game);
    let mut jvm_args = parent.jvm_args;
    jvm_args.extend(child_args.jvm);

    Resolved {
        id: child.id,
        main_class: if child.main_class.is_empty() {
            parent.main_class
        } else {
            child.main_class
        },
        asset_index: child.asset_index.or(parent.asset_index),
        assets_id: child.assets.unwrap_or(parent.assets_id),
        java_major: child
            .java_version
            .map(|j| j.major_version)
            .unwrap_or(parent.java_major),
        client: child.downloads.and_then(|d| d.client).or(parent.client),
        client_id: parent.client_id,
        libraries,
        game_args,
        jvm_args,
        legacy_args: child.minecraft_arguments.or(parent.legacy_args),
    }
}

fn coord_key(name: &str) -> String {
    let parts: Vec<&str> = name.split(':').collect();
    if parts.len() >= 2 {
        format!("{}:{}", parts[0], parts[1])
    } else {
        name.to_string()
    }
}

// ---------------------------------------------------------------------------
// Classpath + natives computation
// ---------------------------------------------------------------------------

/// A library artifact that belongs on the JVM classpath.
pub struct ClasspathEntry {
    pub path: std::path::PathBuf,
}

/// Returns the set of classpath jars and the set of native jars to extract,
/// after applying OS rules.
pub fn split_libraries(
    state: &AppState,
    resolved: &Resolved,
) -> (Vec<DownloadItem>, Vec<DownloadItem>, Vec<std::path::PathBuf>) {
    let empty = HashMap::new();
    let mut classpath_dl = Vec::new();
    let mut natives_dl = Vec::new();
    let mut classpath_paths = Vec::new();
    let lib_root = state.dirs.libraries();

    for lib in &resolved.libraries {
        if !rules_allow(&lib.rules, &empty) {
            continue;
        }

        // New-format native (classifier baked into the maven name).
        // Modern LWJGL (3.3+/MC 1.19+) extracts native DLLs from the natives JAR
        // on the *classpath* itself (to org.lwjgl.system.SharedLibraryExtractPath),
        // so the JAR must be on the classpath — not merely extracted to
        // java.library.path. We add it to both: classpath for LWJGL's own
        // extraction, and natives_dl so the flat DLLs are available via
        // java.library.path as a fallback for non-LWJGL natives.
        if is_native_coord(&lib.name) {
            if let Some(art) = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()) {
                let rel = art.path.clone().unwrap_or_else(|| maven_to_path(&lib.name));
                let path = lib_root.join(&rel);
                classpath_paths.push(path.clone());
                natives_dl.push(DownloadItem::new(art.url.clone(), path, art.sha1.clone()));
            }
            continue;
        }

        // Old-format native (a `natives` map + classifiers).
        if let Some(natives_map) = &lib.natives {
            if let Some(classifier_tmpl) = natives_map.get(os_name()) {
                let classifier = classifier_tmpl.replace(
                    "${arch}",
                    if os_arch() == "x86" { "32" } else { "64" },
                );
                if let Some(cls) = lib
                    .downloads
                    .as_ref()
                    .and_then(|d| d.classifiers.as_ref())
                    .and_then(|c| c.get(&classifier))
                {
                    let rel = cls.path.clone().unwrap_or_else(|| maven_to_path(&lib.name));
                    let path = lib_root.join(&rel);
                    natives_dl.push(DownloadItem::new(cls.url.clone(), path, cls.sha1.clone()));
                }
            }
        }

        // Regular classpath artifact.
        if let Some(art) = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()) {
            let rel = art.path.clone().unwrap_or_else(|| maven_to_path(&lib.name));
            let path = lib_root.join(&rel);
            classpath_paths.push(path.clone());
            if !art.url.is_empty() {
                classpath_dl.push(DownloadItem::new(art.url.clone(), path, art.sha1.clone()));
            }
        } else if let Some(base) = &lib.url {
            // Fabric/Quilt maven-style entry without an explicit download block.
            let rel = maven_to_path(&lib.name);
            let url = format!("{}{}", base.trim_end_matches('/'), format!("/{rel}"));
            let path = lib_root.join(&rel);
            classpath_paths.push(path.clone());
            classpath_dl.push(DownloadItem::new(url, path, None));
        }
    }

    (classpath_dl, natives_dl, classpath_paths)
}

// ---------------------------------------------------------------------------
// Assets
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct AssetIndexFile {
    objects: HashMap<String, AssetObject>,
}

#[derive(Debug, Deserialize)]
struct AssetObject {
    hash: String,
    #[serde(default)]
    #[allow(dead_code)]
    size: u64,
}

async fn asset_download_items(
    state: &AppState,
    resolved: &Resolved,
) -> Result<Vec<DownloadItem>> {
    let Some(index_ref) = &resolved.asset_index else {
        return Ok(Vec::new());
    };

    // Download + cache the index itself.
    let index_path = state
        .dirs
        .assets()
        .join("indexes")
        .join(format!("{}.json", index_ref.id));
    net::download_one(
        &state.http,
        &DownloadItem::new(index_ref.url.clone(), index_path.clone(), Some(index_ref.sha1.clone())),
    )
    .await?;

    let raw = tokio::fs::read(&index_path).await?;
    let index: AssetIndexFile = serde_json::from_slice(&raw)?;

    let objects_dir = state.dirs.assets().join("objects");
    let mut items = Vec::with_capacity(index.objects.len());
    for obj in index.objects.values() {
        let sub = &obj.hash[0..2];
        let url = format!("{RESOURCES_BASE}/{sub}/{}", obj.hash);
        let path = objects_dir.join(sub).join(&obj.hash);
        items.push(DownloadItem::new(url, path, Some(obj.hash.clone())));
    }
    Ok(items)
}

// ---------------------------------------------------------------------------
// Natives extraction
// ---------------------------------------------------------------------------

/// Extract native libraries (`.dll`/`.so`/`.dylib`) from the given jars into the
/// per-instance natives directory, flattening by file name so the JVM's
/// `java.library.path` can find them.
pub fn extract_natives(jars: &[std::path::PathBuf], out_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(out_dir)?;
    for jar in jars {
        let file = match std::fs::File::open(jar) {
            Ok(f) => f,
            Err(_) => continue,
        };
        let mut archive = zip::ZipArchive::new(file)?;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            if entry.is_dir() {
                continue;
            }
            let name = entry.name().to_string();
            if name.starts_with("META-INF") {
                continue;
            }
            let lower = name.to_ascii_lowercase();
            if !(lower.ends_with(".dll")
                || lower.ends_with(".so")
                || lower.ends_with(".dylib")
                || lower.ends_with(".jnilib"))
            {
                continue;
            }
            let file_name = Path::new(&name)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or(name);
            let out_path = out_dir.join(file_name);
            let mut out = std::fs::File::create(&out_path)?;
            std::io::copy(&mut entry, &mut out)?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Top-level install
// ---------------------------------------------------------------------------

/// Download everything needed to launch `version_id`: client jar, libraries,
/// natives and assets. Returns the resolved version for the launcher to use.
pub async fn install(
    app: &AppHandle,
    state: &AppState,
    version_id: &str,
    natives_dir: &Path,
    concurrency: usize,
) -> Result<Resolved> {
    net::emit_progress(app, version_id, "Resolving version", "resolve", 0, 1, false, None);
    let resolved = resolve_version(state, version_id).await?;

    let mut items: Vec<DownloadItem> = Vec::new();

    // Client jar.
    if let Some(client) = &resolved.client {
        let jar = state.dirs.version_jar(&resolved.client_id);
        items.push(DownloadItem::new(client.url.clone(), jar, client.sha1.clone()));
    }

    // Libraries + natives.
    let (cp_dl, natives_dl, _cp_paths) = split_libraries(state, &resolved);
    items.extend(cp_dl);
    items.extend(natives_dl.clone());

    // Assets.
    let assets = asset_download_items(state, &resolved).await?;
    items.extend(assets);

    net::download_many(
        app,
        &state.http,
        version_id,
        "Downloading game files",
        items,
        concurrency,
    )
    .await?;

    // Extract natives for this launch.
    // Modern versions (1.13+) set java.library.path=${natives_directory}/java,
    // so DLLs must go in the java/ subdirectory. Legacy versions (<1.13) have
    // no jvm_args template and set the path to ${natives_directory} directly.
    let native_extract_dir = if resolved.jvm_args.is_empty() {
        natives_dir.to_path_buf()
    } else {
        natives_dir.join("java")
    };
    let native_jars: Vec<std::path::PathBuf> = natives_dl.into_iter().map(|d| d.path).collect();
    extract_natives(&native_jars, &native_extract_dir)?;

    Ok(resolved)
}
