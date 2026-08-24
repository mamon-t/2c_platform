// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

use chrono::{DateTime, Utc};
use futures::StreamExt;
use mongodb::bson::{doc, Document};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::{Id, PlatformError, PlatformResult, UserId};
use crate::db::MongoClient;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserCertificate {
    pub _id: Id,
    pub user_id: UserId,
    pub provider_code: String,
    pub certificate_ref: String,
    pub subject: String,
    pub issuer: String,
    pub serial_number: String,
    pub valid_from: Option<DateTime<Utc>>,
    pub valid_to: Option<DateTime<Utc>>,
    pub fingerprint: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateCertificateInput {
    pub user_id: String,
    pub provider_code: String,
    pub certificate_ref: String,
    pub subject: String,
    pub issuer: String,
    pub serial_number: String,
    pub fingerprint: String,
}

pub struct UserCertificateService;

impl UserCertificateService {
    pub fn new() -> Self {
        Self
    }

    pub async fn list_by_user(
        db: &MongoClient,
        user_id: UserId,
    ) -> PlatformResult<Vec<UserCertificate>> {
        let col = db.collection::<Document>("user_certificates");
        let mut cursor = col
            .find(doc! { "user_id": user_id.0.to_string() })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        let mut result = Vec::new();
        while let Some(res) = cursor.next().await {
            let doc = res.map_err(|e| PlatformError::Database(e.to_string()))?;
            if let Ok(c) = mongodb::bson::from_document::<UserCertificate>(doc) {
                result.push(c);
            }
        }
        Ok(result)
    }

    pub async fn create(
        db: &MongoClient,
        input: CreateCertificateInput,
    ) -> PlatformResult<UserCertificate> {
        let user_id = uuid::Uuid::parse_str(&input.user_id)
            .map_err(|e| PlatformError::Validation(e.to_string()))?;
        let now = Utc::now();
        let cert = UserCertificate {
            _id: Uuid::new_v4(),
            user_id: UserId(user_id),
            provider_code: input.provider_code,
            certificate_ref: input.certificate_ref,
            subject: input.subject,
            issuer: input.issuer,
            serial_number: input.serial_number,
            valid_from: None,
            valid_to: None,
            fingerprint: input.fingerprint,
            is_active: true,
            created_at: now,
            updated_at: now,
        };
        let mut doc = Document::new();
        doc.insert("_id", cert._id.to_string());
        doc.insert("user_id", cert.user_id.0.to_string());
        doc.insert("provider_code", &cert.provider_code);
        doc.insert("certificate_ref", &cert.certificate_ref);
        doc.insert("subject", &cert.subject);
        doc.insert("issuer", &cert.issuer);
        doc.insert("serial_number", &cert.serial_number);
        doc.insert("fingerprint", &cert.fingerprint);
        doc.insert("is_active", cert.is_active);
        doc.insert("created_at", mongodb::bson::to_bson(&cert.created_at).unwrap());
        doc.insert("updated_at", mongodb::bson::to_bson(&cert.updated_at).unwrap());

        let col = db.collection::<Document>("user_certificates");
        col.insert_one(doc)
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(cert)
    }

    pub async fn deactivate(db: &MongoClient, id: Id) -> PlatformResult<()> {
        let col = db.collection::<Document>("user_certificates");
        col.update_one(
            doc! { "_id": id.to_string() },
            doc! { "$set": { "is_active": false, "updated_at": mongodb::bson::to_bson(&Utc::now()).unwrap() } },
        )
        .await
        .map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(())
    }
}
