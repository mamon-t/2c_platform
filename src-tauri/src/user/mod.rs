use chrono::{DateTime, Utc};
use futures::StreamExt;
use mongodb::bson::{doc, Document};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::AuthService;
use crate::core::{CompanyId, Id, PlatformError, PlatformResult, RoleId};
use crate::db::MongoClient;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub _id: Id,
    pub company_id: CompanyId,
    pub username: String,
    pub display_name: String,
    pub email: Option<String>,
    pub password_hash: String,
    pub role_id: RoleId,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserInput {
    pub company_id: CompanyId,
    pub username: String,
    pub display_name: String,
    pub email: Option<String>,
    pub password: String,
    pub role_id: RoleId,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserInput {
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub active: Option<bool>,
    pub role_id: Option<RoleId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPublic {
    pub _id: Id,
    pub company_id: CompanyId,
    pub username: String,
    pub display_name: String,
    pub email: Option<String>,
    pub role_id: RoleId,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<User> for UserPublic {
    fn from(u: User) -> Self {
        Self {
            _id: u._id,
            company_id: u.company_id,
            username: u.username,
            display_name: u.display_name,
            email: u.email,
            role_id: u.role_id,
            active: u.active,
            created_at: u.created_at,
            updated_at: u.updated_at,
        }
    }
}

pub struct UserService;

impl UserService {
    pub fn new() -> Self {
        Self
    }

    pub async fn list(db: &MongoClient, company_id: CompanyId) -> PlatformResult<Vec<UserPublic>> {
        let col = db.collection::<Document>("users");
        let mut cursor = col
            .find(doc! { "company_id": company_id.0.to_string() })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        let mut users = Vec::new();
        while let Some(result) = cursor.next().await {
            let doc = result.map_err(|e| PlatformError::Database(e.to_string()))?;
            if let Ok(user) = mongodb::bson::from_document::<User>(doc) {
                users.push(user.into());
            }
        }
        Ok(users)
    }

    pub async fn get(db: &MongoClient, id: Id) -> PlatformResult<User> {
        let col = db.collection::<Document>("users");
        let doc = col
            .find_one(doc! { "_id": id.to_string() })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?
            .ok_or_else(|| PlatformError::NotFound(format!("Пользователь {id} не найден")))?;
        let user: User =
            mongodb::bson::from_document(doc).map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(user)
    }

    pub async fn get_by_username(db: &MongoClient, username: &str) -> PlatformResult<User> {
        let col = db.collection::<Document>("users");
        let doc = col
            .find_one(doc! { "username": username })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?
            .ok_or_else(|| {
                PlatformError::NotFound(format!("Пользователь '{username}' не найден"))
            })?;
        let user: User =
            mongodb::bson::from_document(doc).map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(user)
    }

    pub async fn create(
        db: &MongoClient,
        input: CreateUserInput,
        auth: &AuthService,
    ) -> PlatformResult<UserPublic> {
        let password_hash = auth.hash_password(&input.password)?;
        let now = Utc::now();
        let user = User {
            _id: Uuid::new_v4(),
            company_id: input.company_id,
            username: input.username,
            display_name: input.display_name,
            email: input.email,
            password_hash,
            role_id: input.role_id,
            active: true,
            created_at: now,
            updated_at: now,
        };
        let col = db.collection::<User>("users");
        col.insert_one(&user).await.map_err(|e| {
            if e.to_string().contains("duplicate key") {
                PlatformError::Validation(format!(
                    "Пользователь '{}' уже существует",
                    user.username
                ))
            } else {
                PlatformError::Database(e.to_string())
            }
        })?;
        Ok(user.into())
    }

    pub async fn update(
        db: &MongoClient,
        id: Id,
        input: UpdateUserInput,
    ) -> PlatformResult<UserPublic> {
        let col = db.collection::<Document>("users");
        let mut update_doc = doc! { "updated_at": mongodb::bson::to_bson(&Utc::now()).unwrap() };
        if let Some(display_name) = input.display_name {
            update_doc.insert("display_name", display_name);
        }
        if let Some(email) = input.email {
            update_doc.insert("email", email);
        }
        if let Some(active) = input.active {
            update_doc.insert("active", active);
        }
        if let Some(role_id) = input.role_id {
            update_doc.insert("role_id", role_id.0.to_string());
        }
        col.update_one(doc! { "_id": id.to_string() }, doc! { "$set": update_doc })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        let user = Self::get(db, id).await?;
        Ok(user.into())
    }

    pub async fn delete(db: &MongoClient, id: Id) -> PlatformResult<()> {
        let col = db.collection::<User>("users");
        let result = col
            .delete_one(doc! { "_id": id.to_string() })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        if result.deleted_count == 0 {
            return Err(PlatformError::NotFound(format!(
                "Пользователь {id} не найден"
            )));
        }
        Ok(())
    }

    pub async fn has_users(db: &MongoClient) -> PlatformResult<bool> {
        let col = db.collection::<Document>("users");
        let count = col
            .count_documents(doc! {})
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(count > 0)
    }

    pub async fn authenticate(
        db: &MongoClient,
        username: &str,
        password: &str,
        auth: &AuthService,
    ) -> PlatformResult<UserPublic> {
        let user = Self::get_by_username(db, username).await?;
        if !user.active {
            return Err(PlatformError::Auth(
                "Пользователь деактивирован".to_string(),
            ));
        }
        let full_user = Self::get(db, user._id).await?;
        let verified = auth.verify_password(password, &full_user.password_hash)?;
        if !verified {
            return Err(PlatformError::Auth("Неверный пароль".to_string()));
        }
        Ok(full_user.into())
    }
}
