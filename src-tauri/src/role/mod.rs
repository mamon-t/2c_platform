use chrono::{DateTime, Utc};
use futures::StreamExt;
use mongodb::bson::{doc, Document};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::{CompanyId, Id, PlatformError, PlatformResult};
use crate::db::MongoClient;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    pub _id: Id,
    pub company_id: CompanyId,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateRoleInput {
    pub company_id: CompanyId,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRoleInput {
    pub name: Option<String>,
    pub description: Option<String>,
}

pub struct RoleService;

impl RoleService {
    pub fn new() -> Self {
        Self
    }

    pub async fn list(db: &MongoClient, company_id: CompanyId) -> PlatformResult<Vec<Role>> {
        let col = db.collection::<Document>("roles");
        let mut cursor = col
            .find(doc! { "company_id": company_id.0.to_string() })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        let mut roles = Vec::new();
        while let Some(result) = cursor.next().await {
            let doc = result.map_err(|e| PlatformError::Database(e.to_string()))?;
            let role: Role =
                mongodb::bson::from_document(doc).map_err(|e| PlatformError::Database(e.to_string()))?;
            roles.push(role);
        }
        Ok(roles)
    }

    pub async fn get(db: &MongoClient, id: Id) -> PlatformResult<Role> {
        let col = db.collection::<Document>("roles");
        let doc = col
            .find_one(doc! { "_id": id.to_string() })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?
            .ok_or_else(|| PlatformError::NotFound(format!("Роль {id} не найдена")))?;
        let role: Role =
            mongodb::bson::from_document(doc).map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(role)
    }

    pub async fn create(db: &MongoClient, input: CreateRoleInput) -> PlatformResult<Role> {
        let now = Utc::now();
        let role = Role {
            _id: Uuid::new_v4(),
            company_id: input.company_id,
            code: input.code,
            name: input.name,
            description: input.description,
            created_at: now,
            updated_at: now,
        };
        let col = db.collection::<Role>("roles");
        col.insert_one(&role)
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(role)
    }

    pub async fn update(db: &MongoClient, id: Id, input: UpdateRoleInput) -> PlatformResult<Role> {
        let col = db.collection::<Document>("roles");
        let mut update_doc = doc! { "updated_at": mongodb::bson::to_bson(&Utc::now()).unwrap() };
        if let Some(name) = input.name {
            update_doc.insert("name", name);
        }
        if let Some(description) = input.description {
            update_doc.insert("description", description);
        }
        col.update_one(doc! { "_id": id.to_string() }, doc! { "$set": update_doc })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Self::get(db, id).await
    }

    pub async fn delete(db: &MongoClient, id: Id) -> PlatformResult<()> {
        let col = db.collection::<Role>("roles");
        let result = col
            .delete_one(doc! { "_id": id.to_string() })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        if result.deleted_count == 0 {
            return Err(PlatformError::NotFound(format!("Роль {id} не найдена")));
        }
        Ok(())
    }
}
