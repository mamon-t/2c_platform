use core_domain::error::DomainError;
use core_domain::event::{Event, StreamType};
use core_domain::object::Object;
use core_domain::types::{AggregateId, Version};
use serde_json::Value;
use std::future::Future;

/// Append-only storage of events, the Pipe in the "Pipe and Board" concept.
/// Implemented by `core-infrastructure` on top of the `events` collection.
pub trait EventStore: Send + Sync {
    /// Persists events atomically, in order, keys-by-stream. Must be idempotent
    /// per event ID so that a retried batch does not duplicate entries.
    fn append(&self, events: &[Event])
        -> impl Future<Output = Result<(), DomainError>> + Send;

    /// Loads the full history of a stream, ordered by `version`.
    fn read_stream(
        &self,
        stream_type: StreamType,
        stream_id: &str,
    ) -> impl Future<Output = Result<Vec<Event>, DomainError>> + Send;
}

/// Storage of materialized objects, the Board. Enables OCC through `version`.
pub trait ObjectRepository: Send + Sync {
    /// Fetches an object together with its current version for optimistic
    /// concurrency checks.
    ///
    /// # Errors
    ///
    /// Returns `DomainError::NotFound` when the object does not exist.
    fn get_with_version(
        &self,
        id: &AggregateId,
    ) -> impl Future<Output = Result<(Object, Version), DomainError>> + Send;
}

/// Host capable of executing a WASM module action and returning its result.
pub trait WasmHost: Send + Sync {
    /// Executes `action` of a module. The host enforces capabilities and
    /// resource limits declared by the module manifest.
    fn execute_module(
        &self,
        module_code: &str,
        action: &str,
        payload: Value,
    ) -> impl Future<Output = Result<Value, DomainError>> + Send;
}