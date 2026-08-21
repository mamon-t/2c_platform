use chrono::{DateTime, Utc};
use futures::StreamExt;
use mongodb::bson::{doc, Document};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::{CompanyId, Id, PlatformError, PlatformResult};
use crate::db::MongoClient;
use crate::permission_policy::{PermissionPolicy, PermissionPolicyService};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    pub _id: Id,
    pub company_id: CompanyId,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub permission_policy_ids: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateRoleInput {
    pub company_id: CompanyId,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub permission_policy_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateRoleInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub permission_policy_ids: Option<Vec<String>>,
}

pub struct RoleService;

impl RoleService {
    pub fn new() -> Self { Self }

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
            permission_policy_ids: input.permission_policy_ids.unwrap_or_default(),
            created_at: now,
            updated_at: now,
        };
        let mut doc = mongodb::bson::to_document(&role).unwrap_or_default();
        doc.insert("_id", role._id.to_string());
        doc.insert("company_id", role.company_id.0.to_string());
        let col = db.collection::<Document>("roles");
        col.insert_one(doc)
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
        if let Some(ids) = input.permission_policy_ids {
            update_doc.insert("permission_policy_ids", ids);
        }
        col.update_one(doc! { "_id": id.to_string() }, doc! { "$set": update_doc })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Self::get(db, id).await
    }

    pub async fn delete(db: &MongoClient, id: Id) -> PlatformResult<()> {
        let col = db.collection::<Document>("roles");
        let result = col
            .delete_one(doc! { "_id": id.to_string() })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        if result.deleted_count == 0 {
            return Err(PlatformError::NotFound(format!("Роль {id} не найдена")));
        }
        Ok(())
    }

    pub async fn get_policies(db: &MongoClient, role: &Role) -> PlatformResult<Vec<PermissionPolicy>> {
        let ids: Vec<Id> = role.permission_policy_ids.iter()
            .filter_map(|s| Id::parse_str(s).ok())
            .collect();
        PermissionPolicyService::get_by_ids(db, &ids).await
    }

    pub fn has_policy_for_action(policies: &[PermissionPolicy], subsystem: &str, action: &str) -> bool {
        policies.iter().any(|p|
            p.subsystem_code == subsystem &&
            p.actions.iter().any(|a| a == action || a == "*") &&
            !p.deny
        )
    }

    pub async fn seed_roles_for_company(db: &MongoClient, company_id: CompanyId) -> PlatformResult<()> {
        let existing = Self::list(db, company_id.clone()).await?;
        if !existing.is_empty() { return Ok(()); }

        let all_policies = PermissionPolicyService::list(db).await?;
        let all_policy_ids: Vec<String> = all_policies.iter().map(|p| p._id.to_string()).collect();

        let user_policy_ids: Vec<String> = all_policies.iter()
            .filter(|p| matches!(p.subsystem_code.as_str(), "platform" | "users" | "contacts" | "documents" | "catalogs" | "reports" | "print" | "numbering"))
            .map(|p| p._id.to_string())
            .collect();

        let viewer_policy_ids: Vec<String> = all_policies.iter()
            .filter(|p| p.actions.iter().any(|a| a == "read") && !p.deny)
            .map(|p| p._id.to_string())
            .collect();

        let _ = Self::create(db, CreateRoleInput {
            company_id: company_id.clone(), code: "SUPERADMIN".into(), name: "Суперадминистратор".into(),
            description: Some("Полный доступ ко всем функциям".into()),
            permission_policy_ids: Some(all_policy_ids),
        }).await;
        let _ = Self::create(db, CreateRoleInput {
            company_id: company_id.clone(), code: "ADMIN".into(), name: "Администратор".into(),
            description: Some("Доступ к управлению пользователями и данным".into()),
            permission_policy_ids: Some(user_policy_ids),
        }).await;
        let _ = Self::create(db, CreateRoleInput {
            company_id, code: "VIEWER".into(), name: "Наблюдатель".into(),
            description: Some("Только просмотр данных".into()),
            permission_policy_ids: Some(viewer_policy_ids),
        }).await;

        Ok(())
    }
}
