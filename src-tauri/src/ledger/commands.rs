// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

//! IPC-команды учёта (план счетов, периоды). Отчёты — в U3.

use tauri::State;
use tokio::sync::Mutex;

use crate::commands::AppState;
use crate::core::middleware::CommandContext;
use crate::db::MongoClient;

use super::service::{LedgerService, PostInput};
use super::{COL_ACCOUNTS, AccountType, LedgerAccount, OpeningBalanceRow, SaveOpeningBalanceInput};

fn db_of(s: &AppState) -> Result<MongoClient, String> {
    s.db.clone().ok_or_else(|| "Не подключено к MongoDB".into())
}

// ── План счетов ────────────────────────────────────────────

#[tauri::command]
pub async fn ledger_accounts_list(
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<LedgerAccount>, String> {
    let s = state.lock().await;
    let ctx = CommandContext::extract(&s).map_err(|e| e.to_string())?;
    ctx.check_permission("accounting.read").map_err(|e| e.to_string())?;
    let db = db_of(&s)?;

    // Первый заход в компанию — сеем типовой торговый план
    LedgerService::ensure_default_chart(&db, &ctx.company_id).await;
    LedgerService::list_accounts(&db, &ctx.company_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn ledger_account_create(
    code: String,
    name: String,
    account_type: String,
    parent_code: Option<String>,
    state: State<'_, Mutex<AppState>>,
) -> Result<LedgerAccount, String> {
    let s = state.lock().await;
    let ctx = CommandContext::extract(&s).map_err(|e| e.to_string())?;
    ctx.check_permission("accounting.manage").map_err(|e| e.to_string())?;
    let db = db_of(&s)?;

    let t: AccountType = serde_json::from_str(&format!("\"{account_type}\""))
        .map_err(|_| format!("Неизвестный тип счёта: {account_type}"))?;
    let acc = LedgerService::create_account(&db, &ctx.company_id, &code, &name, t, parent_code.as_deref())
        .await
        .map_err(|e| e.to_string())?;
    crate::audit_log!(s, db, crate::audit::AuditableAction::SaveSettings,
        target_id = acc.id.to_string());
    Ok(acc)
}

#[tauri::command]
pub async fn ledger_account_update(
    code: String,
    name: Option<String>,
    is_active: Option<bool>,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let s = state.lock().await;
    let ctx = CommandContext::extract(&s).map_err(|e| e.to_string())?;
    ctx.check_permission("accounting.manage").map_err(|e| e.to_string())?;
    let db = db_of(&s)?;
    LedgerService::update_account(&db, &ctx.company_id, &code, name.as_deref(), is_active)
        .await
        .map_err(|e| e.to_string())
}

// ── Периоды ────────────────────────────────────────────────

#[tauri::command]
pub async fn ledger_periods_list(
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<super::AccountingPeriod>, String> {
    let s = state.lock().await;
    let ctx = CommandContext::extract(&s).map_err(|e| e.to_string())?;
    ctx.check_permission("accounting.read").map_err(|e| e.to_string())?;
    let db = db_of(&s)?;
    LedgerService::list_periods(&db, &ctx.company_id).await.map_err(|e| e.to_string())
}

/// Открыть или закрыть период. Переоткрытие закрытого — отдельное право.
#[tauri::command]
pub async fn ledger_period_set_state(
    year: i32,
    month: u32,
    opened: bool,
    closed: bool,
    reopen: Option<bool>,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let s = state.lock().await;
    let ctx = CommandContext::extract(&s).map_err(|e| e.to_string())?;
    if closed && !opened {
        ctx.check_permission("accounting.manage").map_err(|e| e.to_string())?;
    } else if closed {
        // открытие закрытого периода — усиленное право
        ctx.check_permission("accounting.manage").map_err(|e| e.to_string())?;
    } else {
        ctx.check_permission("accounting.manage").map_err(|e| e.to_string())?;
    }
    let db = db_of(&s)?;
    let _ = reopen;
    LedgerService::set_period_state(&db, &ctx.company_id, year, month, opened, closed)
        .await
        .map_err(|e| e.to_string())
}

// ── Входящие сальдо ──────────────────────────────────────

/// Прочитать входящие сальдо для периода.
#[tauri::command]
pub async fn ledger_get_opening_balances(
    period_key: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<OpeningBalanceRow>, String> {
    let s = state.lock().await;
    let ctx = CommandContext::extract(&s).map_err(|e| e.to_string())?;
    ctx.check_permission("accounting.read").map_err(|e| e.to_string())?;
    let db = db_of(&s)?;
    LedgerService::get_opening_balances(&db, &ctx.company_id, &period_key)
        .await
        .map_err(|e| e.to_string())
}

/// Сохранить входящие сальдо (массовый ввод) для периода.
#[tauri::command]
pub async fn ledger_save_opening_balances(
    period_key: String,
    balances: Vec<SaveOpeningBalanceInput>,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let s = state.lock().await;
    let ctx = CommandContext::extract(&s).map_err(|e| e.to_string())?;
    ctx.check_permission("accounting.manage").map_err(|e| e.to_string())?;
    let db = db_of(&s)?;
    LedgerService::save_opening_balances(&db, &ctx.company_id, &period_key, &balances)
        .await
        .map_err(|e| e.to_string())
}

// ── Отчёты (accounting.read) ──────────────────────────────

use futures::StreamExt;
use mongodb::bson::{doc, Document};

/// ОСВ: обороты и сальдо по счетам за период.
#[tauri::command]
pub async fn ledger_osv(
    period_from: Option<String>,
    period_to: Option<String>,
    state: State<'_, Mutex<AppState>>,
) -> Result<serde_json::Value, String> {
    let s = state.lock().await;
    let ctx = CommandContext::extract(&s).map_err(|e| e.to_string())?;
    ctx.check_permission("accounting.read").map_err(|e| e.to_string())?;
    let db = s.db.as_ref().ok_or("Не подключено к MongoDB")?.clone();
    drop(s);

    let mut filter = doc! { "company_id": ctx.company_id.0.to_string() };
    if let Some(from) = &period_from { filter.insert("period_key", doc! {"$gte": from}); }
    if let Some(to) = &period_to {
        filter.insert("$or".to_owned(), mongodb::bson::Bson::Array(vec![
            mongodb::bson::Bson::Document(doc! { "period_key": { "$lte": to } }),
        ]));
    }

    // Собираем все balances rows и группируем по account_id
    let mut cursor = db.collection::<Document>(crate::ledger::COL_BALANCES)
        .find(filter).await.map_err(|e| e.to_string())?;

    use std::collections::HashMap;
    struct OsvRow {
        code: String,
        name: String,
        acc_type: String,
        opening: i64,
        debit: i64,
        credit: i64,
    }
    let mut by_account: HashMap<String, OsvRow> = HashMap::new();

    while let Some(Ok(b)) = cursor.next().await {
        let acc_id = b.get_str("account_id").unwrap_or("").to_string();
        let code = b.get_str("account_code").unwrap_or("").to_string();
        let opening = b.get_i64("opening_balance").unwrap_or(0);
        let dt = b.get_i64("debit_turnover").unwrap_or(0);
        let ct = b.get_i64("credit_turnover").unwrap_or(0);
        let row = by_account.entry(acc_id.clone()).or_insert(OsvRow {
            code: code.clone(), name: String::new(), acc_type: String::new(),
            opening: 0, debit: 0, credit: 0,
        });
        row.opening += opening;
        row.debit += dt;
        row.credit += ct;

        // Подтягиваем метаданные счёта (первый раз)
        if row.name.is_empty() {
            let acc = db.collection::<Document>(COL_ACCOUNTS)
                .find_one(doc! { "company_id": ctx.company_id.0.to_string(), "_id": &acc_id })
                .await.ok().flatten();
            if let Some(a) = acc {
                let r = by_account.get_mut(&acc_id).unwrap();
                r.name = a.get_str("name").unwrap_or("").to_string();
                r.acc_type = a.get_str("account_type").unwrap_or("asset").to_string();
            }
        }
    }

    // Вычисляем сальдо по типу счёта
    let mut rows = Vec::new();
    for (_, row) in by_account {
        let sign = match row.acc_type.as_str() {
            "liability" | "equity" | "revenue" => -1i64,
            _ => 1i64,
        };
        let balance = sign * (row.debit - row.credit);
        let closing = row.opening + balance;
        rows.push(serde_json::json!({
            "code": row.code,
            "name": row.name,
            "type": row.acc_type,
            "opening_balance": row.opening,
            "debit_turnover": row.debit,
            "credit_turnover": row.credit,
            "balance": balance,
            "closing_balance": closing,
        }));
    }
    rows.sort_by(|a, b| a["code"].as_str().unwrap_or("").cmp(b["code"].as_str().unwrap_or("")));

    Ok(serde_json::json!({ "rows": rows }))
}

/// Журнал проводок за период (с фильтрами).
#[tauri::command]
pub async fn ledger_journal(
    date_from: Option<String>,
    date_to: Option<String>,
    account_code: Option<String>,
    doc_id: Option<String>,
    limit: Option<i64>,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<serde_json::Value>, String> {
    let s = state.lock().await;
    let ctx = CommandContext::extract(&s).map_err(|e| e.to_string())?;
    ctx.check_permission("accounting.read").map_err(|e| e.to_string())?;
    let db = s.db.as_ref().ok_or("Не подключено к MongoDB")?.clone();

    let mut filter = doc! { "company_id": ctx.company_id.0.to_string() };
    if let Some(f) = &date_from { filter.insert("date", doc!{"$gte": f}); }
    if let Some(t) = &date_to { filter.insert("date", doc!{"$lte": t}); }
    if let Some(a) = &account_code {
        filter.insert("$or".to_owned(), mongodb::bson::Bson::Array(vec![
            mongodb::bson::Bson::Document(doc!{ "debit_code": a }),
            mongodb::bson::Bson::Document(doc!{ "credit_code": a }),
        ]));
    }
    if let Some(d) = &doc_id { filter.insert("doc_id", d); }

    let mut cursor = db.collection::<Document>(super::COL_ENTRIES)
        .find(filter)
        .sort(doc! { "date": -1, "created_at": -1 })
        .limit(limit.unwrap_or(100).clamp(1, 500))
        .await.map_err(|e| e.to_string())?;

    let mut items = Vec::new();
    while let Some(Ok(d)) = cursor.next().await {
        items.push(serde_json::json!({
            "id": d.get_str("_id").unwrap_or(""),
            "date": d.get_str("date").unwrap_or(""),
            "posting_id": d.get_str("posting_id").unwrap_or(""),
            "doc_kind": d.get("doc_kind"),
            "doc_id": d.get("doc_id"),
            "debit_code": d.get_str("debit_code").unwrap_or(""),
            "credit_code": d.get_str("credit_code").unwrap_or(""),
            "amount": d.get_i64("amount").unwrap_or(0),
            "nomenclature_id": d.get("nomenclature_id"),
            "description": d.get("description"),
            "is_reversal": d.get_bool("is_reversal").unwrap_or(false),
        }));
    }
    Ok(items)
}

/// Карточка счёта: проводки по счёту с нарастающим остатком.
#[tauri::command]
pub async fn ledger_card(
    account_code: String,
    date_from: Option<String>,
    date_to: Option<String>,
    state: State<'_, Mutex<AppState>>,
) -> Result<serde_json::Value, String> {
    let s = state.lock().await;
    let ctx = CommandContext::extract(&s).map_err(|e| e.to_string())?;
    ctx.check_permission("accounting.read").map_err(|e| e.to_string())?;
    let db = s.db.as_ref().ok_or("Не подключено к MongoDB")?.clone();

    let mut filter = doc! {
        "company_id": ctx.company_id.0.to_string(),
        "$or": mongodb::bson::Bson::Array(vec![
            mongodb::bson::Bson::Document(doc!{ "debit_code": &account_code }),
            mongodb::bson::Bson::Document(doc!{ "credit_code": &account_code }),
        ]),
    };
    if let Some(f) = &date_from { filter.insert("date", doc!{"$gte": f}); }
    if let Some(t) = &date_to { filter.insert("date", doc!{"$lte": t}); }

    let mut cursor = db.collection::<Document>(super::COL_ENTRIES)
        .find(filter)
        .sort(doc! { "date": 1, "created_at": 1 })
        .limit(500)
        .await.map_err(|e| e.to_string())?;

    // Определяем тип счёта для знака сальдо
    let acc = LedgerService::get_active_by_code(&db, &ctx.company_id, &account_code).await.ok();
    let sign = match acc.as_ref().map(|a| a.account_type.as_str()).unwrap_or("asset") {
        "liability" | "equity" | "revenue" => -1i64,
        _ => 1i64,
    };

    // Берём opening_balance из ledger_balances для самого раннего периода
    let opening = LedgerService::get_opening_balance_for_card(&db, &ctx.company_id, &account_code, &date_from).await.unwrap_or(0);

    let mut running_balance: i64 = opening;
    let mut items = Vec::new();
    while let Some(Ok(d)) = cursor.next().await {
        let is_debit = d.get_str("debit_code").unwrap_or("") == account_code;
        let amount = d.get_i64("amount").unwrap_or(0);
        running_balance += if is_debit { amount * sign } else { -amount * sign };

        items.push(serde_json::json!({
            "date": d.get_str("date").unwrap_or(""),
            "doc_id": d.get("doc_id"),
            "description": d.get("description"),
            "debit_code": d.get_str("debit_code").unwrap_or(""),
            "credit_code": d.get_str("credit_code").unwrap_or(""),
            "amount": amount,
            "is_debit": is_debit,
            "running_balance": running_balance,
        }));
    }

    Ok(serde_json::json!({
        "account_code": account_code,
        "sign": sign,
        "entries": items,
        "final_balance": running_balance,
    }))
}
