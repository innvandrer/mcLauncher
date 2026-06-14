mod auth;
mod commands;
mod error;
mod instances;
mod java;
mod launch;
mod models;
mod modloader;
mod modrinth;
mod mojang;
mod net;
mod state;

use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir).ok();
            app.manage(AppState::new(data_dir));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_minecraft_versions,
            commands::list_fabric_versions,
            commands::list_quilt_versions,
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
            commands::search_modrinth,
            commands::install_mod,
            commands::list_mods,
            commands::set_mod_enabled,
            commands::delete_mod,
            commands::detect_java,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Beacon");
}
