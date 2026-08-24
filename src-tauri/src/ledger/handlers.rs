//! Обработчики учёта для реестра tx_exec (Блок 2).
//!
//! Выполняются ВНУТРИ транзакции исполнителя: те же проверки Блока 1,
//! записи и обороты — через сессию исполнителя. Атомарно со складом
//! и переходом документа.

use async_trait::async_trait;
use serde_json::Value;

use crate::core::{CompanyId, PlatformError, PlatformResult, UserId};
use crate::db::MongoClient;
use crate::events::ActorSnapshot;

use crate::tx::registry::{TxOpCtx, TxOpHandler};

fn actor_of(ctx: &crate::tx::TxContext) -> (CompanyId, UserId, ActorSnapshot) {
    let company = CompanyId(ctx.company_id.0);
    let user = UserId(ctx.actor.user_id.0);
    let snapshot = ActorSnapshot {
        user_id: user.clone(),
        login: ctx.actor.login.clone(),
        full_name: ctx.actor.full_name.clone(),
        position: None,
        company_id: company.clone(),
    };
    (company, user, snapshot)
}

fn parse_lines(params: &Value) -> PlatformResult<Vec<super::PostingLine>> {
    serde_json::from_value(
        params.get("lines").cloned().unwrap_or(Value::Array(vec![])),
    )
    .map_err(|e| PlatformError::Validation(format!("lines: {e}")))
}

// ── accounting.post ────────────────────────────────────────

/// params: {date, doc_kind?, doc_id?, lines:[{debit_code,credit_code,amount,
///          nomenclature_id?,description?}]}
pub struct AccountingPostHandler;

#[async_trait]
impl TxOpHandler for AccountingPostHandler {
    fn permission(&self) -> &'static str { "accounting.post" }

    async fn execute(&self, op: &mut TxOpCtx<'_>, params: Value) -> PlatformResult<Value> {
        let date = params.get("date").and_then(|v| v.as_str())
            .ok_or_else(|| PlatformError::Validation("требуется date (YYYY-MM-DD)".into()))?
            .to_string();
        let lines = parse_lines(&params)?;
        let (company, user, _actor) = actor_of(op.ctx);

        let input = super::service::PostInput {
            company_id: &company,
            created_by: user,
            date: &date,
            doc_kind: params.get("doc_kind").and_then(|v| v.as_str()),
            doc_id: params.get("doc_id").and_then(|v| v.as_str()),
            lines,
            is_reversal: false,
        };
        let sess = &mut *op.session;
        super::service::LedgerService::post_pairs_in_session(op.db, sess, input)
            .await
    }
}

// ── accounting.reverse_by_doc ──────────────────────────────

/// params: {target_doc_id, date?, new_doc_kind?, new_doc_id?}
pub struct AccountingReverseHandler;

#[async_trait]
impl TxOpHandler for AccountingReverseHandler {
    fn permission(&self) -> &'static str { "accounting.post" }

    async fn execute(&self, op: &mut TxOpCtx<'_>, params: Value) -> PlatformResult<Value> {
        let target = params.get("target_doc_id").and_then(|v| v.as_str())
            .ok_or_else(|| PlatformError::Validation("требуется target_doc_id".into()))?
            .to_string();
        let date = params.get("date").and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| chrono::Utc::now().date_naive().to_string());
        let (company, user, _actor) = actor_of(op.ctx);
        let sess = &mut *op.session;

        super::service::LedgerService::reverse_by_doc_in_session(
            op.db,
            sess,
            &company,
            user,
            &target,
            params.get("new_doc_kind").and_then(|v| v.as_str()),
            params.get("new_doc_id").and_then(|v| v.as_str()),
            &date,
        )
        .await
    }
}
