use chrono::{DateTime, Utc};
use futures::StreamExt;
use mongodb::bson::{doc, Document};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::{Id, PlatformError, PlatformResult, UserId};
use crate::db::MongoClient;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserContact {
    pub _id: Id,
    pub user_id: UserId,
    pub channel_type: String,
    pub value: String,
    pub is_primary: bool,
    pub is_verified: bool,
    pub purposes: Vec<String>,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateContactInput {
    pub user_id: String,
    pub channel_type: String,
    pub value: String,
    pub is_primary: Option<bool>,
    pub purposes: Option<Vec<String>>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateContactInput {
    pub value: Option<String>,
    pub is_primary: Option<bool>,
    pub is_verified: Option<bool>,
    pub purposes: Option<Vec<String>>,
    pub note: Option<String>,
}

fn normalize_phone(raw: &str) -> String {
    let stripped: String = raw.chars().filter(|c| !c.is_whitespace() && *c != '(' && *c != ')' && *c != '-' && *c != '.').collect();
    let mut digits_plus: String = stripped.chars().filter(|c| c.is_ascii_digit() || *c == '+').collect();
    if digits_plus.starts_with('8') && digits_plus.len() >= 11 {
        digits_plus = format!("+7{}", &digits_plus[1..]);
    } else if !digits_plus.starts_with('+') && !digits_plus.is_empty() {
        digits_plus = format!("+{digits_plus}");
    }
    digits_plus
}

fn normalize_value(channel_type: &str, value: &str) -> String {
    if channel_type == "phone" {
        normalize_phone(value)
    } else {
        value.trim().to_string()
    }
}

pub struct UserContactService;

impl UserContactService {
    pub fn new() -> Self {
        Self
    }

    pub async fn list_by_user(db: &MongoClient, user_id: UserId) -> PlatformResult<Vec<UserContact>> {
        let col = db.collection::<Document>("user_contacts");
        let mut cursor = col
            .find(doc! { "user_id": user_id.0.to_string() })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        let mut result = Vec::new();
        while let Some(doc) = cursor.next().await {
            let doc = doc.map_err(|e| PlatformError::Database(e.to_string()))?;
            if let Ok(c) = mongodb::bson::from_document::<UserContact>(doc) {
                result.push(c);
            }
        }
        result.sort_by(|a, b| b.is_primary.cmp(&a.is_primary).then(a.created_at.cmp(&b.created_at)));
        Ok(result)
    }

    pub async fn get(db: &MongoClient, id: Id) -> PlatformResult<UserContact> {
        let col = db.collection::<Document>("user_contacts");
        let doc = col
            .find_one(doc! { "_id": id.to_string() })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?
            .ok_or_else(|| PlatformError::NotFound(format!("Контакт {id} не найден")))?;
        let contact: UserContact =
            mongodb::bson::from_document(doc).map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(contact)
    }

    async fn clear_primary(db: &MongoClient, user_id: UserId, channel_type: &str) -> PlatformResult<()> {
        let col = db.collection::<Document>("user_contacts");
        col.update_many(
            doc! { "user_id": user_id.0.to_string(), "channel_type": channel_type, "is_primary": true },
            doc! { "$set": { "is_primary": false, "updated_at": mongodb::bson::to_bson(&Utc::now()).unwrap() } },
        )
        .await
        .map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(())
    }

    pub async fn create(db: &MongoClient, input: CreateContactInput) -> PlatformResult<UserContact> {
        let user_id = uuid::Uuid::parse_str(&input.user_id)
            .map_err(|e| PlatformError::Validation(e.to_string()))?;
        let is_primary = input.is_primary.unwrap_or(false);
        if is_primary {
            Self::clear_primary(db, UserId(user_id), &input.channel_type).await?;
        }
        let now = Utc::now();
        let contact = UserContact {
            _id: Uuid::new_v4(),
            user_id: UserId(user_id),
            channel_type: input.channel_type.clone(),
            value: normalize_value(&input.channel_type, &input.value),
            is_primary,
            is_verified: false,
            purposes: input.purposes.unwrap_or_default(),
            note: input.note,
            created_at: now,
            updated_at: now,
        };
        let mut doc = Document::new();
        doc.insert("_id", contact._id.to_string());
        doc.insert("user_id", contact.user_id.0.to_string());
        doc.insert("channel_type", &contact.channel_type);
        doc.insert("value", &contact.value);
        doc.insert("is_primary", contact.is_primary);
        doc.insert("is_verified", contact.is_verified);
        doc.insert("purposes", mongodb::bson::to_bson(&contact.purposes).unwrap());
        if let Some(ref note) = contact.note {
            doc.insert("note", note.clone());
        }
        doc.insert("created_at", mongodb::bson::to_bson(&contact.created_at).unwrap());
        doc.insert("updated_at", mongodb::bson::to_bson(&contact.updated_at).unwrap());

        let col = db.collection::<Document>("user_contacts");
        col.insert_one(doc)
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(contact)
    }

    pub async fn update(
        db: &MongoClient,
        id: Id,
        input: UpdateContactInput,
    ) -> PlatformResult<UserContact> {
        let existing = Self::get(db, id).await?;
        let col = db.collection::<Document>("user_contacts");
        let mut update_doc = doc! { "updated_at": mongodb::bson::to_bson(&Utc::now()).unwrap() };
        if let Some(value) = input.value {
            update_doc.insert("value", normalize_value(&existing.channel_type, &value));
        }
        if let Some(is_primary) = input.is_primary {
            if is_primary && !existing.is_primary {
                Self::clear_primary(db, existing.user_id, &existing.channel_type).await?;
            }
            update_doc.insert("is_primary", is_primary);
        }
        if let Some(is_verified) = input.is_verified {
            update_doc.insert("is_verified", is_verified);
        }
        if let Some(purposes) = input.purposes {
            update_doc.insert("purposes", mongodb::bson::to_bson(&purposes).unwrap());
        }
        if let Some(note) = input.note {
            update_doc.insert("note", note);
        }
        col.update_one(doc! { "_id": id.to_string() }, doc! { "$set": update_doc })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Self::get(db, id).await
    }

    pub async fn delete(db: &MongoClient, id: Id) -> PlatformResult<()> {
        let col = db.collection::<Document>("user_contacts");
        let result = col
            .delete_one(doc! { "_id": id.to_string() })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        if result.deleted_count == 0 {
            return Err(PlatformError::NotFound(format!("Контакт {id} не найден")));
        }
        Ok(())
    }
}
