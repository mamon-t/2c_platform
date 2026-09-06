use crate::error::DomainError;
use crate::event::Event;
use crate::types::Version;

/// Контракт для агрегатов на основе событий.
///
/// Реализации должны хранить состояние домена, версию последнего
/// зафиксированного события и буфер незафиксированных событий. `apply` проверяет
/// порядковый номер события по правилам OCC перед его диспетчеризацией.
pub trait AggregateRoot: Send + Sync {
    /// Версия последнего зафиксированного события.
    fn version(&self) -> Version;

    /// События, созданные, но ещё не зафиксированные в хранилище событий.
    fn pending_events(&self) -> &[Event];

    /// Проверяет порядковый номер события и передаёт его в `handle`.
    ///
    /// # Ошибки
    ///
    /// Возвращает [`DomainError::VersionConflict`], когда версия события не
    /// совпадает с ожидаемой следующей версией (`version + pending.len() + 1`).
    fn apply(&mut self, event: Event) -> Result<(), DomainError> {
        let expected = self.version() + self.pending_events().len() as Version + 1;
        if event.version != expected {
            return Err(DomainError::VersionConflict {
                expected,
                actual: event.version,
            });
        }
        self.handle(event)
    }

    /// Специфическая реакция домена на применённое событие. При успехе событие должно
    /// быть добавлено в буфер ожидающих событий.
    fn handle(&mut self, event: Event) -> Result<(), DomainError>;

    /// Очищает буфер ожидающих событий после успешной фиксации и увеличивает
    /// зафиксированную версию на число очищенных событий.
    fn mark_committed(&mut self);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{ActorSnapshot, StreamType};
    use chrono::{DateTime, Utc};
    use serde_json::json;
    use uuid::Uuid;

    struct CounterAggregate {
        value: i64,
        version: Version,
        pending: Vec<Event>,
    }

    impl CounterAggregate {
        fn new() -> Self {
            Self {
                value: 0,
                version: 0,
                pending: Vec::new(),
            }
        }
    }

    impl AggregateRoot for CounterAggregate {
        fn version(&self) -> Version {
            self.version
        }

        fn pending_events(&self) -> &[Event] {
            &self.pending
        }

        fn handle(&mut self, event: Event) -> Result<(), DomainError> {
            if event.event_type == "counter.incremented" {
                self.value += 1;
            }
            self.pending.push(event);
            Ok(())
        }

        fn mark_committed(&mut self) {
            self.version += self.pending.len() as Version;
            self.pending.clear();
        }
    }

    fn make_event(version: Version) -> Event {
        Event {
            id: Uuid::new_v4(),
            stream_type: StreamType::Object,
            stream_id: "counter-1".to_string(),
            event_type: "counter.incremented".to_string(),
            version,
            payload: json!({}),
            metadata: ActorSnapshot {
                user_id: Some(Uuid::new_v4()),
                login: "login".to_string(),
                full_name: "Тест Тестов".to_string(),
                position: None,
                company_id: None,
                ip_address: None,
            },
            company_id: "c1".to_string(),
            correlation_id: "corr-1".to_string(),
            causation_id: None,
            occurred_at: DateTime::<Utc>::from_timestamp(0, 0).unwrap(),
        }
    }

    #[test]
    fn applying_events_in_order_advances_version() {
        let mut aggregate = CounterAggregate::new();

        aggregate.apply(make_event(1)).unwrap();
        aggregate.apply(make_event(2)).unwrap();
        assert_eq!(aggregate.value, 2);
        assert_eq!(aggregate.pending_events().len(), 2);

        aggregate.mark_committed();
        assert_eq!(aggregate.version, 2);
        assert!(aggregate.pending_events().is_empty());
    }

    #[test]
    fn out_of_order_event_is_rejected() {
        let mut aggregate = CounterAggregate::new();
        let err = aggregate.apply(make_event(5)).unwrap_err();

        match err {
            DomainError::VersionConflict { expected, actual } => {
                assert_eq!(expected, 1);
                assert_eq!(actual, 5);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}