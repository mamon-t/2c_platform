//! Реестр операций tx_exec.
//!
//! Имя-операции → обработчик. Исполнитель универсален: добавление
//! операции не трогает цикл. Обработчик получает ОТКРЫТУЮ сессию
//! исполнителя — все записи строго через неё.
//!
//! Реестр принадлежит ядру (v0.1). Расширяемость для модулей — на потом.

use std::collections::HashMap;
use std::sync::Arc;
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
        op: &mut TxOpCtx<'_>,
        params: Value,
    ) -> PlatformResult<Value>;
}

/// Стартовый реестр операций.
static REGISTRY: std::sync::OnceLock<HashMap<String, ArcHandler>> = std::sync::OnceLock::new();

pub type ArcHandler = std::sync::Arc<dyn TxOpHandler>;

fn registry() -> &'static HashMap<String, ArcHandler> {
    REGISTRY.get_or_init(|| {
        use super::handlers::{NoopHandler, ObjectCancelHandler, ObjectPostHandler};
        use crate::stock::handlers::{
            BalancesHandler, CountHandler, HandoverHandler, HandoverReturnHandler,
            IssueHandler, ReceiptHandler, ReverseHandler, TransferHandler,
        };
        let mut m: HashMap<String, ArcHandler> = HashMap::new();
        let mut put = |m: &mut HashMap<String, ArcHandler>, k: &str, h: ArcHandler| {
            m.insert(k.to_string(), h);
        };
        put(&mut m, "test.noop", Arc::new(NoopHandler));
        put(&mut m, "object.post", Arc::new(ObjectPostHandler));
        put(&mut m, "object.cancel", Arc::new(ObjectCancelHandler));

        // ── Склад ──
        put(&mut m, "stock.receipt", Arc::new(ReceiptHandler));
        put(&mut m, "stock.issue", Arc::new(IssueHandler));
        put(&mut m, "stock.transfer", Arc::new(TransferHandler));
        put(&mut m, "stock.handover", Arc::new(HandoverHandler));
        put(&mut m, "stock.handover_return", Arc::new(HandoverReturnHandler));
        put(&mut m, "stock.count", Arc::new(CountHandler));
        put(&mut m, "stock.balances", Arc::new(BalancesHandler));
        put(&mut m, "stock.reverse", Arc::new(ReverseHandler));
        m
    })
}

pub fn get(op: &str) -> Option<ArcHandler> {
    registry().get(op).cloned()
}

pub fn registered() -> Vec<String> {
    registry().keys().cloned().collect()
}
