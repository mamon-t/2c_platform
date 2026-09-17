use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Структурированная ошибка скрипта с позицией в исходнике (1-based line/column).
/// Используется в контракте `script.validate` (`errors: [...]`) и клиентском пре-чеке.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error("{message}")]
pub struct ScriptError {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
}

impl ScriptError {
    /// Ошибка без координат (например, нарушение привязки к типу сущности).
    pub fn new(message: impl Into<String>) -> Self {
        Self { message: message.into(), line: None, column: None }
    }

    /// Ошибка с позицией в исходнике.
    pub fn at(message: impl Into<String>, line: u32, column: u32) -> Self {
        Self { message: message.into(), line: Some(line), column: Some(column) }
    }
}

/// Тип скрипта Rhai (ТЗ §15): определяет сценарий использования и контекст вызова.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptType {
    #[default]
    Formula,
    Validator,
    BeforeAction,
    AfterAction,
    Report,
    EventHandler,
}

impl ScriptType {
    /// Каноническая строковая форма для команд и манифеста.
    pub fn as_str(&self) -> &'static str {
        match self {
            ScriptType::Formula => "formula",
            ScriptType::Validator => "validator",
            ScriptType::BeforeAction => "before_action",
            ScriptType::AfterAction => "after_action",
            ScriptType::Report => "report",
            ScriptType::EventHandler => "event_handler",
        }
    }

    /// Стандартный контекст выполнения (поля `ctx.*`), доступные типу.
    pub fn context_keys(&self) -> &'static [&'static str] {
        match self {
            ScriptType::Formula => &["object", "settings", "user", "company"],
            ScriptType::Validator => &["object", "changes", "action", "user", "company", "settings"],
            ScriptType::BeforeAction | ScriptType::AfterAction => {
                &["object", "changes", "action", "user", "company", "settings"]
            }
            ScriptType::Report => &["object", "settings", "user", "company"],
            ScriptType::EventHandler => &["object", "action", "user", "company", "settings"],
        }
    }
}

impl TryFrom<&str> for ScriptType {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "formula" => Ok(ScriptType::Formula),
            "validator" => Ok(ScriptType::Validator),
            "before_action" => Ok(ScriptType::BeforeAction),
            "after_action" => Ok(ScriptType::AfterAction),
            "report" => Ok(ScriptType::Report),
            "event_handler" => Ok(ScriptType::EventHandler),
            other => Err(format!("неизвестный тип скрипта: {other}")),
        }
    }
}

/// Скрипт Rhai: исходный код, тип и привязки (компания, модуль, тип сущности).
/// Хранится в таблице `scripts` (Доска), события — `script.*` (Труба).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Script {
    pub id: Uuid,
    /// Уникальный машиночитаемый код (уникален в рамках компании; `None` — глобальный).
    pub code: String,
    /// Человекочитаемое имя.
    pub name: String,
    pub script_type: ScriptType,
    /// Исходный код на Rhai.
    pub source: String,
    /// Компания-владелец; `None` — глобальный скрипт платформы.
    pub company_id: Option<Uuid>,
    /// Модуль-источник при декларативной регистрации из манифеста WASM-модуля.
    pub module_code: Option<String>,
    /// Привязка к типу сущности (для validators/event handlers).
    pub entity_type: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn sample() -> Script {
        Script {
            id: Uuid::new_v4(),
            code: "price.formula".to_string(),
            name: "Цена = количество × тариф".to_string(),
            script_type: ScriptType::Formula,
            source: "ctx.object.qty * ctx.object.tariff".to_string(),
            company_id: None,
            module_code: Some("orders".to_string()),
            entity_type: Some("order_line".to_string()),
            is_active: true,
            created_at: Utc.timestamp_opt(0, 0).unwrap(),
            updated_at: Utc.timestamp_opt(0, 0).unwrap(),
        }
    }

    #[test]
    fn serde_round_trip() {
        let script = sample();
        let json = serde_json::to_value(&script).unwrap();
        let back: Script = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(script, back);
        assert_eq!(json["script_type"], "formula");
    }

    #[test]
    fn script_type_str_round_trip() {
        let types = [
            ScriptType::Formula,
            ScriptType::Validator,
            ScriptType::BeforeAction,
            ScriptType::AfterAction,
            ScriptType::Report,
            ScriptType::EventHandler,
        ];
        for t in types {
            assert_eq!(ScriptType::try_from(t.as_str()), Ok(t), "type: {t:?}");
        }
        assert!(ScriptType::try_from("bogus").is_err());
        assert!(ScriptType::try_from("Unknown").is_err());
    }

    #[test]
    fn serde_unknown_type_is_error() {
        let err = serde_json::from_str::<ScriptType>("\"bogus\"");
        assert!(err.is_err());
    }
}