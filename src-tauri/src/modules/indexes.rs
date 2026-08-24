// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

use mongodb::bson::doc;
use mongodb::IndexModel;
use tracing::warn;

use crate::db::MongoClient;
use super::{COLLECTION_MODULES, COLLECTION_COMPANY_MODULES};

pub async fn ensure_indexes(db: &MongoClient) {
    let modules = db.collection::<crate::modules::InstalledModule>(COLLECTION_MODULES);
    let company_modules = db.collection::<crate::modules::CompanyModule>(COLLECTION_COMPANY_MODULES);

    // modules: уникальный индекс на code
    if let Err(e) = modules
        .create_index(IndexModel::builder().keys(doc! { "code": 1 }).build())
        .await
    {
        warn!("Индекс modules.code: {}", e);
    }

    // modules: индекс на api_version
    if let Err(e) = modules
        .create_index(IndexModel::builder().keys(doc! { "api_version": 1 }).build())
        .await
    {
        warn!("Индекс modules.api_version: {}", e);
    }

    // company_modules: составной уникальный индекс company_id + module_id
    if let Err(e) = company_modules
        .create_index(IndexModel::builder().keys(doc! { "company_id": 1, "module_id": 1 }).build())
        .await
    {
        warn!("Индекс company_modules.company_id+module_id: {}", e);
    }

    // company_modules: индекс на company_id
    if let Err(e) = company_modules
        .create_index(IndexModel::builder().keys(doc! { "company_id": 1 }).build())
        .await
    {
        warn!("Индекс company_modules.company_id: {}", e);
    }

    // module_store (KV-хранилище модулей): уникальный ns_key
    let module_store = db.collection::<mongodb::bson::Document>(crate::plugin_manager::storage::COLLECTION_MODULE_STORE);
    if let Err(e) = module_store
        .create_index(
            IndexModel::builder()
                .keys(doc! { "ns_key": 1 })
                .options(mongodb::options::IndexOptions::builder().unique(true).build())
                .build(),
        )
        .await
    {
        warn!("Индекс module_store.ns_key: {}", e);
    }
    if let Err(e) = module_store
        .create_index(IndexModel::builder().keys(doc! { "company_id": 1, "module_code": 1 }).build())
        .await
    {
        warn!("Индекс module_store.company+module: {}", e);
    }

    // notifications: получатель + время
    let notifications = db.collection::<mongodb::bson::Document>(crate::notify::service::NotificationStore::COLLECTION);
    if let Err(e) = notifications
        .create_index(IndexModel::builder().keys(doc! { "recipient_user_id": 1, "created_at": -1 }).build())
        .await
    {
        warn!("Индекс notifications.recipient: {}", e);
    }
}
