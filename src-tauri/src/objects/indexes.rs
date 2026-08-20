use mongodb::IndexModel;
use mongodb::bson::doc;
use tracing::info;

use crate::db::MongoClient;
use crate::core::PlatformResult;

pub async fn ensure_object_indexes(db: &MongoClient) -> PlatformResult<()> {
    let obj = db.collection::<mongodb::bson::Document>("objects");
    let _ = obj.create_index(IndexModel::builder().keys(doc! { "entity_type_id": 1, "company_id": 1 }).build()).await;
    let _ = obj.create_index(IndexModel::builder().keys(doc! { "company_id": 1, "state": 1 }).build()).await;
    let _ = obj.create_index(IndexModel::builder().keys(doc! { "number": 1 }).build()).await;
    let _ = obj.create_index(IndexModel::builder().keys(doc! { "parent_id": 1 }).build()).await;
    let _ = obj.create_index(IndexModel::builder().keys(doc! { "company_id": 1, "updated_at": -1 }).build()).await;

    let snap = db.collection::<mongodb::bson::Document>("object_snapshots");
    let _ = snap.create_index(IndexModel::builder().keys(doc! { "object_id": 1, "version": -1 }).build()).await;

    info!("Object indexes ensured");
    Ok(())
}
