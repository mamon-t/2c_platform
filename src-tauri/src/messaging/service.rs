//! Сервис сообщений: комнаты, сообщения, прочтения.

use chrono::Utc;
use futures::StreamExt;
use mongodb::bson::{doc, Document};

use crate::core::{CompanyId, PlatformError, PlatformResult, UserId};
use crate::db::MongoClient;

use super::{MessagingMessage, MessagingRoom, RoomType, COL_MESSAGES, COL_READS, COL_ROOMS};

pub struct MessagingService;

impl MessagingService {
    // ── Комнаты ────────────────────────────────────────────

    pub async fn list_rooms(
        db: &MongoClient,
        company_id: &CompanyId,
        user_id: &str,
        room_type: Option<&str>,
    ) -> PlatformResult<Vec<serde_json::Value>> {
        let mut filter = doc! {
            "company_id": company_id.0.to_string(),
            "members": user_id,
            "is_archived": { "$ne": true },
        };
        if let Some(rt) = room_type {
            filter.insert("room_type", rt);
        }

        let mut cursor = db.collection::<Document>(COL_ROOMS)
            .find(filter)
            .sort(doc! { "last_message_at": -1 })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;

        let mut rooms = Vec::new();
        while let Some(Ok(d)) = cursor.next().await {
            let rid = d.get_str("_id").unwrap_or("").to_string();
            // Последнее сообщение
            let last_msg = db.collection::<Document>(COL_MESSAGES)
                .find_one(doc! { "room_id": &rid, "is_deleted": { "$ne": true } })
                .sort(doc! { "created_at": -1 })
                .await.ok().flatten();
            // Непрочитанные
            let read = db.collection::<Document>(COL_READS)
                .find_one(doc! { "room_id": &rid, "user_id": user_id })
                .await.ok().flatten();
            let last_read_id = read.as_ref()
                .and_then(|r| r.get_str("last_read_message_id").ok()).unwrap_or("").to_string();
            let unread = db.collection::<Document>(COL_MESSAGES)
                .count_documents(doc! {
                    "room_id": &rid,
                    "_id": { "$gt": last_read_id },
                    "author_id": { "$ne": user_id },
                    "is_deleted": { "$ne": true },
                }).await.unwrap_or(0);

            rooms.push(serde_json::json!({
                "room": d,
                "last_message": last_msg.map(|m| serde_json::json!({
                    "content": m.get_str("content").unwrap_or(""),
                    "author_id": m.get_str("author_id").unwrap_or(""),
                    "created_at": m.get_datetime("created_at")
                        .ok()
                        .and_then(|t| chrono::DateTime::from_timestamp_millis(t.timestamp_millis()))
                        .map(|t| t.to_rfc3339())
                        .unwrap_or_default(),
                })),
                "unread_count": unread,
            }));
        }
        Ok(rooms)
    }

    /// Найти или создать direct-комнату между двумя пользователями.
    pub async fn ensure_direct_room(
        db: &MongoClient,
        company_id: &CompanyId,
        user_a: &str,
        user_b: &str,
    ) -> PlatformResult<Document> {
        let col = db.collection::<Document>(COL_ROOMS);
        let filter = doc! {
            "company_id": company_id.0.to_string(),
            "room_type": "direct",
            "members": { "$all": [user_a, user_b], "$size": 2 },
        };
        if let Some(existing) = col.find_one(filter.clone()).await.map_err(|e| PlatformError::Database(e.to_string()))? {
            return Ok(existing);
        }
        let now = Utc::now();
        let room = doc! {
            "_id": uuid::Uuid::new_v4().to_string(),
            "company_id": company_id.0.to_string(),
            "room_type": "direct",
            "title": null,
            "members": [user_a, user_b],
            "entity_ref": null,
            "created_by": user_a,
            "created_at": mongodb::bson::DateTime::from_millis(now.timestamp_millis()),
            "last_message_at": null,
            "is_archived": false,
        };
        col.insert_one(room.clone()).await.map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(room)
    }

    pub async fn create_group_room(
        db: &MongoClient,
        company_id: &CompanyId,
        title: &str,
        members: Vec<String>,
        created_by: &str,
        entity_ref: Option<serde_json::Value>,
    ) -> PlatformResult<Document> {
        let now = Utc::now();
        let room = doc! {
            "_id": uuid::Uuid::new_v4().to_string(),
            "company_id": company_id.0.to_string(),
            "room_type": if entity_ref.is_some() { "document" } else { "group" },
            "title": title,
            "members": members,
            "entity_ref": entity_ref.map(|v| mongodb::bson::to_bson(&v).unwrap_or_default()),
            "created_by": created_by,
            "created_at": mongodb::bson::DateTime::from_millis(now.timestamp_millis()),
            "last_message_at": null,
            "is_archived": false,
        };
        db.collection::<Document>(COL_ROOMS)
            .insert_one(room.clone())
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(room)
    }

    pub async fn archive_room(
        db: &MongoClient,
        company_id: &CompanyId,
        room_id: &str,
        user_id: &str,
    ) -> PlatformResult<()> {
        let res = db.collection::<Document>(COL_ROOMS)
            .update_one(
                doc! { "_id": room_id, "company_id": company_id.0.to_string(), "created_by": user_id },
                doc! { "$set": { "is_archived": true } },
            )
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        if res.matched_count == 0 {
            return Err(PlatformError::NotFound(format!("Комната {room_id} не найдена")));
        }
        Ok(())
    }

    // ── Сообщения ──────────────────────────────────────────

    pub async fn send_message(
        db: &MongoClient,
        company_id: &CompanyId,
        room_id: &str,
        author_id: &str,
        content: &str,
        reply_to: Option<&str>,
    ) -> PlatformResult<MessagingMessage> {
        if content.trim().is_empty() {
            return Err(PlatformError::Validation("Пустое сообщение".into()));
        }
        let msg = MessagingMessage {
            id: uuid::Uuid::new_v4(),
            company_id: company_id.0.to_string(),
            room_id: room_id.to_string(),
            author_id: author_id.to_string(),
            content: content.to_string(),
            reply_to: reply_to.map(String::from),
            is_deleted: false,
            edited_at: None,
            created_at: Utc::now(),
        };
        let mut d = mongodb::bson::to_document(&msg)
            .map_err(|e| PlatformError::Internal(e.to_string()))?;
        d.insert("_id", msg.id.to_string());

        db.collection::<Document>(COL_MESSAGES)
            .insert_one(d)
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;

        // Обновить last_message_at в комнате
        db.collection::<Document>(COL_ROOMS)
            .update_one(
                doc! { "_id": room_id },
                doc! { "$set": { "last_message_at": mongodb::bson::DateTime::now() } },
            )
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;

        Ok(msg)
    }

    pub async fn list_messages(
        db: &MongoClient,
        company_id: &CompanyId,
        room_id: &str,
        limit: i64,
    ) -> PlatformResult<Vec<MessagingMessage>> {
        let mut cursor = db.collection::<Document>(COL_MESSAGES)
            .find(doc! {
                "company_id": company_id.0.to_string(),
                "room_id": room_id,
                "is_deleted": { "$ne": true },
            })
            .sort(doc! { "created_at": 1 })
            .limit(limit.clamp(1, 500))
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;

        let mut out = Vec::new();
        while let Some(Ok(m)) = cursor.next().await {
            if let Ok(msg) = mongodb::bson::from_document::<MessagingMessage>(m) {
                out.push(msg);
            }
        }
        Ok(out)
    }

    pub async fn edit_message(
        db: &MongoClient,
        company_id: &CompanyId,
        message_id: &str,
        author_id: &str,
        new_content: &str,
    ) -> PlatformResult<()> {
        let res = db.collection::<Document>(COL_MESSAGES)
            .update_one(
                doc! { "_id": message_id, "author_id": author_id, "is_deleted": { "$ne": true } },
                doc! { "$set": { "content": new_content, "edited_at": mongodb::bson::DateTime::now() } },
            )
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        if res.matched_count == 0 {
            return Err(PlatformError::NotFound("Сообщение не найдено".into()));
        }
        Ok(())
    }

    pub async fn delete_message(
        db: &MongoClient,
        company_id: &CompanyId,
        message_id: &str,
        author_id: &str,
    ) -> PlatformResult<()> {
        let res = db.collection::<Document>(COL_MESSAGES)
            .update_one(
                doc! { "_id": message_id, "author_id": author_id },
                doc! { "$set": { "is_deleted": true, "content": "" } },
            )
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        if res.matched_count == 0 {
            return Err(PlatformError::NotFound("Сообщение не найдено".into()));
        }
        Ok(())
    }

    // ── Прочтения ──────────────────────────────────────────

    pub async fn update_read(
        db: &MongoClient,
        company_id: &CompanyId,
        room_id: &str,
        user_id: &UserId,
        last_message_id: &str,
    ) -> PlatformResult<()> {
        let now = Utc::now();
        db.collection::<Document>(COL_READS)
            .update_one(
                doc! { "room_id": room_id, "user_id": user_id.0.to_string() },
                doc! { "$set": {
                    "last_read_message_id": last_message_id,
                    "updated_at": mongodb::bson::DateTime::from_millis(now.timestamp_millis()),
                }, "$setOnInsert": {
                    "_id": uuid::Uuid::new_v4().to_string(),
                    "company_id": company_id.0.to_string(),
                }},
            )
            .upsert(true)
            .await
            .map(|_| ())
            .map_err(|e| PlatformError::Database(e.to_string()))
    }
}

impl MessagingService {
    /// Найти или создать document-комнату.
    pub async fn ensure_document_room(
        db: &MongoClient,
        company_id: &CompanyId,
        doc_id: &str,
        doc_title: &str,
    ) -> PlatformResult<String> {
        let col = db.collection::<Document>(COL_ROOMS);
        let filter = doc! {
            "company_id": company_id.0.to_string(),
            "entity_ref.entity_id": doc_id,
            "is_archived": { "$ne": true },
        };
        if let Some(existing) = col.find_one(filter.clone()).await.map_err(|e| PlatformError::Database(e.to_string()))? {
            return Ok(existing.get_str("_id").unwrap_or("").to_string());
        }
        let room_id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();
        col.insert_one(doc! {
            "_id": &room_id,
            "company_id": company_id.0.to_string(),
            "room_type": "document",
            "title": format!("Обсуждение: {}", doc_title),
            "members": [],
            "entity_ref": { "entity_type": "document", "entity_id": doc_id },
            "created_by": "",
            "created_at": mongodb::bson::DateTime::from_millis(now.timestamp_millis()),
            "last_message_at": null,
            "is_archived": false,
        }).await.map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(room_id)
    }
}
