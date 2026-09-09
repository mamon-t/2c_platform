//! Модуль WASM-плагина: установленная запись (глобальный каталог `modules`)
//! и состояние включения для конкретной компании (`company_modules`).
//!
//! Установка модуля — глобальная операция: запись создаётся в каталоге `modules`
//! с полным манифестом, а последующее включение для компании управляется
//! отдельной проекцией `company_modules`. Код после установки не выполняется
//! до первого вызова плагина.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::event::ActorSnapshot;
use crate::wasm_manifest::ModuleManifest;

/// Жизненный цикл записи модуля в каталоге.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleState {
    /// Модуль установлен и доступен для включения компаниям.
    Installed,
    /// Модуль помечен к удалению; команды выгружены из реестра.
    Uninstalled,
}

impl ModuleState {
    /// Каноническая строка состояния для SQL-запросов.
    pub fn as_str(&self) -> &'static str {
        match self {
            ModuleState::Installed => "installed",
            ModuleState::Uninstalled => "uninstalled",
        }
    }
}

/// Запись установленного WASM-модуля (таблица `modules`).
///
/// Хранит «единственный источник правды» о модуле: манифест, полученный из
/// `get_info()`, свернутый в поля записи. Код плагина после установки не
/// выполняется, пока не потребуется вызвать функцию модуля.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleRecord {
    /// Уникальный код модуля (совпадает с `ModuleManifest.code`).
    pub code: String,
    /// Название модуля (`display_name` из манифеста).
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    /// Версия модуля в формате semver.
    pub version: String,
    /// Версия протокола манифеста (строго `SUPPORTED_API_VERSION`).
    pub api_version: String,
    /// Запрошенные гранты хоста (подмножество `ALLOWED_CAPABILITIES`).
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Версия метаданных модуля для ensure-обновления ресурсов.
    pub metadata_version: u32,
    /// Текущее состояние записи в каталоге.
    pub state: ModuleState,
    /// SHA-256 байтов WASM-модуля (16 символов) — имя файла в модульном кэше.
    pub wasm_sha256: String,
    /// Полный манифест, полученный из `get_info()` при установке — источник
    /// правды для повторной декларативной регистрации (enable для других компаний).
    pub manifest: ModuleManifest,
    /// Момент установки.
    pub installed_at: DateTime<Utc>,
}

/// Проекция включения модуля для компании (таблица `company_modules`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyModule {
    /// Идентификатор компании (uuid).
    pub company_id: String,
    /// Код модуля.
    pub module_code: String,
    /// Включён ли модуль для компании.
    pub enabled: bool,
    /// Момент последнего изменения статуса.
    pub updated_at: DateTime<Utc>,
}

/// Полезная нагрузка события жизненного цикла модуля.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleLifecyclePayload {
    /// Причина события (команда или системный шаг).
    pub reason: String,
}

/// Полезная нагрузка события включения/отключения модуля для компании.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyModulePayload {
    pub company_id: String,
    pub module_code: String,
}

/// Контекст единичного вызова функции плагина из командного слоя платформы
/// (`plugin.{code}.{name}`). Отражает состав `HostCallCtx` хоста, но не
/// зависит от инфраструктуры.
#[derive(Debug, Clone, Default)]
pub struct PluginCallContext {
    /// Код вызываемого модуля.
    pub module_code: String,
    /// Компания, в контексте которой исполняется вызов.
    pub company_id: String,
    /// Исполнитель действия; `None` для системных операций.
    pub actor: Option<ActorSnapshot>,
    /// Гранты модуля (по умолчанию из манифеста).
    pub capabilities: Vec<String>,
    /// Настройки модуля для текущей компании (`module_settings`).
    pub settings: Value,
}