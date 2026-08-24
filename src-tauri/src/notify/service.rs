//! Персистенция уведомлений (in-app outbox).
//!
//! Используется:
//! - host-функцией `notify_user` (WASM-модули)
//! - командами `notifications_list` / `notifications_mark_read` (фронтенд)

use futures::StreamExt;
use mongodb::bson::{doc, Document};

use crate::core::{PlatformError, PlatformResult, UserId};
use crate::db::MongoClient;

use super::{NotificationOutbox, NotificationStatus};

pub struct NotificationStore;

impl NotificationStore {
    pub const COLLECTION: &str = "notifications";

    /// Сохранить уведомление. Возвращает его id.
    pub async fn save(db: &MongoClient, n: &NotificationOutbox) -> PlatformResult<String> {
        let doc_body = doc! {
            "_id": n._id.to_string(),
            "company_id": n.company_id.0.to_string(),
            "template_code": &n.template_code,
            "channel": serde_json::to_string(&n.channel).unwrap_or_default().trim_matches('"').to_string(),
            "recipient_user_id": n.recipient_user_id.0.to_string(),
            "subject": n.subject.clone().unwrap_or_default(),
            "body": &n.body,
            "status": serde_json::to_string(&n.status).unwrap_or_default().trim_matches('"').to_string(),
            "attempts": n.attempts,
            "created_at": mongodb::bson::Bson::DateTime(mongodb::bson::DateTime::from_millis(n.created_at.timestamp_millis())),
        };

        db.collection::<Document>(Self::COLLECTION)
            .insert_one(doc_body)
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;

        Ok(n._id.to_string())
    }

    /// Список уведомлений пользователя (новые сверху).
    pub async fn list_for_user(
        db: &MongoClient,
        user_id: &UserId,
        limit: i64,
    ) -> PlatformResult<Vec<NotificationOutbox>> {
        let col = db.collection::<Document>(Self::COLLECTION);
        let mut cursor = col
            .find(doc! { "recipient_user_id": user_id.0.to_string() })
            .sort(doc! { "created_at": -1 })
            .limit(limit.clamp(1, 200))
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;

        let mut result = Vec::new();
        while let Some(Ok(d)) = cursor.next().await {
            if let Some(n) = deserialize_notification(&d) {
                result.push(n);
            }
        }
        Ok(result)
    }

    /// Отметить прочитанным(и). id = None → все уведомления пользователя.
    /// Возвращает количество обновлённых.
    pub async fn mark_read(
        db: &MongoClient,
        user_id: &UserId,
        notification_id: Option<&str>,
    ) -> PlatformResult<u64> {
        let col = db.collection::<Document>(Self::COLLECTION);
        let mut filter = doc! {
            "recipient_user_id": user_id.0.to_string(),
            "status": { "$ne": "read" },
        };
        if let Some(id) = notification_id {
            filter.insert("_id", id);
        }

        let res = col
            .update_many(filter, doc! { "$set": { "status": "read" } })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(res.modified_count)
    }

    /// Сохранить уведомление (новая модель с severity/entity_ref).
    pub async fn save_notification(
        db: &MongoClient,
        n: &crate::notify::Notification,
    ) -> crate::core::PlatformResult<()> {
        let mut d = mongodb::bson::to_document(n)
            .map_err(|e| crate::core::PlatformError::Internal(e.to_string()))?;
        d.insert("_id", n.id.to_string());
        db.collection::<Document>(Self::COLLECTION)
            .insert_one(d)
            .await
            .map_err(|e| crate::core::PlatformError::Database(e.to_string()))?;
        Ok(())
    }

    /// Количество непрочитанных уведомлений пользователя.
    pub async fn count_unread(
        db: &MongoClient,
        user_id: &UserId,
    ) -> crate::core::PlatformResult<i64> {
        db.collection::<Document>(Self::COLLECTION)
            .count_documents(doc! {
                "user_id": user_id.0.to_string(),
                "status": { "$nin": ["read", "archived"] },
            })
            .await
            .map(|c| c as i64)
            .map_err(|e| crate::core::PlatformError::Database(e.to_string()))
    }
}

fn deserialize_notification(d: &Document) -> Option<NotificationOutbox> {
    let _id = uuid::Uuid::parse_str(d.get_str("_id").ok()?).ok()?;
    let company_id = crate::core::CompanyId(uuid::Uuid::parse_str(d.get_str("company_id").ok()?).ok()?);
    let recipient_user_id = UserId(uuid::Uuid::parse_str(d.get_str("recipient_user_id").ok()?).ok()?);
    let channel = match d.get_str("channel").unwrap_or("in_app") {
        "email" => super::NotificationChannel::Email,
        _ => super::NotificationChannel::InApp,
    };
    let status = match d.get_str("status").unwrap_or("sent") {
        "pending" => NotificationStatus::Pending,
        "failed" => NotificationStatus::Failed,
        "read" => NotificationStatus::Read,
        _ => NotificationStatus::Sent,
    };

    Some(NotificationOutbox {
        _id,
        company_id,
        template_code: d.get_str("template_code").unwrap_or("").to_string(),
        channel,
        recipient_user_id,
        subject: d.get_str("subject").ok().filter(|s| !s.is_empty()).map(String::from),
        body: d.get_str("body").unwrap_or("").to_string(),
        status,
        attempts: d.get_i32("attempts").unwrap_or(0),
        last_error: None,
        created_at: d
            .get_datetime("created_at")
            .ok()
            .and_then(|v| chrono::DateTime::from_timestamp_millis(v.timestamp_millis()))
            .unwrap_or_else(chrono::Utc::now),
        sent_at: None,
    })
}
