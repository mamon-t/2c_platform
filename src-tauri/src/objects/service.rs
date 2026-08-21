use chrono::Utc;
use futures::StreamExt;
use mongodb::bson::{doc, Document};
use tracing::info;

use super::*;
use super::validation::validate_data;
use crate::core::{CompanyId, PlatformError, PlatformResult, UserId};
use crate::db::MongoClient;
use crate::events::{EventService, StreamType, ActorSnapshot};
use crate::meta::service::EntityFieldService;

const COLLECTION: &str = "objects";
const SNAPSHOTS: &str = "object_snapshots";

pub struct ObjectService;

impl ObjectService {
    /// Создать объект. version = 1, state = Draft. Валидация data + транзакция.
    pub async fn create(
        db: &MongoClient,
        input: CreateObjectInput,
        company_id: CompanyId,
        user_id: UserId,
        actor: ActorSnapshot,
    ) -> PlatformResult<Object> {
        let et_id = uuid::Uuid::parse_str(&input.entity_type_id)
            .map_err(|_| PlatformError::Validation("Невалидный entity_type_id".into()))?;

        let et_col = db.collection::<Document>("entity_types");
        let et_doc = et_col.find_one(doc! { "_id": input.entity_type_id.clone() }).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        let kind = et_doc.as_ref()
            .and_then(|d| d.get_str("kind").ok())
            .unwrap_or("document")
            .to_string();

        let fields = EntityFieldService::list_by_type(db, et_id).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        if !fields.is_empty() {
            validate_data(&input.data, &fields, false)?;
        }

        let mut session = db.client().start_session().await
            .map_err(|e| PlatformError::Database(format!("start_session: {}", e)))?;
        session.start_transaction().await
            .map_err(|e| PlatformError::Database(format!("start_transaction: {}", e)))?;

        let result = Self::create_inner(db, &mut session, &input, &company_id, &user_id, &actor, &kind).await;

        match result {
            Ok(obj) => {
                session.commit_transaction().await
                    .map_err(|e| PlatformError::Database(format!("commit_transaction: {}", e)))?;
                info!("Object created: {} ({})", obj._id, obj.entity_type_id);
                Ok(obj)
            }
            Err(e) => {
                session.abort_transaction().await.ok();
                Err(e)
            }
        }
    }

    async fn create_inner(
        db: &MongoClient,
        session: &mut mongodb::ClientSession,
        input: &CreateObjectInput,
        company_id: &CompanyId,
        user_id: &UserId,
        actor: &ActorSnapshot,
        kind: &str,
    ) -> PlatformResult<Object> {
        let now = Utc::now();
        let obj = Object {
            _id: uuid::Uuid::new_v4(),
            entity_type_id: input.entity_type_id.clone(),
            kind: kind.to_string(),
            company_id: company_id.clone(),
            state: ObjectState::Draft,
            data: input.data.clone(),
            computed: None,
            number: None,
            date: input.date.clone(),
            parent_id: input.parent_id.clone(),
            version: 1,
            created_by: user_id.clone(),
            updated_by: user_id.clone(),
            created_at: now,
            updated_at: now,
        };

        let doc = serialize_object(&obj)?;
        db.collection::<Document>(COLLECTION).insert_one(doc)
            .session(&mut *session).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;

        save_snapshot_with_session(db, session, &obj, user_id.clone(), Some("Создание объекта".into())).await?;

        let svc = EventService::new();
        let payload = serde_json::json!({
            "entity_type_id": obj.entity_type_id,
            "kind": obj.kind,
            "state": "draft",
            "data": obj.data,
        });
        let _ = svc.append_with_session(db, session, StreamType::Object, &obj._id.to_string(), "object.created", payload, actor.clone(), company_id.clone(), None, None).await;

        Ok(obj)
    }

    /// Прочитать объект по ID
    pub async fn get(db: &MongoClient, id: uuid::Uuid) -> PlatformResult<Object> {
        let col = db.collection::<Document>(COLLECTION);
        let doc = col.find_one(doc! { "_id": id.to_string() }).await
            .map_err(|e| PlatformError::Database(e.to_string()))?
            .ok_or_else(|| PlatformError::NotFound(format!("Объект {id} не найден")))?;
        deserialize_object(&doc).map_err(PlatformError::NotFound)
    }

    /// Обновить данные объекта (оптимистичная блокировка по version). Валидация + транзакция.
    pub async fn update(
        db: &MongoClient,
        id: uuid::Uuid,
        input: UpdateObjectInput,
        user_id: UserId,
        actor: ActorSnapshot,
        company_id: CompanyId,
    ) -> PlatformResult<Object> {
        let old = Self::get(db, id).await?;

        if input.version != old.version {
            return Err(PlatformError::Validation(
                format!("Конфликт версий: ожидается v{}, получен v{}", old.version, input.version)
            ));
        }

        let et_id = uuid::Uuid::parse_str(&old.entity_type_id)
            .map_err(|_| PlatformError::Validation("Невалидный entity_type_id".into()))?;
        let fields = EntityFieldService::list_by_type(db, et_id).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        if !fields.is_empty() {
            validate_data(&input.data, &fields, true)?;
        }

        let mut session = db.client().start_session().await
            .map_err(|e| PlatformError::Database(format!("start_session: {}", e)))?;
        session.start_transaction().await
            .map_err(|e| PlatformError::Database(format!("start_transaction: {}", e)))?;

        let result = Self::update_inner(db, &mut session, &old, &input, &user_id, &actor, &company_id).await;

        match result {
            Ok(obj) => {
                session.commit_transaction().await
                    .map_err(|e| PlatformError::Database(format!("commit_transaction: {}", e)))?;
                Ok(obj)
            }
            Err(e) => {
                session.abort_transaction().await.ok();
                Err(e)
            }
        }
    }

    async fn update_inner(
        db: &MongoClient,
        session: &mut mongodb::ClientSession,
        old: &Object,
        input: &UpdateObjectInput,
        user_id: &UserId,
        actor: &ActorSnapshot,
        company_id: &CompanyId,
    ) -> PlatformResult<Object> {
        let new_version = old.version + 1;
        let now = Utc::now();

        let col = db.collection::<Document>(COLLECTION);
        let set = doc! {
            "data": mongodb::bson::to_bson(&input.data).map_err(|e| PlatformError::Database(e.to_string()))?,
            "version": new_version,
            "updated_by": user_id.0.to_string(),
            "updated_at": mongodb::bson::to_bson(&now).unwrap(),
        };
        let result = col.update_one(doc! { "_id": old._id.to_string(), "version": input.version }, doc! { "$set": set })
            .session(&mut *session).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;

        if result.matched_count == 0 {
            return Err(PlatformError::Validation("Конфликт версий: объект был изменён другим пользователем".into()));
        }

        let mut updated = old.clone();
        updated.data = input.data.clone();
        updated.version = new_version;
        updated.updated_by = user_id.clone();
        updated.updated_at = now;

        save_snapshot_with_session(db, session, &updated, user_id.clone(), input.reason.clone()).await?;

        let svc = EventService::new();
        let payload = serde_json::json!({
            "version": new_version,
            "data": input.data,
            "state": format!("{:?}", updated.state).to_lowercase(),
        });
        let _ = svc.append_with_session(db, session, StreamType::Object, &old._id.to_string(), "object.updated", payload, actor.clone(), company_id.clone(), None, None).await;

        Ok(updated)
    }

    /// Провести объект (Draft → Posted). Номер присваивается атомарно в транзакции.
    pub async fn post(
        db: &MongoClient,
        id: uuid::Uuid,
        version: i64,
        user_id: UserId,
        actor: ActorSnapshot,
        company_id: CompanyId,
    ) -> PlatformResult<Object> {
        let old = Self::get(db, id).await?;
        if old.state != ObjectState::Draft {
            return Err(PlatformError::Validation("Провести можно только черновик".into()));
        }
        if version != old.version {
            return Err(PlatformError::Validation("Конфликт версий".into()));
        }

        let mut session = db.client().start_session().await
            .map_err(|e| PlatformError::Database(format!("start_session: {}", e)))?;
        session.start_transaction().await
            .map_err(|e| PlatformError::Database(format!("start_transaction: {}", e)))?;

        let result = Self::post_inner(db, &mut session, &old, &user_id, &actor, &company_id).await;

        match result {
            Ok(obj) => {
                session.commit_transaction().await
                    .map_err(|e| PlatformError::Database(format!("commit_transaction: {}", e)))?;
                info!("Object posted: {} → №{}", id, obj.number.as_deref().unwrap_or("?"));
                Ok(obj)
            }
            Err(e) => {
                session.abort_transaction().await.ok();
                Err(e)
            }
        }
    }

    async fn post_inner(
        db: &MongoClient,
        session: &mut mongodb::ClientSession,
        old: &Object,
        user_id: &UserId,
        actor: &ActorSnapshot,
        company_id: &CompanyId,
    ) -> PlatformResult<Object> {
        let new_version = old.version + 1;
        let now = Utc::now();
        let number = crate::numbering::NumberingService::next_number_with_session(
            db, session, company_id, &old.entity_type_id, &old.entity_type_id,
        ).await?;

        let col = db.collection::<Document>(COLLECTION);
        let set = doc! {
            "state": "posted",
            "version": new_version,
            "number": &number,
            "updated_by": user_id.0.to_string(),
            "updated_at": mongodb::bson::to_bson(&now).unwrap(),
        };
        col.update_one(doc! { "_id": old._id.to_string(), "version": old.version }, doc! { "$set": set })
            .session(&mut *session).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;

        let mut updated = old.clone();
        updated.state = ObjectState::Posted;
        updated.version = new_version;
        updated.number = Some(number.clone());
        updated.updated_by = user_id.clone();
        updated.updated_at = now;

        save_snapshot_with_session(db, session, &updated, user_id.clone(), Some(format!("Проведён. Номер: {number}"))).await?;

        let svc = EventService::new();
        let payload = serde_json::json!({
            "number": number,
            "version": new_version,
            "state": "posted",
        });
        let _ = svc.append_with_session(db, session, StreamType::Object, &old._id.to_string(), "object.posted", payload, actor.clone(), company_id.clone(), None, None).await;

        Ok(updated)
    }

    /// Отменить проведение (Posted → Cancelled). Транзакция.
    pub async fn cancel(
        db: &MongoClient,
        id: uuid::Uuid,
        version: i64,
        user_id: UserId,
        actor: ActorSnapshot,
        company_id: CompanyId,
    ) -> PlatformResult<Object> {
        let old = Self::get(db, id).await?;
        if old.state != ObjectState::Posted {
            return Err(PlatformError::Validation("Отменить можно только проведённый документ".into()));
        }
        if version != old.version {
            return Err(PlatformError::Validation("Конфликт версий".into()));
        }

        let mut session = db.client().start_session().await
            .map_err(|e| PlatformError::Database(format!("start_session: {}", e)))?;
        session.start_transaction().await
            .map_err(|e| PlatformError::Database(format!("start_transaction: {}", e)))?;

        let result = Self::cancel_inner(db, &mut session, &old, &user_id, &actor, &company_id).await;

        match result {
            Ok(obj) => {
                session.commit_transaction().await
                    .map_err(|e| PlatformError::Database(format!("commit_transaction: {}", e)))?;
                Ok(obj)
            }
            Err(e) => {
                session.abort_transaction().await.ok();
                Err(e)
            }
        }
    }

    async fn cancel_inner(
        db: &MongoClient,
        session: &mut mongodb::ClientSession,
        old: &Object,
        user_id: &UserId,
        actor: &ActorSnapshot,
        company_id: &CompanyId,
    ) -> PlatformResult<Object> {
        let new_version = old.version + 1;
        let now = Utc::now();

        let col = db.collection::<Document>(COLLECTION);
        let set = doc! {
            "state": "cancelled",
            "version": new_version,
            "updated_by": user_id.0.to_string(),
            "updated_at": mongodb::bson::to_bson(&now).unwrap(),
        };
        col.update_one(doc! { "_id": old._id.to_string(), "version": old.version }, doc! { "$set": set })
            .session(&mut *session).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;

        let mut updated = old.clone();
        updated.state = ObjectState::Cancelled;
        updated.version = new_version;
        updated.updated_by = user_id.clone();
        updated.updated_at = now;

        save_snapshot_with_session(db, session, &updated, user_id.clone(), Some("Отмена проведения".into())).await?;

        let svc = EventService::new();
        let payload = serde_json::json!({ "version": new_version, "state": "cancelled" });
        let _ = svc.append_with_session(db, session, StreamType::Object, &old._id.to_string(), "object.cancelled", payload, actor.clone(), company_id.clone(), None, None).await;

        Ok(updated)
    }

    /// Восстановить предыдущую версию. Транзакция.
    pub async fn restore_version(
        db: &MongoClient,
        id: uuid::Uuid,
        target_version: i64,
        user_id: UserId,
        actor: ActorSnapshot,
        company_id: CompanyId,
    ) -> PlatformResult<Object> {
        let old = Self::get(db, id).await?;

        let snap_col = db.collection::<Document>(SNAPSHOTS);
        let snap_doc = snap_col.find_one(doc! {
            "object_id": id.to_string(),
            "version": target_version,
        }).await.map_err(|e| PlatformError::Database(e.to_string()))?
            .ok_or_else(|| PlatformError::NotFound(format!("Версия {target_version} не найдена")))?;

        let snap_data = snap_doc.get("data")
            .and_then(|v| v.as_document())
            .and_then(|d| mongodb::bson::from_document(d.clone()).ok())
            .unwrap_or(serde_json::Value::Null);

        let mut session = db.client().start_session().await
            .map_err(|e| PlatformError::Database(format!("start_session: {}", e)))?;
        session.start_transaction().await
            .map_err(|e| PlatformError::Database(format!("start_transaction: {}", e)))?;

        let result = Self::restore_inner(db, &mut session, &old, target_version, snap_data, &user_id, &actor, &company_id).await;

        match result {
            Ok(obj) => {
                session.commit_transaction().await
                    .map_err(|e| PlatformError::Database(format!("commit_transaction: {}", e)))?;
                Ok(obj)
            }
            Err(e) => {
                session.abort_transaction().await.ok();
                Err(e)
            }
        }
    }

    async fn restore_inner(
        db: &MongoClient,
        session: &mut mongodb::ClientSession,
        old: &Object,
        target_version: i64,
        snap_data: serde_json::Value,
        user_id: &UserId,
        actor: &ActorSnapshot,
        company_id: &CompanyId,
    ) -> PlatformResult<Object> {
        let new_version = old.version + 1;
        let now = Utc::now();

        let col = db.collection::<Document>(COLLECTION);
        let set = doc! {
            "data": mongodb::bson::to_bson(&snap_data).map_err(|e| PlatformError::Database(e.to_string()))?,
            "version": new_version,
            "updated_by": user_id.0.to_string(),
            "updated_at": mongodb::bson::to_bson(&now).unwrap(),
        };
        col.update_one(doc! { "_id": old._id.to_string() }, doc! { "$set": set })
            .session(&mut *session).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;

        let mut updated = old.clone();
        updated.data = snap_data;
        updated.version = new_version;
        updated.updated_by = user_id.clone();
        updated.updated_at = now;

        save_snapshot_with_session(db, session, &updated, user_id.clone(), Some(format!("Восстановлена версия {target_version}"))).await?;

        let svc = EventService::new();
        let payload = serde_json::json!({ "target_version": target_version, "new_version": new_version });
        let _ = svc.append_with_session(db, session, StreamType::Object, &old._id.to_string(), "object.restored", payload, actor.clone(), company_id.clone(), None, None).await;

        Ok(updated)
    }

    /// Список объектов с фильтрами (company_id всегда на сервере)
    pub async fn list(db: &MongoClient, company_id: CompanyId, filters: ObjectFilters) -> PlatformResult<ObjectPage> {
        let col = db.collection::<Document>(COLLECTION);
        let mut f = doc! { "company_id": company_id.0.to_string() };

        if let Some(ref et) = filters.entity_type_id { f.insert("entity_type_id", et); }
        if let Some(ref s) = filters.state { f.insert("state", s); }
        if let Some(ref pid) = filters.parent_id { f.insert("parent_id", pid); }

        let limit = filters.limit.unwrap_or(50).min(200);
        let offset = filters.offset.unwrap_or(0).max(0);

        let total_count = col.count_documents(f.clone()).await
            .map_err(|e| PlatformError::Database(e.to_string()))? as i64;

        let mut cursor = col.find(f).sort(doc! { "updated_at": -1 }).skip(offset as u64).limit(limit).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;

        let mut objects = Vec::new();
        while let Some(res) = cursor.next().await {
            let doc = res.map_err(|e| PlatformError::Database(e.to_string()))?;
            if let Ok(obj) = deserialize_object(&doc) { objects.push(obj); }
        }

        Ok(ObjectPage {
            objects,
            total_count,
            has_more: (offset + limit) < total_count,
        })
    }

    /// История версий объекта
    pub async fn list_versions(db: &MongoClient, object_id: uuid::Uuid) -> PlatformResult<Vec<ObjectSnapshot>> {
        let col = db.collection::<Document>(SNAPSHOTS);
        let mut cursor = col.find(doc! { "object_id": object_id.to_string() })
            .sort(doc! { "version": -1 }).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        let mut result = Vec::new();
        while let Some(res) = cursor.next().await {
            let doc = res.map_err(|e| PlatformError::Database(e.to_string()))?;
            if let Ok(snap) = deserialize_snapshot(&doc) { result.push(snap); }
        }
        Ok(result)
    }
}

// ── Helpers ─────────────────────────────────────────────────

fn serialize_object(obj: &Object) -> Result<Document, PlatformError> {
    let mut doc = Document::new();
    doc.insert("_id", obj._id.to_string());
    doc.insert("entity_type_id", &obj.entity_type_id);
    doc.insert("kind", &obj.kind);
    doc.insert("company_id", obj.company_id.0.to_string());
    doc.insert("state", format!("{:?}", obj.state).to_lowercase());
    if let Ok(bson) = mongodb::bson::to_bson(&obj.data) { doc.insert("data", bson); }
    if let Some(ref c) = obj.computed { if let Ok(bson) = mongodb::bson::to_bson(c) { doc.insert("computed", bson); } }
    if let Some(ref n) = obj.number { doc.insert("number", n); }
    if let Some(ref d) = obj.date { doc.insert("date", d); }
    if let Some(ref p) = obj.parent_id { doc.insert("parent_id", p); }
    doc.insert("version", obj.version);
    doc.insert("created_by", obj.created_by.0.to_string());
    doc.insert("updated_by", obj.updated_by.0.to_string());
    doc.insert("created_at", mongodb::bson::to_bson(&obj.created_at).unwrap());
    doc.insert("updated_at", mongodb::bson::to_bson(&obj.updated_at).unwrap());
    Ok(doc)
}

fn deserialize_object(doc: &Document) -> Result<Object, String> {
    let _id = uuid::Uuid::parse_str(doc.get_str("_id").unwrap_or("")).map_err(|e| e.to_string())?;
    let company_id = crate::core::CompanyId(
        uuid::Uuid::parse_str(doc.get_str("company_id").unwrap_or("")).map_err(|e| e.to_string())?
    );
    let state_str = doc.get_str("state").unwrap_or("draft");
    let state: ObjectState = serde_json::from_str(&format!("\"{state_str}\"")).unwrap_or(ObjectState::Draft);
    let data = doc.get("data").and_then(|v| {
        if let Some(d) = v.as_document() { mongodb::bson::from_document(d.clone()).ok() }
        else { Some(serde_json::Value::Null) }
    }).unwrap_or(serde_json::Value::Null);
    Ok(Object {
        _id,
        entity_type_id: doc.get_str("entity_type_id").unwrap_or("").to_string(),
        kind: doc.get_str("kind").unwrap_or("document").to_string(),
        company_id,
        state,
        data,
        computed: doc.get("computed").and_then(|v| {
            if let Some(d) = v.as_document() { mongodb::bson::from_document(d.clone()).ok() }
            else { None }
        }),
        number: doc.get_str("number").ok().map(String::from),
        date: doc.get_str("date").ok().map(String::from),
        parent_id: doc.get_str("parent_id").ok().map(String::from),
        version: doc.get_i64("version").unwrap_or(1),
        created_by: crate::core::UserId(
            uuid::Uuid::parse_str(doc.get_str("created_by").unwrap_or("")).unwrap_or_default()
        ),
        updated_by: crate::core::UserId(
            uuid::Uuid::parse_str(doc.get_str("updated_by").unwrap_or("")).unwrap_or_default()
        ),
        created_at: doc.get_datetime("created_at").ok()
            .and_then(|v| chrono::DateTime::from_timestamp_millis(v.timestamp_millis()))
            .unwrap_or_else(Utc::now),
        updated_at: doc.get_datetime("updated_at").ok()
            .and_then(|v| chrono::DateTime::from_timestamp_millis(v.timestamp_millis()))
            .unwrap_or_else(Utc::now),
    })
}

fn deserialize_snapshot(doc: &Document) -> Result<ObjectSnapshot, String> {
    let _id = uuid::Uuid::parse_str(doc.get_str("_id").unwrap_or("")).map_err(|e| e.to_string())?;
    let state_str = doc.get_str("state").unwrap_or("draft");
    let state: ObjectState = serde_json::from_str(&format!("\"{state_str}\"")).unwrap_or(ObjectState::Draft);
    let data = doc.get("data").and_then(|v| {
        if let Some(d) = v.as_document() { mongodb::bson::from_document(d.clone()).ok() }
        else { Some(serde_json::Value::Null) }
    }).unwrap_or(serde_json::Value::Null);
    Ok(ObjectSnapshot {
        _id,
        object_id: doc.get_str("object_id").unwrap_or("").to_string(),
        version: doc.get_i64("version").unwrap_or(1),
        data,
        state,
        created_by: crate::core::UserId(
            uuid::Uuid::parse_str(doc.get_str("created_by").unwrap_or("")).unwrap_or_default()
        ),
        created_at: doc.get_datetime("created_at").ok()
            .and_then(|v| chrono::DateTime::from_timestamp_millis(v.timestamp_millis()))
            .unwrap_or_else(Utc::now),
        reason: doc.get_str("reason").ok().map(String::from),
    })
}

/// Сохранить снимок версии в рамках сессии (транзакции)
async fn save_snapshot_with_session(
    db: &MongoClient,
    session: &mut mongodb::ClientSession,
    obj: &Object,
    user_id: UserId,
    reason: Option<String>,
) -> PlatformResult<()> {
    let snap = ObjectSnapshot {
        _id: uuid::Uuid::new_v4(),
        object_id: obj._id.to_string(),
        version: obj.version,
        data: obj.data.clone(),
        state: obj.state.clone(),
        created_by: user_id,
        created_at: Utc::now(),
        reason,
    };
    let mut doc = Document::new();
    doc.insert("_id", snap._id.to_string());
    doc.insert("object_id", &snap.object_id);
    doc.insert("version", snap.version);
    if let Ok(bson) = mongodb::bson::to_bson(&snap.data) { doc.insert("data", bson); }
    doc.insert("state", format!("{:?}", snap.state).to_lowercase());
    doc.insert("created_by", snap.created_by.0.to_string());
    doc.insert("created_at", mongodb::bson::to_bson(&snap.created_at).unwrap());
    if let Some(ref r) = snap.reason { doc.insert("reason", r); }

    db.collection::<Document>(SNAPSHOTS).insert_one(doc)
        .session(&mut *session).await
        .map_err(|e| PlatformError::Database(e.to_string()))?;
    Ok(())
}
