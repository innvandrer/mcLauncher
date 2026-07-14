//! Assembles the Java command line for an instance and launches the game,
//! streaming stdout/stderr back to the UI as `instance://log` events.

use crate::error::{Error, Result};
use crate::models::{InstanceState, LogLine};
use crate::mojang::{self, ArgValue, Argument};
use crate::state::AppState;
use crate::{auth, instances, java, modloader, modrinth};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn substitute(s: &str, vars: &HashMap<&str, String>) -> String {
    let mut out = s.to_string();
    for (k, v) in vars {
        out = out.replace(&format!("${{{k}}}"), v);
    }
    out
}

fn push_arg(
    buf: &mut Vec<String>,
    arg: &Argument,
    vars: &HashMap<&str, String>,
    features: &HashMap<String, bool>,
) {
    match arg {
        Argument::Plain(s) => buf.push(substitute(s, vars)),
        Argument::Conditional { rules, value } => {
            if mojang::rules_allow(rules, features) {
                match value {
                    ArgValue::One(s) => buf.push(substitute(s, vars)),
                    ArgValue::Many(v) => {
                        for s in v {
                            buf.push(substitute(s, vars));
                        }
                    }
                }
            }
        }
    }
}

fn spawn_reader<R: Read + Send + 'static>(
    app: AppHandle,
    instance_id: String,
    stream: Option<R>,
    is_err: bool,
    shader_guard: Arc<AtomicBool>,
    startup: Arc<crate::startup::StartupTracker>,
) {
    let Some(stream) = stream else { return };
    std::thread::spawn(move || {
        let reader = BufReader::new(stream);
        for line in reader.lines() {
            match line {
                Ok(line) => {
                    startup.observe_line(&line);
                    maybe_autoinstall_shader(&app, &instance_id, &line, &shader_guard);
                    let _ = app.emit(
                        "instance://log",
                        LogLine {
                            instance_id: instance_id.clone(),
                            line,
                            is_err,
                        },
                    );
                }
                Err(_) => break,
            }
        }
    });
}

/// Parse the Complementary version token (e.g. `r5.7.1`) out of a EuphoriaPatcher
/// log line such as `Required: ComplementaryShaders r5.7.1`.
fn parse_complementary_version(line: &str) -> Option<String> {
    if !line.to_lowercase().contains("complementary") {
        return None;
    }
    for raw in line.split_whitespace() {
        let tok = raw.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '.');
        let mut chars = tok.chars();
        if matches!(chars.next(), Some('r') | Some('R'))
            && matches!(chars.next(), Some(d) if d.is_ascii_digit())
            && tok.contains('.')
        {
            return Some(tok.to_string());
        }
    }
    None
}

/// When EuphoriaPatcher reports that its required base Complementary shader is
/// missing, download it from Modrinth into the instance automatically. Fires at
/// most once per launch (guarded by `shader_guard`); the user just needs to
/// relaunch for the patcher to pick it up.
fn maybe_autoinstall_shader(
    app: &AppHandle,
    instance_id: &str,
    line: &str,
    shader_guard: &Arc<AtomicBool>,
) {
    let lower = line.to_lowercase();
    if !(lower.contains("euphoriapatcher") || lower.contains("shader not found")) {
        return;
    }
    let Some(version) = parse_complementary_version(line) else {
        return;
    };
    // Only fire once per launch.
    if shader_guard.swap(true, Ordering::SeqCst) {
        return;
    }

    let app = app.clone();
    let instance_id = instance_id.to_string();
    tauri::async_runtime::spawn(async move {
        let _ = app.emit(
            "instance://log",
            LogLine {
                instance_id: instance_id.clone(),
                line: format!(
                    "[EZMapa] EuphoriaPatcher needs Complementary {version} — downloading it from Modrinth..."
                ),
                is_err: false,
            },
        );
        let state = app.state::<AppState>();
        match modrinth::ensure_complementary_shader(&state, &instance_id, &version).await {
            Ok(Some(file_name)) => {
                let _ = app.emit(
                    "instance://log",
                    LogLine {
                        instance_id: instance_id.clone(),
                        line: format!(
                            "[EZMapa] Installed {file_name}. Relaunch to enable EuphoriaPatcher shaders."
                        ),
                        is_err: false,
                    },
                );
                let _ = app.emit(
                    "instance://shader-installed",
                    serde_json::json!({
                        "instanceId": instance_id,
                        "fileName": file_name,
                        "version": version,
                    }),
                );
            }
            Ok(None) => {}
            Err(e) => {
                let _ = app.emit(
                    "instance://log",
                    LogLine {
                        instance_id: instance_id.clone(),
                        line: format!(
                            "[EZMapa] Could not auto-install Complementary {version}: {e}"
                        ),
                        is_err: true,
                    },
                );
            }
        }
    });
}

/// Launch an instance. Returns once the process has started; the game then runs
/// independently and reports state via events.
pub async fn launch(
    app: &AppHandle,
    state: &AppState,
    instance_id: &str,
    quick_world: Option<&str>,
    quick_server: Option<&str>,
) -> Result<()> {
    if state.running.lock().unwrap().contains_key(instance_id) {
        return Err(Error::Other("This instance is already running.".into()));
    }

    let instance = instances::get_instance(state, instance_id)?;
    let settings = instances::load_settings(state);

    // Drop duplicate mod jars (same mod, two versions) before the game starts.
    let removed = crate::tools::resolve_mod_conflicts(state, instance_id);
    if !removed.is_empty() {
        let _ = app.emit(
            "instance://log",
            LogLine {
                instance_id: instance_id.to_string(),
                line: format!(
                    "[EZMapa] Removed {} duplicate mod {} before launch: {}",
                    removed.len(),
                    if removed.len() == 1 { "file" } else { "files" },
                    removed.join(", ")
                ),
                is_err: false,
            },
        );
    }

    // --- Account (refresh Microsoft session when missing or expired) ---------
    let mut account = instances::active_account(state)?;
    if account.kind == "microsoft" {
        let needs_refresh = account.access_token.is_empty()
            || account.expires_at <= chrono::Utc::now().timestamp();
        if needs_refresh {
            match account.refresh_token.clone().filter(|rt| !rt.is_empty()) {
                Some(rt) => {
                    account = auth::refresh(state, &rt).await?;
                    instances::upsert_account(state, account.clone())?;
                }
                // An expired session with no refresh token can't be renewed:
                // fail with clear guidance now instead of launching with a
                // stale token and letting server joins fail confusingly.
                None => {
                    return Err(crate::error::Error::Auth(
                        "Your Microsoft session has expired and can't be renewed \
                         automatically. Sign out of your Microsoft account in \
                         EZMapa, sign in again, then try launching."
                            .into(),
                    ));
                }
            }
        }
        if account.access_token.is_empty() {
            return Err(crate::error::Error::Auth(
                "No valid Microsoft session is stored. Sign out of your Microsoft \
                 account in EZMapa, sign in again, then try launching."
                    .into(),
            ));
        }
    }

    // --- Resolve + install game files ---------------------------------------
    let version_id = modloader::launch_version_id(
        app,
        state,
        &instance.mc_version,
        instance.loader,
        instance.loader_version.as_deref(),
    )
    .await?;

    let natives_dir = state.dirs.natives().join(&instance.id);
    let _ = std::fs::remove_dir_all(&natives_dir);

    let resolved = mojang::install(
        app,
        state,
        &version_id,
        &natives_dir,
        settings.max_concurrent_downloads,
    )
    .await?;

    // --- Java ----------------------------------------------------------------
    let java_path = match instance.java_path.clone().or(settings.java_path.clone()) {
        Some(p) if !p.trim().is_empty() => p,
        _ => java::ensure(state, resolved.java_major).await?,
    };

    // --- Classpath -----------------------------------------------------------
    let (_, _, cp_paths) = mojang::split_libraries(state, &resolved);
    let mut cp: Vec<String> = cp_paths
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    // The vanilla Minecraft client jar is downloaded under its base version id
    // (e.g. `versions/1.21.1/1.21.1.jar`). Module-based loaders (Forge/NeoForge
    // BootstrapLauncher) pass `-DignoreList=client-extra,${version_name}.jar` so
    // they can skip the raw client jar when building their transforming module
    // layer and use their own patched `minecraft` module instead. `version_name`
    // resolves to the launched version id, so the client jar on the classpath
    // must be named after that launched id (not the base version) — otherwise
    // the jar slips past the ignore list, gets loaded as an automatic module
    // (e.g. `_1._21._1`), and clashes with NeoForge's `minecraft` module:
    //   "Modules <id> and minecraft both export package ... to module ...".
    let client_src = state.dirs.version_jar(&resolved.client_id);
    let client_cp = if resolved.id != resolved.client_id {
        let dest = state.dirs.version_jar(&resolved.id);
        if client_src.exists() {
            if let Some(parent) = dest.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            // Re-copy only when missing or out of date so we don't pay the cost
            // on every launch.
            let needs_copy = match (std::fs::metadata(&dest), std::fs::metadata(&client_src)) {
                (Ok(d), Ok(s)) => d.len() != s.len(),
                _ => true,
            };
            if needs_copy {
                let _ = std::fs::copy(&client_src, &dest);
            }
        }
        dest
    } else {
        client_src
    };
    cp.push(client_cp.to_string_lossy().to_string());
    let sep = if cfg!(windows) { ";" } else { ":" };
    let classpath = cp.join(sep);

    // --- Placeholder variables ----------------------------------------------
    let game_dir = state.dirs.game_dir(&instance.id);
    std::fs::create_dir_all(game_dir.join("mods"))?;
    let assets_dir = state.dirs.assets();

    let mut vars: HashMap<&str, String> = HashMap::new();
    vars.insert("auth_player_name", account.username.clone());
    vars.insert("version_name", resolved.id.clone());
    vars.insert("game_directory", game_dir.to_string_lossy().to_string());
    vars.insert("assets_root", assets_dir.to_string_lossy().to_string());
    vars.insert("game_assets", assets_dir.to_string_lossy().to_string());
    vars.insert("assets_index_name", resolved.assets_id.clone());
    vars.insert("auth_uuid", account.id.clone());
    vars.insert("auth_access_token", account.access_token.clone());
    vars.insert("auth_xuid", account.xuid.clone().unwrap_or_default());
    vars.insert("clientid", String::new());
    vars.insert(
        "user_type",
        if account.kind == "microsoft" {
            "msa".to_string()
        } else {
            "legacy".to_string()
        },
    );
    vars.insert("version_type", "release".to_string());
    vars.insert("natives_directory", natives_dir.to_string_lossy().to_string());
    vars.insert("launcher_name", "ezmapa".to_string());
    vars.insert("launcher_version", env!("CARGO_PKG_VERSION").to_string());
    vars.insert("classpath", classpath.clone());
    vars.insert("classpath_separator", sep.to_string());
    vars.insert(
        "library_directory",
        state.dirs.libraries().to_string_lossy().to_string(),
    );

    let features: HashMap<String, bool> = HashMap::new();

    // Forge/NeoForge processors are NOT run here: they live in the installer's
    // install_profile.json and the official installer executes them during
    // `--installClient` (see `forge::run_installer`), so the version JSON we
    // resolve is already fully patched.

    // --- JVM arguments -------------------------------------------------------
    let mut jvm: Vec<String> = Vec::new();
    let mem = instance.memory_mb.unwrap_or(settings.memory_mb);
    jvm.push(format!("-Xmx{mem}M"));
    jvm.push(format!("-Xms{}M", (mem / 2).max(512)));

    let user_jvm = instance.jvm_args.clone().unwrap_or(settings.jvm_args.clone());
    for tok in user_jvm.split_whitespace() {
        jvm.push(tok.to_string());
    }

    if resolved.jvm_args.is_empty() {
        // Legacy versions (<1.13) have no JVM arg template.
        jvm.push(format!("-Djava.library.path={}", natives_dir.to_string_lossy()));
        jvm.push("-cp".to_string());
        jvm.push(classpath.clone());
    } else {
        for arg in &resolved.jvm_args {
            push_arg(&mut jvm, arg, &vars, &features);
        }
    }

    // --- Game arguments ------------------------------------------------------
    let mut game: Vec<String> = Vec::new();
    if !resolved.game_args.is_empty() {
        for arg in &resolved.game_args {
            push_arg(&mut game, arg, &vars, &features);
        }
    } else if let Some(legacy) = &resolved.legacy_args {
        for tok in legacy.split_whitespace() {
            game.push(substitute(tok, &vars));
        }
    }

    // --- Quick Play (jump straight into a world / server) --------------------
    // Supported on 1.20+; older clients simply ignore the unknown args.
    if let Some(world) = quick_world.filter(|s| !s.is_empty()) {
        game.push("--quickPlaySingleplayer".to_string());
        game.push(world.to_string());
    } else if let Some(server) = quick_server.filter(|s| !s.is_empty()) {
        game.push("--quickPlayMultiplayer".to_string());
        game.push(server.to_string());
    }

    // --- Window size ---------------------------------------------------------
    if let (Some(w), Some(h)) = (instance.window_width, instance.window_height) {
        if w > 0 && h > 0 {
            game.push("--width".to_string());
            game.push(w.to_string());
            game.push("--height".to_string());
            game.push(h.to_string());
        }
    }

    // --- Spawn ---------------------------------------------------------------
    let mut cmd = std::process::Command::new(&java_path);
    cmd.args(&jvm);
    cmd.arg(&resolved.main_class);
    cmd.args(&game);
    cmd.current_dir(&game_dir);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    // Per-instance environment variables (KEY=VALUE lines).
    if let Some(env) = &instance.env_vars {
        for line in env.lines() {
            if let Some((k, v)) = line.split_once('=') {
                let k = k.trim();
                if !k.is_empty() {
                    cmd.env(k, v.trim());
                }
            }
        }
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    // Put the JVM in its own process group (pgid = its own pid) so `stop()`
    // can signal the whole tree — including any native subprocesses the game
    // or a mod spawns — instead of just the JVM itself.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    // Pre-launch hook (waits for the command, but off the async runtime — a
    // long-running hook must not pin a worker thread).
    if let Some(pre) = instance.pre_launch.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let _ = app.emit(
            "instance://log",
            LogLine {
                instance_id: instance.id.clone(),
                line: format!("[EZMapa] Running pre-launch command: {pre}"),
                is_err: false,
            },
        );
        let pre = pre.to_string();
        let hook_dir = game_dir.clone();
        tokio::task::spawn_blocking(move || run_shell(&pre, &hook_dir)).await?;
    }

    let mut child = cmd.spawn().map_err(|e| {
        Error::Other(format!(
            "Failed to start Java ({java_path}): {e}. Check the Java path in Settings."
        ))
    })?;
    let pid = child.id();

    let shader_guard = Arc::new(AtomicBool::new(false));
    // Measure time-to-playable from the readiness markers in the log, keyed
    // to the JVM settings this session launched with.
    let startup_tracker = crate::startup::StartupTracker::new(
        state.dirs.clone(),
        instance.id.clone(),
        crate::startup::args_fingerprint(mem, &user_jvm),
    );
    spawn_reader(
        app.clone(),
        instance.id.clone(),
        child.stdout.take(),
        false,
        shader_guard.clone(),
        startup_tracker.clone(),
    );
    spawn_reader(
        app.clone(),
        instance.id.clone(),
        child.stderr.take(),
        true,
        shader_guard,
        startup_tracker,
    );

    state
        .running
        .lock()
        .unwrap()
        .insert(instance.id.clone(), pid);
    let _ = app.emit(
        "instance://state",
        InstanceState {
            instance_id: instance.id.clone(),
            running: true,
            exit_code: None,
        },
    );
    let _ = app.emit(
        "instance://log",
        LogLine {
            instance_id: instance.id.clone(),
            line: format!("[EZMapa] Launching {} ({})", instance.name, resolved.id),
            is_err: false,
        },
    );
    state.discord.set_playing(
        &instance.name,
        &instance.mc_version,
        instance.loader.as_str(),
    );

    if settings.close_on_launch {
        if let Some(win) = app.get_webview_window("main") {
            let _ = win.hide();
        }
    }

    // --- Watcher thread ------------------------------------------------------
    let app2 = app.clone();
    let running_map = state.running.clone();
    let dirs = state.dirs.clone();
    let discord = state.discord.clone();
    let inst_id = instance.id.clone();
    let close_on_launch = settings.close_on_launch;
    let post_exit = instance.post_exit.clone();
    let start = std::time::Instant::now();
    let started_at = chrono::Utc::now().timestamp();
    std::thread::spawn(move || {
        let status = child.wait();
        let secs = start.elapsed().as_secs();
        instances::record_play_dirs(&dirs, &inst_id, secs);
        instances::record_session_dirs(&dirs, &inst_id, started_at, secs);
        discord.set_idle();
        if let Some(cmd) = post_exit.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            run_shell(cmd, &dirs.game_dir(&inst_id));
        }
        if let Ok(mut m) = running_map.lock() {
            m.remove(&inst_id);
        }
        let code = status.ok().and_then(|s| s.code());
        if close_on_launch {
            if let Some(win) = app2.get_webview_window("main") {
                let _ = win.show();
                let _ = win.set_focus();
            }
        }
        let _ = app2.emit(
            "instance://log",
            LogLine {
                instance_id: inst_id.clone(),
                line: format!("[EZMapa] Process exited with code {code:?}"),
                is_err: code.unwrap_or(0) != 0,
            },
        );
        let _ = app2.emit(
            "instance://state",
            InstanceState {
                instance_id: inst_id,
                running: false,
                exit_code: code,
            },
        );
    });

    Ok(())
}

/// Forcefully stop a running instance.
pub fn stop(state: &AppState, instance_id: &str) -> Result<()> {
    let pid = state.running.lock().unwrap().get(instance_id).copied();
    let Some(pid) = pid else {
        return Ok(());
    };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
    }
    #[cfg(not(windows))]
    {
        // The JVM was spawned into its own process group (see `launch`), so
        // signalling `-pid` (the group) reaches the game and any native
        // subprocesses it spawned, not just the JVM's own pid.
        let _ = std::process::Command::new("kill")
            .args(["-TERM", "--", &format!("-{pid}")])
            .output();
    }
    Ok(())
}

pub fn running_ids(state: &AppState) -> Vec<String> {
    state.running.lock().unwrap().keys().cloned().collect()
}

/// Run a user-provided shell command (pre-launch / post-exit hook), blocking
/// until it finishes. Failures are ignored — hooks are best-effort.
fn run_shell(command: &str, dir: &std::path::Path) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("cmd")
            .args(["/C", command])
            .current_dir(dir)
            .creation_flags(CREATE_NO_WINDOW)
            .status();
    }
    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new("sh")
            .args(["-c", command])
            .current_dir(dir)
            .status();
    }
}

#[cfg(all(test, unix))]
mod process_group_tests {
    /// `stop()`'s group-kill (`kill -TERM -- -pid`) only reaches the right
    /// processes if the child's process group id actually equals its own
    /// pid, per `process_group(0)`'s documented behavior. That's the one
    /// precondition worth pinning down with a test — it's deterministic and
    /// doesn't depend on the host's signal-delivery behavior, which sandboxed
    /// dev containers can virtualize in ways a real target machine won't (and
    /// this crate's CI only runs on windows-latest, so a cfg(unix) test never
    /// executes there regardless — this validates the Unix half locally).
    #[test]
    fn process_group_zero_makes_the_child_its_own_group_leader() {
        use std::os::unix::process::CommandExt;

        let mut child = std::process::Command::new("sleep")
            .arg("5")
            .process_group(0)
            .spawn()
            .expect("spawn child");
        let pid = child.id();

        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).expect("read /proc/pid/stat");
        // Field 5 is pgid; field 2 (comm) may itself contain spaces/parens,
        // so split on the closing paren of comm rather than by whitespace.
        let after_comm = stat.rsplit_once(')').map(|(_, rest)| rest).unwrap_or(&stat);
        let pgid: u32 = after_comm
            .split_whitespace()
            .nth(2) // state, ppid, pgid -> index 2
            .and_then(|s| s.parse().ok())
            .expect("parse pgid from /proc/pid/stat");

        assert_eq!(pgid, pid, "process_group(0) must make the child its own group leader");

        let _ = child.kill();
        let _ = child.wait();
    }
}
