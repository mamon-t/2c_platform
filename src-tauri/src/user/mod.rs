use chrono::{DateTime, Utc};
use futures::StreamExt;
use mongodb::bson::{doc, Document};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::AuthService;
use crate::audit::AuditChanges;
use crate::core::{CompanyId, Id, PlatformError, PlatformResult, UserId};
use crate::core::middleware::CommandOutcome;
use crate::db::MongoClient;
use crate::events::{ActorSnapshot, EventService, StreamType};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UserStatus {
    #[serde(rename = "invited")]
    Invited,
    #[serde(rename = "active")]
    Active,
    #[serde(rename = "disabled")]
    Disabled,
    #[serde(rename = "locked")]
    Locked,
    #[serde(rename = "archived")]
    Archived,
}

impl UserStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Invited => "invited",
            Self::Active => "active",
            Self::Disabled => "disabled",
            Self::Locked => "locked",
            Self::Archived => "archived",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub _id: Id,
    pub login: String,
    pub password_hash: String,
    pub person_id: Option<Id>,
    pub status: String,
    pub role_ids: Vec<String>,
    pub locale: Option<String>,
    pub timezone: Option<String>,
    pub password_changed_at: Option<DateTime<Utc>>,
    pub must_change_password: bool,
    pub failed_login_count: i32,
    pub locked_until: Option<DateTime<Utc>>,
    pub last_login_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserInput {
    pub login: String,
    pub password: String,
    pub display_name: Option<String>,
    pub last_name: Option<String>,
    pub first_name: Option<String>,
    pub middle_name: Option<String>,
    pub email: Option<String>,
    pub company_id: Option<String>,
    pub role_id: Option<String>,
    pub position: Option<String>,
    pub department: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserInput {
    pub status: Option<String>,
    pub locale: Option<String>,
    pub timezone: Option<String>,
    pub new_password: Option<String>,
    pub must_change_password: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPublic {
    pub _id: Id,
    pub login: String,
    pub person_id: Option<Id>,
    pub display_name: String,
    pub status: String,
    pub role_ids: Vec<String>,
    pub locale: Option<String>,
    pub timezone: Option<String>,
    pub last_login_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl User {
    pub fn to_document(&self) -> Document {
        let mut doc = mongodb::bson::to_document(self).unwrap_or_default();
        doc.insert("_id", self._id.to_string());
        doc.insert("role_ids", mongodb::bson::to_bson(&self.role_ids).unwrap());
        doc
    }
}

impl From<User> for UserPublic {
    fn from(u: User) -> Self {
        Self {
            _id: u._id,
            login: u.login,
            person_id: u.person_id,
            display_name: String::new(),
            status: u.status,
            role_ids: u.role_ids,
            locale: u.locale,
            timezone: u.timezone,
            last_login_at: u.last_login_at,
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

    pub async fn get_by_login(db: &MongoClient, login: &str) -> PlatformResult<User> {
        let col = db.collection::<Document>("users");
        let doc = col
            .find_one(doc! { "login": login })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?
            .ok_or_else(|| {
                PlatformError::NotFound(format!("Пользователь '{login}' не найден"))
            })?;
        let user: User =
            mongodb::bson::from_document(doc).map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(user)
    }

    pub async fn list(db: &MongoClient) -> PlatformResult<Vec<User>> {
        let col = db.collection::<Document>("users");
        let mut cursor = col
            .find(doc! {})
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        let mut users = Vec::new();
        while let Some(result) = cursor.next().await {
            let doc = result.map_err(|e| PlatformError::Database(e.to_string()))?;
            if let Ok(user) = mongodb::bson::from_document::<User>(doc) {
                users.push(user);
            }
        }
        Ok(users)
    }

    pub async fn create(
        db: &MongoClient,
        input: CreateUserInput,
        auth: &AuthService,
        actor: ActorSnapshot,
    ) -> PlatformResult<CommandOutcome<UserPublic>> {
        let password_hash = auth.hash_password(&input.password)?;
        let now = Utc::now();

        let mut session = db.client().start_session().await
            .map_err(|e| PlatformError::Database(format!("start_session: {}", e)))?;
        session.start_transaction().await
            .map_err(|e| PlatformError::Database(format!("start_transaction: {}", e)))?;

        let mut person_id: Option<Id> = None;
        let mut display_name = input.display_name.clone().unwrap_or_else(|| input.login.clone());

        let has_name = input.last_name.is_some() || input.first_name.is_some();
        if has_name {
            let person = crate::person::PersonService::create(
                db,
                crate::person::CreatePersonInput {
                    last_name: input.last_name.clone().unwrap_or_default(),
                    first_name: input.first_name.clone().unwrap_or_default(),
                    middle_name: input.middle_name.clone(),
                    display_name: input.display_name.clone(),
                },
            ).await?;
            display_name = person.display_name.clone();
            person_id = Some(person._id);
        }

        let user = User {
            _id: Uuid::new_v4(),
            login: input.login.clone(),
            password_hash,
            person_id,
            status: UserStatus::Active.as_str().to_string(),
            role_ids: Vec::new(),
            locale: None,
            timezone: None,
            password_changed_at: None,
            must_change_password: false,
            failed_login_count: 0,
            locked_until: None,
            last_login_at: None,
            created_at: now,
            updated_at: now,
        };

        let mut doc = user.to_document();
        doc.insert("login", &user.login);
        doc.insert("status", &user.status);

        let col = db.collection::<Document>("users");
        col.insert_one(doc).session(&mut session).await
            .map_err(|e| {
                if e.to_string().contains("duplicate key") {
                    PlatformError::Validation(format!("Пользователь '{}' уже существует", user.login))
                } else {
                    PlatformError::Database(e.to_string())
                }
            })?;

        if let Some(ref email) = input.email {
            let _ = crate::user_contact::UserContactService::create(
                db,
                crate::user_contact::CreateContactInput {
                    user_id: user._id.to_string(),
                    channel_type: "email".to_string(),
                    value: email.clone(),
                    is_primary: Some(true),
                    purposes: Some(vec!["login".to_string(), "notifications".to_string()]),
                    note: None,
                },
            ).await;
        }

        if let (Some(ref company_id), Some(ref role_id)) = (&input.company_id, &input.role_id) {
            let _ = crate::user_profile::UserProfileService::add(
                db,
                crate::user_profile::CreateProfileInput {
                    user_id: user._id.to_string(),
                    company_id: company_id.clone(),
                    role_id: role_id.clone(),
                    position: input.position.clone(),
                    department: input.department.clone(),
                    employee_number: None,
                    is_primary: Some(true),
                },
            ).await;
        }

        let svc = EventService::new();
        let payload = serde_json::json!({
            "login": user.login,
            "status": user.status,
        });
        let cid = actor.company_id.clone();
        let _ = svc.append_with_session(db, &mut session, StreamType::User, &user._id.to_string(), "user.created", payload, actor, cid, None, None).await;

        session.commit_transaction().await
            .map_err(|e| PlatformError::Database(format!("commit_transaction: {}", e)))?;

        let mut pub_user: UserPublic = user.into();
        pub_user.display_name = display_name;

        let changes = AuditChanges::new()
            .field_new("login", &pub_user.login);

        Ok(CommandOutcome { result: pub_user, changes: Some(changes), event_id: None, signature_ref: None })
    }

    pub async fn update(
        db: &MongoClient,
        id: Id,
        input: UpdateUserInput,
        auth: &AuthService,
        actor: ActorSnapshot,
    ) -> PlatformResult<CommandOutcome<()>> {
        let old = Self::get(db, id).await?;

        let mut session = db.client().start_session().await
            .map_err(|e| PlatformError::Database(format!("start_session: {}", e)))?;
        session.start_transaction().await
            .map_err(|e| PlatformError::Database(format!("start_transaction: {}", e)))?;

        let mut update_doc = doc! { "updated_at": mongodb::bson::to_bson(&Utc::now()).unwrap() };
        if let Some(ref status) = input.status { update_doc.insert("status", status.clone()); }
        if let Some(ref locale) = input.locale { update_doc.insert("locale", locale.clone()); }
        if let Some(ref timezone) = input.timezone { update_doc.insert("timezone", timezone.clone()); }
        if let Some(ref new_password) = input.new_password {
            let hash = auth.hash_password(new_password)?;
            update_doc.insert("password_hash", hash);
            update_doc.insert("password_changed_at", mongodb::bson::to_bson(&Utc::now()).unwrap());
        }
        if let Some(must) = input.must_change_password {
            update_doc.insert("must_change_password", must);
        }

        let col = db.collection::<Document>("users");
        col.update_one(doc! { "_id": id.to_string() }, doc! { "$set": update_doc })
            .session(&mut session).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;

        let svc = EventService::new();
        let payload = serde_json::json!({
            "status": input.status,
            "locale": input.locale,
        });
        let cid = actor.company_id.clone();
        let _ = svc.append_with_session(db, &mut session, StreamType::User, &id.to_string(), "user.updated", payload, actor, cid, None, None).await;

        session.commit_transaction().await
            .map_err(|e| PlatformError::Database(format!("commit_transaction: {}", e)))?;

        let mut changes = AuditChanges::new();
        if let Some(ref s) = input.status {
            changes = changes.field("status", &old.status, s);
        }

        Ok(CommandOutcome { result: (), changes: Some(changes), event_id: None, signature_ref: None })
    }

    pub async fn delete(
        db: &MongoClient,
        id: Id,
        actor: ActorSnapshot,
    ) -> PlatformResult<CommandOutcome<()>> {
        let old = Self::get(db, id).await?;

        let mut session = db.client().start_session().await
            .map_err(|e| PlatformError::Database(format!("start_session: {}", e)))?;
        session.start_transaction().await
            .map_err(|e| PlatformError::Database(format!("start_transaction: {}", e)))?;

        let col = db.collection::<Document>("users");
        col.update_one(
            doc! { "_id": id.to_string() },
            doc! { "$set": { "status": "archived", "updated_at": mongodb::bson::to_bson(&Utc::now()).unwrap() } },
        ).session(&mut session).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;

        let svc = EventService::new();
        let payload = serde_json::json!({ "login": old.login });
        let cid = actor.company_id.clone();
        let _ = svc.append_with_session(db, &mut session, StreamType::User, &id.to_string(), "user.deleted", payload, actor, cid, None, None).await;

        session.commit_transaction().await
            .map_err(|e| PlatformError::Database(format!("commit_transaction: {}", e)))?;

        let changes = AuditChanges::new()
            .field("status", &old.status, "archived");

        Ok(CommandOutcome { result: (), changes: Some(changes), event_id: None, signature_ref: None })
    }

    pub async fn has_users(db: &MongoClient) -> PlatformResult<bool> {
        let col = db.collection::<Document>("users");
        let count = col
            .count_documents(doc! {})
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(count > 0)
    }

    pub async fn is_last_admin(db: &MongoClient, user_id: Id) -> PlatformResult<bool> {
        let profiles_col = db.collection::<Document>("user_profiles");
        let roles_col = db.collection::<Document>("roles");
        let users_col = db.collection::<Document>("users");

        let mut cursor = profiles_col
            .find(doc! { "user_id": user_id.to_string(), "is_active": true })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;

        while let Some(doc) = cursor.next().await {
            let doc = doc.map_err(|e| PlatformError::Database(e.to_string()))?;
            let company_id = doc.get_str("company_id").unwrap_or("");
            let role_id = doc.get_str("role_id").unwrap_or("");

            let role = roles_col.find_one(doc! { "_id": role_id }).await
                .map_err(|e| PlatformError::Database(e.to_string()))?;
            if let Some(role_doc) = role {
                let code = role_doc.get_str("code").unwrap_or("");
                if code != "ADMIN" && code != "SUPERADMIN" {
                    continue;
                }
            } else {
                continue;
            }

            let active_admins = profiles_col
                .count_documents(doc! {
                    "company_id": company_id,
                    "is_active": true,
                })
                .await
                .map_err(|e| PlatformError::Database(e.to_string()))?;

            let admin_user_ids: Vec<String> = {
                let mut pc = profiles_col
                    .find(doc! { "company_id": company_id, "is_active": true })
                    .await
                    .map_err(|e| PlatformError::Database(e.to_string()))?;
                let mut ids = Vec::new();
                while let Some(d) = pc.next().await {
                    if let Ok(d) = d {
                        if let Ok(uid) = d.get_str("user_id") {
                            ids.push(uid.to_string());
                        }
                    }
                }
                ids
            };

            let mut active_admin_count = 0i64;
            for uid in &admin_user_ids {
                let user_doc = users_col.find_one(doc! { "_id": uid.as_str(), "status": "active" }).await
                    .map_err(|e| PlatformError::Database(e.to_string()))?;
                if user_doc.is_some() {
                    let user_roles_col = profiles_col.clone();
                    let profile = user_roles_col.find_one(doc! { "user_id": uid.as_str(), "company_id": company_id, "is_active": true }).await
                        .map_err(|e| PlatformError::Database(e.to_string()))?;
                    if let Some(profile_doc) = profile {
                        if let Ok(rid) = profile_doc.get_str("role_id") {
                            let r = roles_col.find_one(doc! { "_id": rid }).await
                                .map_err(|e| PlatformError::Database(e.to_string()))?;
                            if let Some(rd) = r {
                                if let Ok(code) = rd.get_str("code") {
                                    if code == "ADMIN" || code == "SUPERADMIN" {
                                        active_admin_count += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if active_admin_count <= 1 {
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub async fn authenticate(
        db: &MongoClient,
        login: &str,
        password: &str,
        auth: &AuthService,
    ) -> PlatformResult<UserPublic> {
        let user = Self::get_by_login(db, login).await?;
        if user.status != "active" {
            return Err(PlatformError::Auth(
                "Пользователь не активен".to_string(),
            ));
        }
        let verified = auth.verify_password(password, &user.password_hash)?;
        if !verified {
            return Err(PlatformError::Auth("Неверный пароль".to_string()));
        }

        let col = db.collection::<Document>("users");
        col.update_one(
            doc! { "_id": user._id.to_string() },
            doc! { "$set": { "last_login_at": mongodb::bson::to_bson(&Utc::now()).unwrap(), "failed_login_count": 0 } },
        )
        .await
        .ok();

        let mut pub_user: UserPublic = user.clone().into();

        if let Some(person_id) = user.person_id {
            if let Ok(person) = crate::person::PersonService::get(db, person_id).await {
                pub_user.display_name = person.display_name;
            }
        }
        if pub_user.display_name.is_empty() {
            pub_user.display_name = user.login;
        }

        Ok(pub_user)
    }

    pub async fn resolve_display_name(db: &MongoClient, user: &User) -> String {
        if let Some(person_id) = user.person_id {
            if let Ok(person) = crate::person::PersonService::get(db, person_id).await {
                if !person.display_name.is_empty() {
                    return person.display_name;
                }
                return format!("{} {} {}", person.last_name, person.first_name, person.middle_name.as_deref().unwrap_or("")).trim().to_string();
            }
        }
        user.login.clone()
    }
}
