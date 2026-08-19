use chrono::{DateTime, Utc};
use futures::StreamExt;
use mongodb::bson::{doc, Document};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::{Id, PlatformError, PlatformResult};
use crate::db::MongoClient;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Company {
    pub _id: Id,
    pub code: String,
    pub name: String,
    pub inn: Option<String>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateCompanyInput {
    pub code: String,
    pub name: String,
    pub inn: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCompanyInput {
    pub name: Option<String>,
    pub inn: Option<String>,
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
            active: true,
            created_at: now,
            updated_at: now,
        }
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
            let company: Company =
                mongodb::bson::from_document(doc).map_err(|e| PlatformError::Database(e.to_string()))?;
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
        let company: Company =
            mongodb::bson::from_document(doc).map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(company)
    }

    pub async fn create(db: &MongoClient, input: CreateCompanyInput) -> PlatformResult<Company> {
        let company = Company::new(input);
        let col = db.collection::<Company>("companies");
        col.insert_one(&company)
            .await
            .map_err(|e| {
                if e.to_string().contains("duplicate key") {
                    PlatformError::Validation(format!(
                        "Компания с кодом '{}' уже существует",
                        company.code
                    ))
                } else {
                    PlatformError::Database(e.to_string())
                }
            })?;
        Ok(company)
    }

    pub async fn update(
        db: &MongoClient,
        id: Id,
        input: UpdateCompanyInput,
    ) -> PlatformResult<Company> {
        let col = db.collection::<Document>("companies");
        let mut update_doc = doc! { "updated_at": mongodb::bson::to_bson(&Utc::now()).unwrap() };
        if let Some(name) = input.name {
            update_doc.insert("name", name);
        }
        if let Some(inn) = input.inn {
            update_doc.insert("inn", inn);
        }
        if let Some(active) = input.active {
            update_doc.insert("active", active);
        }
        col.update_one(doc! { "_id": id.to_string() }, doc! { "$set": update_doc })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Self::get(db, id).await
    }

    pub async fn delete(db: &MongoClient, id: Id) -> PlatformResult<()> {
        let col = db.collection::<Company>("companies");
        let result = col
            .delete_one(doc! { "_id": id.to_string() })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        if result.deleted_count == 0 {
            return Err(PlatformError::NotFound(format!("Компания {id} не найдена")));
        }
        Ok(())
    }
}
