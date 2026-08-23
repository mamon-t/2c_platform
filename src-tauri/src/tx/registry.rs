//! Реестр операций tx_exec.
//!
//! Имя-операции → обработчик. Исполнитель универсален: добавление
//! операции не трогает цикл. Обработчик получает ОТКРЫТУЮ сессию
//! исполнителя — все записи строго через неё.
//!
//! Реестр принадлежит ядру (v0.1). Расширяемость для модулей — на потом.

use std::collections::HashMap;
use std::sync::LazyLock;

use async_trait::async_trait;
use serde_json::Value;

use crate::core::{PlatformResult};
use crate::db::MongoClient;

use super::TxContext;

/// Контекст одной операции внутри транзакции.
pub struct TxOpCtx<'a> {
    pub ctx: &'a TxContext,
    pub db: &'a MongoClient,
    /// Открытая сессия ИСПОЛНИТЕЛЯ — все записи только через неё.
    pub session: &'a mut mongodb::ClientSession,
}

#[async_trait]
pub trait TxOpHandler: Send + Sync {
    /// subsystem.action — проверяется для вызывающего перед выполнением
    /// (второй уровень защиты, после права на пачку).
    fn permission(&self) -> &'static str;

    async fn execute(
        &self,
        op: &TxOpCtx<'_>,
        params: Value,
    ) -> PlatformResult<Value>;
}

/// Стартовый реестр (X2 наполнит: test.noop, object.transition).
static REGISTRY: std::sync::OnceLock<HashMap<String, ArcHandler>> = std::sync::OnceLock::new();

pub type ArcHandler = std::sync::Arc<dyn TxOpHandler>;

fn registry() -> &'static HashMap<String, ArcHandler> {
    REGISTRY.get_or_init(|| {
        // X2: REGISTRY.insert("test.noop", ...); и т.д.
        HashMap::new()
    })
}

pub fn get(op: &str) -> Option<ArcHandler> {
    registry().get(op).cloned()
}

pub fn registered() -> Vec<String> {
    registry().keys().cloned().collect()
}
