// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

pub mod commands;
pub mod indexes;
pub mod service;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::core::PlatformError;

pub const COLLECTION_MODULES: &str = "modules";
pub const COLLECTION_COMPANY_MODULES: &str = "company_modules";

// ── Capability System ──────────────────────────────────────

/// Допустимые capabilities для WASM-модулей.
/// Модуль запрашивает capabilities в манифесте,
/// host проверяет при каждом вызове host-функции.
pub const VALID_CAPABILITIES: &[&str] = &[
    "objects.create",
    "objects.read",
    "objects.update",
    "objects.delete",
    "metadata.read",
    "events.emit",
    "numbering.next",
    "logging",
    "notifications",
    "storage",
    "scripts",
    "transactions",
    "signature",
];

/// Маппинг: имя host-функции → требуемая capability.
/// Используется при plugin_call для проверки прав модуля.
pub fn required_capability(function_name: &str) -> Option<&'static str> {
    match function_name {
        "create_object" => Some("objects.create"),
        "get_object" | "list_objects" => Some("objects.read"),
        "update_object" => Some("objects.update"),
        "delete_object" => Some("objects.delete"),
        "transition_object" => Some("objects.update"),
        "get_entity_type" | "list_entity_fields" => Some("metadata.read"),
        "emit_event" => Some("events.emit"),
        "next_number" => Some("numbering.next"),
        "log_message" => Some("logging"),
        "notify_user" => Some("notifications"),
        "kv_put" | "kv_get" | "kv_list" | "kv_delete" | "kv_put_if_absent" => Some("storage"),
        "run_script" => Some("scripts"),
        "tx_begin" | "tx_add_op" | "tx_commit" => Some("transactions"),
                "signature_required" => Some("signature"),
        "cms_verify" => Some("signature"),
        "users_by_role" => Some("notifications"),
        _ => None,
    }
}

// ── API Version ────────────────────────────────────────────

/// Текущая версия API хост-функций.
/// Модуль указывает api_version в манифесте;
/// если версия не совместима — установка блокируется.
pub const CURRENT_API_VERSION: &str = "1.0";

// ── Types ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModuleStatus {
    Installed,
    Enabled,
    Disabled,
}

/// Функция, экспортируемая WASM-модулем.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleFunction {
    pub name: String,
    pub label: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// Манифест модуля (возвращается из get_info()).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleManifest {
    pub code: String,
    pub name: String,
    pub version: String,
    pub api_version: String,
    pub author: String,
    pub description: String,
    pub capabilities: Vec<String>,
    /// RBAC-политики, которые модуль требует для работы.
    /// Формат каждой записи: "subsystem.action" (например "requests.approve").
    /// При install хост создаёт недостающие PermissionPolicy.
    #[serde(default)]
    pub permissions: Vec<String>,
    /// Коды entity_type, чьё проведение оркестрирует модуль:
    /// post_object/cancel_object делегируют on_post/on_cancel.
    #[serde(default)]
    pub handles_documents: Vec<String>,
    pub functions: Vec<ModuleFunction>,
}

/// Установленный модуль в коллекции modules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledModule {
    #[serde(rename = "_id")]
    pub id: uuid::Uuid,
    pub code: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: String,
    pub api_version: String,
    pub capabilities: Vec<String>,
    pub functions: Vec<ModuleFunction>,
    pub status: ModuleStatus,
    pub wasm_bytes: Vec<u8>,
    pub manifest: serde_json::Value,
    pub installed_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Привязка модуля к компании (per-company включение/отключение + настройки).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyModule {
    #[serde(rename = "_id")]
    pub id: uuid::Uuid,
    pub company_id: String,
    pub module_id: String,
    pub enabled: bool,
    pub settings: serde_json::Value,
    pub installed_at: DateTime<Utc>,
}

// ── Commands (IPC input types) ─────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct InstallModuleInput {
    pub wasm_bytes: Vec<u8>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateModuleSettingsInput {
    pub settings: serde_json::Value,
}

// ── Error helpers ──────────────────────────────────────────

pub fn module_not_found(code: &str) -> PlatformError {
    PlatformError::NotFound(format!("Модуль '{}' не найден", code))
}

pub fn already_installed(code: &str) -> PlatformError {
    PlatformError::Validation(format!("Модуль '{}' уже установлен", code))
}

pub fn capability_denied(module_code: &str, function: &str, capability: &str) -> PlatformError {
    PlatformError::PermissionDenied(format!(
        "Модуль '{}' не имеет capability '{}' для вызова '{}'",
        module_code, capability, function
    ))
}

pub fn api_version_mismatch(required: &str, actual: &str) -> PlatformError {
    PlatformError::Validation(format!(
        "Несовместимая версия API: требуется {}, модуль предоставляет {}",
        required, actual
    ))
}

pub fn invalid_manifest(details: &str) -> PlatformError {
    PlatformError::Validation(format!("Невалидный манифест модуля: {}", details))
}

pub fn not_enabled(module_code: &str) -> PlatformError {
    PlatformError::Validation(format!(
        "Модуль '{}' не активирован для текущей компании",
        module_code
    ))
}
