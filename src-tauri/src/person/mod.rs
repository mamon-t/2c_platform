// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

use chrono::{DateTime, Utc};
use futures::StreamExt;
use mongodb::bson::{doc, Document};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::{Id, PlatformError, PlatformResult};
use crate::db::MongoClient;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Person {
    pub _id: Id,
    pub last_name: String,
    pub first_name: String,
    pub middle_name: Option<String>,
    pub display_name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePersonInput {
    pub last_name: String,
    pub first_name: String,
    pub middle_name: Option<String>,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdatePersonInput {
    pub last_name: Option<String>,
    pub first_name: Option<String>,
    pub middle_name: Option<String>,
    pub display_name: Option<String>,
}

impl Person {
    pub fn new(input: CreatePersonInput) -> Self {
        let now = Utc::now();
        let display_name = input.display_name.unwrap_or_else(|| {
            let mut parts = vec![input.last_name.as_str(), input.first_name.as_str()];
            if let Some(ref m) = input.middle_name {
                parts.push(m.as_str());
            }
            parts.join(" ")
        });
        Self {
            _id: Uuid::new_v4(),
            last_name: input.last_name,
            first_name: input.first_name,
            middle_name: input.middle_name,
            display_name,
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

pub struct PersonService;

impl PersonService {
    pub fn new() -> Self {
        Self
    }

    pub async fn get(db: &MongoClient, id: Id) -> PlatformResult<Person> {
        let col = db.collection::<Document>("persons");
        let doc = col
            .find_one(doc! { "_id": id.to_string() })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?
            .ok_or_else(|| PlatformError::NotFound(format!("Персона {id} не найдена")))?;
        let person: Person =
            mongodb::bson::from_document(doc).map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(person)
    }

    pub async fn create(db: &MongoClient, input: CreatePersonInput) -> PlatformResult<Person> {
        let person = Person::new(input);
        let col = db.collection::<Document>("persons");
        col.insert_one(person.to_document())
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(person)
    }

    pub async fn update(
        db: &MongoClient,
        id: Id,
        input: UpdatePersonInput,
    ) -> PlatformResult<Person> {
        let col = db.collection::<Document>("persons");
        let mut update_doc = doc! { "updated_at": mongodb::bson::to_bson(&Utc::now()).unwrap() };
        if let Some(last_name) = input.last_name {
            update_doc.insert("last_name", last_name);
        }
        if let Some(first_name) = input.first_name {
            update_doc.insert("first_name", first_name);
        }
        if let Some(middle_name) = input.middle_name {
            update_doc.insert("middle_name", middle_name);
        }
        if let Some(display_name) = input.display_name {
            update_doc.insert("display_name", display_name);
        }
        col.update_one(doc! { "_id": id.to_string() }, doc! { "$set": update_doc })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Self::get(db, id).await
    }
}
