//! Shared helpers for persisting domain events in SurrealDB.

use core_domain::error::DomainError;
use core_domain::event::{Event, StreamType};
use serde_json::Value;
use std::future::Future;
use surrealdb::engine::any::Any;
use surrealdb::method::Transaction;
use surrealdb::Surreal;

/// Encodes an event into its stored record. The record id equals the event id
/// (`events/<uuid>`), which makes a retried append idempotent: `upsert`
/// overwrites the same record instead of creating a duplicate.
pub fn event_record(event: &Event) -> Result<Value, DomainError> {
    let mut value = serde_json::to_value(event)
        .map_err(|e| DomainError::Storage(format!("event encode: {e}")))?;
    value
        .as_object_mut()
        .ok_or_else(|| DomainError::Storage("event is not an object".into()))?
        .remove("id");
    Ok(value)
}

/// Appends events on the client connection, outside of a transaction.
/// Bounded by a per-event `event_record` conversion.
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

/// Assigns per-stream `version` numbers to the events, counting already
/// persisted events inside the transaction. Events of the same stream receive
/// consecutive versions in the order they appear in `events`.
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

/// Persists events inside the open transaction `txn`.
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

/// Runs `f` inside a SurrealDB transaction, committing on success and
/// cancelling (rollback) on error so the session never stays in a dangling
/// transaction state. The closure owns the `Transaction` (it borrows it
/// internally) and must return it back alongside its outcome, which is
/// propagated as the function's result.
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