use crate::error::DomainError;
use crate::event::Event;
use crate::types::Version;

/// Contract for event-sourced aggregates.
///
/// Implementations must store a domain state, the version of the last
/// committed event and a buffer of uncommitted events. `apply` validates the
/// sequence number of the event against OCC rules before dispatching it.
pub trait AggregateRoot: Send + Sync {
    /// Version of the last committed event.
    fn version(&self) -> Version;

    /// Events produced but not yet committed to the Event Store.
    fn pending_events(&self) -> &[Event];

    /// Validates the event's sequence number and dispatches it to `handle`.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::VersionConflict`] when the event version does not
    /// match the expected next version (`version + pending.len() + 1`).
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

    /// Domain-specific reaction to an applied event. On success the event must
    /// be appended to the pending buffer.
    fn handle(&mut self, event: Event) -> Result<(), DomainError>;

    /// Clears the pending buffer after a successful commit and advances the
    /// committed version by the number of cleared events.
    fn mark_committed(&mut self);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{EventMetadata, StreamType};
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
            metadata: EventMetadata {
                actor_user_id: "u1".to_string(),
                actor_login: "login".to_string(),
                actor_full_name: "Тест Тестов".to_string(),
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