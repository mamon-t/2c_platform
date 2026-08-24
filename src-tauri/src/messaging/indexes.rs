// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

use mongodb::bson::doc;
use mongodb::IndexModel;
use tracing::warn;

use crate::db::MongoClient;
use super::{COL_MESSAGES, COL_READS, COL_ROOMS};

pub async fn ensure_indexes(db: &MongoClient) {
    let rooms = db.collection::<mongodb::bson::Document>(COL_ROOMS);
    let messages = db.collection::<mongodb::bson::Document>(COL_MESSAGES);
    let reads = db.collection::<mongodb::bson::Document>(COL_READS);

    // Комнаты пользователя
    if let Err(e) = rooms.create_index(IndexModel::builder()
        .keys(doc! { "company_id": 1, "members": 1, "last_message_at": -1 }).build()).await {
        warn!("messaging_rooms members: {e}");
    }

    // Direct-комнаты: уникальная пара участников
    if let Err(e) = rooms.create_index(IndexModel::builder()
        .keys(doc! { "company_id": 1, "room_type": 1, "members": 1 })
        .options(mongodb::options::IndexOptions::builder().unique(true).build())
        .build()).await {
        warn!("messaging_rooms direct uniq: {e}");
    }

    // Сообщения комнаты (пагинация)
    for keys in [
        doc! { "room_id": 1, "created_at": 1 },
        doc! { "company_id": 1, "author_id": 1 },
        doc! { "nomenclature_id": 1 },
    ] {
        if let Err(e) = messages.create_index(IndexModel::builder().keys(keys).build()).await {
            warn!("messaging_messages index: {e}");
        }
    }

    // Прочтения: unique room+user
    if let Err(e) = reads.create_index(IndexModel::builder()
        .keys(doc! { "room_id": 1, "user_id": 1 })
        .options(mongodb::options::IndexOptions::builder().unique(true).build())
        .build()).await {
        warn!("messaging_reads uniq: {e}");
    }
}
