//! Политики криптоподписи (настраиваемые, default OFF).
//!
//! Политика: {company_id, module, action, condition?, required}.
//! condition v0.1: {"nomenclature_category": "<категория>"} — применима,
//! если хотя бы одна строка документа ссылается на номенклатуру этой
//! категории. Политика без условия применима всегда.
//!
//! Оценка: required = true, если ХОТЯ БЫ одна применимая политика требует.

use futures::StreamExt;
use mongodb::bson::{doc, Document};

use crate::core::{CompanyId, PlatformError, PlatformResult};
use crate::db::MongoClient;

pub const COLLECTION: &str = "signature_policies";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SignaturePolicy {
    #[serde(rename = "_id")]
    pub id: String,
    pub company_id: String,
    pub module: String,
    /// Действие в нотации модуля: "handover.post", "move.post"…
    pub action: String,
    pub name: String,
    /// {"nomenclature_category": "..."} | {} (всегда)
    #[serde(default)]
    pub condition: serde_json::Value,
    pub required: bool,
}

pub struct SignatureService;

impl SignatureService {
    pub async fn list(
        db: &MongoClient,
        company_id: &CompanyId,
        module: Option<&str>,
    ) -> PlatformResult<Vec<SignaturePolicy>> {
        let mut filter = doc! { "company_id": company_id.0.to_string() };
        if let Some(m) = module { filter.insert("module", m); }
        let mut cursor = db.collection::<Document>(COLLECTION)
            .find(filter).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        let mut out = Vec::new();
        while let Some(Ok(d)) = cursor.next().await {
            if let Ok(p) = mongodb::bson::from_document::<SignaturePolicy>(d) {
                out.push(p);
            }
        }
        Ok(out)
    }

    /// Создать/обновить политику по (module, action).
    pub async fn upsert(
        db: &MongoClient,
        company_id: &CompanyId,
        policy: SignaturePolicy,
    ) -> PlatformResult<()> {
        let mut d = mongodb::bson::to_document(&policy)
            .map_err(|e| PlatformError::Internal(e.to_string()))?;
        d.insert("_id", policy.id.clone());
        db.collection::<Document>(COLLECTION)
            .replace_one(
                doc! {
                    "company_id": company_id.0.to_string(),
                    "module": &policy.module,
                    "action": &policy.action,
                },
                d,
            )
            .upsert(true)
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(())
    }

    pub async fn delete(
        db: &MongoClient,
        company_id: &CompanyId,
        module: &str,
        action: &str,
    ) -> PlatformResult<u64> {
        db.collection::<Document>(COLLECTION)
            .delete_one(doc! {
                "company_id": company_id.0.to_string(),
                "module": module,
                "action": action,
            })
            .await
            .map(|r| r.deleted_count)
            .map_err(|e| PlatformError::Database(e.to_string()))
    }

    /// Оценить: требуется ли подпись для действия над документом.
    ///
    /// * `doc` — данные объекта-документа (data JSON);
    /// * резолвер номенклатуры нужен для условия по категории строк.
    pub async fn evaluate(
        db: &MongoClient,
        company_id: &CompanyId,
        module: &str,
        action: &str,
        doc_data: &serde_json::Value,
    ) -> PlatformResult<bool> {
        let all = Self::list(db, company_id, Some(module)).await?;
        let applicable: Vec<&SignaturePolicy> = all.iter()
            .filter(|p| p.action == action)
            .collect();

        for p in applicable {
            if Self::condition_matches(db, company_id, &p.condition, doc_data).await? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    async fn condition_matches(
        db: &MongoClient,
        company_id: &CompanyId,
        condition: &serde_json::Value,
        doc_data: &serde_json::Value,
    ) -> PlatformResult<bool> {
        // Пустое условие — применимо всегда
        let Some(category) = condition.get("nomenclature_category").and_then(|v| v.as_str()) else {
            return Ok(true);
        };

        // Категорийное условие: хотя бы одна строка → номенклатура категории
        let lines = doc_data.get("lines").and_then(|v| v.as_array());
        let Some(lines) = lines else { return Ok(false) };

        for line in lines {
            let Some(nom_id) = line.get("nomenclature_id").and_then(|v| v.as_str()) else {
                continue;
            };
            let nom = db.collection::<Document>("objects")
                .find_one(doc! { "_id": nom_id })
                .await
                .map_err(|e| PlatformError::Database(e.to_string()))?;
            if let Some(n) = nom {
                let data = n.get("data").cloned()
                    .and_then(|b| mongodb::bson::from_bson::<serde_json::Value>(b).ok())
                    .unwrap_or_default();
                if data["category"] == serde_json::json!(category) {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}
