//! Assembles the Java command line for an instance and launches the game,
//! streaming stdout/stderr back to the UI as `instance://log` events.

use crate::error::{Error, Result};
use crate::models::{InstanceState, LogLine};
use crate::mojang::{self, ArgValue, Argument};
use crate::state::AppState;
use crate::{auth, instances, java, modloader};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::process::Stdio;
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
) {
    let Some(stream) = stream else { return };
    std::thread::spawn(move || {
        let reader = BufReader::new(stream);
        for line in reader.lines() {
            match line {
                Ok(line) => {
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

    // --- Account (refresh Microsoft token if expired) ------------------------
    let mut account = instances::active_account(state)
        .ok_or_else(|| Error::Auth("No account selected. Add an account first.".into()))?;
    if account.kind == "microsoft" && account.expires_at <= chrono::Utc::now().timestamp() {
        if let Some(rt) = account.refresh_token.clone() {
            account = auth::refresh(state, &rt).await?;
            instances::upsert_account(state, account.clone())?;
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
    cp.push(
        state
            .dirs
            .version_jar(&resolved.client_id)
            .to_string_lossy()
            .to_string(),
    );
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
    vars.insert(
        "auth_access_token",
        if account.access_token.is_empty() {
            "0".to_string()
        } else {
            account.access_token.clone()
        },
    );
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
    vars.insert("launcher_name", "beacon".to_string());
    vars.insert("launcher_version", env!("CARGO_PKG_VERSION").to_string());
    vars.insert("classpath", classpath.clone());
    vars.insert("classpath_separator", sep.to_string());
    vars.insert(
        "library_directory",
        state.dirs.libraries().to_string_lossy().to_string(),
    );

    let features: HashMap<String, bool> = HashMap::new();

    // --- Steg 2: Kjør Forge/NeoForge Processors -----------------------------
    if !resolved.processors.is_empty() {
        let _ = app.emit(
            "instance://log",
            LogLine {
                instance_id: instance.id.clone(),
                line: "[Beacon] Kjører Forge-prosessorer (patcher spillet)...".into(),
                is_err: false,
            },
        );

        for proc in &resolved.processors {
            let mut proc_args = Vec::new();
            for arg in &proc.args {
                proc_args.push(substitute(arg, &vars));
            }

            let mut cmd = std::process::Command::new(&java_path);
            // Forge-prosessorer trenger tilgang til hele classpath for å patche korrekt
            cmd.arg("-cp").arg(&classpath);
            cmd.arg(&proc.main_class);
            cmd.args(&proc_args);
            cmd.current_dir(&game_dir);

            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(CREATE_NO_WINDOW);
            }

            let status = cmd.status().map_err(|e| Error::Other(format!("Prosessor feilet: {e}")))?;
            if !status.success() {
                return Err(Error::Other(format!("Prosessor {} feilet med kode {:?}", proc.main_class, status.code())));
            }
        }
    }

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

    // --- Spawn ---------------------------------------------------------------
    let mut cmd = std::process::Command::new(&java_path);
    cmd.args(&jvm);
    cmd.arg(&resolved.main_class);
    cmd.args(&game);
    cmd.current_dir(&game_dir);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd.spawn().map_err(|e| {
        Error::Other(format!(
            "Failed to start Java ({java_path}): {e}. Check the Java path in Settings."
        ))
    })?;
    let pid = child.id();

    spawn_reader(app.clone(), instance.id.clone(), child.stdout.take(), false);
    spawn_reader(app.clone(), instance.id.clone(), child.stderr.take(), true);

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
            line: format!("[Beacon] Launching {} ({})", instance.name, resolved.id),
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
    let start = std::time::Instant::now();
    std::thread::spawn(move || {
        let status = child.wait();
        let secs = start.elapsed().as_secs();
        instances::record_play_dirs(&dirs, &inst_id, secs);
        discord.set_idle();
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
                line: format!("[Beacon] Process exited with code {code:?}"),
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
        let _ = std::process::Command::new("kill")
            .arg(pid.to_string())
            .output();
    }
    Ok(())
}

pub fn running_ids(state: &AppState) -> Vec<String> {
    state.running.lock().unwrap().keys().cloned().collect()
}
