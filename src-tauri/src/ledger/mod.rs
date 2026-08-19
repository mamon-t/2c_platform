use crate::core::*;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountType {
    Asset,
    Liability,
    Equity,
    Revenue,
    Expense,
    OffBalance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub _id: Id,
    pub company_id: CompanyId,
    pub code: String,
    pub name: String,
    pub account_type: AccountType,
    pub parent_id: Option<Id>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub _id: Id,
    pub company_id: CompanyId,
    pub period_id: Id,
    pub document_id: Option<Id>,
    pub account_debit: Id,
    pub account_credit: Id,
    pub amount: i64,
    pub date: NaiveDate,
    pub description: Option<String>,
    pub created_by: UserId,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerBalance {
    pub _id: Id,
    pub company_id: CompanyId,
    pub period_id: Id,
    pub account_id: Id,
    pub debit_turnover: i64,
    pub credit_turnover: i64,
    pub debit_balance: i64,
    pub credit_balance: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountingPeriod {
    pub _id: Id,
    pub company_id: CompanyId,
    pub year: i32,
    pub month: u32,
    pub opened: bool,
    pub opened_at: Option<DateTime<Utc>>,
    pub closed: bool,
    pub closed_at: Option<DateTime<Utc>>,
}

pub struct LedgerService;

impl LedgerService {
    pub fn new() -> Self {
        Self
    }

    pub fn create_account(
        &self,
        company_id: CompanyId,
        code: &str,
        name: &str,
        account_type: AccountType,
    ) -> Account {
        let now = Utc::now();
        Account {
            _id: uuid::Uuid::new_v4(),
            company_id,
            code: code.to_string(),
            name: name.to_string(),
            account_type,
            parent_id: None,
            active: true,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn create_entry(
        &self,
        company_id: CompanyId,
        period_id: Id,
        account_debit: Id,
        account_credit: Id,
        amount: i64,
        date: NaiveDate,
        created_by: UserId,
    ) -> LedgerEntry {
        LedgerEntry {
            _id: uuid::Uuid::new_v4(),
            company_id,
            period_id,
            document_id: None,
            account_debit,
            account_credit,
            amount,
            date,
            description: None,
            created_by,
            created_at: Utc::now(),
        }
    }

    /// Форматирование суммы: i64 минорные единицы -> строка "1 234,56"
    pub fn format_amount(amount: i64, currency_decimals: u32) -> String {
        let sign = if amount < 0 { "-" } else { "" };
        let abs = amount.unsigned_abs();
        let divisor = 10i64.pow(currency_decimals) as u64;
        let whole = abs / divisor;
        let frac = abs % divisor;

        format!(
            "{}{},{:0width$}",
            sign,
            whole,
            frac,
            width = currency_decimals as usize
        )
    }
}
