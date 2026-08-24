// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

use mongodb::bson::{doc, Document};
use serde::{Deserialize, Serialize};

use crate::core::{PlatformError, PlatformResult};
use crate::db::MongoClient;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingEntry {
    pub code: String,
    pub name: String,
}

pub struct SettingsService;

impl SettingsService {
    pub fn new() -> Self { Self }

    pub async fn get_setting(db: &MongoClient, key: &str) -> PlatformResult<Option<serde_json::Value>> {
        let col = db.collection::<Document>("app_settings");
        let doc = col
            .find_one(doc! { "_id": key })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        if let Some(doc) = doc {
            let value: serde_json::Value = serde_json::to_value(&doc).map_err(|e| PlatformError::Database(e.to_string()))?;
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }

    pub async fn save_setting(db: &MongoClient, key: &str, value: serde_json::Value) -> PlatformResult<()> {
        let col = db.collection::<Document>("app_settings");
        let bson_val = mongodb::bson::to_bson(&value).map_err(|e| PlatformError::Database(e.to_string()))?;
        let mut doc = match bson_val {
            mongodb::bson::Bson::Document(d) => d,
            _ => return Err(PlatformError::Database("Ожидается документ".to_string())),
        };
        doc.insert("_id", key);
        col.replace_one(doc! { "_id": key }, doc)
            .upsert(true)
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(())
    }

    pub async fn get_contact_types(db: &MongoClient) -> PlatformResult<Vec<SettingEntry>> {
        let default = vec![
            SettingEntry { code: "email".to_string(), name: "Email".to_string() },
            SettingEntry { code: "phone".to_string(), name: "Телефон".to_string() },
            SettingEntry { code: "telegram".to_string(), name: "Telegram".to_string() },
            SettingEntry { code: "web".to_string(), name: "Веб".to_string() },
        ];
        match Self::get_setting(db, "contact_types").await? {
            Some(val) => {
                let entries: Vec<SettingEntry> = serde_json::from_value(val).unwrap_or(default);
                Ok(entries)
            }
            None => {
                Self::save_setting(db, "contact_types", serde_json::to_value(&default).unwrap_or_default()).await?;
                Ok(default)
            }
        }
    }
}
