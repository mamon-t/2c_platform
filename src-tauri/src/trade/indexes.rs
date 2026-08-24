// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

//! Частичные индексы objects для справочников торговли.
//!
//! Фильтр по entity_type UUID решает проблему «раздутых» индексов
//! универсальной коллекции: каждый индекс покрывает только свой тип.

use mongodb::bson::doc;
use mongodb::IndexModel;
use tracing::{info, warn};

use crate::db::MongoClient;

/// Резолвить entity_type_id по коду из коллекции entity_types.
async fn type_id(db: &MongoClient, code: &str) -> Option<String> {
    db.collection::<mongodb::bson::Document>("entity_types")
        .find_one(doc! { "code": code })
        .await
        .ok()
        .flatten()
        .and_then(|d| d.get_str("_id").ok().map(String::from))
}

pub async fn ensure_indexes(db: &MongoClient) {
    let col = db.collection::<mongodb::bson::Document>("objects");

    for (type_code, indexes) in [
        (
            super::ET_COUNTERPARTY,
            vec![
                // Поиск по названию
                doc! { "data.name": 1 },
                // Поиск по ИНН (частичный — поле есть только у контрагентов)
                doc! { "data.inn": 1 },
            ],
        ),
        (
            super::ET_PRICE_TYPE,
            vec![
                // Уникальный код типа цены в рамках компании
                doc! { "company_id": 1, "data.code": 1 },
                doc! { "data.purpose": 1 },
            ],
        ),
        (
            super::ET_PRICE,
            vec![
                // Главный: цена на дату (номенклатура + тип + дата desc)
                doc! { "data.nomenclature_id": 1, "data.price_type_id": 1, "data.valid_from": -1 },
                // Все цены конкретного типа
                doc! { "data.price_type_id": 1 },
            ],
        ),
    ] {
        let Some(et_id) = type_id(db, type_code).await else {
            info!("[trade] entity_type {} не найден — индексы пропущены", type_code);
            continue;
        };

        for (i, keys) in indexes.iter().enumerate() {
            let mut keys_doc = keys.clone();
            keys_doc.insert("entity_type_id", 1); // всегда в конце

            let opts = mongodb::options::IndexOptions::builder()
                .name(Some(format!("{}_{}", type_code.to_lowercase(), i)))
                .partial_filter_expression(doc! { "entity_type_id": &et_id })
                .build();

            if let Err(e) = col
                .create_index(IndexModel::builder().keys(keys_doc).options(opts).build())
                .await
            {
                warn!("[trade] индекс {}[{}]: {e}", type_code, i);
            }
        }
    }
}
