use mongodb::bson::doc;
use mongodb::IndexModel;
use tracing::warn;

use crate::db::MongoClient;
use super::service::COLLECTION;

pub async fn ensure_indexes(db: &MongoClient) {
    let col = db.collection::<mongodb::bson::Document>(COLLECTION);
    if let Err(e) = col
        .create_index(IndexModel::builder().keys(doc! { "company_id": 1, "kind": 1 }).build())
        .await
    {
        warn!("Индекс devices.company_id+kind: {e}");
    }
}
