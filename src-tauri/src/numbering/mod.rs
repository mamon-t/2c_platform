pub mod commands;

use chrono::{DateTime, Utc};
use mongodb::bson::{doc, Document};
use serde::{Deserialize, Serialize};

use crate::core::{CompanyId, PlatformError, PlatformResult};
use crate::db::MongoClient;

const COLLECTION: &str = "number_sequences";

/// Формат номера для типа объекта
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NumberFormat {
    pub prefix: String,
    pub padding: i32,
    pub suffix: String,
}

impl Default for NumberFormat {
    fn default() -> Self {
        Self {
            prefix: String::new(),
            padding: 6,
            suffix: String::new(),
        }
    }
}

/// Последовательность нумерации для типа объекта в пределах компании
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NumberSequence {
    pub _id: String,
    pub company_id: String,
    pub entity_type_id: String,
    pub entity_type_name: String,
    pub prefix: String,
    pub padding: i32,
    pub suffix: String,
    pub current_value: i64,
    pub updated_at: DateTime<Utc>,
}

/// Входные данные для обновления формата нумерации
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateNumberFormatInput {
    pub prefix: Option<String>,
    pub padding: Option<i32>,
    pub suffix: Option<String>,
}

pub struct NumberingService;

impl NumberingService {
    pub fn new() -> Self { Self }

    /// Атомарно получить следующий номер для типа объекта и компании.
    /// Использует findOneAndUpdate с $inc для гарантированной уникальности.
    pub async fn next_number(
        db: &MongoClient,
        company_id: &CompanyId,
        entity_type_id: &str,
        entity_type_name: &str,
    ) -> PlatformResult<String> {
        let col = db.collection::<Document>(COLLECTION);
        let seq_key = format!("{}:{}", company_id.0, entity_type_id);

        let filter = doc! { "_id": &seq_key };
        let update = doc! {
            "$inc": { "current_value": 1 },
            "$set": { "updated_at": mongodb::bson::to_bson(&Utc::now()).unwrap() },
            "$setOnInsert": {
                "_id": &seq_key,
                "company_id": company_id.0.to_string(),
                "entity_type_id": entity_type_id,
                "entity_type_name": entity_type_name,
                "prefix": "",
                "padding": 6,
                "suffix": "",
            }
        };
        let opts = mongodb::options::FindOneAndUpdateOptions::builder()
            .upsert(true)
            .return_document(mongodb::options::ReturnDocument::After)
            .build();

        let result = col.find_one_and_update(filter, update)
            .with_options(opts)
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;

        let doc = result.ok_or_else(|| PlatformError::Database("Не удалось получить номер".into()))?;

        let current_value = doc.get_i64("current_value").unwrap_or(1);
        let prefix = doc.get_str("prefix").unwrap_or("");
        let padding = doc.get_i32("padding").unwrap_or(6) as usize;
        let suffix = doc.get_str("suffix").unwrap_or("");

        Ok(format!("{}{:0>width$}{}", prefix, current_value, suffix, width = padding))
    }

    /// Сбросить последовательность нумерации (для администратора)
    pub async fn reset_sequence(
        db: &MongoClient,
        company_id: &CompanyId,
        entity_type_id: &str,
        new_value: Option<i64>,
    ) -> PlatformResult<()> {
        let col = db.collection::<Document>(COLLECTION);
        let seq_key = format!("{}:{}", company_id.0, entity_type_id);
        let reset_to = new_value.unwrap_or(0);

        col.update_one(
            doc! { "_id": &seq_key },
            doc! {
                "$set": {
                    "current_value": reset_to,
                    "updated_at": mongodb::bson::to_bson(&Utc::now()).unwrap(),
                }
            },
        )
        .with_options(mongodb::options::UpdateOptions::builder().upsert(true).build())
        .await
        .map_err(|e| PlatformError::Database(e.to_string()))?;

        Ok(())
    }

    /// Обновить формат номера (prefix, padding, suffix)
    pub async fn update_format(
        db: &MongoClient,
        company_id: &CompanyId,
        entity_type_id: &str,
        entity_type_name: &str,
        input: UpdateNumberFormatInput,
    ) -> PlatformResult<NumberSequence> {
        let col = db.collection::<Document>(COLLECTION);
        let seq_key = format!("{}:{}", company_id.0, entity_type_id);

        let mut set = doc! {
            "updated_at": mongodb::bson::to_bson(&Utc::now()).unwrap(),
        };
        if let Some(ref p) = input.prefix { set.insert("prefix", p); }
        if let Some(p) = input.padding { set.insert("padding", p); }
        if let Some(ref s) = input.suffix { set.insert("suffix", s); }

        let opts = mongodb::options::FindOneAndUpdateOptions::builder()
            .upsert(true)
            .return_document(mongodb::options::ReturnDocument::After)
            .build();

        let filter = doc! { "_id": &seq_key };
        let update = doc! {
            "$set": set,
            "$setOnInsert": {
                "_id": &seq_key,
                "company_id": company_id.0.to_string(),
                "entity_type_id": entity_type_id,
                "entity_type_name": entity_type_name,
                "current_value": 0,
            }
        };

        let result = col.find_one_and_update(filter, update)
            .with_options(opts)
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;

        let doc = result.ok_or_else(|| PlatformError::Database("Не удалось обновить формат".into()))?;
        deserialize_sequence(&doc)
    }

    /// Получить последовательность нумерации по типу объекта
    pub async fn get_sequence(
        db: &MongoClient,
        company_id: &CompanyId,
        entity_type_id: &str,
    ) -> PlatformResult<Option<NumberSequence>> {
        let col = db.collection::<Document>(COLLECTION);
        let seq_key = format!("{}:{}", company_id.0, entity_type_id);

        let doc = col.find_one(doc! { "_id": &seq_key }).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;

        Ok(doc.map(|d| deserialize_sequence(&d)).transpose()?)
    }

    /// Список всех последовательностей нумерации для компании
    pub async fn list_sequences(
        db: &MongoClient,
        company_id: &CompanyId,
    ) -> PlatformResult<Vec<NumberSequence>> {
        let col = db.collection::<Document>(COLLECTION);
        let prefix = format!("{}:", company_id.0);

        let mut cursor = col.find(doc! { "_id": { "$regex": &prefix } })
            .sort(doc! { "entity_type_name": 1 })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;

        let mut sequences = Vec::new();
        while let Some(res) = cursor.next().await {
            let doc = res.map_err(|e| PlatformError::Database(e.to_string()))?;
            if let Ok(seq) = deserialize_sequence(&doc) {
                sequences.push(seq);
            }
        }

        Ok(sequences)
    }
}

fn deserialize_sequence(doc: &Document) -> PlatformResult<NumberSequence> {
    Ok(NumberSequence {
        _id: doc.get_str("_id").unwrap_or("").to_string(),
        company_id: doc.get_str("company_id").unwrap_or("").to_string(),
        entity_type_id: doc.get_str("entity_type_id").unwrap_or("").to_string(),
        entity_type_name: doc.get_str("entity_type_name").unwrap_or("").to_string(),
        prefix: doc.get_str("prefix").unwrap_or("").to_string(),
        padding: doc.get_i32("padding").unwrap_or(6),
        suffix: doc.get_str("suffix").unwrap_or("").to_string(),
        current_value: doc.get_i64("current_value").unwrap_or(0),
        updated_at: doc.get_datetime("updated_at").ok()
            .and_then(|v| chrono::DateTime::from_timestamp_millis(v.timestamp_millis()))
            .unwrap_or_else(Utc::now),
    })
}

use futures::StreamExt;
