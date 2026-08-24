// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

//! Учёт — нативный движок двойной записи.
//!
//! Инварианты (гарантируются сервисом, живут нативно как движок склада):
//! - каждая проводка сбалансирована (пара Дт/Кт, сумма одна);
//! - счета существуют и активны;
//! - проводка возможна только в ОТКРЫТЫЙ период её даты;
//! - обороты по счетам наращиваются в `ledger_balances` той же транзакцией,
//!   что и записи; сальдо вычисляется читателем по типу счёта.
//!
//! Счета в настройках торговли адресуются КОДАМИ ("41", "60"…);
//! при проведении код резолвится в активный счёт компании.

pub mod commands;
pub mod handlers;
pub mod indexes;
pub mod service;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ── Коллекции ──────────────────────────────────────────────

pub const COL_ACCOUNTS: &str = "ledger_accounts";
pub const COL_ENTRIES: &str = "ledger_entries";
pub const COL_BALANCES: &str = "ledger_balances";
pub const COL_PERIODS: &str = "accounting_periods";

// ── План счетов ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountType {
    Asset,     // актив
    Liability, // пассив
    Equity,    // капитал
    Revenue,   // доход
    Expense,   // расход
    OffBalance, // забалансовый
}

impl AccountType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Asset => "asset",
            Self::Liability => "liability",
            Self::Equity => "equity",
            Self::Revenue => "revenue",
            Self::Expense => "expense",
            Self::OffBalance => "off_balance",
        }
    }

    /// Сальдо по типу счёта: актив/расход — Дт минус Кт;
    /// пассив/капитал/доход — Кт минус Дт. Забалансовые — обороты без сальдо.
    pub fn balance_sign(&self) -> i64 {
        match self {
            Self::Asset | Self::Expense => 1,
            Self::OffBalance => 1,
            _ => -1,
        }
    }
}

/// Счёт плана счетов компании. Код уникален в рамках компании.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerAccount {
    #[serde(rename = "_id")]
    pub id: crate::core::Id,
    pub company_id: CompanyIdStr,
    pub code: String,
    pub name: String,
    pub account_type: AccountType,
    /// Родительский счёт (иерархия), опционально.
    #[serde(default)]
    pub parent_code: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Псевдоним для читаемости: компания хранится строкой UUID.
pub type CompanyIdStr = String;

// ── Проводка (пара Дт/Кт = один документ коллекции) ───────

/// Одна корреспонденция счетов.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostingLine {
    pub debit_code: String,
    pub credit_code: String,
    /// Сумма в минорных единицах (копейки), > 0.
    pub amount: i64,
    #[serde(default)]
    pub nomenclature_id: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

/// Запись журнала — одна корреспонденция. Группируется в постинг
/// через posting_id; документ-источник — через doc_id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEntry {
    #[serde(rename = "_id")]
    pub id: crate::core::Id,
    pub company_id: CompanyIdStr,
    /// "YYYY-MM"
    pub period_key: String,
    /// "YYYY-MM-DD"
    pub date: String,
    pub posting_id: String,
    #[serde(default)]
    pub doc_kind: Option<String>,
    #[serde(default)]
    pub doc_id: Option<String>,

    pub debit_code: String,
    pub credit_code: String,
    pub amount: i64,

    /// Измерение для построчной себестоимости (возвраты покупателя).
    #[serde(default)]
    pub nomenclature_id: Option<String>,
    #[serde(default)]
    pub description: Option<String>,

    #[serde(default)]
    pub is_reversal: bool,

    pub created_by: UserIdStr,
    pub created_at: DateTime<Utc>,
}

pub type UserIdStr = String;

/// Обороты счёта за период. Сальдо считает читатель:
/// sign(типа) × (Дт − Кт) накопленно по периодам ≤ целевого.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerBalance {
    #[serde(rename = "_id")]
    pub id: crate::core::Id,
    pub company_id: CompanyIdStr,
    /// "YYYY-MM"
    pub period_key: String,
    pub account_id: crate::core::Id,
    pub account_code: String,
    pub account_type: AccountType,
    pub debit_turnover: i64,
    pub credit_turnover: i64,
    pub updated_at: DateTime<Utc>,
}

// ── Периоды ────────────────────────────────────────────────

/// Учётный период (месяц).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountingPeriod {
    #[serde(rename = "_id")]
    pub id: crate::core::Id,
    pub company_id: CompanyIdStr,
    /// "YYYY-MM"
    pub period_key: String,
    pub year: i32,
    pub month: u32,
    pub opened: bool,
    pub closed: bool,
    /// RFC3339 строка (bson DateTime ≠ chrono serde)
    pub created_at: String,
}

impl AccountingPeriod {
    pub fn period_key(year: i32, month: u32) -> String {
        format!("{year:04}-{month:02}")
    }
}

pub fn period_key_of_date(date: &str) -> String {
    date.get(..7).unwrap_or("0000-00").to_string()
}
