//! Манифест WASM-модуля (дескриптор v2, раздел 9 ТЗ): полный набор
//! декларативных сведений о модуле, получаемый хостом из функции
//! `get_info()`. Манифест — единственный источник правды: при установке и
//! включении компании хост сам применяет схемы, политики и навигацию,
//! а код плагина при этом не выполняется.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

use crate::error::DomainError;
use crate::object::ObjectKind;
use crate::permission::{PermissionScopeType, RecordAccessLevel};

/// Capabilities, известные хосту (Приложение №5 ТЗ v3.0). Неизвестная
/// capability в манифесте приводит к отказу установки модуля.
pub const ALLOWED_CAPABILITIES: &[&str] = &[
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

/// Версия протокола манифеста, поддерживаемая хостом.
pub const SUPPORTED_API_VERSION: &str = "2.0";

/// Код модуля: строчные латинские буквы/цифры, точка и подчёркивание;
/// первый символ — буква.
fn is_valid_code(code: &str) -> bool {
    let mut chars = code.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_lowercase()
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '.')
}

/// Полный дескриптор модуля v2 (раздел 9 ТЗ v3.1, Приложение №2 ТЗ v3.0).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleManifest {
    /// Уникальный код модуля (например, `core.nomenclature`).
    pub code: String,
    /// Версия модуля в формате semver.
    pub version: String,
    /// Человекочитаемое название для интерфейсов.
    pub display_name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    /// Версия протокола манифеста: хост принимает только `SUPPORTED_API_VERSION`.
    pub api_version: String,
    /// Запрашиваемые гранты хоста (подмножество `ALLOWED_CAPABILITIES`).
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Декларативные команды модуля для регистрации в `CommandRegistry`.
    #[serde(default)]
    pub commands: Vec<ManifestCommand>,
    /// Политики доступа, поставляемые модулем.
    #[serde(default)]
    pub permissions: Vec<ManifestPermission>,
    /// Схемы типов сущностей.
    #[serde(default)]
    pub object_schemas: Vec<ManifestObjectSchema>,
    /// Печатные формы.
    #[serde(default)]
    pub print_templates: Vec<ManifestResource>,
    /// Скрипты Rhai.
    #[serde(default)]
    pub scripts: Vec<ManifestResource>,
    /// Версия метаданных модуля, влияет на ensure-обновление ресурсов.
    #[serde(default)]
    pub metadata_version: u32,
    /// Коды документов, проведение/отмена которых делегируется функции
    /// `on_post`/`on_cancel` модуля (Приложение №7 ТЗ v3.0).
    #[serde(default)]
    pub handles_documents: Vec<String>,
    /// Пункты меню модуля.
    #[serde(default)]
    pub navigation: Vec<ManifestNavItem>,
    /// Демонстрационные данные.
    #[serde(default)]
    pub demo: Option<Value>,
    /// JSON-схема настроек модуля для компании (`module_settings`).
    #[serde(default)]
    pub settings_schema: Option<Value>,
    /// Зависимости модуля от других модулей (Приложение №3 ТЗ v3.0).
    #[serde(default)]
    pub dependencies: Vec<DependencySpec>,
}

/// Декларативная команда модуля.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestCommand {
    pub code: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// Политика доступа модуля (становится `PermissionPolicy` по коду).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestPermission {
    pub code: String,
    pub description: String,
    #[serde(default)]
    pub scope_type: PermissionScopeType,
    #[serde(default)]
    pub record_access: RecordAccessLevel,
    #[serde(default)]
    pub actions: Vec<ManifestAction>,
}

/// Действие (entity_type.compose_action), разрешённое политикой.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestAction {
    pub entity_type: String,
    #[serde(default)]
    pub compose_action: bool,
    #[serde(default)]
    pub actions: Vec<String>,
}

/// Схема типа сущности, поставляемая модулем.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestObjectSchema {
    pub code: String,
    pub name: String,
    pub kind: ObjectKind,
    #[serde(default)]
    pub fields: Vec<ManifestField>,
}

/// Поле схемы объекта.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestField {
    pub code: String,
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub required: bool,
}

/// Ресурс модуля (печатная форма или скрипт).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestResource {
    pub code: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// Пункт навигации модуля.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestNavItem {
    pub code: String,
    pub title: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub route: Option<String>,
}

/// Спецификация зависимости от другого модуля (Приложение №3 ТЗ v3.0).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DependencySpec {
    /// Код модуля, от которого зависит текущий.
    pub code: String,
    /// Требуемая версия в спецификации semver (например, `^1.0.0`).
    pub version: String,
    /// Обязательность зависимости.
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub description: Option<String>,
}

impl ModuleManifest {
    /// Проверяет согласованность манифеста по правилам раздела 9 ТЗ:
    /// версия протокола, формат кода модуля, допустимые capabilities,
    /// строку версии semver и отсутствие дубликатов кодов команд и схем.
    ///
    /// # Ошибки
    ///
    /// Возвращает `DomainError::ValidationError` при первом нарушении правила.
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.api_version != SUPPORTED_API_VERSION {
            return Err(DomainError::ValidationError(format!(
                "манифест модуля: не поддерживаемая версия протокола: {} (ожидалась {})",
                self.api_version, SUPPORTED_API_VERSION
            )));
        }
        if !is_valid_code(&self.code) {
            return Err(DomainError::ValidationError(format!(
                "манифест модуля: некорректный код модуля: {}",
                self.code
            )));
        }
        if self.display_name.trim().is_empty() {
            return Err(DomainError::ValidationError(
                "манифест модуля: пустое display_name".to_string(),
            ));
        }
        semver::Version::parse(&self.version).map_err(|e| {
            DomainError::ValidationError(format!("манифест модуля: некорректная версия: {e}"))
        })?;

        let allowed: HashSet<&str> = ALLOWED_CAPABILITIES.iter().copied().collect();
        for cap in &self.capabilities {
            if !allowed.contains(cap.as_str()) {
                return Err(DomainError::ValidationError(format!(
                    "манифест модуля: неизвестная capability: {cap}"
                )));
            }
        }

        let mut command_codes = HashSet::new();
        for cmd in &self.commands {
            if cmd.code.trim().is_empty() {
                return Err(DomainError::ValidationError(
                    "манифест модуля: команда с пустым code".to_string(),
                ));
            }
            if !command_codes.insert(cmd.code.as_str()) {
                return Err(DomainError::ValidationError(format!(
                    "манифест модуля: дубликат кода команды: {}",
                    cmd.code
                )));
            }
        }

        let mut schema_codes = HashSet::new();
        for schema in &self.object_schemas {
            if !is_valid_code(&schema.code) {
                return Err(DomainError::ValidationError(format!(
                    "манифест модуля: некорректный код схемы объекта: {}",
                    schema.code
                )));
            }
            if !schema_codes.insert(schema.code.as_str()) {
                return Err(DomainError::ValidationError(format!(
                    "манифест модуля: дубликат кода схемы объекта: {}",
                    schema.code
                )));
            }
            for field in &schema.fields {
                if field.code.trim().is_empty() {
                    return Err(DomainError::ValidationError(
                        "манифест модуля: поле схемы с пустым code".to_string(),
                    ));
                }
            }
        }

        for doc_code in &self.handles_documents {
            if !schema_codes.contains(doc_code.as_str()) {
                return Err(DomainError::ValidationError(format!(
                    "манифест модуля: handles_documents ссылается на отсутствующую схему: {doc_code}"
                )));
            }
        }

        for dep in &self.dependencies {
            semver::VersionReq::parse(&dep.version).map_err(|e| {
                DomainError::ValidationError(format!(
                    "манифест модуля: некорректное требование версии зависимости {}: {e}",
                    dep.code
                ))
            })?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_manifest() -> ModuleManifest {
        serde_json::from_value(serde_json::json!({
            "code": "hello",
            "version": "1.0.0",
            "display_name": "Привет",
            "api_version": "2.0",
            "capabilities": ["logging", "storage"],
            "commands": [{ "code": "greet", "name": "Поздороваться" }],
            "object_schemas": [{
                "code": "greeting",
                "name": "Приветствие",
                "kind": "catalog",
                "fields": [{ "code": "text", "name": "Текст", "kind": "string" }]
            }],
            "handles_documents": [],
            "dependencies": []
        }))
        .unwrap()
    }

    #[test]
    fn valid_manifest_passes() {
        base_manifest().validate().unwrap();
    }

    #[test]
    fn rejects_wrong_api_version() {
        let mut m = base_manifest();
        m.api_version = "1.0".to_string();
        assert!(m.validate().is_err());
    }

    #[test]
    fn rejects_invalid_module_code() {
        let mut m = base_manifest();
        m.code = "Hello Mod".to_string();
        assert!(m.validate().is_err());
        m.code = "hello-mod".to_string();
        assert!(m.validate().is_err());
        m.code = "hello_2.mod".to_string();
        assert!(m.validate().is_ok());
    }

    #[test]
    fn rejects_unknown_capability() {
        let mut m = base_manifest();
        m.capabilities = vec!["terraform.all".to_string()];
        assert!(m.validate().is_err());
    }

    #[test]
    fn rejects_invalid_version() {
        let mut m = base_manifest();
        m.version = "not-a-version".to_string();
        assert!(m.validate().is_err());
    }

    #[test]
    fn rejects_duplicate_command_codes() {
        let mut m = base_manifest();
        m.commands = vec![
            ManifestCommand {
                code: "greet".to_string(),
                name: "A".to_string(),
                description: None,
            },
            ManifestCommand {
                code: "greet".to_string(),
                name: "B".to_string(),
                description: None,
            },
        ];
        assert!(m.validate().is_err());
    }

    #[test]
    fn rejects_document_without_schema() {
        let mut m = base_manifest();
        m.handles_documents = vec!["missing_doc".to_string()];
        assert!(m.validate().is_err());
    }

    #[test]
    fn rejects_bad_dependency_requirement() {
        let mut m = base_manifest();
        m.dependencies = vec![DependencySpec {
            code: "core.nomenclature".to_string(),
            version: ">= баг".to_string(),
            required: true,
            description: None,
        }];
        assert!(m.validate().is_err());
    }
}