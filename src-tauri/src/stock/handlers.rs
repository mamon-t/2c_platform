// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

//! Мост движка склада в реестр tx_exec.
//!
//! Движок — поставщик атомарных кубиков: каждый обработчик — тонкая
//! обёртка над engine-функцией. Пачку собирает оркестратор (плагин
//! или обработчик документа), движок не знает её состава.

use async_trait::async_trait;
use serde_json::Value;

use crate::core::{CompanyId, PlatformError, PlatformResult, UserId};

use crate::tx::registry::{TxOpCtx, TxOpHandler};
use crate::stock::engine::EngineCtx;

/// Разобрать общий хвост параметров: doc_ref, ответственный, срок.
struct Common {
    doc_kind: Option<String>,
    doc_id: Option<String>,
    responsible_user_id: Option<String>,
    expected_return_date: Option<String>,
}

fn parse_common(params: &Value) -> Common {
    Common {
        doc_kind: params.get("doc_kind").and_then(|v| v.as_str()).map(String::from),
        doc_id: params.get("doc_id").and_then(|v| v.as_str()).map(String::from),
        responsible_user_id: params.get("responsible_user_id").and_then(|v| v.as_str()).map(String::from),
        expected_return_date: params.get("expected_return_date").and_then(|v| v.as_str()).map(String::from),
    }
}

fn parse_lines_receipt(params: &Value) -> PlatformResult<Vec<super::ReceiptLine>> {
    serde_json::from_value(
        params.get("lines").cloned().unwrap_or(Value::Array(vec![])),
    )
    .map_err(|e| PlatformError::Validation(format!("lines: {e}")))
}

fn parse_lines_issue(params: &Value) -> PlatformResult<Vec<super::IssueLine>> {
    serde_json::from_value(
        params.get("lines").cloned().unwrap_or(Value::Array(vec![])),
    )
    .map_err(|e| PlatformError::Validation(format!("lines: {e}")))
}

fn require_str(params: &Value, field: &str) -> PlatformResult<String> {
    params
        .get(field)
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| PlatformError::Validation(format!("требуется поле {field}")))
}

macro_rules! stock_handler {
    ($name:ident, $perm:expr) => {
        pub struct $name;

        #[async_trait]
        impl TxOpHandler for $name {
            fn permission(&self) -> &'static str {
                $perm
            }
            async fn execute(&self, op: &mut TxOpCtx<'_>, params: Value) -> PlatformResult<Value> {
                let company_id = CompanyId(op.ctx.company_id.0);
                let actor = crate::events::ActorSnapshot {
                    user_id: UserId(op.ctx.actor.user_id.0),
                    login: op.ctx.actor.login.clone(),
                    full_name: op.ctx.actor.full_name.clone(),
                    position: None,
                    company_id: company_id.clone(),
                };
                run(<$name as Tagged>::tag(), op, company_id, actor, params).await
            }
        }
    };
}

stock_handler!(ReceiptHandler, "stock.use");
stock_handler!(IssueHandler, "stock.use");
stock_handler!(TransferHandler, "stock.use");
stock_handler!(HandoverHandler, "stock.use");
stock_handler!(HandoverReturnHandler, "stock.use");
stock_handler!(CountHandler, "stock.use");
stock_handler!(ReverseHandler, "stock.use");

// balances — чтение
pub struct BalancesHandler;

#[async_trait]
impl TxOpHandler for BalancesHandler {
    fn permission(&self) -> &'static str { "stock.read" }

    async fn execute(&self, op: &mut TxOpCtx<'_>, params: Value) -> PlatformResult<Value> {
        let company_id = CompanyId(op.ctx.company_id.0);
        let actor = crate::events::ActorSnapshot {
            user_id: UserId(op.ctx.actor.user_id.0),
            login: op.ctx.actor.login.clone(),
            full_name: op.ctx.actor.full_name.clone(),
            position: None,
            company_id: company_id.clone(),
        };
        let mut e = EngineCtx { db: op.db, session: op.session, company_id, actor };
        super::engine::balances(
            &mut e,
            params.get("location_id").and_then(|v| v.as_str()),
            params.get("nomenclature_id").and_then(|v| v.as_str()),
        )
        .await
    }
}

/// Тег для диспетчеризации в run().
#[derive(Clone, Copy, PartialEq)]
enum Tag {
    Receipt,
    Issue,
    Transfer,
    Handover,
    HandoverReturn,
    Count,
    Reverse,
}

trait Tagged {
    fn tag() -> Tag;
}
impl Tagged for ReceiptHandler { fn tag() -> Tag { Tag::Receipt } }
impl Tagged for IssueHandler { fn tag() -> Tag { Tag::Issue } }
impl Tagged for TransferHandler { fn tag() -> Tag { Tag::Transfer } }
impl Tagged for HandoverHandler { fn tag() -> Tag { Tag::Handover } }
impl Tagged for HandoverReturnHandler { fn tag() -> Tag { Tag::HandoverReturn } }
impl Tagged for CountHandler { fn tag() -> Tag { Tag::Count } }
impl Tagged for ReverseHandler { fn tag() -> Tag { Tag::Reverse } }

async fn run(
    tag: Tag,
    op: &mut TxOpCtx<'_>,
    company_id: CompanyId,
    actor: crate::events::ActorSnapshot,
    params: Value,
) -> PlatformResult<Value> {
    let c = parse_common(&params);
    let doc_ref = match (&c.doc_kind, &c.doc_id) {
        (Some(k), Some(i)) => Some((k.clone(), i.clone())),
        _ => None,
    };

    let mut e = EngineCtx { db: op.db, session: op.session, company_id, actor };

    match tag {
        Tag::Receipt => {
            let location_id = require_str(&params, "location_id")?;
            let lines = parse_lines_receipt(&params)?;
            super::engine::receipt(&mut e, &location_id, lines, doc_ref).await
        }
        Tag::Issue => {
            let location_id = require_str(&params, "location_id")?;
            let lines = parse_lines_issue(&params)?;
            super::engine::issue(&mut e, &location_id, lines, super::MovementKind::Issue, doc_ref).await
        }
        Tag::Count => {
            let location_id = require_str(&params, "location_id")?;
            let facts = parse_lines_issue(&params)?;
            super::engine::count(&mut e, &location_id, facts, doc_ref).await
        }
        Tag::Transfer | Tag::Handover | Tag::HandoverReturn => {
            let from = require_str(&params, "from_location_id")?;
            let to = require_str(&params, "to_location_id")?;
            let lines = parse_lines_issue(&params)?;
            let handover = matches!(tag, Tag::Handover);
            // Возврат из подотчёта — handover-пара движений + ссылка на выдачу
            if tag == Tag::Handover && c.responsible_user_id.is_none() {
                return Err(PlatformError::Validation(
                    "выдача под отчёт требует responsible_user_id".into(),
                ));
            }
            super::engine::transfer(
                &mut e,
                &from,
                &to,
                lines,
                handover || tag == Tag::HandoverReturn,
                c.responsible_user_id,
                c.expected_return_date,
                c.doc_id,
                doc_ref,
            )
            .await
        }
        Tag::Reverse => {
            let doc_id = require_str(&params, "target_doc_id")?;
            super::engine::reverse_document(&mut e, &doc_id).await
        }
    }
}
