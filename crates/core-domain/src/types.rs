use uuid::Uuid;

/// Уникальный идентификатор агрегата.
pub type AggregateId = Uuid;

/// Версия, используемая OCC (оптимистичной блокировкой).
pub type Version = u64;