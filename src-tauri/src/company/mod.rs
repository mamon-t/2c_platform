// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

use chrono::{DateTime, Utc};
use futures::StreamExt;
use mongodb::bson::{doc, Document};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::audit::AuditChanges;
use crate::core::{CompanyId, Id, PlatformError, PlatformResult};
use crate::core::middleware::CommandOutcome;
use crate::db::MongoClient;
use crate::events::{ActorSnapshot, EventService, StreamType};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Company {
    pub _id: Id,
    pub code: String,
    pub name: String,
    pub inn: Option<String>,
    #[serde(default)]
    pub kpp: Option<String>,
    #[serde(default)]
    pub ogrn: Option<String>,
    #[serde(default)]
    pub okved: Option<String>,
    #[serde(default)]
    pub legal_address: Option<String>,
    #[serde(default)]
    pub postal_address: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub website: Option<String>,
    #[serde(default)]
    pub bank_name: Option<String>,
    #[serde(default)]
    pub bank_bik: Option<String>,
    #[serde(default)]
    pub bank_account: Option<String>,
    #[serde(default)]
    pub bank_correspondent_account: Option<String>,
    #[serde(default)]
    pub director_name: Option<String>,
    #[serde(default)]
    pub director_position: Option<String>,
    #[serde(default)]
    pub accountant_name: Option<String>,
    /// Налоговый режим: false = ОСН (по умолчанию), true = упрощённая система налогообложения
    #[serde(default)]
    pub tax_regime_usn: bool,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateCompanyInput {
    pub code: String,
    pub name: String,
    pub inn: Option<String>,
    #[serde(default)]
    pub kpp: Option<String>,
    #[serde(default)]
    pub ogrn: Option<String>,
    #[serde(default)]
    pub okved: Option<String>,
    #[serde(default)]
    pub legal_address: Option<String>,
    #[serde(default)]
    pub postal_address: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub website: Option<String>,
    #[serde(default)]
    pub bank_name: Option<String>,
    #[serde(default)]
    pub bank_bik: Option<String>,
    #[serde(default)]
    pub bank_account: Option<String>,
    #[serde(default)]
    pub bank_correspondent_account: Option<String>,
    #[serde(default)]
    pub director_name: Option<String>,
    #[serde(default)]
    pub director_position: Option<String>,
    #[serde(default)]
    pub accountant_name: Option<String>,
    #[serde(default)]
    pub tax_regime_usn: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateCompanyInput {
    pub name: Option<String>,
    pub inn: Option<String>,
    pub kpp: Option<String>,
    pub ogrn: Option<String>,
    pub okved: Option<String>,
    pub legal_address: Option<String>,
    pub postal_address: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub website: Option<String>,
    pub bank_name: Option<String>,
    pub bank_bik: Option<String>,
    pub bank_account: Option<String>,
    pub bank_correspondent_account: Option<String>,
    pub director_name: Option<String>,
    pub director_position: Option<String>,
    pub accountant_name: Option<String>,
    pub tax_regime_usn: Option<bool>,
    pub active: Option<bool>,
}

impl Company {
    pub fn new(input: CreateCompanyInput) -> Self {
        let now = Utc::now();
        Self {
            _id: Uuid::new_v4(),
            code: input.code,
            name: input.name,
            inn: input.inn,
            kpp: input.kpp,
            ogrn: input.ogrn,
            okved: input.okved,
            legal_address: input.legal_address,
            postal_address: input.postal_address,
            phone: input.phone,
            email: input.email,
            website: input.website,
            bank_name: input.bank_name,
            bank_bik: input.bank_bik,
            bank_account: input.bank_account,
            bank_correspondent_account: input.bank_correspondent_account,
            director_name: input.director_name,
            director_position: input.director_position,
            accountant_name: input.accountant_name,
            tax_regime_usn: input.tax_regime_usn,
            active: true,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn to_document(&self) -> Document {
        let mut doc = mongodb::bson::to_document(self).unwrap_or_default();
        doc.insert("_id", self._id.to_string());
        doc
    }
}

pub struct CompanyService;

impl CompanyService {
    pub fn new() -> Self {
        Self
    }

    pub async fn list(db: &MongoClient) -> PlatformResult<Vec<Company>> {
        let col = db.collection::<Document>("companies");
        let mut cursor = col
            .find(doc! {})
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        let mut companies = Vec::new();
        while let Some(result) = cursor.next().await {
            let doc = result.map_err(|e| PlatformError::Database(e.to_string()))?;
            let company: Company = mongodb::bson::from_document(doc)
                .map_err(|e| PlatformError::Database(e.to_string()))?;
            companies.push(company);
        }
        Ok(companies)
    }

    pub async fn get(db: &MongoClient, id: Id) -> PlatformResult<Company> {
        let col = db.collection::<Document>("companies");
        let doc = col
            .find_one(doc! { "_id": id.to_string() })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?
            .ok_or_else(|| PlatformError::NotFound(format!("Компания {id} не найдена")))?;
        let company: Company = mongodb::bson::from_document(doc)
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(company)
    }

    pub async fn create(
        db: &MongoClient,
        input: CreateCompanyInput,
        actor: ActorSnapshot,
    ) -> PlatformResult<CommandOutcome<Company>> {
        let company = Company::new(input);
        let cid = CompanyId(company._id);

        let mut session = db.client().start_session().await
            .map_err(|e| PlatformError::Database(format!("start_session: {}", e)))?;
        session.start_transaction().await
            .map_err(|e| PlatformError::Database(format!("start_transaction: {}", e)))?;

        let col = db.collection::<Document>("companies");
        let result = col.insert_one(company.to_document())
            .session(&mut session).await;

        match result {
            Ok(_) => {
                let svc = EventService::new();
                let payload = serde_json::json!({
                    "code": company.code,
                    "name": company.name,
                    "inn": company.inn,
                    "tax_regime_usn": company.tax_regime_usn,
                });
                let _ = svc.append_with_session(db, &mut session, StreamType::Object, &company._id.to_string(), "company.created", payload, actor, cid.clone(), None, None).await;

                session.commit_transaction().await
                    .map_err(|e| PlatformError::Database(format!("commit_transaction: {}", e)))?;

                let changes = AuditChanges::new()
                    .field_new("code", &company.code)
                    .field_new("name", &company.name);

                Ok(CommandOutcome { result: company, changes: Some(changes), event_id: None, signature_ref: None })
            }
            Err(e) => {
                session.abort_transaction().await.ok();
                if e.to_string().contains("duplicate key") {
                    Err(PlatformError::Validation(format!(
                        "Компания с кодом '{}' уже существует", company.code
                    )))
                } else {
                    Err(PlatformError::Database(e.to_string()))
                }
            }
        }
    }

    pub async fn update(
        db: &MongoClient,
        id: Id,
        input: UpdateCompanyInput,
        actor: ActorSnapshot,
    ) -> PlatformResult<CommandOutcome<Company>> {
        let old = Self::get(db, id).await?;
        let cid = CompanyId(id);

        let mut session = db.client().start_session().await
            .map_err(|e| PlatformError::Database(format!("start_session: {}", e)))?;
        session.start_transaction().await
            .map_err(|e| PlatformError::Database(format!("start_transaction: {}", e)))?;

        let mut update_doc = doc! { "updated_at": mongodb::bson::to_bson(&Utc::now()).unwrap() };
        macro_rules! patch { ($($field:ident),*) => { $(if let Some(ref v) = input.$field { update_doc.insert(stringify!($field), v.clone()); })* }; }
        patch!(name, inn, kpp, ogrn, okved, legal_address, postal_address,
               phone, email, website, bank_name, bank_bik, bank_account,
               bank_correspondent_account, director_name, director_position, accountant_name);
        if let Some(active) = input.active { update_doc.insert("active", active); }
        if let Some(usn) = input.tax_regime_usn { update_doc.insert("tax_regime_usn", usn); }

        let col = db.collection::<Document>("companies");
        col.update_one(doc! { "_id": id.to_string() }, doc! { "$set": update_doc })
            .session(&mut session).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;

        let updated = Self::get(db, id).await?;

        let svc = EventService::new();
        let payload = serde_json::json!({
            "name": updated.name,
            "inn": updated.inn,
            "active": updated.active,
            "tax_regime_usn": updated.tax_regime_usn,
        });
        let _ = svc.append_with_session(db, &mut session, StreamType::Object, &id.to_string(), "company.updated", payload, actor, cid, None, None).await;

        session.commit_transaction().await
            .map_err(|e| PlatformError::Database(format!("commit_transaction: {}", e)))?;

        let mut changes = AuditChanges::new();
        if old.name != updated.name {
            changes = changes.field("name", &old.name, &updated.name);
        }
        if old.inn != updated.inn {
            changes = changes.field("inn",
                old.inn.as_deref().unwrap_or(""),
                updated.inn.as_deref().unwrap_or(""));
        }

        Ok(CommandOutcome { result: updated, changes: Some(changes), event_id: None, signature_ref: None })
    }

    pub async fn delete(
        db: &MongoClient,
        id: Id,
        actor: ActorSnapshot,
    ) -> PlatformResult<CommandOutcome<()>> {
        let old = Self::get(db, id).await?;
        let cid = CompanyId(id);

        let mut session = db.client().start_session().await
            .map_err(|e| PlatformError::Database(format!("start_session: {}", e)))?;
        session.start_transaction().await
            .map_err(|e| PlatformError::Database(format!("start_transaction: {}", e)))?;

        let col = db.collection::<Document>("companies");
        let result = col.delete_one(doc! { "_id": id.to_string() })
            .session(&mut session).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;

        if result.deleted_count == 0 {
            session.abort_transaction().await.ok();
            return Err(PlatformError::NotFound(format!("Компания {id} не найдена")));
        }

        let svc = EventService::new();
        let payload = serde_json::json!({ "code": old.code, "name": old.name });
        let _ = svc.append_with_session(db, &mut session, StreamType::Object, &id.to_string(), "company.deleted", payload, actor, cid, None, None).await;

        session.commit_transaction().await
            .map_err(|e| PlatformError::Database(format!("commit_transaction: {}", e)))?;

        let changes = AuditChanges::new()
            .field_old("code", &old.code)
            .field_old("name", &old.name);

        Ok(CommandOutcome { result: (), changes: Some(changes), event_id: None, signature_ref: None })
    }
}
