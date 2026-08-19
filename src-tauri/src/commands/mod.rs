use crate::db::{DiagnosticsInfo, MongoClient};
use crate::rhai::Sandbox;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub app_name: String,
    pub version: String,
    pub mongodb_uri: Option<String>,
    pub mongodb_database: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DiagnosticsReport {
    pub app_version: String,
    pub mongodb: DiagnosticsInfo,
    pub modules: Vec<ModuleInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleInfo {
    pub code: String,
    pub name: String,
    pub version: String,
    pub active: bool,
}

pub struct AppState {
    pub db: Option<MongoClient>,
}

impl AppState {
    pub fn new() -> Self {
        Self { db: None }
    }
}

#[tauri::command]
pub async fn get_diagnostics(state: State<'_, AppState>) -> Result<DiagnosticsReport, String> {
    let mongodb_info = match &state.db {
        Some(client) => client.diagnostics().await,
        None => DiagnosticsInfo {
            connected: false,
            host: "не подключено".to_string(),
            version: None,
            replica_set: None,
            ok: false,
        },
    };

    Ok(DiagnosticsReport {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        mongodb: mongodb_info,
        modules: vec![ModuleInfo {
            code: "core".to_string(),
            name: "Ядро платформы".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            active: true,
        }],
    })
}

#[tauri::command]
pub async fn validate_rhai_script(source: String) -> Result<(), String> {
    let sandbox = Sandbox::new(5000, 10000);
    sandbox.validate(&source).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn execute_rhai_script(source: String, context: String) -> Result<serde_json::Value, String> {
    let sandbox = Sandbox::new(5000, 10000);
    sandbox.execute(&source, &context).map_err(|e| e.to_string())
}
