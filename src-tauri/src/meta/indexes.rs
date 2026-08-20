use mongodb::IndexModel;
use mongodb::bson::doc;
use tracing::info;

use crate::db::MongoClient;
use crate::core::PlatformResult;

pub async fn ensure_meta_indexes(db: &MongoClient) -> PlatformResult<()> {
    // entity_types: unique code per company
    let et = db.collection::<mongodb::bson::Document>("entity_types");
    let _ = et.create_index(IndexModel::builder().keys(doc! { "company_id": 1, "code": 1 }).build()).await;

    // entity_fields: ordered list per entity_type
    let ef = db.collection::<mongodb::bson::Document>("entity_fields");
    let _ = ef.create_index(IndexModel::builder().keys(doc! { "entity_type_id": 1, "order": 1 }).build()).await;

    // entity_states: ordered list per entity_type
    let es = db.collection::<mongodb::bson::Document>("entity_states");
    let _ = es.create_index(IndexModel::builder().keys(doc! { "entity_type_id": 1, "order": 1 }).build()).await;

    // entity_transitions: per entity_type
    let etr = db.collection::<mongodb::bson::Document>("entity_transitions");
    let _ = etr.create_index(IndexModel::builder().keys(doc! { "entity_type_id": 1 }).build()).await;

    // entity_forms: per entity_type
    let efrm = db.collection::<mongodb::bson::Document>("entity_forms");
    let _ = efrm.create_index(IndexModel::builder().keys(doc! { "entity_type_id": 1 }).build()).await;

    // entity_actions: per entity_type
    let ea = db.collection::<mongodb::bson::Document>("entity_actions");
    let _ = ea.create_index(IndexModel::builder().keys(doc! { "entity_type_id": 1 }).build()).await;

    info!("Meta indexes ensured");
    Ok(())
}
