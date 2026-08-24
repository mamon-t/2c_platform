// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

//! LedgerService: план счетов, периоды, проводки, балансы.
//!
//! Все записи выполняются через ОПЦИОНАЛЬНУЮ сессию: внутри tx_exec
//! передаётся сессия исполнителя (атомарность со складом и документом),
//! при прямом вызове — None.

use chrono::Utc;
use futures::StreamExt;
use mongodb::bson::{doc, Document};
use mongodb::options::{FindOneOptions, UpdateOptions};

use crate::core::{CompanyId, PlatformError, PlatformResult};
use crate::db::MongoClient;

use super::{
    period_key_of_date, AccountType, AccountingPeriod, LedgerAccount, LedgerBalance, LedgerEntry,
    PostingLine, COL_ACCOUNTS, COL_BALANCES, COL_ENTRIES, COL_PERIODS,
};

pub struct LedgerService;

/// Параметры одного постинга.
pub struct PostInput<'a> {
    pub company_id: &'a CompanyId,
    pub created_by: crate::core::UserId,
    pub date: &'a str, // "YYYY-MM-DD"
    pub doc_kind: Option<&'a str>,
    pub doc_id: Option<&'a str>,
    pub lines: Vec<PostingLine>,
    /// Сторно-постинг (зеркальный): помечает записи и НЕ требует
    /// отдельной проверки направлений.
    pub is_reversal: bool,
}

impl LedgerService {
    // ── План счетов ────────────────────────────────────────

    pub async fn list_accounts(
        db: &MongoClient,
        company_id: &CompanyId,
    ) -> PlatformResult<Vec<LedgerAccount>> {
        let mut cursor = db
            .collection::<Document>(COL_ACCOUNTS)
            .find(doc! { "company_id": company_id.0.to_string() })
            .sort(doc! { "code": 1 })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        let mut out = Vec::new();
        while let Some(Ok(a)) = cursor.next().await {
            if let Ok(a) = mongodb::bson::from_document::<LedgerAccount>(a) {
                out.push(a);
            }
        }
        Ok(out)
    }

    pub async fn get_active_by_code(
        db: &MongoClient,
        company_id: &CompanyId,
        code: &str,
    ) -> PlatformResult<LedgerAccount> {
        db.collection::<Document>(COL_ACCOUNTS)
            .find_one(doc! {
                "company_id": company_id.0.to_string(),
                "code": code,
                "is_active": true,
            })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?
            .map(mongodb::bson::from_document::<LedgerAccount>)
            .transpose()
            .map_err(|e| PlatformError::Internal(e.to_string()))?
            .ok_or_else(|| {
                PlatformError::NotFound(format!("Счёт {code:?} не найден или неактивен"))
            })
    }

    pub async fn create_account(
        db: &MongoClient,
        company_id: &CompanyId,
        code: &str,
        name: &str,
        account_type: AccountType,
        parent_code: Option<&str>,
    ) -> PlatformResult<LedgerAccount> {
        if code.trim().is_empty() || name.trim().is_empty() {
            return Err(PlatformError::Validation("Код и название обязательны".into()));
        }
        let dup = db
            .collection::<Document>(COL_ACCOUNTS)
            .count_documents(doc! { "company_id": company_id.0.to_string(), "code": code })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        if dup > 0 {
            return Err(PlatformError::Validation(format!(
                "Счёт с кодом {code} уже существует"
            )));
        }

        let now = Utc::now();
        let acc = LedgerAccount {
            id: uuid::Uuid::new_v4(),
            company_id: company_id.0.to_string(),
            code: code.to_string(),
            name: name.to_string(),
            account_type,
            parent_code: parent_code.map(String::from),
            is_active: true,
            created_at: now,
            updated_at: now,
        };
        let mut d = mongodb::bson::to_document(&acc)
            .map_err(|e| PlatformError::Internal(e.to_string()))?;
        d.insert("_id", acc.id.to_string());
        d.insert("company_id", company_id.0.to_string());
        db.collection::<Document>(COL_ACCOUNTS)
            .insert_one(d)
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(acc)
    }

    pub async fn update_account(
        db: &MongoClient,
        company_id: &CompanyId,
        code: &str,
        name: Option<&str>,
        is_active: Option<bool>,
    ) -> PlatformResult<()> {
        let mut set = doc! { "updated_at": mongodb::bson::DateTime::now() };
        if let Some(n) = name { set.insert("name", n); }
        if let Some(a) = is_active { set.insert("is_active", a); }
        let res = db
            .collection::<Document>(COL_ACCOUNTS)
            .update_one(
                doc! { "company_id": company_id.0.to_string(), "code": code },
                doc! { "$set": set },
            )
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        if res.matched_count == 0 {
            return Err(PlatformError::NotFound(format!("Счёт {code} не найден")));
        }
        Ok(())
    }

    /// Seed стандартного торгового плана, если у компании счетов нет.
    pub async fn ensure_default_chart(db: &MongoClient, company_id: &CompanyId) {
        let existing = Self::list_accounts(db, company_id).await.unwrap_or_default();
        if !existing.is_empty() { return; }
        let chart: &[(&str, &str, AccountType)] = &[
            ("41", "Товары", AccountType::Asset),
            ("44", "Расходы на продажу", AccountType::Expense),
            ("50", "Касса", AccountType::Asset),
            ("51", "Банковский счёт", AccountType::Asset),
            ("60", "Расчёты с поставщиками", AccountType::Liability),
            ("62", "Расчёты с покупателями", AccountType::Asset),
            ("90.1", "Выручка", AccountType::Revenue),
            ("90.2", "Себестоимость продаж", AccountType::Expense),
        ];
        for (code, name, t) in chart {
            let _ = Self::create_account(db, company_id, code, name, t.clone(), None).await;
        }
    }

    // ── Периоды ────────────────────────────────────────────

    async fn ensure_period(
        session: &mut mongodb::ClientSession,
        db: &MongoClient,
        company_id: &CompanyId,
        period_key: &str,
    ) -> PlatformResult<AccountingPeriod> {
        let (year, month) = parse_period_key(period_key)?;
        let filter = doc! {
            "company_id": company_id.0.to_string(),
            "period_key": period_key,
        };

        // Апсерт открытого периода ($setOnInsert — идемпотентно)
        let set_on_insert = doc! {
            "_id": uuid::Uuid::new_v4().to_string(),
            "company_id": company_id.0.to_string(),
            "period_key": period_key,
            "year": year,
            "month": month as i32,
            "opened": true,
            "closed": false,
            "created_at": Utc::now().to_rfc3339(),
        };
        let col = db.collection::<Document>(COL_PERIODS);
        col.update_one(filter.clone(), doc! { "$setOnInsert": set_on_insert })
            .upsert(true)
            .session(&mut *session)
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;

        let d = col.find_one(filter)
            .session(&mut *session)
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?
        .ok_or_else(|| PlatformError::Internal("период не создался".into()))?;

        mongodb::bson::from_document::<AccountingPeriod>(d)
            .map_err(|e| PlatformError::Internal(format!("период: {e}")))
    }

    pub async fn list_periods(
        db: &MongoClient,
        company_id: &CompanyId,
    ) -> PlatformResult<Vec<AccountingPeriod>> {
        let mut cursor = db.collection::<Document>(COL_PERIODS)
            .find(doc! { "company_id": company_id.0.to_string() })
            .sort(doc! { "period_key": -1 })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        let mut out = Vec::new();
        while let Some(Ok(d)) = cursor.next().await {
            if let Ok(p) = mongodb::bson::from_document::<AccountingPeriod>(d) {
                out.push(p);
            }
        }
        Ok(out)
    }

    pub async fn set_period_state(
        db: &MongoClient,
        company_id: &CompanyId,
        year: i32,
        month: u32,
        opened: bool,
        closed: bool,
    ) -> PlatformResult<()> {
        let key = AccountingPeriod::period_key(year, month);
        let res = db.collection::<Document>(COL_PERIODS)
            .update_one(
                doc! { "company_id": company_id.0.to_string(), "period_key": &key },
                doc! { "$set": { "opened": opened, "closed": closed } },
            )
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        if res.matched_count == 0 {
            return Err(crate::core::PlatformError::NotFound(format!(
                "Период {key} не существует (создаётся первой проводкой)"
            )));
        }
        Ok(())
    }

    // ── Проводки ───────────────────────────────────────────

    /// Провести пачку пар Дт/Кт. Все проверки Блока 1; записи и обороты —
    /// через переданную опциональную сессию.
    pub async fn post_pairs_in_session(
        db: &MongoClient,
        session: &mut mongodb::ClientSession,
        input: PostInput<'_>,
    ) -> PlatformResult<serde_json::Value> {
        if input.lines.is_empty() {
            return Err(PlatformError::Validation("нет проводок".into()));
        }
        for l in &input.lines {
            if l.amount <= 0 {
                return Err(PlatformError::Validation(
                    format!("сумма должна быть > 0 (получено {})", l.amount),
                ));
            }
            if l.debit_code == l.credit_code {
                return Err(PlatformError::Validation(format!(
                    "дебет и кредит совпадают ({})", l.debit_code
                )));
            }
        }

        let period_key = period_key_of_date(input.date);
        let _period = Self::ensure_period(session, db, input.company_id, &period_key).await?;
        if !_period.opened || _period.closed {
            return Err(PlatformError::Validation(format!(
                "период {period_key} закрыт для проводок"
            )));
        }
        // Дата вне периода (месяц даты ≠ период ключа) — запрещаем:
        // дата определяет период жёстко.
        if period_key_of_date(input.date) != period_key {
            unreachable!();
        }

        // Резолв кодов в активные счета + денормализация типа для балансов
        let mut resolved: Vec<(LedgerAccount, LedgerAccount, &PostingLine)> = Vec::new();
        for l in &input.lines {
            let dt = Self::get_active_by_code(db, input.company_id, &l.debit_code).await?;
            let ct = Self::get_active_by_code(db, input.company_id, &l.credit_code).await?;
            resolved.push((dt, ct, l));
        }

        let posting_id = uuid::Uuid::new_v4();
        let entries_col = db.collection::<Document>(COL_ENTRIES);
        let bal_col = db.collection::<Document>(COL_BALANCES);

        for (dt_acc, ct_acc, line) in resolved {
            let entry = LedgerEntry {
                id: uuid::Uuid::new_v4(),
                company_id: input.company_id.0.to_string(),
                period_key: period_key.clone(),
                date: input.date.to_string(),
                posting_id: posting_id.to_string(),
                doc_kind: input.doc_kind.map(String::from),
                doc_id: input.doc_id.map(String::from),
                debit_code: line.debit_code.clone(),
                credit_code: line.credit_code.clone(),
                amount: line.amount,
                nomenclature_id: line.nomenclature_id.clone(),
                description: line.description.clone(),
                is_reversal: input.is_reversal,
                created_by: input.created_by.0.to_string(),
                created_at: Utc::now(),
            };
            let mut edoc = mongodb::bson::to_document(&entry)
                .map_err(|e| PlatformError::Internal(e.to_string()))?;
            edoc.insert("_id", entry.id.to_string());

            entries_col.insert_one(edoc)
                .session(&mut *session).await
                .map_err(|e| PlatformError::Database(e.to_string()))?;

            // Обороты обеих сторон пары
            Self::bump_turnover(&bal_col, session, input.company_id, &period_key, &dt_acc, line.amount, 0).await?;
            Self::bump_turnover(&bal_col, session, input.company_id, &period_key, &ct_acc, 0, line.amount).await?;
        }

        Ok(serde_json::json!({
            "posting_id": posting_id.to_string(),
            "entries_count": input.lines.len(),
            "period": period_key,
        }))
    }

    #[allow(clippy::too_many_arguments)]
    async fn bump_turnover(
        col: &mongodb::Collection<Document>,
        session: &mut mongodb::ClientSession,
        company_id: &CompanyId,
        period_key: &str,
        account: &LedgerAccount,
        add_debit: i64,
        add_credit: i64,
    ) -> PlatformResult<()> {
        let filter = doc! {
            "company_id": company_id.0.to_string(),
            "period_key": period_key,
            "account_id": account.id.to_string(),
        };
        let update = doc! {
            "$inc": {
                "debit_turnover": add_debit,
                "credit_turnover": add_credit,
            },
            "$set": {
                "account_code": &account.code,
                "account_type": account.account_type.as_str(),
                "updated_at": mongodb::bson::DateTime::now(),
            },
            "$setOnInsert": {
                "company_id": company_id.0.to_string(),
                "period_key": period_key,
                "account_id": account.id.to_string(),
            },
        };
        col.update_one(filter, update)
            .upsert(true)
            .session(session)
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(())
    }

    /// Зеркальное сторно всех записей документа: те же счета/суммы,
    /// дебет↔кредит поменян местами, is_reversal=true.
    /// Каждая исходная запись помечается reversed=true (повторный реверс невозможен).
    pub async fn reverse_by_doc_in_session(
        db: &MongoClient,
        session: &mut mongodb::ClientSession,
        company_id: &CompanyId,
        created_by: crate::core::UserId,
        target_doc_id: &str,
        new_doc_kind: Option<&str>,
        new_doc_id: Option<&str>,
        date: &str,
    ) -> PlatformResult<serde_json::Value> {
        let period_key = period_key_of_date(date);
        let _period = Self::ensure_period(session, db, company_id, &period_key).await?;

        let entries_filter = doc! {
            "company_id": company_id.0.to_string(),
            "doc_id": target_doc_id,
            "is_reversal": { "$ne": true },
            "reversed_by": { "$exists": false },
        };
        let mut cursor = db.collection::<Document>(COL_ENTRIES)
            .find(entries_filter.clone())
            .session(&mut *session)
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;

        let mut originals: Vec<LedgerEntry> = Vec::new();
        while let Some(d) = cursor.next(&mut *session).await {
            match d {
                Ok(doc) => {
                    if let Ok(e) = mongodb::bson::from_document::<LedgerEntry>(doc) {
                        originals.push(e);
                    }
                }
                Err(er) => return Err(PlatformError::Database(er.to_string())),
            }
        }

        if originals.is_empty() {
            return Err(PlatformError::NotFound(format!(
                "проводки документа {target_doc_id} не найдены (или уже сторнированы)"
            )));
        }

        let new_posting_id = uuid::Uuid::new_v4();
        let entries_col = db.collection::<Document>(COL_ENTRIES);
        let bal_col = db.collection::<Document>(COL_BALANCES);

        for orig in &originals {
            let mirror = LedgerEntry {
                id: uuid::Uuid::new_v4(),
                company_id: orig.company_id.clone(),
                period_key: period_key.clone(),
                date: date.to_string(),
                posting_id: new_posting_id.to_string(),
                doc_kind: new_doc_kind.map(String::from),
                doc_id: new_doc_id.map(String::from),
                debit_code: orig.credit_code.clone(),
                credit_code: orig.debit_code.clone(),
                amount: orig.amount,
                nomenclature_id: orig.nomenclature_id.clone(),
                description: Some(format!(
                    "Сторно: {}",
                    orig.description.as_deref().unwrap_or(orig.doc_id.as_deref().unwrap_or(""))
                )),
                is_reversal: true,
                created_by: created_by.0.to_string(),
                created_at: Utc::now(),
            };
            let mut edoc = mongodb::bson::to_document(&mirror)
                .map_err(|e| PlatformError::Internal(e.to_string()))?;
            edoc.insert("_id", mirror.id.to_string());

            entries_col.insert_one(edoc)
                .session(&mut *session).await
                .map_err(|e| PlatformError::Database(e.to_string()))?;

            // Обороты: зеркально (Дт исходника → Кт сторно и наоборот)
            let dt_acc = Self::get_active_by_code(db, company_id, &mirror.debit_code).await?;
            let ct_acc = Self::get_active_by_code(db, company_id, &mirror.credit_code).await?;
            Self::bump_turnover(&bal_col, session, company_id, &period_key, &dt_acc, orig.amount, 0).await?;
            Self::bump_turnover(&bal_col, session, company_id, &period_key, &ct_acc, 0, orig.amount).await?;

            // Пометить исходную запись
            let mark = doc! { "_id": &orig.id.to_string() };
            let setrev = doc! { "$set": { "reversed_by": mirror.id.to_string() } };
            entries_col.update_one(mark, setrev)
                .session(&mut *session).await
                .map_err(|e| PlatformError::Database(e.to_string()))?;
        }

        Ok(serde_json::json!({
            "reversal_posting_id": new_posting_id.to_string(),
            "entries_reversed": originals.len(),
        }))
    }
}

fn parse_period_key(key: &str) -> PlatformResult<(i32, u32)> {
    let mut it = key.split('-');
    let y = it.next().and_then(|s| s.parse().ok())
        .ok_or_else(|| PlatformError::Validation(format!("период {key:?}: год")))?;
    let m = it.next().and_then(|s| s.parse().ok())
        .ok_or_else(|| PlatformError::Validation(format!("период {key:?}: месяц")))?;
    Ok((y, m))
}

