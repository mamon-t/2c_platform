// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

//! Стартовые обработчики tx_exec.
//!
//! object.post / object.cancel — проведение и отмена объекта
//! через переданную сессию исполнителя (результат доступен
//! последующим операциям по $ref: номер, новая версия).
//! test.noop — для отладки цикла и $ref без БД.

use async_trait::async_trait;
use serde_json::Value;

use crate::core::{CompanyId, PlatformError, PlatformResult, UserId};

use super::registry::{TxOpCtx, TxOpHandler};
use super::TxContext;

fn actor_from(ctx: &TxContext) -> (UserId, CompanyId, crate::events::ActorSnapshot) {
    let user_id = UserId(ctx.actor.user_id.0);
    let company_id = CompanyId(ctx.company_id.0);
    let snapshot = crate::events::ActorSnapshot {
        user_id: user_id.clone(),
        login: ctx.actor.login.clone(),
        full_name: ctx.actor.full_name.clone(),
        position: None,
        company_id: company_id.clone(),
    };
    (user_id, company_id, snapshot)
}

// ── test.noop ──────────────────────────────────────────────

pub struct NoopHandler;

#[async_trait]
impl TxOpHandler for NoopHandler {
    fn permission(&self) -> &'static str { "" }

    async fn execute(&self, _op: &mut TxOpCtx<'_>, params: Value) -> PlatformResult<Value> {
        Ok(params)
    }
}

// ── object.post ────────────────────────────────────────────

pub struct ObjectPostHandler;

/// params: { object_id, expected_version }
#[async_trait]
impl TxOpHandler for ObjectPostHandler {
    fn permission(&self) -> &'static str { "documents.approve" }

    async fn execute(&self, op: &mut TxOpCtx<'_>, params: Value) -> PlatformResult<Value> {
        let object_id = parse_object_id(&params)?;
        let expected_version = parse_version(&params)?;

        let (user_id, company_id, actor) = actor_from(op.ctx);
        crate::objects::service::ObjectService::post_with_session(
            op.db, &mut *op.session, object_id, expected_version, user_id, actor, company_id,
        )
        .await
    }
}

// ── object.cancel ──────────────────────────────────────────

pub struct ObjectCancelHandler;

/// params: { object_id, expected_version }
#[async_trait]
impl TxOpHandler for ObjectCancelHandler {
    fn permission(&self) -> &'static str { "documents.cancel" }

    async fn execute(&self, op: &mut TxOpCtx<'_>, params: Value) -> PlatformResult<Value> {
        let object_id = parse_object_id(&params)?;
        let expected_version = parse_version(&params)?;

        let (user_id, company_id, actor) = actor_from(op.ctx);
        crate::objects::service::ObjectService::cancel_with_session(
            op.db, &mut *op.session, object_id, expected_version, user_id, actor, company_id,
        )
        .await
    }
}

// ── Парсинг параметров ─────────────────────────────────────

fn parse_object_id(params: &Value) -> PlatformResult<uuid::Uuid> {
    let s = params
        .get("object_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| PlatformError::Validation("object.transition: требуется object_id".into()))?;
    uuid::Uuid::parse_str(s).map_err(|e| PlatformError::Validation(format!("object_id: {e}")))
}

fn parse_version(params: &Value) -> PlatformResult<i64> {
    params
        .get("expected_version")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| {
            PlatformError::Validation("object.transition: требуется числовой expected_version".into())
        })
}
