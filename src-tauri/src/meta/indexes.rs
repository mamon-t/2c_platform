// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

use mongodb::IndexModel;
use mongodb::bson::doc;
use tracing::{info, warn};

use crate::db::MongoClient;
use crate::core::PlatformResult;

pub async fn ensure_meta_indexes(db: &MongoClient) -> PlatformResult<()> {
    let indexes: Vec<(&str, &str, mongodb::bson::Document)> = vec![
        ("entity_types",      "company_id + code",          doc! { "company_id": 1, "code": 1 }),
        ("entity_fields",     "entity_type_id + order",     doc! { "entity_type_id": 1, "order": 1 }),
        ("entity_states",     "entity_type_id + order",     doc! { "entity_type_id": 1, "order": 1 }),
        ("entity_transitions","entity_type_id",             doc! { "entity_type_id": 1 }),
        ("entity_forms",      "entity_type_id",             doc! { "entity_type_id": 1 }),
        ("entity_actions",    "entity_type_id",             doc! { "entity_type_id": 1 }),
    ];

    for (collection, name, keys) in indexes {
        let col = db.collection::<mongodb::bson::Document>(collection);
        if let Err(e) = col.create_index(IndexModel::builder().keys(keys).build()).await {
            warn!("Не удалось создать индекс {collection}/{name}: {e}");
        }
    }

    info!("Meta indexes ensured");
    Ok(())
}
