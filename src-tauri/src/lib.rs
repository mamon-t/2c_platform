mod auth;
mod audit;
mod commands;
mod core;
mod crypto;
mod db;
mod events;
mod ledger;
mod meta;
mod notify;
mod rhai;

use commands::AppState;
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
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::get_diagnostics,
            commands::validate_rhai_script,
            commands::execute_rhai_script,
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
