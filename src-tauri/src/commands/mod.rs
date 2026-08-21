use crate::auth::AuthService;
use crate::company::{Company, CompanyService, CreateCompanyInput, UpdateCompanyInput};
use crate::db::{DiagnosticsInfo, MongoClient};
use crate::person::{Person, PersonService, CreatePersonInput, UpdatePersonInput};
use crate::role::{CreateRoleInput, Role, RoleService};
use crate::user::{CreateUserInput, UpdateUserInput, UserPublic, UserService};
use crate::user_contact::{UserContact, UserContactService, CreateContactInput, UpdateContactInput};
use crate::user_profile::{UserProfileService, UserProfileWithDetails, CreateProfileInput, UpdateProfileInput};
use crate::user_certificate::{UserCertificate, UserCertificateService, CreateCertificateInput};
use crate::rhai::Sandbox;
use crate::settings::{SettingsService, SettingEntry};
use crate::audit::{AuditEntry, AuditEntryView, AuditFilters, AuditableAction, MongoAuditService};
use crate::audit::service::AuditService as AuditServiceTrait;
use crate::permission_policy::{PermissionPolicy, PermissionPolicyService, CreatePermissionPolicyInput};
use crate::core::CompanyId;
use crate::plugin_manager::WasmPlugin;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
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
pub struct AuthResultWithCompanies {
    pub token: String,
    pub user: UserPublic,
    pub companies: Vec<UserProfileWithDetails>,
    pub role_code: Option<String>,
    pub role_name: Option<String>,
    pub role_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SwitchCompanyInput {
    pub company_id: String,
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
    pub current_company_id: Option<String>,
    pub current_role_id: Option<String>,
    pub current_policies: Option<Vec<crate::permission_policy::PermissionPolicy>>,
    pub wasm_modules: Option<HashMap<String, Arc<std::sync::Mutex<WasmPlugin>>>>,
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
            current_company_id: None,
            current_role_id: None,
            current_policies: None,
            wasm_modules: None,
        }
    }

    /// Проверить доступ по cached policies. Deny-by-default.
    pub fn check_access(&self, subsystem: &str, entity_type: Option<&str>, action: &str) -> bool {
        match &self.current_policies {
            Some(policies) => crate::permission_policy::PermissionPolicyService::check_access(
                policies, subsystem, entity_type, action,
            ),
            None => false,
        }
    }
}

macro_rules! get_db {
    ($state:expr) => {
        $state.db.as_ref().ok_or_else(|| "Не подключено к MongoDB".to_string())?
    };
}

// ── Диагностика ───────────────────────────────────────────────

#[tauri::command]
pub async fn get_diagnostics(state: State<'_, Mutex<AppState>>) -> Result<DiagnosticsReport, String> {
    let state = state.lock().await;
    let mongodb_info = match &state.db {
        Some(client) => client.diagnostics().await,
        None => DiagnosticsInfo { connected: false, host: "не подключено".to_string(), version: None, replica_set: None, ok: false },
    };
    Ok(DiagnosticsReport {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        mongodb: mongodb_info,
        modules: vec![ModuleInfo { code: "core".to_string(), name: "Ядро платформы".to_string(), version: env!("CARGO_PKG_VERSION").to_string(), active: true }],
    })
}

// ── Подключение к БД ──────────────────────────────────────────

#[tauri::command]
pub async fn connect_db(input: ConnectInput, state: State<'_, Mutex<AppState>>) -> Result<DiagnosticsInfo, String> {
    let client = MongoClient::connect(&input.uri, &input.db_name).await.map_err(|e| e.to_string())?;
    let info = client.diagnostics().await;
    { let mut state = state.lock().await; state.db = Some(client.clone()); state.config.mongodb_uri = Some(input.uri); state.config.mongodb_database = Some(input.db_name); }
    // Создаём индексы при подключении
    crate::audit::indexes::ensure_audit_indexes(&client).await.map_err(|e| e.to_string())?;
    crate::events::indexes::ensure_event_indexes(&client).await.map_err(|e| e.to_string())?;
    crate::meta::indexes::ensure_meta_indexes(&client).await.map_err(|e| e.to_string())?;
    crate::objects::indexes::ensure_object_indexes(&client).await.map_err(|e| e.to_string())?;
    crate::modules::indexes::ensure_indexes(&client).await;
    Ok(info)
}

// ── Конфиг ─────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_app_config(state: State<'_, Mutex<AppState>>) -> Result<AppConfig, String> {
    let state = state.lock().await;
    Ok(state.config.clone())
}

#[tauri::command]
pub async fn save_app_config(config: AppConfig, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let mut state = state.lock().await; state.config = config; Ok(())
}

// ── Компании ──────────────────────────────────────────────────

#[tauri::command]
pub async fn list_companies(state: State<'_, Mutex<AppState>>) -> Result<Vec<Company>, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    CompanyService::list(db).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_company(id: String, state: State<'_, Mutex<AppState>>) -> Result<Company, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let id = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    CompanyService::get(db, id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_company(input: CreateCompanyInput, state: State<'_, Mutex<AppState>>) -> Result<Company, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let company = CompanyService::create(db, input).await.map_err(|e| e.to_string())?;
    let _ = RoleService::create(db, CreateRoleInput {
        company_id: crate::core::CompanyId(company._id),
        code: "ADMIN".to_string(), name: "Администратор".to_string(),
        description: Some("Администратор компании".to_string()),
        permission_policy_ids: None,
    }).await;
    crate::audit_log!(state, db, AuditableAction::CreateCompany,
        target_id = company._id.to_string());
    Ok(company)
}

#[tauri::command]
pub async fn update_company(id: String, input: UpdateCompanyInput, state: State<'_, Mutex<AppState>>) -> Result<Company, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let cid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    let old = CompanyService::get(db, cid).await.ok();
    let result = CompanyService::update(db, cid, input.clone()).await.map_err(|e| e.to_string())?;
    {
        let mut changes = crate::audit::AuditChanges::new();
        let mut has_changes = false;
        if let Some(ref n) = input.name {
            let old_val = old.as_ref().map(|c| c.name.as_str()).unwrap_or("");
            changes = changes.field("name", old_val, n.as_str());
            has_changes = true;
        }
        if let Some(ref inn) = input.inn {
            let old_val = old.as_ref().and_then(|c| c.inn.as_deref()).unwrap_or("");
            changes = changes.field("inn", old_val, inn.as_str());
            has_changes = true;
        }
        if let Some(a) = input.active {
            let old_val = old.as_ref().map(|c| c.active).unwrap_or(true);
            changes = changes.field("active", if old_val { "true" } else { "false" }, if a { "true" } else { "false" });
            has_changes = true;
        }
        if has_changes {
            crate::audit::macros::fire_audit(
                &state, db, AuditableAction::UpdateCompany,
                Some(id.clone()), None, None,
                Some(changes), None,
            ).await;
        }
    }
    Ok(result)
}

#[tauri::command]
pub async fn delete_company(id: String, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let cid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    crate::audit_log!(state, db, AuditableAction::DeleteCompany,
        target_id = id.clone());
    CompanyService::delete(db, cid).await.map_err(|e| e.to_string())
}

// ── Пользователи ──────────────────────────────────────────────

#[tauri::command]
pub async fn list_users(state: State<'_, Mutex<AppState>>) -> Result<Vec<UserPublic>, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let users = UserService::list(db).await.map_err(|e| e.to_string())?;
    let mut result = Vec::new();
    for u in users {
        let mut pub_u: UserPublic = u.clone().into();
        pub_u.display_name = UserService::resolve_display_name(db, &u).await;
        result.push(pub_u);
    }
    Ok(result)
}

#[tauri::command]
pub async fn get_user(id: String, state: State<'_, Mutex<AppState>>) -> Result<UserPublic, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let id = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    let user = UserService::get(db, id).await.map_err(|e| e.to_string())?;
    let mut pub_user: UserPublic = user.clone().into();
    pub_user.display_name = UserService::resolve_display_name(db, &user).await;
    Ok(pub_user)
}

#[tauri::command]
pub async fn create_user(input: CreateUserInput, state: State<'_, Mutex<AppState>>) -> Result<UserPublic, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let result = UserService::create(db, input, &state.auth).await.map_err(|e| e.to_string())?;
    crate::audit_log!(state, db, AuditableAction::CreateUser,
        target_id = result._id.to_string());
    Ok(result)
}

#[tauri::command]
pub async fn update_user(id: String, input: UpdateUserInput, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let mut state = state.lock().await;
    let db = get_db!(state);
    let id = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    if let Some(ref status) = input.status {
        if status == "disabled" || status == "archived" {
            if UserService::is_last_admin(db, id).await.map_err(|e| e.to_string())? {
                return Err("Невозможно заблокировать последнего администратора компании".to_string());
            }
        }
    }
    let old_user = UserService::get(db, id).await.ok();
    if let Some(ref s) = input.status {
        if s == "disabled" || s == "archived" {
            if UserService::is_last_admin(db, id).await.map_err(|e| e.to_string())? {
                return Err("Невозможно заблокировать последнего администратора компании".to_string());
            }
        }
    }
    {
        let mut changes = crate::audit::AuditChanges::new();
        let mut has_changes = false;
        if let Some(ref s) = input.status {
            let old_val = old_user.as_ref().map(|u| u.status.as_str()).unwrap_or("unknown");
            changes = changes.field("status", old_val, s.as_str());
            has_changes = true;
        }
        if input.new_password.is_some() {
            changes = changes.field("password", "***", "***");
            has_changes = true;
        }
        if let Some(ref tz) = input.timezone {
            let old_val = old_user.as_ref().and_then(|u| u.timezone.as_deref()).unwrap_or("");
            changes = changes.field("timezone", old_val, tz.as_str());
            has_changes = true;
        }
        if let Some(ref loc) = input.locale {
            let old_val = old_user.as_ref().and_then(|u| u.locale.as_deref()).unwrap_or("");
            changes = changes.field("locale", old_val, loc.as_str());
            has_changes = true;
        }
        if let Some(mcp) = input.must_change_password {
            let old_val = old_user.as_ref().map(|u| if u.must_change_password { "true" } else { "false" }).unwrap_or("false");
            changes = changes.field("must_change_password", old_val, if mcp { "true" } else { "false" });
            has_changes = true;
        }
        if has_changes {
            crate::audit::macros::fire_audit(
                &state, db, AuditableAction::UpdateUser,
                Some(id.to_string()), None, None,
                Some(changes), None,
            ).await;
        }
    }
    UserService::update(db, id, input, &state.auth).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_user(id: String, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let uid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    if UserService::is_last_admin(db, uid).await.map_err(|e| e.to_string())? {
        return Err("Невозможно удалить последнего администратора компании".to_string());
    }
    crate::audit_log!(state, db, AuditableAction::DeleteUser,
        target_id = id.clone());
    UserService::delete(db, uid).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn authenticate(login: String, password: String, state: State<'_, Mutex<AppState>>) -> Result<AuthResultWithCompanies, String> {
    let mut state = state.lock().await;
    let db = state.db.as_ref().ok_or_else(|| "Не подключено к MongoDB".to_string())?.clone();

    let has_users = UserService::has_users(&db).await.map_err(|e| e.to_string())?;

    if !has_users && login == "admin" && password == "admin" {
        PermissionPolicyService::ensure_seed_policies(&db).await.map_err(|e| e.to_string())?;

        let company = CompanyService::create(&db, CreateCompanyInput {
            code: "MAIN".to_string(), name: "Основная компания".to_string(), inn: None,
        }).await.map_err(|e| format!("Ошибка создания компании: {e}"))?;

        RoleService::seed_roles_for_company(&db, crate::core::CompanyId(company._id)).await
            .map_err(|e| format!("Ошибка создания ролей: {e}"))?;

        let roles = RoleService::list(&db, crate::core::CompanyId(company._id)).await
            .map_err(|e| e.to_string())?;
        let role = roles.iter().find(|r| r.code == "SUPERADMIN")
            .or_else(|| roles.first())
            .ok_or_else(|| "Нет ролей".to_string())?;

        let user = UserService::create(&db, CreateUserInput {
            login: "admin".to_string(),
            password: "admin".to_string(),
            last_name: Some("Администратор".to_string()),
            first_name: Some("Системный".to_string()),
            middle_name: None,
            display_name: Some("Администратор".to_string()),
            email: Some("admin@example.com".to_string()),
            company_id: Some(company._id.to_string()),
            role_id: Some(role._id.to_string()),
            position: Some("Системный администратор".to_string()),
            department: None,
        }, &state.auth).await.map_err(|e| format!("Ошибка создания пользователя: {e}"))?;

        let companies = UserProfileService::list_with_details(&db, crate::core::UserId(user._id))
            .await.map_err(|e| e.to_string())?;

        let first_profile = companies.first();
        let company_id_str = first_profile.map(|p| p.company_id.clone()).unwrap_or_default();
        let role_id_str = first_profile.map(|p| p.role_id.clone()).unwrap_or_default();
        let company_id = uuid::Uuid::parse_str(&company_id_str).map_err(|e| e.to_string())?;
        let role_id = uuid::Uuid::parse_str(&role_id_str).map_err(|e| e.to_string())?;

        let token = state.auth.create_token(
            &crate::core::UserId(user._id),
            &crate::core::CompanyId(company_id),
            &crate::core::RoleId(role_id),
        ).map_err(|e| e.to_string())?;

        state.current_user = Some(user.clone());
        state.current_company_id = Some(company_id.to_string());
        state.current_role_id = Some(role_id.to_string());
        state.current_policies = RoleService::get_policies(&db, role).await.ok();
        return Ok(AuthResultWithCompanies { token, user, companies, role_code: Some(role.code.clone()), role_name: Some(role.name.clone()), role_id: Some(role_id.to_string()) });
    }

    let user = UserService::authenticate(&db, &login, &password, &state.auth).await.map_err(|e| e.to_string())?;

    let companies = UserProfileService::list_with_details(&db, crate::core::UserId(user._id))
        .await.map_err(|e| e.to_string())?;

    let first_profile = companies.first();
    let company_id_str = first_profile.map(|p| p.company_id.clone()).unwrap_or_default();
    let role_id_str = first_profile.map(|p| p.role_id.clone()).unwrap_or_default();
    let company_id = uuid::Uuid::parse_str(&company_id_str).map_err(|e| e.to_string())?;
    let role_id = uuid::Uuid::parse_str(&role_id_str).map_err(|e| e.to_string())?;

    let token = state.auth.create_token(
        &crate::core::UserId(user._id),
        &crate::core::CompanyId(company_id),
        &crate::core::RoleId(role_id),
    ).map_err(|e| e.to_string())?;

    state.current_user = Some(user.clone());
    state.current_company_id = Some(company_id.to_string());
    state.current_role_id = Some(role_id.to_string());

    crate::audit_log!(state, db, AuditableAction::Login,
        target_id = user._id.to_string());

    let role = RoleService::get(&db, role_id).await.ok();
    state.current_policies = match role.as_ref() {
        Some(r) => RoleService::get_policies(&db, r).await.ok(),
        None => None,
    };
    Ok(AuthResultWithCompanies {
        token, user, companies,
        role_code: role.as_ref().map(|r| r.code.clone()),
        role_name: role.as_ref().map(|r| r.name.clone()),
        role_id: Some(role_id.to_string()),
    })
}

#[tauri::command]
pub async fn get_me(state: State<'_, Mutex<AppState>>) -> Result<Option<UserPublic>, String> {
    let state = state.lock().await;
    Ok(state.current_user.clone())
}

// ── Персоны ───────────────────────────────────────────────────

#[tauri::command]
pub async fn get_person(id: String, state: State<'_, Mutex<AppState>>) -> Result<Person, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let id = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    PersonService::get(db, id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_person(id: String, input: UpdatePersonInput, state: State<'_, Mutex<AppState>>) -> Result<Person, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let id = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    let old = PersonService::get(db, id).await.ok();
    let result = PersonService::update(db, id, input.clone()).await.map_err(|e| e.to_string())?;
    {
        let mut changes = crate::audit::AuditChanges::new();
        let mut has_changes = false;
        if let Some(ln) = &input.last_name {
            let old_val = old.as_ref().map(|p| p.last_name.as_str()).unwrap_or("");
            if old_val != ln.as_str() { changes = changes.field("last_name", old_val, ln.as_str()); has_changes = true; }
        }
        if let Some(fn_) = &input.first_name {
            let old_val = old.as_ref().map(|p| p.first_name.as_str()).unwrap_or("");
            if old_val != fn_.as_str() { changes = changes.field("first_name", old_val, fn_.as_str()); has_changes = true; }
        }
        if let Some(mn) = &input.middle_name {
            let old_val = old.as_ref().and_then(|p| p.middle_name.as_deref()).unwrap_or("");
            changes = changes.field("middle_name", old_val, mn.as_str());
            has_changes = true;
        }
        if let Some(dn) = &input.display_name {
            let old_val = old.as_ref().map(|p| p.display_name.as_str()).unwrap_or("");
            if old_val != dn.as_str() { changes = changes.field("display_name", old_val, dn.as_str()); has_changes = true; }
        }
        if has_changes {
            crate::audit::macros::fire_audit(
                &state, db, AuditableAction::UpdatePerson,
                Some(result._id.to_string()), None, None,
                Some(changes), None,
            ).await;
        }
    }
    Ok(result)
}

// ── Контакты ──────────────────────────────────────────────────

#[tauri::command]
pub async fn list_user_contacts(user_id: String, state: State<'_, Mutex<AppState>>) -> Result<Vec<UserContact>, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let uid = uuid::Uuid::parse_str(&user_id).map_err(|e| e.to_string())?;
    UserContactService::list_by_user(db, crate::core::UserId(uid)).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_contact(input: CreateContactInput, state: State<'_, Mutex<AppState>>) -> Result<UserContact, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let channel_type = input.channel_type.clone();
    let value = input.value.clone();
    let result = UserContactService::create(db, input).await.map_err(|e| e.to_string())?;
    crate::audit::macros::fire_audit(
        &state, db, AuditableAction::CreateContact,
        Some(result._id.to_string()), None, None,
        Some(crate::audit::AuditChanges::new()
            .field_new("channel_type", &channel_type)
            .field_new("value", &value)),
        None,
    ).await;
    Ok(result)
}

#[tauri::command]
pub async fn update_contact(id: String, input: UpdateContactInput, state: State<'_, Mutex<AppState>>) -> Result<UserContact, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let cid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    let old = UserContactService::get(db, cid).await.ok();
    let result = UserContactService::update(db, cid, input.clone()).await.map_err(|e| e.to_string())?;
    {
        let mut changes = crate::audit::AuditChanges::new();
        let mut has_changes = false;
        if let Some(ref v) = input.value {
            let old_val = old.as_ref().map(|c| c.value.as_str()).unwrap_or("");
            changes = changes.field("value", old_val, v.as_str());
            has_changes = true;
        }
        if let Some(ip) = input.is_primary {
            let old_val = old.as_ref().map(|c| c.is_primary).unwrap_or(false);
            changes = changes.field("is_primary", if old_val { "true" } else { "false" }, if ip { "true" } else { "false" });
            has_changes = true;
        }
        if let Some(iv) = input.is_verified {
            let old_val = old.as_ref().map(|c| c.is_verified).unwrap_or(false);
            changes = changes.field("is_verified", if old_val { "true" } else { "false" }, if iv { "true" } else { "false" });
            has_changes = true;
        }
        if has_changes {
            crate::audit::macros::fire_audit(
                &state, db, AuditableAction::UpdateContact,
                Some(result._id.to_string()), None, None,
                Some(changes), None,
            ).await;
        }
    }
    Ok(result)
}

#[tauri::command]
pub async fn delete_contact(id: String, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let id = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    let old = UserContactService::get(db, id).await.ok();
    crate::audit::macros::fire_audit(
        &state, db, AuditableAction::DeleteContact,
        Some(id.to_string()), None, None,
        old.map(|c| crate::audit::AuditChanges::new()
            .field_old("channel_type", &c.channel_type)
            .field_old("value", &c.value)),
        None,
    ).await;
    UserContactService::delete(db, id).await.map_err(|e| e.to_string())
}

// ── Рабочие профили ───────────────────────────────────────────

#[tauri::command]
pub async fn list_user_profiles(user_id: String, state: State<'_, Mutex<AppState>>) -> Result<Vec<UserProfileWithDetails>, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let uid = uuid::Uuid::parse_str(&user_id).map_err(|e| e.to_string())?;
    UserProfileService::list_with_details(db, crate::core::UserId(uid)).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_user_profile(input: CreateProfileInput, state: State<'_, Mutex<AppState>>) -> Result<UserProfileWithDetails, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let company_id_str = input.company_id.clone();
    let role_id_str = input.role_id.clone();
    let position_str = input.position.clone().unwrap_or_default();
    let profile = UserProfileService::add(db, input).await.map_err(|e| e.to_string())?;
    crate::audit::macros::fire_audit(
        &state, db, AuditableAction::AddUserProfile,
        Some(profile._id.to_string()), None, None,
        Some(crate::audit::AuditChanges::new()
            .field_new("company_id", &company_id_str)
            .field_new("role_id", &role_id_str)
            .field_new("position", &position_str)),
        None,
    ).await;
    let uid = profile.user_id;
    UserProfileService::list_with_details(db, uid).await
        .map(|v| v.into_iter().find(|p| p._id == profile._id.to_string()).unwrap_or_default())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_user_profile(id: String, input: UpdateProfileInput, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let id = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    let old = UserProfileService::get(db, id).await.ok();
    UserProfileService::update(db, id, input.clone()).await.map_err(|e| e.to_string())?;
    {
        let mut changes = crate::audit::AuditChanges::new();
        let mut has_changes = false;
        if let Some(ref rid) = input.role_id {
            let old_val = old.as_ref().map(|p| p.role_id.0.to_string()).unwrap_or_default();
            changes = changes.field("role_id", &old_val, rid.as_str());
            has_changes = true;
        }
        if let Some(ref pos) = input.position {
            let old_val = old.as_ref().and_then(|p| p.position.as_deref()).unwrap_or("");
            changes = changes.field("position", old_val, pos.as_str());
            has_changes = true;
        }
        if let Some(ref dept) = input.department {
            let old_val = old.as_ref().and_then(|p| p.department.as_deref()).unwrap_or("");
            changes = changes.field("department", old_val, dept.as_str());
            has_changes = true;
        }
        if let Some(ip) = input.is_primary {
            let old_val = old.as_ref().map(|p| p.is_primary).unwrap_or(false);
            changes = changes.field("is_primary", if old_val { "true" } else { "false" }, if ip { "true" } else { "false" });
            has_changes = true;
        }
        if let Some(ia) = input.is_active {
            let old_val = old.as_ref().map(|p| p.is_active).unwrap_or(true);
            changes = changes.field("is_active", if old_val { "true" } else { "false" }, if ia { "true" } else { "false" });
            has_changes = true;
        }
        if has_changes {
            crate::audit::macros::fire_audit(
                &state, db, AuditableAction::UpdateUserProfile,
                Some(id.to_string()), None, None,
                Some(changes), None,
            ).await;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn remove_user_profile(id: String, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let id = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    let old = UserProfileService::get(db, id).await.ok();
    crate::audit::macros::fire_audit(
        &state, db, AuditableAction::RemoveUserProfile,
        Some(id.to_string()), None, None,
        old.map(|p| crate::audit::AuditChanges::new()
            .field_old("company_id", &p.company_id.0.to_string())
            .field_old("role_id", &p.role_id.0.to_string())),
        None,
    ).await;
    UserProfileService::remove(db, id).await.map_err(|e| e.to_string())
}

// ── Сертификаты ───────────────────────────────────────────────

#[tauri::command]
pub async fn list_user_certificates(user_id: String, state: State<'_, Mutex<AppState>>) -> Result<Vec<UserCertificate>, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let uid = uuid::Uuid::parse_str(&user_id).map_err(|e| e.to_string())?;
    UserCertificateService::list_by_user(db, crate::core::UserId(uid)).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn deactivate_certificate(id: String, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let id = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    crate::audit_log!(state, db, AuditableAction::DeactivateCertificate,
        target_id = id.to_string());
    UserCertificateService::deactivate(db, id).await.map_err(|e| e.to_string())
}

// ── Мультикомпания ────────────────────────────────────────────

#[tauri::command]
pub async fn switch_company(input: SwitchCompanyInput, state: State<'_, Mutex<AppState>>) -> Result<AuthResultWithCompanies, String> {
    let mut state = state.lock().await;
    let current_user = state.current_user.clone().ok_or_else(|| "Необходима авторизация".to_string())?;
    let company_id = uuid::Uuid::parse_str(&input.company_id).map_err(|e| e.to_string())?;

    let role_id = {
        let db = state.db.as_ref().ok_or_else(|| "Не подключено к MongoDB".to_string())?;
        UserProfileService::get_role_for_company(db, crate::core::UserId(current_user._id), crate::core::CompanyId(company_id))
            .await.map_err(|e| e.to_string())?
    };

    let token = state.auth.create_token(&crate::core::UserId(current_user._id), &crate::core::CompanyId(company_id), &role_id).map_err(|e| e.to_string())?;

    let companies = {
        let db = state.db.as_ref().ok_or_else(|| "Не подключено к MongoDB".to_string())?;
        UserProfileService::list_with_details(db, crate::core::UserId(current_user._id))
            .await.map_err(|e| e.to_string())?
    };

    let user_pub = current_user.clone();
    state.current_user = Some(user_pub.clone());
    state.current_company_id = Some(input.company_id.clone());
    state.current_role_id = Some(role_id.0.to_string());

    if let Some(ref db) = state.db {
        crate::audit_log!(state, db, AuditableAction::SwitchCompany,
            target_id = input.company_id.clone());
    }

    let role_info = {
        let db = state.db.as_ref().ok_or_else(|| "Не подключено к MongoDB".to_string())?;
        RoleService::get(db, role_id.0).await.ok()
    };

    state.current_policies = match role_info.as_ref() {
        Some(r) => {
            let db = state.db.as_ref().unwrap();
            RoleService::get_policies(db, r).await.ok()
        }
        None => None,
    };

    Ok(AuthResultWithCompanies {
        token, user: user_pub, companies,
        role_code: role_info.as_ref().map(|r| r.code.clone()),
        role_name: role_info.as_ref().map(|r| r.name.clone()),
        role_id: Some(role_id.0.to_string()),
    })
}

// ── Роли ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn create_role(input: CreateRoleInput, state: State<'_, Mutex<AppState>>) -> Result<Role, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let role = RoleService::create(db, input).await.map_err(|e| e.to_string())?;
    crate::audit_log!(state, db, AuditableAction::CreateRole,
        target_id = role._id.to_string());
    Ok(role)
}

#[tauri::command]
pub async fn list_roles(company_id: String, state: State<'_, Mutex<AppState>>) -> Result<Vec<Role>, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let cid = uuid::Uuid::parse_str(&company_id).map_err(|e| e.to_string())?;
    RoleService::list(db, crate::core::CompanyId(cid)).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_role(id: String, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let rid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    crate::audit_log!(state, db, AuditableAction::DeleteRole,
        target_id = id.clone());
    RoleService::delete(db, rid).await.map_err(|e| e.to_string())
}

// ── Rhai ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn validate_rhai_script(source: String, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let state = state.lock().await;
    if !state.check_access("scripts", None, "read") {
        return Err("Доступ запрещён: нет права scripts.read".into());
    }
    let sandbox = Sandbox::new(5000, 10000);
    sandbox.validate(&source).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn execute_rhai_script(source: String, context: String, state: State<'_, Mutex<AppState>>) -> Result<serde_json::Value, String> {
    let state = state.lock().await;
    if !state.check_access("scripts", None, "execute") {
        return Err("Доступ запрещён: нет права scripts.execute".into());
    }
    let sandbox = Sandbox::new(5000, 10000);
    sandbox.execute(&source, &context).map_err(|e| e.to_string())
}

// ── Настройки ─────────────────────────────────────────────────

#[tauri::command]
pub async fn get_contact_types(state: State<'_, Mutex<AppState>>) -> Result<Vec<SettingEntry>, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    SettingsService::get_contact_types(db).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_contact_types(types: Vec<SettingEntry>, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let val = serde_json::to_value(&types).map_err(|e| e.to_string())?;
    SettingsService::save_setting(db, "contact_types", val).await.map_err(|e| e.to_string())?;
    crate::audit_log!(state, db, AuditableAction::SaveSettings);
    Ok(())
}

// ── Аудит ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AuditLogFilters {
    pub actions: Option<Vec<String>>,
    pub target_type: Option<String>,
    pub user_id: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub limit: Option<u32>,
    pub before: Option<String>,
    pub after: Option<String>,
}

#[tauri::command]
pub async fn list_audit_logs(filters: Option<AuditLogFilters>, state: State<'_, Mutex<AppState>>) -> Result<crate::audit::AuditPage, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let _ = state.current_user.as_ref().ok_or_else(|| "Необходима авторизация".to_string())?;
    let company_id_str = state.current_company_id.as_ref()
        .ok_or_else(|| "Не выбрана компания".to_string())?;
    let cid = uuid::Uuid::parse_str(company_id_str).map_err(|e| e.to_string())?;

    let mut audit_filters = AuditFilters::default();
    if let Some(f) = filters {
        if let Some(actions) = f.actions { audit_filters.actions = actions; }
        audit_filters.target_type = f.target_type;
        audit_filters.user_id = f.user_id;
        audit_filters.limit = f.limit;
        if let Some(ref from) = f.date_from {
            audit_filters.date_from = chrono::DateTime::parse_from_rfc3339(from)
                .ok().map(|dt| dt.with_timezone(&chrono::Utc));
        }
        if let Some(ref to) = f.date_to {
            audit_filters.date_to = chrono::DateTime::parse_from_rfc3339(to)
                .ok().map(|dt| dt.with_timezone(&chrono::Utc));
        }
        if let Some(ref b) = f.before {
            audit_filters.before = chrono::DateTime::parse_from_rfc3339(b)
                .ok().map(|dt| dt.with_timezone(&chrono::Utc));
        }
        if let Some(ref a) = f.after {
            audit_filters.after = chrono::DateTime::parse_from_rfc3339(a)
                .ok().map(|dt| dt.with_timezone(&chrono::Utc));
        }
    }

    let svc = MongoAuditService::new();
    svc.list(db, CompanyId(cid), audit_filters).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_audit_entry(id: String, state: State<'_, Mutex<AppState>>) -> Result<Option<AuditEntryView>, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let _ = state.current_user.as_ref().ok_or_else(|| "Необходима авторизация".to_string())?;
    let eid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    let svc = MongoAuditService::new();
    svc.get_entry(db, eid).await.map_err(|e| e.to_string())
}

// ── Permission Policies ─────────────────────────────────────

#[tauri::command]
pub async fn list_permission_policies(state: State<'_, Mutex<AppState>>) -> Result<Vec<PermissionPolicy>, String> {
    let state = state.lock().await;
    let _ = get_db!(state);
    let _ = state.current_user.as_ref().ok_or_else(|| "Необходима авторизация".to_string())?;
    PermissionPolicyService::list(get_db!(state)).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_permission_policy(input: CreatePermissionPolicyInput, state: State<'_, Mutex<AppState>>) -> Result<PermissionPolicy, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let _ = state.current_user.as_ref().ok_or_else(|| "Необходима авторизация".to_string())?;
    let result = PermissionPolicyService::create(db, input).await.map_err(|e| e.to_string())?;
    crate::audit_log!(state, db, AuditableAction::CreatePermissionPolicy,
        target_id = result._id.to_string());
    Ok(result)
}

#[tauri::command]
pub async fn delete_permission_policy(id: String, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let _ = state.current_user.as_ref().ok_or_else(|| "Необходима авторизация".to_string())?;
    let uid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    crate::audit_log!(state, db, AuditableAction::DeletePermissionPolicy,
        target_id = id.clone());
    PermissionPolicyService::delete(db, uid).await.map_err(|e| e.to_string())
}

// ── Roles (update) ─────────────────────────────────────────

#[tauri::command]
pub async fn update_role(id: String, input: crate::role::UpdateRoleInput, state: State<'_, Mutex<AppState>>) -> Result<Role, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let rid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    let old = RoleService::get(db, rid).await.ok();
    let result = RoleService::update(db, rid, input.clone()).await.map_err(|e| e.to_string())?;
    {
        let mut changes = crate::audit::AuditChanges::new();
        let mut has_changes = false;
        if let Some(ref n) = input.name {
            let old_val = old.as_ref().map(|r| r.name.as_str()).unwrap_or("");
            changes = changes.field("name", old_val, n.as_str());
            has_changes = true;
        }
        if let Some(ref d) = input.description {
            let old_val = old.as_ref().and_then(|r| r.description.as_deref()).unwrap_or("");
            changes = changes.field("description", old_val, d.as_str());
            has_changes = true;
        }
        if let Some(ref pids) = input.permission_policy_ids {
            let old_val = old.as_ref().map(|r| r.permission_policy_ids.join(",")).unwrap_or_default();
            changes = changes.field("permission_policy_ids", &old_val, &pids.join(","));
            has_changes = true;
        }
        if has_changes {
            crate::audit::macros::fire_audit(
                &state, db, AuditableAction::UpdateRole,
                Some(result._id.to_string()), None, None,
                Some(changes), None,
            ).await;
        }
    }
    Ok(result)
}

// ── Мой доступ ──────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct MyPermissionsResult {
    pub role_code: String,
    pub role_name: String,
    pub permissions: Vec<PermissionPolicy>,
}

#[tauri::command]
pub async fn get_my_permissions(state: State<'_, Mutex<AppState>>) -> Result<MyPermissionsResult, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let role_id_str = state.current_role_id.as_ref()
        .ok_or_else(|| "Не выбрана роль".to_string())?;
    let rid = uuid::Uuid::parse_str(role_id_str).map_err(|e| e.to_string())?;
    let role = RoleService::get(db, rid).await.map_err(|e| e.to_string())?;
    let policies = RoleService::get_policies(db, &role).await.map_err(|e| e.to_string())?;
    Ok(MyPermissionsResult {
        role_code: role.code,
        role_name: role.name,
        permissions: policies,
    })
}

// ── Event Store ────────────────────────────────────────────

#[tauri::command]
pub async fn list_events(
    filters: crate::events::EventFilters,
    state: State<'_, Mutex<AppState>>,
) -> Result<crate::events::EventPage, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let company_id = state.current_company_id.as_ref()
        .ok_or_else(|| "Не выбрана компания".to_string())?;
    let cid = uuid::Uuid::parse_str(company_id).map_err(|e| e.to_string())?;
    let svc = crate::events::EventService::new();
    svc.list(db, crate::core::CompanyId(cid), filters).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_event(id: String, state: State<'_, Mutex<AppState>>) -> Result<crate::events::Event, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let eid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    let svc = crate::events::EventService::new();
    svc.get(db, eid).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_stream_events(
    stream_type: String,
    stream_id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<crate::events::Event>, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let st: crate::events::StreamType = stream_type.parse().map_err(|e: crate::core::PlatformError| e.to_string())?;
    let svc = crate::events::EventService::new();
    svc.list_stream(db, st, &stream_id).await.map_err(|e| e.to_string())
}

// ── Метаданные ────────────────────────────────────────────

#[tauri::command]
pub async fn list_entity_types(state: State<'_, Mutex<AppState>>) -> Result<Vec<crate::meta::EntityType>, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let cid = state.current_company_id.as_ref()
        .and_then(|s| uuid::Uuid::parse_str(s).ok())
        .map(crate::core::CompanyId);
    crate::meta::service::EntityTypeService::list(db, cid).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_entity_type(id: String, state: State<'_, Mutex<AppState>>) -> Result<crate::meta::EntityType, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let uid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    crate::meta::service::EntityTypeService::get(db, uid).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_entity_type(input: crate::meta::CreateEntityTypeInput, state: State<'_, Mutex<AppState>>) -> Result<crate::meta::EntityType, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let cid = state.current_company_id.as_ref()
        .and_then(|s| uuid::Uuid::parse_str(s).ok())
        .map(crate::core::CompanyId);
    crate::meta::service::EntityTypeService::create(db, cid, input).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_entity_type(id: String, input: crate::meta::UpdateEntityTypeInput, state: State<'_, Mutex<AppState>>) -> Result<crate::meta::EntityType, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let uid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    crate::meta::service::EntityTypeService::update(db, uid, input).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_entity_type(id: String, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let uid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    crate::meta::service::EntityTypeService::delete(db, uid).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_entity_fields(entity_type_id: String, state: State<'_, Mutex<AppState>>) -> Result<Vec<crate::meta::EntityField>, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let uid = uuid::Uuid::parse_str(&entity_type_id).map_err(|e| e.to_string())?;
    crate::meta::service::EntityFieldService::list_by_type(db, uid).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_entity_field(input: crate::meta::CreateEntityFieldInput, state: State<'_, Mutex<AppState>>) -> Result<crate::meta::EntityField, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    crate::meta::service::EntityFieldService::create(db, input).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_entity_field(id: String, input: crate::meta::UpdateEntityFieldInput, state: State<'_, Mutex<AppState>>) -> Result<crate::meta::EntityField, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let uid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    crate::meta::service::EntityFieldService::update(db, uid, input).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_entity_field(id: String, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let uid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    crate::meta::service::EntityFieldService::delete(db, uid).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_entity_states(entity_type_id: String, state: State<'_, Mutex<AppState>>) -> Result<Vec<crate::meta::EntityState>, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let uid = uuid::Uuid::parse_str(&entity_type_id).map_err(|e| e.to_string())?;
    crate::meta::service::EntityStateService::list_by_type(db, uid).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_entity_state(input: crate::meta::CreateEntityStateInput, state: State<'_, Mutex<AppState>>) -> Result<crate::meta::EntityState, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    crate::meta::service::EntityStateService::create(db, input).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_entity_state(id: String, input: crate::meta::UpdateEntityStateInput, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let uid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    crate::meta::service::EntityStateService::update(db, uid, input).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_entity_state(id: String, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let uid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    crate::meta::service::EntityStateService::delete(db, uid).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_entity_transitions(entity_type_id: String, state: State<'_, Mutex<AppState>>) -> Result<Vec<crate::meta::EntityTransition>, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let uid = uuid::Uuid::parse_str(&entity_type_id).map_err(|e| e.to_string())?;
    crate::meta::service::EntityTransitionService::list_by_type(db, uid).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_entity_transition(input: crate::meta::CreateEntityTransitionInput, state: State<'_, Mutex<AppState>>) -> Result<crate::meta::EntityTransition, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    crate::meta::service::EntityTransitionService::create(db, input).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_entity_transition(id: String, input: crate::meta::UpdateEntityTransitionInput, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let uid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    crate::meta::service::EntityTransitionService::update(db, uid, input).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_entity_transition(id: String, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let uid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    crate::meta::service::EntityTransitionService::delete(db, uid).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_entity_forms(entity_type_id: String, state: State<'_, Mutex<AppState>>) -> Result<Vec<crate::meta::EntityForm>, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let uid = uuid::Uuid::parse_str(&entity_type_id).map_err(|e| e.to_string())?;
    crate::meta::service::EntityFormService::list_by_type(db, uid).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_entity_form(input: crate::meta::CreateEntityFormInput, state: State<'_, Mutex<AppState>>) -> Result<crate::meta::EntityForm, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    crate::meta::service::EntityFormService::create(db, input).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_entity_form(id: String, input: crate::meta::UpdateEntityFormInput, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let uid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    crate::meta::service::EntityFormService::update(db, uid, input).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_entity_form(id: String, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let uid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    crate::meta::service::EntityFormService::delete(db, uid).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_entity_actions(entity_type_id: String, state: State<'_, Mutex<AppState>>) -> Result<Vec<crate::meta::EntityAction>, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let uid = uuid::Uuid::parse_str(&entity_type_id).map_err(|e| e.to_string())?;
    crate::meta::service::EntityActionService::list_by_type(db, uid).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_entity_action(input: crate::meta::CreateEntityActionInput, state: State<'_, Mutex<AppState>>) -> Result<crate::meta::EntityAction, String> {
    let state = state.lock().await;
    let db = get_db!(state);
    crate::meta::service::EntityActionService::create(db, input).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_entity_action(id: String, input: crate::meta::UpdateEntityActionInput, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let uid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    crate::meta::service::EntityActionService::update(db, uid, input).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_entity_action(id: String, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let state = state.lock().await;
    let db = get_db!(state);
    let uid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    crate::meta::service::EntityActionService::delete(db, uid).await.map_err(|e| e.to_string())
}

// ── Objects (Доска) ─────────────────────────────────────────

#[tauri::command]
pub async fn list_objects(filters: crate::objects::ObjectFilters, state: State<'_, Mutex<AppState>>) -> Result<crate::objects::ObjectPage, String> {
    let state = state.lock().await;
    if !state.check_access("documents", None, "read") {
        return Err("Доступ запрещён: нет права documents.read".into());
    }
    let db = get_db!(state);
    let company_id = state.current_company_id.as_ref()
        .ok_or_else(|| "Не выбрана компания".to_string())?;
    let cid = uuid::Uuid::parse_str(company_id).map_err(|e| e.to_string())?;
    crate::objects::service::ObjectService::list(db, crate::core::CompanyId(cid), filters).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_object(id: String, state: State<'_, Mutex<AppState>>) -> Result<crate::objects::Object, String> {
    let state = state.lock().await;
    if !state.check_access("documents", None, "read") {
        return Err("Доступ запрещён: нет права documents.read".into());
    }
    let db = get_db!(state);
    let uid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    let company_id = state.current_company_id.as_ref()
        .ok_or_else(|| "Не выбрана компания".to_string())?;
    let obj = crate::objects::service::ObjectService::get(db, uid).await.map_err(|e| e.to_string())?;
    if obj.company_id.0.to_string() != *company_id {
        return Err("Доступ запрещён: объект другой компании".into());
    }
    Ok(obj)
}

#[tauri::command]
pub async fn create_object(input: crate::objects::CreateObjectInput, state: State<'_, Mutex<AppState>>) -> Result<crate::objects::Object, String> {
    let state = state.lock().await;
    if !state.check_access("documents", None, "create") {
        return Err("Доступ запрещён: нет права documents.create".into());
    }
    let db = get_db!(state);
    let company_id = state.current_company_id.as_ref()
        .ok_or_else(|| "Не выбрана компания".to_string())?;
    let cid = uuid::Uuid::parse_str(company_id).map_err(|e| e.to_string())?;
    let user = state.current_user.as_ref()
        .ok_or_else(|| "Необходима авторизация".to_string())?;
    let user_id = crate::core::UserId(user._id);
    let actor = build_actor(&state);
    let obj = crate::objects::service::ObjectService::create(db, input, crate::core::CompanyId(cid), user_id, actor).await.map_err(|e| e.to_string())?;
    crate::audit_log!(state, db, AuditableAction::CreateDocument,
        target_id = obj._id.to_string());
    Ok(obj)
}

#[tauri::command]
pub async fn update_object(id: String, input: crate::objects::UpdateObjectInput, state: State<'_, Mutex<AppState>>) -> Result<crate::objects::Object, String> {
    let state = state.lock().await;
    if !state.check_access("documents", None, "update") {
        return Err("Доступ запрещён: нет права documents.update".into());
    }
    let db = get_db!(state);
    let uid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    let user = state.current_user.as_ref()
        .ok_or_else(|| "Необходима авторизация".to_string())?;
    let user_id = crate::core::UserId(user._id);
    let company_id = state.current_company_id.as_ref()
        .ok_or_else(|| "Не выбрана компания".to_string())?;
    let cid = uuid::Uuid::parse_str(company_id).map_err(|e| e.to_string())?;
    let actor = build_actor(&state);
    let obj = crate::objects::service::ObjectService::update(db, uid, input, user_id, actor, crate::core::CompanyId(cid)).await.map_err(|e| e.to_string())?;
    crate::audit_log!(state, db, AuditableAction::UpdateDocument,
        target_id = obj._id.to_string());
    Ok(obj)
}

#[tauri::command]
pub async fn post_object(id: String, version: i64, state: State<'_, Mutex<AppState>>) -> Result<crate::objects::Object, String> {
    let state = state.lock().await;
    if !state.check_access("documents", None, "approve") {
        return Err("Доступ запрещён: нет права documents.approve".into());
    }
    let db = get_db!(state);
    let uid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    let user = state.current_user.as_ref()
        .ok_or_else(|| "Необходима авторизация".to_string())?;
    let user_id = crate::core::UserId(user._id);
    let company_id = state.current_company_id.as_ref()
        .ok_or_else(|| "Не выбрана компания".to_string())?;
    let cid = uuid::Uuid::parse_str(company_id).map_err(|e| e.to_string())?;
    let actor = build_actor(&state);
    let obj = crate::objects::service::ObjectService::post(db, uid, version, user_id, actor, crate::core::CompanyId(cid)).await.map_err(|e| e.to_string())?;
    crate::audit_log!(state, db, AuditableAction::PostDocument,
        target_id = obj._id.to_string());
    Ok(obj)
}

#[tauri::command]
pub async fn cancel_object(id: String, version: i64, state: State<'_, Mutex<AppState>>) -> Result<crate::objects::Object, String> {
    let state = state.lock().await;
    if !state.check_access("documents", None, "cancel") {
        return Err("Доступ запрещён: нет права documents.cancel".into());
    }
    let db = get_db!(state);
    let uid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    let user = state.current_user.as_ref()
        .ok_or_else(|| "Необходима авторизация".to_string())?;
    let user_id = crate::core::UserId(user._id);
    let company_id = state.current_company_id.as_ref()
        .ok_or_else(|| "Не выбрана компания".to_string())?;
    let cid = uuid::Uuid::parse_str(company_id).map_err(|e| e.to_string())?;
    let actor = build_actor(&state);
    let obj = crate::objects::service::ObjectService::cancel(db, uid, version, user_id, actor, crate::core::CompanyId(cid)).await.map_err(|e| e.to_string())?;
    crate::audit_log!(state, db, AuditableAction::CancelDocument,
        target_id = obj._id.to_string());
    Ok(obj)
}

#[tauri::command]
pub async fn restore_object_version(id: String, target_version: i64, state: State<'_, Mutex<AppState>>) -> Result<crate::objects::Object, String> {
    let state = state.lock().await;
    if !state.check_access("documents", None, "update") {
        return Err("Доступ запрещён: нет права documents.update".into());
    }
    let db = get_db!(state);
    let uid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    let user = state.current_user.as_ref()
        .ok_or_else(|| "Необходима авторизация".to_string())?;
    let user_id = crate::core::UserId(user._id);
    let company_id = state.current_company_id.as_ref()
        .ok_or_else(|| "Не выбрана компания".to_string())?;
    let cid = uuid::Uuid::parse_str(company_id).map_err(|e| e.to_string())?;
    let actor = build_actor(&state);
    let obj = crate::objects::service::ObjectService::restore_version(db, uid, target_version, user_id, actor, crate::core::CompanyId(cid)).await.map_err(|e| e.to_string())?;
    crate::audit_log!(state, db, AuditableAction::RestoreDocument,
        target_id = obj._id.to_string());
    Ok(obj)
}

#[tauri::command]
pub async fn list_object_versions(id: String, state: State<'_, Mutex<AppState>>) -> Result<Vec<crate::objects::ObjectSnapshot>, String> {
    let state = state.lock().await;
    if !state.check_access("documents", None, "read") {
        return Err("Доступ запрещён: нет права documents.read".into());
    }
    let db = get_db!(state);
    let uid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    let company_id = state.current_company_id.as_ref()
        .ok_or_else(|| "Не выбрана компания".to_string())?;
    let obj = crate::objects::service::ObjectService::get(db, uid).await.map_err(|e| e.to_string())?;
    if obj.company_id.0.to_string() != *company_id {
        return Err("Доступ запрещён: объект другой компании".into());
    }
    crate::objects::service::ObjectService::list_versions(db, uid).await.map_err(|e| e.to_string())
}

fn build_actor(state: &AppState) -> crate::events::ActorSnapshot {
    let user = state.current_user.as_ref();
    crate::events::ActorSnapshot {
        user_id: user.map(|u| crate::core::UserId(u._id)).unwrap_or(crate::core::UserId(uuid::Uuid::nil())),
        login: user.map(|u| u.login.clone()).unwrap_or_default(),
        full_name: user.map(|u| u.display_name.clone()).filter(|s| !s.is_empty()),
        position: None,
        company_id: state.current_company_id.as_ref()
            .and_then(|s| uuid::Uuid::parse_str(s).ok())
            .map(crate::core::CompanyId)
            .unwrap_or(crate::core::CompanyId(uuid::Uuid::nil())),
    }
}
