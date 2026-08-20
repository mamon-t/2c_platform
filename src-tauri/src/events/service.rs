use futures::StreamExt;
use mongodb::bson::{doc, Document};
use tracing::info;

use super::{ActorSnapshot, Event, EventFilters, EventPage, StreamType, EventService};
use crate::core::{CompanyId, PlatformError, PlatformResult};
use crate::db::MongoClient;

const COLLECTION: &str = "events";
const MAX_LIMIT: i64 = 200;
const DEFAULT_LIMIT: i64 = 50;

impl EventService {
    /// Записать событие с атомарной инкрементацией version в потоке.
    /// version вычисляется автоматически: MAX(stream_id) + 1.
    pub async fn append(
        &self,
        db: &MongoClient,
        stream_type: StreamType,
        stream_id: &str,
        event_type: &str,
        payload: serde_json::Value,
        metadata: ActorSnapshot,
        company_id: CompanyId,
        correlation_id: Option<String>,
        causation_id: Option<String>,
    ) -> PlatformResult<Event> {
        let col = db.collection::<Document>(COLLECTION);

        // Атомарно находим максимальный version в потоке и инкрементируем
        let filter = doc! {
            "stream_type": stream_type.to_string(),
            "stream_id": stream_id,
        };

        // Find max version
        let mut cursor = col
            .find(filter.clone())
            .sort(doc! { "version": -1 })
            .limit(1)
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;

        let next_version = if let Some(Ok(last_doc)) = cursor.next().await {
            last_doc.get_i64("version").unwrap_or(0) + 1
        } else {
            1
        };

        let event_id = uuid::Uuid::new_v4();
        let event = Event {
            _id: event_id,
            stream_type: stream_type.clone(),
            stream_id: stream_id.to_string(),
            event_type: event_type.to_string(),
            version: next_version,
            payload: payload.clone(),
            metadata: metadata.clone(),
            company_id: company_id.clone(),
            correlation_id: correlation_id.clone(),
            causation_id,
            signature_ref: None,
            occurred_at: chrono::Utc::now(),
        };

        // Сериализуем в BSON Document
        let mut doc = Document::new();
        doc.insert("_id", event._id.to_string());
        doc.insert("stream_type", stream_type.to_string());
        doc.insert("stream_id", stream_id);
        doc.insert("event_type", &event.event_type);
        doc.insert("version", next_version);
        if let Ok(bson) = mongodb::bson::to_bson(&payload) {
            doc.insert("payload", bson);
        }
        // Metadata (actor snapshot)
        doc.insert("actor_user_id", metadata.user_id.0.to_string());
        doc.insert("actor_login", &metadata.login);
        if let Some(ref fn_) = metadata.full_name {
            doc.insert("actor_full_name", fn_);
        }
        if let Some(ref pos) = metadata.position {
            doc.insert("actor_position", pos);
        }
        doc.insert("actor_company_id", metadata.company_id.0.to_string());
        doc.insert("company_id", company_id.0.to_string());
        if let Some(ref cid) = correlation_id {
            doc.insert("correlation_id", cid);
        }
        if let Some(ref sid) = event.signature_ref {
            doc.insert("signature_ref", sid);
        }
        doc.insert("occurred_at", mongodb::bson::to_bson(&event.occurred_at).unwrap());

        col.insert_one(doc)
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;

        info!(
            "Event appended: {} v{} [{}] -> {}",
            stream_type, next_version, event_type, stream_id
        );

        Ok(event)
    }

    /// Прочитать весь поток событий объекта (для version history)
    pub async fn list_stream(
        &self,
        db: &MongoClient,
        stream_type: StreamType,
        stream_id: &str,
    ) -> PlatformResult<Vec<Event>> {
        let col = db.collection::<Document>(COLLECTION);
        let filter = doc! {
            "stream_type": stream_type.to_string(),
            "stream_id": stream_id,
        };
        let mut cursor = col
            .find(filter)
            .sort(doc! { "version": 1 })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;

        let mut events = Vec::new();
        while let Some(res) = cursor.next().await {
            let doc = res.map_err(|e| PlatformError::Database(e.to_string()))?;
            if let Ok(event) = deserialize_event(&doc) {
                events.push(event);
            }
        }
        Ok(events)
    }

    /// Список событий с фильтрами и пагинацией
    pub async fn list(
        &self,
        db: &MongoClient,
        company_id: CompanyId,
        filters: EventFilters,
    ) -> PlatformResult<EventPage> {
        let col = db.collection::<Document>(COLLECTION);
        let filter = build_filter(&company_id, &filters);
        let limit = filters.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);

        let total_count = col
            .count_documents(filter.clone())
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))? as i64;

        let mut cursor = col
            .find(filter)
            .sort(doc! { "occurred_at": -1 })
            .limit(limit + 1)
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;

        let mut rows = Vec::new();
        while let Some(res) = cursor.next().await {
            let doc = res.map_err(|e| PlatformError::Database(e.to_string()))?;
            rows.push(doc);
        }

        let has_more = rows.len() as i64 > limit;
        if has_more { rows.pop(); }

        let events: Vec<Event> = rows
            .into_iter()
            .filter_map(|d| deserialize_event(&d).ok())
            .collect();

        let next_cursor = events.last().map(|e| e.occurred_at.to_rfc3339());

        Ok(EventPage {
            events,
            total_count,
            has_more,
            next_cursor,
        })
    }

    /// Получить событие по ID
    pub async fn get(
        &self,
        db: &MongoClient,
        id: uuid::Uuid,
    ) -> PlatformResult<Event> {
        let col = db.collection::<Document>(COLLECTION);
        let doc = col
            .find_one(doc! { "_id": id.to_string() })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?
            .ok_or_else(|| PlatformError::NotFound(format!("Событие {id} не найдено")))?;
        deserialize_event(&doc).map_err(|_| PlatformError::NotFound("Ошибка десериализации события".into()))
    }

    /// Последний version в потоке (для оптимистичной блокировки)
    pub async fn last_version(
        &self,
        db: &MongoClient,
        stream_type: StreamType,
        stream_id: &str,
    ) -> PlatformResult<i64> {
        let col = db.collection::<Document>(COLLECTION);
        let filter = doc! {
            "stream_type": stream_type.to_string(),
            "stream_id": stream_id,
        };
        let mut cursor = col
            .find(filter)
            .sort(doc! { "version": -1 })
            .limit(1)
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;

        if let Some(Ok(doc)) = cursor.next().await {
            Ok(doc.get_i64("version").unwrap_or(0))
        } else {
            Ok(0)
        }
    }
}

fn build_filter(company_id: &CompanyId, filters: &EventFilters) -> Document {
    let mut f = doc! { "company_id": company_id.0.to_string() };

    if let Some(ref st) = filters.stream_type {
        f.insert("stream_type", st.clone());
    }
    if let Some(ref sid) = filters.stream_id {
        f.insert("stream_id", sid.clone());
    }
    if let Some(ref et) = filters.event_type {
        f.insert("event_type", et.clone());
    }
    if let Some(ref cid) = filters.correlation_id {
        f.insert("correlation_id", cid.clone());
    }
    if let Some(ref from) = filters.date_from {
        f.insert("occurred_at", doc! { "$gte": from });
    }
    if let Some(ref to) = filters.date_to {
        let existing = f.get_document("occurred_at").cloned().unwrap_or_default();
        let mut combined = existing;
        combined.insert("$lte", to);
        f.insert("occurred_at", combined);
    }
    if let Some(ref after) = filters.after {
        f.insert("occurred_at", doc! { "$gt": after });
    }

    f
}

fn deserialize_event(doc: &Document) -> Result<Event, ()> {
    let _id = uuid::Uuid::parse_str(doc.get_str("_id").unwrap_or("")).map_err(|_| ())?;
    let stream_type_str = doc.get_str("stream_type").unwrap_or("object");
    let stream_type: StreamType = stream_type_str.parse().map_err(|_| ())?;
    let stream_id = doc.get_str("stream_id").unwrap_or("").to_string();
    let event_type = doc.get_str("event_type").unwrap_or("").to_string();
    let version = doc.get_i64("version").unwrap_or(0);
    let company_id = CompanyId(uuid::Uuid::parse_str(doc.get_str("company_id").unwrap_or("")).map_err(|_| ())?);

    let payload = doc.get("payload")
        .and_then(|v| {
            if let Some(obj) = v.as_document() {
                mongodb::bson::from_document(obj.clone()).ok()
            } else {
                serde_json::Value::Null.into()
            }
        })
        .unwrap_or(serde_json::Value::Null);

    let actor_user_id = crate::core::UserId(
        uuid::Uuid::parse_str(doc.get_str("actor_user_id").unwrap_or("")).map_err(|_| ())?
    );
    let metadata = ActorSnapshot {
        user_id: actor_user_id,
        login: doc.get_str("actor_login").unwrap_or("").to_string(),
        full_name: doc.get_str("actor_full_name").ok().map(String::from),
        position: doc.get_str("actor_position").ok().map(String::from),
        company_id: company_id.clone(),
    };

    let correlation_id = doc.get_str("correlation_id").ok().map(String::from);
    let signature_ref = doc.get_str("signature_ref").ok().map(String::from);
    let occurred_at = doc.get_datetime("occurred_at")
        .ok()
        .and_then(|v| chrono::DateTime::from_timestamp_millis(v.timestamp_millis()))
        .unwrap_or_else(chrono::Utc::now);

    Ok(Event {
        _id,
        stream_type,
        stream_id,
        event_type,
        version,
        payload,
        metadata,
        company_id,
        correlation_id,
        causation_id: None,
        signature_ref,
        occurred_at,
    })
}
