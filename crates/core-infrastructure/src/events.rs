//! Общие хелперы для сохранения доменных событий в SurrealDB.

use core_domain::error::DomainError;
use core_domain::event::{Event, StreamType};
use serde_json::Value;
use std::future::Future;
use surrealdb::engine::any::Any;
use surrealdb::method::Transaction;
use surrealdb::Surreal;

/// Кодирует событие в его сохраняемую запись. Идентификатор записи равен
/// идентификатору события (`events/<uuid>`), что делает повторный append
/// идемпотентным: `upsert` перезаписывает ту же запись вместо создания дубля.
pub fn event_record(event: &Event) -> Result<Value, DomainError> {
    let mut value = serde_json::to_value(event)
        .map_err(|e| DomainError::Storage(format!("event encode: {e}")))?;
    value
        .as_object_mut()
        .ok_or_else(|| DomainError::Storage("event is not an object".into()))?
        .remove("id");
    Ok(value)
}

/// Добавляет события на клиентском подключении вне транзакции.
/// Ограничивается конвертацией по одному событию через `event_record`.
pub async fn append_events(db: &Surreal<Any>, events: &[Event]) -> Result<(), DomainError> {
    for event in events {
        let value = event_record(event)?;
        let id = format!("{}", event.id);
        let _: Option<surrealdb::types::Value> = db
            .upsert(("events", id))
            .content(value)
            .await
            .map_err(|e| DomainError::Storage(format!("append event: {e}")))?;
    }
    Ok(())
}

/// Присваивает событиям номера `version` по потокам, подсчитывая уже
/// сохранённые события внутри транзакции. События одного потока получают
/// последовательные версии в порядке их появления в `events`.
pub async fn assign_versions(
    txn: &Transaction<Any>,
    events: &mut [Event],
) -> Result<(), DomainError> {
    let mut streams: Vec<(StreamType, String)> = Vec::new();
    for event in events.iter() {
        let key = (event.stream_type, event.stream_id.clone());
        if !streams.contains(&key) {
            streams.push(key);
        }
    }
    for (stream_type, stream_id) in streams {
        let base = stream_base_version(txn, stream_type, &stream_id).await?;
        let mut next = base + 1;
        for event in events.iter_mut() {
            if event.stream_type == stream_type && event.stream_id == stream_id {
                event.version = next;
                next += 1;
            }
        }
    }
    Ok(())
}

async fn stream_base_version(
    txn: &Transaction<Any>,
    stream_type: StreamType,
    stream_id: &str,
) -> Result<u64, DomainError> {
    let mut response = txn
        .query(
            "SELECT count() AS total FROM events \
             WHERE stream_type = $st AND stream_id = $sid GROUP ALL",
        )
        .bind(("st", stream_type.as_str()))
        .bind(("sid", stream_id.to_string()))
        .await
        .map_err(|e| DomainError::Storage(format!("count stream: {e}")))?;
    let row: Option<Value> = response
        .take(0)
        .map_err(|e| DomainError::Storage(format!("count stream take: {e}")))?;
    Ok(row
        .and_then(|v| v.get("total").cloned())
        .and_then(|v| v.as_u64())
        .unwrap_or(0))
}

/// Сохраняет события внутри открытой транзакции `txn`.
pub async fn write_events(txn: &Transaction<Any>, events: &[Event]) -> Result<(), DomainError> {
    for event in events {
        let value = event_record(event)?;
        let id = format!("{}", event.id);
        let _: Option<surrealdb::types::Value> = txn
            .upsert(("events", id))
            .content(value)
            .await
            .map_err(|e| DomainError::Storage(format!("write event: {e}")))?;
    }
    Ok(())
}

/// Выполняет `f` внутри транзакции SurrealDB: фиксирует при успехе и
/// отменяет (откат) при ошибке, чтобы сессия никогда не оставалась в
/// подвешенном состоянии транзакции. Замыкание владеет `Transaction`
/// (использует её внутри по заимствованию) и должно вернуть её вместе
/// с результатом, который становится результатом функции.
pub async fn with_transaction<O, F, Fut>(db: &Surreal<Any>, f: F) -> Result<O, DomainError>
where
    F: FnOnce(Transaction<Any>) -> Fut,
    Fut: Future<Output = (Transaction<Any>, Result<O, DomainError>)>,
{
    let txn = db
        .clone()
        .begin()
        .await
        .map_err(|e| DomainError::Storage(format!("begin: {e}")))?;
    let (txn, outcome) = f(txn).await;
    match outcome {
        Ok(value) => {
            txn.commit()
                .await
                .map_err(|e| DomainError::Storage(format!("commit: {e}")))?;
            Ok(value)
        }
        Err(e) => {
            let _ = txn.cancel().await;
            Err(e)
        }
    }
}