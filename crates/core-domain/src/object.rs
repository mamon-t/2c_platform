use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::DomainError;
use crate::metadata::{EntityField, EntityState, FieldType};
use crate::types::{AggregateId, Version};

/// Kind of a business object as declared by its entity type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectKind {
    Document,
    Catalog,
    Register,
    Task,
    Contract,
    Project,
    Setting,
    Custom,
}

/// Universal business object stored in the `objects` collection.
/// A document is an object with `kind == Document`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Object {
    pub id: AggregateId,
    /// Reference to the object's entity type descriptor (`entity_types`).
    pub entity_type: String,
    pub kind: ObjectKind,
    pub company_id: String,
    /// Current state code from the type's state machine.
    pub state: String,
    /// User-provided field values.
    pub data: serde_json::Value,
    /// Computed values (formula fields, balances).
    pub computed: serde_json::Value,
    /// Document number; unique per entity type and company, assigned on post.
    pub number: Option<String>,
    pub date: Option<NaiveDate>,
    pub parent_id: Option<AggregateId>,
    /// Incremented on every write to enable OCC.
    pub version: Version,
    pub created_by: String,
    pub updated_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Object {
    /// Whether this object is a document (gets a number, has a posting flow).
    pub fn is_document(&self) -> bool {
        self.kind == ObjectKind::Document
    }

    /// Validates user-provided `data` against the entity type's field
    /// declarations: required fields, primitive types, enum options and
    /// reference targets. Formula and computed fields are excluded (they are
    /// produced by the system, not the user).
    ///
    /// # Errors
    ///
    /// Returns `DomainError::ValidationError` for the first violated rule.
    pub fn validate(
        &self,
        fields: &[EntityField],
        states: &[EntityState],
    ) -> Result<(), DomainError> {
        let data = self
            .data
            .as_object()
            .ok_or_else(|| DomainError::ValidationError("data должен быть объектом".to_string()))?;

        if !states.is_empty() && !states.iter().any(|s| s.code == self.state) {
            return Err(DomainError::ValidationError(format!(
                "Состояние '{}' не существует у типа '{}'",
                self.state, self.entity_type
            )));
        }

        for field in fields {
            if matches!(field.data_type, FieldType::Formula | FieldType::Computed) {
                continue;
            }
            let value = match data.get(&field.code) {
                None | Some(serde_json::Value::Null) => None,
                Some(v) => Some(v),
            };
            match value {
                None if field.required => {
                    return Err(DomainError::ValidationError(format!(
                        "Поле '{}' ({}): обязательное",
                        field.code, field.label
                    )));
                }
                None => continue,
                Some(v) => validate_value(field, v)?,
            }
        }
        Ok(())
    }
}

fn validate_value(field: &EntityField, value: &serde_json::Value) -> Result<(), DomainError> {
    let rule: Result<(), String> = (|| -> Result<(), String> {
        match field.data_type {
        FieldType::String | FieldType::Text | FieldType::File | FieldType::User
        | FieldType::Company => value
            .as_str()
            .map(|_| ())
            .ok_or_else(|| "ожидается строка".to_string()),
        FieldType::Integer => value
            .as_i64()
            .map(|_| ())
            .ok_or_else(|| "ожидается целое число".to_string()),
        FieldType::Money => value
            .as_f64()
            .or_else(|| value.as_i64().map(|v| v as f64))
            .map(|_| ())
            .ok_or_else(|| "ожидается число".to_string()),
        FieldType::Date => {
            let raw = value
                .as_str()
                .ok_or_else(|| "ожидается дата (YYYY-MM-DD)".to_string())?;
            NaiveDate::parse_from_str(raw, "%Y-%m-%d")
                .map(|_| ())
                .map_err(|_| format!("некорректная дата '{raw}'"))
        }
        FieldType::Datetime => {
            let raw = value
                .as_str()
                .ok_or_else(|| "ожидается дата-время (RFC3339)".to_string())?;
            DateTime::parse_from_rfc3339(raw)
                .map(|_| ())
                .map_err(|_| format!("некорректная дата-время '{raw}'"))
        }
        FieldType::Boolean => value
            .as_bool()
            .map(|_| ())
            .ok_or_else(|| "ожидается булево значение".to_string()),
        FieldType::Enum => {
            let raw = value
                .as_str()
                .ok_or_else(|| "ожидается строковое значение".to_string())?;
            let allowed = field
                .options
                .as_array()
                .ok_or_else(|| format!("поле '{}': options не заданы", field.code))?;
            let allowed: Vec<&str> = allowed
                .iter()
                .filter_map(|v| v.as_str())
                .collect();
            if allowed.contains(&raw) {
                Ok(())
            } else {
                Err(format!(
                    "значение '{raw}' не входит в {:?}",
                    allowed
                ))
            }
        }
        FieldType::Reference => {
            let raw = value
                .as_str()
                .ok_or_else(|| "ожидается UUID ссылки".to_string())?;
            Uuid::parse_str(raw)
                .map(|_| ())
                .map_err(|_| format!("некорректный UUID ссылки '{raw}'"))
        }
        FieldType::Array | FieldType::Table => value
            .as_array()
            .map(|_| ())
            .ok_or_else(|| "ожидается массив".to_string()),
        FieldType::Json => value
            .as_object()
            .map(|_| ())
            .ok_or_else(|| "ожидается объект JSON".to_string()),
        FieldType::Formula | FieldType::Computed => Ok(()),
        }
    })();
    rule.map_err(|detail| {
        DomainError::ValidationError(format!(
            "Поле '{}' ({}): {detail}",
            field.code, field.label
        ))
    })
}

/// Immutable history record of an object's version (criterion 5 of the spec):
/// the `data`/`state` as of `version`, captured at write time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectSnapshot {
    pub id: Uuid,
    pub object_id: Uuid,
    pub version: Version,
    /// Snapshot of `data` for this version.
    pub data: serde_json::Value,
    pub state: String,
    /// Login of the author taken from the actor snapshot.
    pub changed_by: String,
    pub changed_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn base_object() -> Object {
        Object {
            id: Uuid::new_v4(),
            entity_type: "invoice".to_string(),
            kind: ObjectKind::Document,
            company_id: String::new(),
            state: "draft".to_string(),
            data: json!({"sum": 1000}),
            computed: json!({}),
            number: None,
            date: None,
            parent_id: None,
            version: 1,
            created_by: "system".to_string(),
            updated_by: "system".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn field(code: &str, data_type: FieldType, required: bool, options: serde_json::Value) -> EntityField {
        EntityField {
            id: Uuid::new_v4(),
            entity_type: "invoice".to_string(),
            code: code.to_string(),
            label: code.to_string(),
            data_type,
            required,
            is_unique: false,
            is_indexed: false,
            options,
            is_system: false,
            order: 1,
        }
    }

    fn states() -> Vec<EntityState> {
        vec![EntityState {
            id: Uuid::new_v4(),
            entity_type: "invoice".to_string(),
            code: "draft".to_string(),
            label: "Черновик".to_string(),
            color: None,
            is_initial: true,
            is_final: false,
        }]
    }

    #[test]
    fn is_document_matches_kind() {
        let mut obj = base_object();
        assert!(obj.is_document());
        obj.kind = ObjectKind::Catalog;
        assert!(!obj.is_document());
    }

    #[test]
    fn validates_valid_data() {
        let fields = vec![
            field("sum", FieldType::Money, true, json!({})),
            field("status", FieldType::Enum, false, json!(["draft", "posted"])),
        ];
        let obj = base_object();
        obj.validate(&fields, &states()).unwrap();
    }

    #[test]
    fn missing_required_field_rejected() {
        let fields = vec![field("customer", FieldType::Reference, true, json!({}))];
        let obj = base_object();
        let err = obj.validate(&fields, &states()).unwrap_err();
        assert!(matches!(err, DomainError::ValidationError(_)));
    }

    #[test]
    fn wrong_type_rejected() {
        let fields = vec![field("sum", FieldType::Integer, true, json!({}))];
        let mut obj = base_object();
        obj.data = json!({ "sum": "abc" });
        let err = obj.validate(&fields, &states()).unwrap_err();
        assert!(matches!(err, DomainError::ValidationError(_)));
    }

    #[test]
    fn invalid_enum_value_rejected() {
        let fields = vec![field("status", FieldType::Enum, true, json!(["draft", "posted"]))];
        let mut obj = base_object();
        obj.data = json!({ "status": "deleted" });
        let err = obj.validate(&fields, &states()).unwrap_err();
        assert!(matches!(err, DomainError::ValidationError(_)));
    }

    #[test]
    fn invalid_reference_rejected() {
        let fields = vec![field("customer", FieldType::Reference, true, json!({}))];
        let mut obj = base_object();
        obj.data = json!({ "customer": "not-a-uuid" });
        let err = obj.validate(&fields, &states()).unwrap_err();
        assert!(matches!(err, DomainError::ValidationError(_)));
    }

    #[test]
    fn unknown_state_rejected() {
        let mut obj = base_object();
        obj.state = "reviewed".to_string();
        let err = obj.validate(&[], &states()).unwrap_err();
        assert!(matches!(err, DomainError::ValidationError(_)));
    }

    #[test]
    fn formula_and_computed_fields_are_ignored() {
        let fields = vec![
            field("total", FieldType::Formula, true, json!({})),
            field("balance", FieldType::Computed, true, json!({})),
        ];
        let obj = base_object();
        obj.validate(&fields, &states()).unwrap();
    }

    #[test]
    fn snapshot_serde_round_trip() {
        let snapshot = ObjectSnapshot {
            id: Uuid::new_v4(),
            object_id: Uuid::new_v4(),
            version: 2,
            data: json!({"sum": 500}),
            state: "draft".to_string(),
            changed_by: "system".to_string(),
            changed_at: Utc::now(),
        };
        let json = serde_json::to_value(&snapshot).unwrap();
        let back: ObjectSnapshot = serde_json::from_value(json).unwrap();
        assert_eq!(back.version, 2);
        assert_eq!(back.state, "draft");
    }
}