mod auth;
mod audit;
mod commands;
mod company;
mod core;
mod crypto;
mod db;
mod events;
mod ledger;
mod meta;
mod notify;
mod rhai;
mod role;
mod user;

use commands::AppState;
use std::sync::Mutex;
use tauri::Manager;
use tracing_subscriber::EnvFilter;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,twoplat=debug")),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::default().build())
        .plugin(tauri_plugin_shell::init())
        .manage(Mutex::new(AppState::new()))
        .invoke_handler(tauri::generate_handler![
            commands::get_diagnostics,
            commands::validate_rhai_script,
            commands::execute_rhai_script,
            commands::connect_db,
            commands::list_companies,
            commands::get_company,
            commands::create_company,
            commands::update_company,
            commands::delete_company,
            commands::list_users,
            commands::get_user,
            commands::create_user,
            commands::update_user,
            commands::delete_user,
            commands::authenticate,
            commands::create_role,
            commands::list_roles,
            commands::delete_role,
            commands::get_me,
            commands::get_app_config,
            commands::save_app_config,
        ])
        .setup(|app| {
            tracing::info!("2C Platform запускается...");
            let window = app.get_webview_window("main").unwrap();
            window.set_title("2C Platform v0.1")?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("ошибка при запуске 2C Platform");
}
