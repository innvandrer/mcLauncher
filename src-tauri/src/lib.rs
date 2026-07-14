mod account_tokens;
mod archive;
mod auth;
mod commands;
mod crosssource;
mod discord;
mod curseforge;
mod export;
mod forge;
mod skin;
mod error;
mod instances;
mod java;
mod launch;
mod models;
mod modloader;
mod modrinth;
mod mojang;
mod net;
mod jvmtune;
mod preflight;
mod server_mods;
mod servers;
mod shortcuts;
mod startup;
mod state;
mod system;
mod tools;
mod turbo;

use state::AppState;
use tauri::Manager;

/// Move data from the old Beacon app identifier on first launch after rebrand.
fn migrate_legacy_data_dir(data_dir: &std::path::Path) {
    if data_dir.exists() {
        return;
    }
    let Some(roaming) = dirs::data_dir() else {
        return;
    };
    let legacy = roaming.join("com.beacon.launcher");
    if !legacy.is_dir() {
        return;
    }
    if std::fs::rename(&legacy, data_dir).is_ok() {
        return;
    }
    // Fall back to a recursive copy when rename fails (e.g. cross-volume).
    let _ = instances::copy_dir(&legacy, data_dir);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Set by a desktop shortcut (see `shortcuts.rs`): jump straight into a
    // world/server on startup instead of waiting for the user to click Play.
    let quick_launch = shortcuts::parse_quick_launch(&std::env::args().skip(1).collect::<Vec<_>>());

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init());

    // The self-updater is desktop-only; mobile targets don't ship installers.
    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    }

    builder
        .setup(move |app| {
            let data_dir = app.path().app_data_dir()?;
            migrate_legacy_data_dir(&data_dir);
            std::fs::create_dir_all(&data_dir).ok();
            app.manage(AppState::new(data_dir));

            // System tray
            #[cfg(desktop)]
            setup_tray(app)?;

            if let Some(ql) = quick_launch {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let state = handle.state::<AppState>();
                    if let Err(e) = launch::launch(
                        &handle,
                        state.inner(),
                        &ql.instance_id,
                        ql.world.as_deref(),
                        ql.server.as_deref(),
                    )
                    .await
                    {
                        eprintln!("quick launch failed: {e}");
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_minecraft_versions,
            commands::list_fabric_versions,
            commands::list_quilt_versions,
            commands::list_forge_versions,
            commands::list_neoforge_versions,
            commands::get_skin,
            commands::set_skin_url,
            commands::set_skin_file,
            commands::get_settings,
            commands::save_settings,
            commands::list_accounts,
            commands::login_microsoft,
            commands::add_offline_account,
            commands::set_active_account,
            commands::remove_account,
            commands::list_instances,
            commands::get_instance,
            commands::create_instance,
            commands::update_instance,
            commands::delete_instance,
            commands::duplicate_instance,
            commands::open_instance_folder,
            commands::launch_instance,
            commands::stop_instance,
            commands::running_instances,
            commands::create_shortcut,
            commands::search_modrinth,
            commands::search_curseforge,
            commands::install_mod,
            commands::install_content,
            commands::install_curseforge_content,
            commands::install_mrpack,
            commands::install_modrinth_modpack,
            commands::install_curseforge_modpack,
            commands::list_modrinth_versions,
            commands::list_curseforge_files,
            commands::check_mod_updates,
            commands::apply_mod_update,
            commands::set_mod_source,
            commands::auto_update_instance_content,
            commands::export_instance,
            commands::import_instance,
            commands::export_mrpack,
            commands::export_curseforge_pack,
            commands::prepare_pack_export,
            commands::import_mrpack,
            commands::list_mods,
            commands::set_mod_enabled,
            commands::delete_mod,
            commands::save_loadout,
            commands::apply_loadout,
            commands::delete_loadout,
            commands::save_png,
            commands::list_resource_packs,
            commands::delete_resource_pack,
            commands::list_shaders,
            commands::delete_shader,
            commands::list_worlds,
            commands::delete_world,
            commands::open_world_folder,
            commands::list_screenshots,
            commands::open_screenshot,
            commands::instance_disk_usage,
            commands::list_snapshots,
            commands::create_snapshot,
            commands::restore_snapshot,
            commands::delete_snapshot,
            commands::scan_mod_conflicts,
            commands::resolve_mod_conflicts,
            commands::preflight_check,
            commands::apply_turbo,
            commands::cancel_task,
            commands::check_modpack_update,
            commands::apply_modpack_update,
            commands::detect_java,
            commands::suggest_jvm_args,
            commands::startup_stats,
            commands::list_servers,
            commands::ping_server,
            commands::analyze_server_mods,
            commands::create_instance_from_server,
            commands::list_sessions,
            commands::list_saved_skins,
            commands::save_skin,
            commands::save_skin_file,
            commands::delete_saved_skin,
            commands::fetch_player_skin,
            commands::system_memory_mb,
            commands::get_modrinth_project_body,
            commands::get_curseforge_description,
            commands::install_content_version,
            commands::install_curseforge_file,
            commands::open_url,
            commands::upload_log,
        ])
        .run(tauri::generate_context!())
        .expect("error while running EZMapa");
}

#[cfg(desktop)]
fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    use tauri::menu::{Menu, MenuItem};

    let show = MenuItem::with_id(app, "show", "Open EZMapa", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .tooltip("EZMapa Launcher")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // Left-click on tray icon: toggle window visibility.
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(win) = app.get_webview_window("main") {
                    if win.is_visible().unwrap_or(false) {
                        let _ = win.hide();
                    } else {
                        let _ = win.show();
                        let _ = win.set_focus();
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
}
