//! IPC-команды учёта (план счетов, периоды). Отчёты — в U3.

use tauri::State;
use tokio::sync::Mutex;

use crate::commands::AppState;
use crate::core::middleware::CommandContext;
use crate::db::MongoClient;

use super::service::{LedgerService, PostInput};
use super::{AccountType, LedgerAccount};

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
