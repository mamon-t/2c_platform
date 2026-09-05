use uuid::Uuid;

/// Unique identifier of an aggregate.
pub type AggregateId = Uuid;

/// Version used by OCC (Optimistic Concurrency Control).
pub type Version = u64;