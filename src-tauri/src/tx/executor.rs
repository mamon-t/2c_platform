//! Исполнитель пачек транзакционных операций.
//!
//! Фазы:
//!   1. Валидация + идемпотентность + права — ДО открытия транзакции
//!   2. Открытие транзакции MongoDB
//!   3. Последовательное выполнение: resolve $refs → реестр → op_results
//!   4. Журнал ВНУТРИ транзакции → коммит (дубликат ключа = вернуть результат победителя)
//!   5. TxResult или TxError с указанием упавшей операции

use std::collections::HashMap;

use tracing::{info, warn};

use crate::audit::service::{AuditService as _, MongoAuditService};

use super::journal::TxJournal;
use super::registry;
use super::{TransactionPackage, TxContext, TxError, TxResult};

/// Лимиты v0.1 — уточнятся нагрузочным тестированием.
const MAX_PACKAGE_OPS: usize = 100;
const EXECUTE_TIMEOUT_SECS: u64 = 30;

pub async fn execute(
    db: &crate::db::MongoClient,
    pkg: TransactionPackage,
) -> Result<TxResult, TxError> {
    // ── Фаза 1: валидация и идемпотентность — до открытия транзакции ──

    pkg.validate().map_err(TxError::new)?;

    if pkg.operations.len() > MAX_PACKAGE_OPS {
        return Err(TxError::new(format!(
            "пачка превышает лимит операций ({}/{MAX_PACKAGE_OPS})",
            pkg.operations.len()
        )));
    }
    if pkg.is_expired(chrono::Utc::now()) {
        return Err(TxError::new("пачка истекла (expires_at)"));
    }

    let ctx = &pkg.context;

    // Идемпотентный повтор: уже закоммичена — вернуть сохранённый результат.
    if let Some(result) = TxJournal::find_committed(db, &ctx.company_id, &pkg.idempotency_key)
        .await
        .map_err(TxError::from)?
    {
        info!(
            "[tx] идемпотентный повтор {}: возврат сохранённого результата",
            pkg.idempotency_key
        );
        return Ok(TxResult { op_results: serde_json::from_value(result).unwrap_or_default() });
    }

    // Право на шаблон пачки (цельное бизнес-действие).
    if let Some(perm) = &pkg.required_permission {
        ctx.check_permission(perm).map_err(TxError::from)?;
    }

    // ── Фаза 2–4: одна транзакция на всю пачку ──

    let mut session = db
        .client()
        .start_session()
        .await
        .map_err(|e| TxError::new(format!("start_session: {e}")))?;
    session
        .start_transaction()
        .await
        .map_err(|e| TxError::new(format!("start_transaction: {e}")))?;

    match execute_inner(db, &mut session, &pkg).await {
        Ok(tx_result) => {
            // ── Фаза 4: журнал внутри той же транзакции ──
            let result_json = serde_json::to_value(&tx_result)
                .map_err(|e| TxError::new(format!("BSON результата: {e}")))?;

            if let Err(e) = TxJournal::insert_committed_in_session(
                db,
                &mut session,
                &ctx.company_id,
                &pkg.idempotency_key,
                pkg.operations.len(),
                &result_json,
            )
            .await
            {
                session.abort_transaction().await.ok();
                return finish_on_duplicate(db, ctx, &pkg.idempotency_key, &e.to_string()).await;
            }

            match session.commit_transaction().await {
                Ok(_) => {
                    info!("[tx] committed {}: {} ops", pkg.idempotency_key, pkg.operations.len());
                    audit_committed(db, ctx, &pkg).await;
                    Ok(tx_result)
                }
                Err(commit_err) => {
                    session.abort_transaction().await.ok();
                    finish_on_duplicate(db, ctx, &pkg.idempotency_key, &commit_err.to_string()).await
                }
            }
        }
        Err(tx_err) => {
            // ── Фаза 5: откат ──
            session.abort_transaction().await.ok();
            warn!(
                "[tx] rollback {}: {}",
                pkg.idempotency_key,
                tx_err.failed_op.as_deref().unwrap_or("-")
            );
            Err(tx_err)
        }
    }
}

/// Фаза 3: последовательное выполнение операций в открытой сессии.
async fn execute_inner(
    db: &crate::db::MongoClient,
    session: &mut mongodb::ClientSession,
    pkg: &TransactionPackage,
) -> Result<TxResult, TxError> {
    let mut op_results: HashMap<String, serde_json::Value> = HashMap::new();

    let body = async {
        for op in &pkg.operations {
            let handler = registry::get(&op.op)
                .ok_or_else(|| TxError::new(format!("неизвестная операция {:?}", op.op)).with_failed_op(&op.op_id))?;

            // Право обработчика — второй уровень защиты.
            pkg.context
                .check_permission(handler.permission())
                .map_err(|e| TxError::from(e).with_failed_op(&op.op_id))?;

            let params =
                super::resolve_refs(&op.params, &op_results).map_err(|e| TxError::new(e).with_failed_op(&op.op_id))?;

            let mut op_ctx = registry::TxOpCtx { ctx: &pkg.context, db, session };
            let value = handler
                .execute(&mut op_ctx, params)
                .await
                .map_err(|e| TxError::from(e).with_failed_op(&op.op_id))?;

            op_results.insert(op.op_id.clone(), value);
        }
        Ok::<TxResult, TxError>(TxResult { op_results })
    };

    tokio::time::timeout(std::time::Duration::from_secs(EXECUTE_TIMEOUT_SECS), body)
        .await
        .unwrap_or_else(|_| Err(TxError::new(format!("таймаут исполнения {EXECUTE_TIMEOUT_SECS}s"))))
}

/// Дубликат ключа (вставка журнала или коммит): победитель уже закоммитился.
/// Откат выполнен вызывающим — перечитываем журнал и возвращаем его результат.
/// Если победителя не видно — конкурентный конфликт.
async fn finish_on_duplicate(
    db: &crate::db::MongoClient,
    ctx: &TxContext,
    key: &str,
    err_msg: &str,
) -> Result<TxResult, TxError> {
    if !is_duplicate_key(err_msg) {
        return Err(TxError::new(format!("журнал/commit: {err_msg}")));
    }
    match TxJournal::find_committed(db, &ctx.company_id, key).await {
        Ok(Some(result)) => {
            info!("[tx] конкурентный повтор {key}: возвращён результат победителя");
            Ok(TxResult { op_results: serde_json::from_value(result).unwrap_or_default() })
        }
        _ => Err(TxError::new(format!(
            "конкурентный конфликт: пачка {key:?} выполняется параллельно"
        ))),
    }
}

fn is_duplicate_key(msg: &str) -> bool {
    msg.contains("E11000") || msg.contains("duplicate key")
}

/// Аудит после коммита (warn-and-forget): побочки — снаружи транзакции.
async fn audit_committed(db: &crate::db::MongoClient, ctx: &TxContext, pkg: &TransactionPackage) {
    use crate::core::UserId;
    let entry = crate::audit::AuditEntry::new(
        crate::audit::AuditableAction::ExecuteTransaction,
        UserId(ctx.actor.user_id.0),
        ctx.company_id.clone(),
        Some(pkg.idempotency_key.clone()),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    if let Err(e) = MongoAuditService::new().log(db, entry).await {
        warn!("[tx] audit write failed: {e}");
    }
}
