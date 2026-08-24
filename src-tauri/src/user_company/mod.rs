// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

use futures::StreamExt;
use mongodb::bson::{doc, Document};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::{CompanyId, Id, PlatformError, PlatformResult, RoleId, UserId};
use crate::db::MongoClient;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserCompany {
    pub _id: Id,
    pub user_id: UserId,
    pub company_id: CompanyId,
    pub role_id: RoleId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserCompanyWithDetails {
    pub company_id: String,
    pub company_name: String,
    pub company_code: String,
    pub role_id: String,
    pub role_name: String,
}

pub struct UserCompanyService;

impl UserCompanyService {
    pub fn new() -> Self {
        Self
    }

    pub async fn list_by_user(
        db: &MongoClient,
        user_id: UserId,
    ) -> PlatformResult<Vec<UserCompany>> {
        let col = db.collection::<Document>("user_companies");
        let mut cursor = col
            .find(doc! { "user_id": user_id.0.to_string() })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        let mut result = Vec::new();
        while let Some(res) = cursor.next().await {
            let doc = res.map_err(|e| PlatformError::Database(e.to_string()))?;
            let uc: UserCompany =
                mongodb::bson::from_document(doc).map_err(|e| PlatformError::Database(e.to_string()))?;
            result.push(uc);
        }
        Ok(result)
    }

    pub async fn get_role_for_company(
        db: &MongoClient,
        user_id: UserId,
        company_id: CompanyId,
    ) -> PlatformResult<RoleId> {
        let col = db.collection::<Document>("user_companies");
        let doc = col
            .find_one(doc! {
                "user_id": user_id.0.to_string(),
                "company_id": company_id.0.to_string()
            })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?
            .ok_or_else(|| {
                PlatformError::NotFound(format!(
                    "Привязка пользователя к компании не найдена"
                ))
            })?;
        let uc: UserCompany =
            mongodb::bson::from_document(doc).map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(uc.role_id)
    }

    pub async fn add(
        db: &MongoClient,
        user_id: UserId,
        company_id: CompanyId,
        role_id: RoleId,
    ) -> PlatformResult<()> {
        let col = db.collection::<Document>("user_companies");
        let mut doc = Document::new();
        doc.insert("_id", Uuid::new_v4().to_string());
        doc.insert("user_id", user_id.0.to_string());
        doc.insert("company_id", company_id.0.to_string());
        doc.insert("role_id", role_id.0.to_string());
        col.insert_one(doc)
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(())
    }

    pub async fn remove(
        db: &MongoClient,
        user_id: UserId,
        company_id: CompanyId,
    ) -> PlatformResult<()> {
        let col = db.collection::<Document>("user_companies");
        col.delete_one(doc! {
            "user_id": user_id.0.to_string(),
            "company_id": company_id.0.to_string()
        })
        .await
        .map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(())
    }

    pub async fn list_companies_with_details(
        db: &MongoClient,
        user_id: UserId,
    ) -> PlatformResult<Vec<UserCompanyWithDetails>> {
        let links = Self::list_by_user(db, user_id).await?;
        let mut result = Vec::new();

        for link in links {
            let company =
                crate::company::CompanyService::get(db, link.company_id.0).await?;
            let role = crate::role::RoleService::get(db, link.role_id.0).await?;
            result.push(UserCompanyWithDetails {
                company_id: link.company_id.0.to_string(),
                company_name: company.name,
                company_code: company.code,
                role_id: link.role_id.0.to_string(),
                role_name: role.name,
            });
        }

        Ok(result)
    }
}
