// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

use chrono::{DateTime, NaiveDate, Utc};
use futures::StreamExt;
use mongodb::bson::{doc, Document};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::{CompanyId, Id, PlatformError, PlatformResult, RoleId, UserId};
use crate::db::MongoClient;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub _id: Id,
    pub user_id: UserId,
    pub company_id: CompanyId,
    pub role_id: RoleId,
    pub position: Option<String>,
    pub department: Option<String>,
    pub employee_number: Option<String>,
    pub is_primary: bool,
    pub is_active: bool,
    pub valid_from: Option<NaiveDate>,
    pub valid_to: Option<NaiveDate>,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateProfileInput {
    pub user_id: String,
    pub company_id: String,
    pub role_id: String,
    pub position: Option<String>,
    pub department: Option<String>,
    pub employee_number: Option<String>,
    pub is_primary: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateProfileInput {
    pub role_id: Option<String>,
    pub position: Option<String>,
    pub department: Option<String>,
    pub employee_number: Option<String>,
    pub is_primary: Option<bool>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserProfileWithDetails {
    pub _id: String,
    pub company_id: String,
    pub company_name: String,
    pub company_code: String,
    pub role_id: String,
    pub role_name: String,
    pub position: Option<String>,
    pub department: Option<String>,
    pub employee_number: Option<String>,
    pub is_primary: bool,
    pub is_active: bool,
}

pub struct UserProfileService;

impl UserProfileService {
    pub fn new() -> Self {
        Self
    }

    pub async fn list_by_user(
        db: &MongoClient,
        user_id: UserId,
    ) -> PlatformResult<Vec<UserProfile>> {
        let col = db.collection::<Document>("user_company_profiles");
        let mut cursor = col
            .find(doc! { "user_id": user_id.0.to_string() })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        let mut result = Vec::new();
        while let Some(res) = cursor.next().await {
            let doc = res.map_err(|e| PlatformError::Database(e.to_string()))?;
            if let Ok(p) = mongodb::bson::from_document::<UserProfile>(doc) {
                result.push(p);
            }
        }
        Ok(result)
    }

    pub async fn get(
        db: &MongoClient,
        id: Id,
    ) -> PlatformResult<UserProfile> {
        let col = db.collection::<Document>("user_company_profiles");
        let doc = col
            .find_one(doc! { "_id": id.to_string() })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?
            .ok_or_else(|| PlatformError::NotFound(format!("Профиль {id} не найден")))?;
        mongodb::bson::from_document(doc).map_err(|e| PlatformError::Database(e.to_string()))
    }

    pub async fn get_role_for_company(
        db: &MongoClient,
        user_id: UserId,
        company_id: CompanyId,
    ) -> PlatformResult<RoleId> {
        let col = db.collection::<Document>("user_company_profiles");
        let doc = col
            .find_one(doc! {
                "user_id": user_id.0.to_string(),
                "company_id": company_id.0.to_string(),
                "is_active": true
            })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?
            .ok_or_else(|| {
                PlatformError::NotFound("Привязка пользователя к компании не найдена".to_string())
            })?;
        let profile: UserProfile =
            mongodb::bson::from_document(doc).map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(profile.role_id)
    }

    pub async fn add(
        db: &MongoClient,
        input: CreateProfileInput,
    ) -> PlatformResult<UserProfile> {
        let user_id = uuid::Uuid::parse_str(&input.user_id)
            .map_err(|e| PlatformError::Validation(e.to_string()))?;
        let company_id = uuid::Uuid::parse_str(&input.company_id)
            .map_err(|e| PlatformError::Validation(e.to_string()))?;
        let role_id = uuid::Uuid::parse_str(&input.role_id)
            .map_err(|e| PlatformError::Validation(e.to_string()))?;

        let now = Utc::now();
        let profile = UserProfile {
            _id: Uuid::new_v4(),
            user_id: UserId(user_id),
            company_id: CompanyId(company_id),
            role_id: RoleId(role_id),
            position: input.position,
            department: input.department,
            employee_number: input.employee_number,
            is_primary: input.is_primary.unwrap_or(false),
            is_active: true,
            valid_from: None,
            valid_to: None,
            note: None,
            created_at: now,
            updated_at: now,
        };

        let mut doc = Document::new();
        doc.insert("_id", profile._id.to_string());
        doc.insert("user_id", profile.user_id.0.to_string());
        doc.insert("company_id", profile.company_id.0.to_string());
        doc.insert("role_id", profile.role_id.0.to_string());
        if let Some(ref p) = profile.position { doc.insert("position", p.clone()); }
        if let Some(ref d) = profile.department { doc.insert("department", d.clone()); }
        if let Some(ref e) = profile.employee_number { doc.insert("employee_number", e.clone()); }
        doc.insert("is_primary", profile.is_primary);
        doc.insert("is_active", profile.is_active);
        doc.insert("created_at", mongodb::bson::to_bson(&profile.created_at).unwrap());
        doc.insert("updated_at", mongodb::bson::to_bson(&profile.updated_at).unwrap());

        let col = db.collection::<Document>("user_company_profiles");
        col.insert_one(doc)
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(profile)
    }

    pub async fn update(
        db: &MongoClient,
        id: Id,
        input: UpdateProfileInput,
    ) -> PlatformResult<()> {
        let col = db.collection::<Document>("user_company_profiles");
        let mut update_doc = doc! { "updated_at": mongodb::bson::to_bson(&Utc::now()).unwrap() };
        if let Some(role_id) = input.role_id {
            update_doc.insert("role_id", role_id);
        }
        if let Some(position) = input.position {
            update_doc.insert("position", position);
        }
        if let Some(department) = input.department {
            update_doc.insert("department", department);
        }
        if let Some(employee_number) = input.employee_number {
            update_doc.insert("employee_number", employee_number);
        }
        if let Some(is_primary) = input.is_primary {
            update_doc.insert("is_primary", is_primary);
        }
        if let Some(is_active) = input.is_active {
            update_doc.insert("is_active", is_active);
        }
        col.update_one(doc! { "_id": id.to_string() }, doc! { "$set": update_doc })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(())
    }

    pub async fn remove(db: &MongoClient, id: Id) -> PlatformResult<()> {
        let col = db.collection::<Document>("user_company_profiles");
        col.delete_one(doc! { "_id": id.to_string() })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(())
    }

    pub async fn list_with_details(
        db: &MongoClient,
        user_id: UserId,
    ) -> PlatformResult<Vec<UserProfileWithDetails>> {
        let profiles = Self::list_by_user(db, user_id).await?;
        let mut result = Vec::new();
        for p in profiles {
            let company_name = crate::company::CompanyService::get(db, p.company_id.0)
                .await
                .map(|c| c.name)
                .unwrap_or_default();
            let company_code = crate::company::CompanyService::get(db, p.company_id.0)
                .await
                .map(|c| c.code)
                .unwrap_or_default();
            let role_name = crate::role::RoleService::get(db, p.role_id.0)
                .await
                .map(|r| r.name)
                .unwrap_or_default();
            result.push(UserProfileWithDetails {
                _id: p._id.to_string(),
                company_id: p.company_id.0.to_string(),
                company_name,
                company_code,
                role_id: p.role_id.0.to_string(),
                role_name,
                position: p.position,
                department: p.department,
                employee_number: p.employee_number,
                is_primary: p.is_primary,
                is_active: p.is_active,
            });
        }
        Ok(result)
    }
}
