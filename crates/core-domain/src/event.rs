use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::Version;

/// Type of the stream an event belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamType {
    Object,
    User,
    Person,
    UserContact,
    UserProfile,
    UserCert,
    Company,
    Role,
    Module,
    Metadata,
}

impl StreamType {
    /// Canonical string form, consistent with the `snake_case` serialization
    /// used to store and query events in SurrealDB.
    pub fn as_str(&self) -> &'static str {
        match self {
            StreamType::Object => "object",
            StreamType::User => "user",
            StreamType::Person => "person",
            StreamType::UserContact => "user_contact",
            StreamType::UserProfile => "user_profile",
            StreamType::UserCert => "user_cert",
            StreamType::Company => "company",
            StreamType::Role => "role",
            StreamType::Module => "module",
            StreamType::Metadata => "metadata",
        }
    }
}

impl TryFrom<&str> for StreamType {
    type Error = ();

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "object" => Ok(StreamType::Object),
            "user" => Ok(StreamType::User),
            "person" => Ok(StreamType::Person),
            "user_contact" => Ok(StreamType::UserContact),
            "user_profile" => Ok(StreamType::UserProfile),
            "user_cert" => Ok(StreamType::UserCert),
            "company" => Ok(StreamType::Company),
            "role" => Ok(StreamType::Role),
            "module" => Ok(StreamType::Module),
            "metadata" => Ok(StreamType::Metadata),
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_type_round_trips_through_string() {
        for st in [
            StreamType::Object,
            StreamType::User,
            StreamType::Person,
            StreamType::UserContact,
            StreamType::UserProfile,
            StreamType::UserCert,
            StreamType::Company,
            StreamType::Role,
            StreamType::Module,
            StreamType::Metadata,
        ] {
            let s = st.as_str();
            assert_eq!(StreamType::try_from(s), Ok(st), "stream: {s}");
        }
        assert!(StreamType::try_from("bogus").is_err());
    }

    #[test]
    fn stream_type_serializes_snake_case() {
        assert_eq!(serde_json::to_value(StreamType::UserContact).unwrap(), "user_contact");
        assert_eq!(serde_json::to_value(StreamType::Company).unwrap(), "company");
    }

    #[test]
    fn stream_type_deserializes_snake_case() {
        assert_eq!(
            serde_json::from_str::<StreamType>("\"user_cert\"").unwrap(),
            StreamType::UserCert
        );
    }
}

/// A fact that has already happened and is written to the Event Store.
/// Stored in the `events` collection, an append-only journal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: Uuid,
    pub stream_type: StreamType,
    pub stream_id: String,
    pub event_type: String,
    pub version: Version,
    pub payload: serde_json::Value,
    pub metadata: ActorSnapshot,
    pub company_id: String,
    pub correlation_id: String,
    pub causation_id: Option<String>,
    pub occurred_at: DateTime<Utc>,
}

/// Snapshot of the actor who produced the event, kept for readable audit
/// history even if the person's name, position or employment later changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorSnapshot {
    pub user_id: Option<Uuid>, // None for the system actor
    pub login: String,
    pub full_name: String,
    pub position: Option<String>,
    pub company_id: Option<Uuid>,
    pub ip_address: Option<String>,
}

impl ActorSnapshot {
    /// System actor used for commands executed before authentication exists
    /// (Phase 2 bootstrap and debug flows).
    pub fn system() -> Self {
        Self {
            user_id: None,
            login: "system".to_string(),
            full_name: "Система".to_string(),
            position: None,
            company_id: None,
            ip_address: None,
        }
    }
}