use crate::auth::AuthService;
use crate::company::{Company, CompanyService, CreateCompanyInput, UpdateCompanyInput};
use crate::db::{DiagnosticsInfo, MongoClient};
use crate::role::{CreateRoleInput, Role, RoleService};
use crate::user::{CreateUserInput, UpdateUserInput, UserPublic, UserService};
use crate::rhai::Sandbox;
use serde::{Deserialize, Serialize};
use tauri::State;
use tokio::sync::Mutex;

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

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectInput {
    pub uri: String,
    pub db_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthResult {
    pub token: String,
    pub user: UserPublic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub mongodb_uri: Option<String>,
    pub mongodb_database: Option<String>,
}

pub struct AppState {
    pub db: Option<MongoClient>,
    pub auth: AuthService,
    pub config: AppConfig,
    pub current_user: Option<UserPublic>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            db: None,
            auth: AuthService::new("2c-platform-dev-secret-key-change-in-production"),
            config: AppConfig {
                mongodb_uri: None,
                mongodb_database: None,
            },
            current_user: None,
        }
    }
}

// ── Диагностика ───────────────────────────────────────────────

#[tauri::command]
pub async fn get_diagnostics(state: State<'_, Mutex<AppState>>) -> Result<DiagnosticsReport, String> {
    let state = state.lock().await;
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

// ── Подключение к БД ──────────────────────────────────────────

#[tauri::command]
pub async fn connect_db(
    state: State<'_, Mutex<AppState>>,
    input: ConnectInput,
) -> Result<DiagnosticsInfo, String> {
    let client = MongoClient::connect(&input.uri, &input.db_name)
        .await
        .map_err(|e| e.to_string())?;
    let info = client.diagnostics().await;

    {
        let mut state = state.lock().await;
        state.db = Some(client);
        state.config.mongodb_uri = Some(input.uri);
        state.config.mongodb_database = Some(input.db_name);
    }

    Ok(info)
}

// ── Конфиг ─────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_app_config(state: State<'_, Mutex<AppState>>) -> Result<AppConfig, String> {
    let state = state.lock().await;
    Ok(state.config.clone())
}

#[tauri::command]
pub async fn save_app_config(
    state: State<'_, Mutex<AppState>>,
    config: AppConfig,
) -> Result<(), String> {
    let mut state = state.lock().await;
    state.config = config;
    Ok(())
}

// ── Компании ──────────────────────────────────────────────────

#[tauri::command]
pub async fn list_companies(state: State<'_, Mutex<AppState>>) -> Result<Vec<Company>, String> {
    let state = state.lock().await;
    let db = state
        .db
        .as_ref()
        .ok_or_else(|| "Не подключено к MongoDB".to_string())?;
    CompanyService::list(db).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_company(
    state: State<'_, Mutex<AppState>>,
    id: String,
) -> Result<Company, String> {
    let state = state.lock().await;
    let db = state
        .db
        .as_ref()
        .ok_or_else(|| "Не подключено к MongoDB".to_string())?;
    let id = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    CompanyService::get(db, id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_company(
    state: State<'_, Mutex<AppState>>,
    input: CreateCompanyInput,
) -> Result<Company, String> {
    let state = state.lock().await;
    let db = state
        .db
        .as_ref()
        .ok_or_else(|| "Не подключено к MongoDB".to_string())?;
    CompanyService::create(db, input)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_company(
    state: State<'_, Mutex<AppState>>,
    id: String,
    input: UpdateCompanyInput,
) -> Result<Company, String> {
    let state = state.lock().await;
    let db = state
        .db
        .as_ref()
        .ok_or_else(|| "Не подключено к MongoDB".to_string())?;
    let id = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    CompanyService::update(db, id, input)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_company(
    state: State<'_, Mutex<AppState>>,
    id: String,
) -> Result<(), String> {
    let state = state.lock().await;
    let db = state
        .db
        .as_ref()
        .ok_or_else(|| "Не подключено к MongoDB".to_string())?;
    let id = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    CompanyService::delete(db, id).await.map_err(|e| e.to_string())
}

// ── Пользователи ──────────────────────────────────────────────

#[tauri::command]
pub async fn list_users(
    state: State<'_, Mutex<AppState>>,
    company_id: String,
) -> Result<Vec<UserPublic>, String> {
    let state = state.lock().await;
    let db = state
        .db
        .as_ref()
        .ok_or_else(|| "Не подключено к MongoDB".to_string())?;
    let cid = uuid::Uuid::parse_str(&company_id).map_err(|e| e.to_string())?;
    UserService::list(db, crate::core::CompanyId(cid))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_user(
    state: State<'_, Mutex<AppState>>,
    id: String,
) -> Result<UserPublic, String> {
    let state = state.lock().await;
    let db = state
        .db
        .as_ref()
        .ok_or_else(|| "Не подключено к MongoDB".to_string())?;
    let id = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    let user = UserService::get(db, id).await.map_err(|e| e.to_string())?;
    Ok(user.into())
}

#[tauri::command]
pub async fn create_user(
    state: State<'_, Mutex<AppState>>,
    input: CreateUserInput,
) -> Result<UserPublic, String> {
    let state = state.lock().await;
    let db = state
        .db
        .as_ref()
        .ok_or_else(|| "Не подключено к MongoDB".to_string())?;
    UserService::create(db, input, &state.auth)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_user(
    state: State<'_, Mutex<AppState>>,
    id: String,
    input: UpdateUserInput,
) -> Result<UserPublic, String> {
    let state = state.lock().await;
    let db = state
        .db
        .as_ref()
        .ok_or_else(|| "Не подключено к MongoDB".to_string())?;
    let id = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    UserService::update(db, id, input)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_user(
    state: State<'_, Mutex<AppState>>,
    id: String,
) -> Result<(), String> {
    let state = state.lock().await;
    let db = state
        .db
        .as_ref()
        .ok_or_else(|| "Не подключено к MongoDB".to_string())?;
    let id = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    UserService::delete(db, id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn authenticate(
    state: State<'_, Mutex<AppState>>,
    username: String,
    password: String,
) -> Result<AuthResult, String> {
    let mut state = state.lock().await;
    let db = state
        .db
        .as_ref()
        .ok_or_else(|| "Не подключено к MongoDB".to_string())?;

    let has_users = UserService::has_users(db)
        .await
        .map_err(|e| e.to_string())?;

    if !has_users && username == "admin" && password == "admin" {
        use crate::company::CreateCompanyInput;
        use crate::role::CreateRoleInput;

        let company = CompanyService::create(
            db,
            CreateCompanyInput {
                code: "MAIN".to_string(),
                name: "Основная компания".to_string(),
                inn: None,
            },
        )
        .await
        .map_err(|e| format!("Ошибка создания компании: {e}"))?;

        let role = RoleService::create(
            db,
            CreateRoleInput {
                company_id: crate::core::CompanyId(company._id),
                code: "SUPERADMIN".to_string(),
                name: "Суперадминистратор".to_string(),
                description: Some("Полный доступ ко всем функциям".to_string()),
            },
        )
        .await
        .map_err(|e| format!("Ошибка создания роли: {e}"))?;

        let user = UserService::create(
            db,
            CreateUserInput {
                company_id: crate::core::CompanyId(company._id),
                username: "admin".to_string(),
                display_name: "Администратор".to_string(),
                email: Some("admin@example.com".to_string()),
                password: "admin".to_string(),
                role_id: crate::core::RoleId(role._id),
            },
            &state.auth,
        )
        .await
        .map_err(|e| format!("Ошибка создания пользователя: {e}"))?;

        let token = state
            .auth
            .create_token(
                &crate::core::UserId(user._id),
                &user.company_id,
                &user.role_id,
            )
            .map_err(|e| e.to_string())?;

        state.current_user = Some(user.clone());

        return Ok(AuthResult { token, user });
    }

    let user = UserService::authenticate(db, &username, &password, &state.auth)
        .await
        .map_err(|e| e.to_string())?;

    let token = state
        .auth
        .create_token(
            &crate::core::UserId(user._id),
            &user.company_id,
            &user.role_id,
        )
        .map_err(|e| e.to_string())?;

    state.current_user = Some(user.clone());

    Ok(AuthResult { token, user })
}

#[tauri::command]
pub async fn get_me(
    state: State<'_, Mutex<AppState>>,
) -> Result<Option<UserPublic>, String> {
    let state = state.lock().await;
    Ok(state.current_user.clone())
}

// ── Роли ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn create_role(
    state: State<'_, Mutex<AppState>>,
    input: CreateRoleInput,
) -> Result<Role, String> {
    let state = state.lock().await;
    let db = state
        .db
        .as_ref()
        .ok_or_else(|| "Не подключено к MongoDB".to_string())?;
    RoleService::create(db, input)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_roles(
    state: State<'_, Mutex<AppState>>,
    company_id: String,
) -> Result<Vec<Role>, String> {
    let state = state.lock().await;
    let db = state
        .db
        .as_ref()
        .ok_or_else(|| "Не подключено к MongoDB".to_string())?;
    let cid = uuid::Uuid::parse_str(&company_id).map_err(|e| e.to_string())?;
    RoleService::list(db, crate::core::CompanyId(cid))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_role(
    state: State<'_, Mutex<AppState>>,
    id: String,
) -> Result<(), String> {
    let state = state.lock().await;
    let db = state
        .db
        .as_ref()
        .ok_or_else(|| "Не подключено к MongoDB".to_string())?;
    let id = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    RoleService::delete(db, id).await.map_err(|e| e.to_string())
}

// ── Rhai ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn validate_rhai_script(source: String) -> Result<(), String> {
    let sandbox = Sandbox::new(5000, 10000);
    sandbox.validate(&source).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn execute_rhai_script(
    source: String,
    context: String,
) -> Result<serde_json::Value, String> {
    let sandbox = Sandbox::new(5000, 10000);
    sandbox
        .execute(&source, &context)
        .map_err(|e| e.to_string())
}
