use crate::error::{Error, Result};
use crate::instances;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Instant, UNIX_EPOCH};
use tokio::process::Command;

const MAX_LOG_CHARS: usize = 180_000;

pub fn is_enabled() -> bool {
    cfg!(debug_assertions)
        || std::env::args().any(|arg| arg == "--developer-hub")
        || std::env::var("EZMAPA_DEVELOPER_HUB").as_deref() == Ok("1")
}

pub fn require_enabled() -> Result<()> {
    if is_enabled() {
        Ok(())
    } else {
        Err(Error::Other(
            "Developer Hub is disabled. Start EZMapa with --developer-hub to enable it.".into(),
        ))
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeveloperProject {
    pub path: String,
    pub name: String,
    pub version: Option<String>,
    pub kind: String,
    pub git_state: String,
    pub modified: bool,
    pub artifact_path: Option<String>,
    pub artifact_name: Option<String>,
    pub artifact_modified_at: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeveloperTaskResult {
    pub success: bool,
    pub task: String,
    pub command: String,
    pub duration_ms: u64,
    pub output: String,
    pub artifact_path: Option<String>,
    pub artifact_name: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeveloperInstallResult {
    pub file_name: String,
    pub destination: String,
    pub backup: Option<String>,
}

#[derive(Default, Serialize, Deserialize)]
struct InstallRegistry {
    /// canonical project path -> instance id -> installed filename
    projects: HashMap<String, HashMap<String, String>>,
}

pub fn discover_projects() -> Vec<DeveloperProject> {
    let mut roots = Vec::new();
    if let Some(desktop) = dirs::desktop_dir() {
        roots.push(desktop.join("Claude"));
        roots.push(desktop.join("Projects"));
    }
    if let Some(documents) = dirs::document_dir() {
        roots.push(documents);
    }

    let mut seen = HashSet::new();
    let mut projects = Vec::new();
    for root in roots {
        for candidate in root_and_children(&root) {
            let Ok(project) = inspect_project(&candidate) else {
                continue;
            };
            if seen.insert(project.path.clone()) {
                projects.push(project);
            }
        }
    }
    projects.sort_by_key(|project| project.name.to_lowercase());
    projects
}

pub fn inspect_project(path: &Path) -> Result<DeveloperProject> {
    let path = path
        .canonicalize()
        .map_err(|_| Error::NotFound(format!("Project folder: {}", path.display())))?;
    if !path.is_dir() {
        return Err(Error::Other("The selected project is not a folder.".into()));
    }

    let kind = detect_kind(&path).ok_or_else(|| {
        Error::Other(
            "No supported project found. Choose a folder containing gradlew or package.json."
                .into(),
        )
    })?;
    let (name, version) = project_identity(&path, kind);
    let artifact = latest_jar(&path);
    let (artifact_path, artifact_name, artifact_modified_at) = match artifact {
        Some((jar, modified)) => (
            Some(jar.to_string_lossy().to_string()),
            jar.file_name()
                .map(|name| name.to_string_lossy().to_string()),
            Some(modified),
        ),
        None => (None, None, None),
    };

    let git_state = git_state(&path);
    Ok(DeveloperProject {
        path: path.to_string_lossy().to_string(),
        name,
        version,
        kind: kind.to_string(),
        modified: matches!(git_state.as_str(), "modified" | "uncommitted"),
        git_state,
        artifact_path,
        artifact_name,
        artifact_modified_at,
    })
}

pub async fn run_task(path: &Path, task: &str) -> Result<DeveloperTaskResult> {
    if !matches!(task, "build" | "test") {
        return Err(Error::Other(format!("Unsupported developer task: {task}")));
    }
    let project = inspect_project(path)?;
    let project_path = PathBuf::from(&project.path);
    let (program, args, display) = task_command(&project_path, &project.kind, task)?;
    let started = Instant::now();
    let output = Command::new(&program)
        .args(&args)
        .current_dir(&project_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .output()
        .await?;

    let mut combined = String::new();
    if !output.stdout.is_empty() {
        combined.push_str(&String::from_utf8_lossy(&output.stdout));
    }
    if !output.stderr.is_empty() {
        if !combined.ends_with('\n') && !combined.is_empty() {
            combined.push('\n');
        }
        combined.push_str(&String::from_utf8_lossy(&output.stderr));
    }
    let combined = tail_chars(&combined, MAX_LOG_CHARS);
    let artifact = latest_jar(&project_path);

    Ok(DeveloperTaskResult {
        success: output.status.success(),
        task: task.to_string(),
        command: display,
        duration_ms: started.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
        output: combined,
        artifact_path: artifact
            .as_ref()
            .map(|(path, _)| path.to_string_lossy().to_string()),
        artifact_name: artifact.and_then(|(path, _)| {
            path.file_name()
                .map(|name| name.to_string_lossy().to_string())
        }),
    })
}

pub fn install_artifact(
    state: &AppState,
    project_path: &Path,
    instance_id: &str,
) -> Result<DeveloperInstallResult> {
    let project = inspect_project(project_path)?;
    if project.kind != "gradle" {
        return Err(Error::Other(
            "Only Gradle mod projects produce installable JAR files.".into(),
        ));
    }
    instances::get_instance(state, instance_id)?;
    let (artifact, _) = latest_jar(Path::new(&project.path))
        .ok_or_else(|| Error::NotFound("Built mod JAR. Run Build mod first.".into()))?;
    let file_name = artifact
        .file_name()
        .ok_or_else(|| Error::Other("Built JAR has no filename.".into()))?
        .to_string_lossy()
        .to_string();

    let mods_dir = state.dirs.game_dir(instance_id).join("mods");
    std::fs::create_dir_all(&mods_dir)?;
    let registry_path = state.dirs.root.join("developer_installs.json");
    let mut registry = load_registry(&registry_path);
    let prior_name = registry
        .projects
        .get(&project.path)
        .and_then(|instances| instances.get(instance_id))
        .cloned();

    let mut backup = None;
    if let Some(prior_name) = prior_name {
        let prior = mods_dir.join(&prior_name);
        if prior.is_file() {
            let backup_dir = mods_dir.join(".ezmapa-dev-backups");
            std::fs::create_dir_all(&backup_dir)?;
            let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
            let backup_path = backup_dir.join(format!("{stamp}-{prior_name}"));
            std::fs::rename(&prior, &backup_path)?;
            backup = Some(backup_path.to_string_lossy().to_string());
        }
    }

    let destination = mods_dir.join(&file_name);
    if destination.exists() {
        let backup_dir = mods_dir.join(".ezmapa-dev-backups");
        std::fs::create_dir_all(&backup_dir)?;
        let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
        let backup_path = backup_dir.join(format!("{stamp}-{file_name}"));
        std::fs::rename(&destination, &backup_path)?;
        backup = Some(backup_path.to_string_lossy().to_string());
    }
    std::fs::copy(&artifact, &destination)?;

    registry
        .projects
        .entry(project.path)
        .or_default()
        .insert(instance_id.to_string(), file_name.clone());
    save_registry(&registry_path, &registry)?;

    Ok(DeveloperInstallResult {
        file_name,
        destination: destination.to_string_lossy().to_string(),
        backup,
    })
}

fn root_and_children(root: &Path) -> Vec<PathBuf> {
    if !root.is_dir() {
        return Vec::new();
    }
    let mut paths = vec![root.to_path_buf()];
    if let Ok(entries) = std::fs::read_dir(root) {
        paths.extend(
            entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.is_dir() && !is_ignored_dir(path)),
        );
    }
    paths
}

fn is_ignored_dir(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some(
            ".git" | ".gradle" | "build" | "dist" | "node_modules" | "target" | "AppData" | "Codex"
        )
    )
}

fn detect_kind(path: &Path) -> Option<&'static str> {
    if path.join("gradlew").is_file() || path.join("gradlew.bat").is_file() {
        Some("gradle")
    } else if path.join("package.json").is_file() && path.join("src-tauri").is_dir() {
        Some("tauri")
    } else if path.join("package.json").is_file() {
        Some("node")
    } else {
        None
    }
}

fn project_identity(path: &Path, kind: &str) -> (String, Option<String>) {
    if matches!(kind, "node" | "tauri") {
        if let Ok(text) = std::fs::read_to_string(path.join("package.json")) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                let name = json
                    .get("name")
                    .and_then(|value| value.as_str())
                    .map(pretty_name)
                    .unwrap_or_else(|| folder_name(path));
                let version = json
                    .get("version")
                    .and_then(|value| value.as_str())
                    .map(str::to_string);
                return (name, version);
            }
        }
    }

    let properties = read_properties(&path.join("gradle.properties"));
    let name = properties
        .get("mod_name")
        .or_else(|| properties.get("mod_id"))
        .cloned()
        .or_else(|| gradle_root_name(path))
        .unwrap_or_else(|| folder_name(path));
    let version = properties
        .get("mod_version")
        .or_else(|| properties.get("version"))
        .cloned();
    (pretty_name(&name), version)
}

fn read_properties(path: &Path) -> HashMap<String, String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return HashMap::new();
    };
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (key, value) = line.split_once('=')?;
            Some((key.trim().to_string(), value.trim().to_string()))
        })
        .collect()
}

fn gradle_root_name(path: &Path) -> Option<String> {
    for file in ["settings.gradle", "settings.gradle.kts"] {
        let Ok(text) = std::fs::read_to_string(path.join(file)) else {
            continue;
        };
        for line in text.lines() {
            let compact = line.trim();
            if !compact.starts_with("rootProject.name") {
                continue;
            }
            let (_, value) = compact.split_once('=')?;
            return Some(
                value
                    .trim()
                    .trim_matches(|c| c == '\'' || c == '"')
                    .to_string(),
            );
        }
    }
    None
}

fn latest_jar(path: &Path) -> Option<(PathBuf, u64)> {
    let libs = path.join("build").join("libs");
    let entries = std::fs::read_dir(libs).ok()?;
    entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_string_lossy().to_lowercase();
            if path.extension().and_then(|ext| ext.to_str()) != Some("jar")
                || name.contains("-sources")
                || name.contains("-javadoc")
                || name.contains("-dev")
            {
                return None;
            }
            let modified = entry
                .metadata()
                .ok()?
                .modified()
                .ok()?
                .duration_since(UNIX_EPOCH)
                .ok()?
                .as_secs();
            Some((path, modified))
        })
        .max_by_key(|(_, modified)| *modified)
}

fn git_output(path: &Path, args: &[&str]) -> Option<std::process::Output> {
    std::process::Command::new("git")
        .args(args)
        .current_dir(path)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()
}

fn git_state(path: &Path) -> String {
    let inside = git_output(path, &["rev-parse", "--is-inside-work-tree"])
        .map(|output| output.status.success())
        .unwrap_or(false);
    if !inside {
        return "notRepository".into();
    }

    let has_head = git_output(path, &["rev-parse", "--verify", "HEAD"])
        .map(|output| output.status.success())
        .unwrap_or(false);
    if !has_head {
        return "uncommitted".into();
    }

    let changed = git_output(path, &["status", "--porcelain"])
        .map(|output| output.status.success() && !output.stdout.is_empty())
        .unwrap_or(false);
    if changed {
        "modified".into()
    } else {
        "clean".into()
    }
}

fn task_command(path: &Path, kind: &str, task: &str) -> Result<(PathBuf, Vec<String>, String)> {
    match kind {
        "gradle" => {
            #[cfg(target_os = "windows")]
            let wrapper = path.join("gradlew.bat");
            #[cfg(not(target_os = "windows"))]
            let wrapper = path.join("gradlew");
            if !wrapper.is_file() {
                return Err(Error::NotFound("Gradle wrapper.".into()));
            }
            #[cfg(target_os = "windows")]
            return Ok((
                PathBuf::from("cmd.exe"),
                vec![
                    "/D".into(),
                    "/S".into(),
                    "/C".into(),
                    format!("gradlew.bat --console=plain {task}"),
                ],
                format!("gradlew.bat --console=plain {task}"),
            ));
            #[cfg(not(target_os = "windows"))]
            let args = vec!["--console=plain".to_string(), task.to_string()];
            #[cfg(not(target_os = "windows"))]
            Ok((
                wrapper.clone(),
                args,
                format!("{} --console=plain {task}", wrapper.display()),
            ))
        }
        "node" | "tauri" => {
            #[cfg(target_os = "windows")]
            return Ok((
                PathBuf::from("cmd.exe"),
                vec![
                    "/D".into(),
                    "/S".into(),
                    "/C".into(),
                    format!("npm.cmd run {task}"),
                ],
                format!("npm run {task}"),
            ));
            #[cfg(not(target_os = "windows"))]
            let npm = PathBuf::from("npm");
            #[cfg(not(target_os = "windows"))]
            Ok((
                npm,
                vec!["run".into(), task.into()],
                format!("npm run {task}"),
            ))
        }
        _ => Err(Error::Other("Unsupported project type.".into())),
    }
}

fn folder_name(path: &Path) -> String {
    path.file_name()
        .map(|name| pretty_name(&name.to_string_lossy()))
        .unwrap_or_else(|| "Local project".into())
}

fn pretty_name(value: &str) -> String {
    value
        .trim()
        .trim_end_matches("-main")
        .replace(['_', '-'], " ")
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn tail_chars(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    let start = value
        .char_indices()
        .nth(value.chars().count() - max)
        .map(|(index, _)| index)
        .unwrap_or(0);
    format!("[Earlier output trimmed]\n{}", &value[start..])
}

fn load_registry(path: &Path) -> InstallRegistry {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save_registry(path: &Path, registry: &InstallRegistry) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, serde_json::to_vec_pretty(registry)?)?;
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    std::fs::rename(temp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pretty_names_project_slugs() {
        assert_eq!(pretty_name("aetherium_tools-main"), "Aetherium Tools");
        assert_eq!(pretty_name("ezmapa-launcher"), "Ezmapa Launcher");
    }

    #[test]
    fn picks_release_jar_over_sources() {
        let root = std::env::temp_dir().join(format!("ezmapa_dev_{}", uuid::Uuid::new_v4()));
        let libs = root.join("build").join("libs");
        std::fs::create_dir_all(&libs).unwrap();
        std::fs::write(libs.join("example-1.0-sources.jar"), b"sources").unwrap();
        std::fs::write(libs.join("example-1.0.jar"), b"release").unwrap();
        let selected = latest_jar(&root).unwrap().0;
        assert_eq!(
            selected.file_name().and_then(|name| name.to_str()),
            Some("example-1.0.jar")
        );
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn distinguishes_git_states() {
        let root = std::env::temp_dir().join(format!("ezmapa_git_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        assert_eq!(git_state(&root), "notRepository");

        let status = std::process::Command::new("git")
            .args(["init"])
            .current_dir(&root)
            .status()
            .unwrap();
        assert!(status.success());
        assert_eq!(git_state(&root), "uncommitted");
        std::fs::remove_dir_all(root).ok();
    }
}
