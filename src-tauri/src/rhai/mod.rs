use crate::core::*;
use rhai::{Engine, Scope};
use serde::{Deserialize, Serialize};
use std::time::Duration;


#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScriptType {
    Formula,
    Validator,
    BeforeAction,
    AfterAction,
    Report,
    EventHandler,
    NotificationCondition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Script {
    pub _id: Id,
    pub company_id: Option<CompanyId>,
    pub code: String,
    pub name: String,
    pub script_type: ScriptType,
    pub source: String,
    pub version: i32,
    pub active: bool,
    pub timeout_ms: u64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

pub struct Sandbox {
    timeout: Duration,
    max_ops: u64,
}

impl Sandbox {
    pub fn new(timeout_ms: u64, max_ops: u64) -> Self {
        Self {
            timeout: Duration::from_millis(timeout_ms),
            max_ops,
        }
    }

    pub fn execute(&self, source: &str, context: &str) -> PlatformResult<serde_json::Value> {
        let engine = Engine::new();
        let mut scope = Scope::new();

        let result = engine
            .eval_with_scope::<rhai::Dynamic>(&mut scope, source)
            .map_err(|e| PlatformError::Script(format!("{context}: {e}")))?;

        serde_json::to_value(&result.to_string())
            .map_err(|e| PlatformError::Internal(format!("Ошибка сериализации результата: {e}")))
    }

    pub fn validate(&self, source: &str) -> PlatformResult<()> {
        let engine = Engine::new();
        engine
            .compile(source)
            .map_err(|e| PlatformError::Script(format!("Ошибка компиляции: {e}")))?;
        Ok(())
    }
}
