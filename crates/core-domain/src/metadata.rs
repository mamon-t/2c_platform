use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use crate::object::ObjectKind as EntityKind;

/// Тип данных поля сущности, согласно разделу 7 ТЗ. Поля `formula`
/// вычисляются при чтении из других полей, `computed` берутся из модулей.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    String,
    Text,
    Integer,
    Money,
    Date,
    Datetime,
    Boolean,
    Enum,
    Reference,
    Array,
    Table,
    Json,
    File,
    User,
    Company,
    Formula,
    Computed,
}

/// Кардинальность связи сущности.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationKind {
    OneToOne,
    OneToMany,
    ManyToMany,
}

/// Поведение, применяемое к связанным объектам при удалении исходного объекта.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnDelete {
    Cascade,
    Restrict,
    SetNull,
}

/// Дескриптор типа бизнес-сущности (метатип), хранится в
/// `entity_types`. Уникальность обеспечивается по `(code, company_id)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityType {
    pub id: Uuid,
    /// Уникальный машиночитаемый код в рамках компании, например `"invoice"`.
    pub code: String,
    pub name: String,
    pub kind: EntityKind,
    /// Пусто для общеплатформенных типов, UUID компании для типов в рамках компании.
    pub company_id: String,
    /// Версия декларативной схемы (ensure-семантика, согласно разделу 9).
    pub metadata_version: u32,
    /// Системные типы не могут быть удалены или переименованы пользователями.
    pub is_system: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Декларация поля типа сущности, хранится в `entity_fields`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityField {
    pub id: Uuid,
    pub entity_type: String,
    /// Уникальный машиночитаемый код в рамках типа сущности, например `"number"`.
    pub code: String,
    pub label: String,
    pub data_type: FieldType,
    pub required: bool,
    pub is_unique: bool,
    pub is_indexed: bool,
    /// Варианты enum или целевая ссылка, в зависимости от `data_type`.
    pub options: serde_json::Value,
    pub is_system: bool,
    pub order: u32,
}

/// Состояние конечного автомата типа сущности, хранится в `entity_states`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityState {
    pub id: Uuid,
    pub entity_type: String,
    /// Уникальный код в рамках типа сущности, например `"draft"`, `"posted"`.
    pub code: String,
    pub label: String,
    pub color: Option<String>,
    pub is_initial: bool,
    pub is_final: bool,
}

/// Разрешённый переход между двумя состояниями типа сущности, хранится в
/// `entity_transitions`. Обе конечные точки должны существовать в `entity_states`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityTransition {
    pub id: Uuid,
    pub entity_type: String,
    pub code: String,
    pub label: String,
    pub from_state: String,
    pub to_state: String,
}

/// Декларативная UI-форма типа сущности, хранится в `entity_forms`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityForm {
    pub id: Uuid,
    pub entity_type: String,
    pub code: String,
    pub label: String,
    /// Метаданные разметки, потребляемые SDUI-рендерером (Фаза 10).
    pub layout: serde_json::Value,
}

/// Декларативное действие типа сущности, хранится в `entity_actions`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityAction {
    pub id: Uuid,
    pub entity_type: String,
    pub code: String,
    pub label: String,
    /// Имя обработчика; фактическая реализация появится вместе со слоем WASM.
    pub handler: String,
}

/// Связь с другим типом сущности, хранится в `entity_relations`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityRelation {
    pub id: Uuid,
    pub entity_type: String,
    pub code: String,
    /// `entity_types.code` ссылочного типа.
    pub target_type: String,
    pub kind: RelationKind,
    pub on_delete: OnDelete,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn base_type() -> EntityType {
        EntityType {
            id: Uuid::new_v4(),
            code: "invoice".to_string(),
            name: "Счёт".to_string(),
            kind: EntityKind::Document,
            company_id: String::new(),
            metadata_version: 1,
            is_system: false,
            created_at: Utc.timestamp_opt(0, 0).unwrap(),
            updated_at: Utc.timestamp_opt(0, 0).unwrap(),
        }
    }

    #[test]
    fn entity_type_serde_round_trip() {
        let json = serde_json::to_value(base_type()).unwrap();
        let back: EntityType = serde_json::from_value(json).unwrap();
        assert_eq!(back.code, "invoice");
        assert_eq!(back.kind, EntityKind::Document);
    }

    #[test]
    fn field_type_serializes_snake_case() {
        for (variant, expected) in [
            (FieldType::String, "string"),
            (FieldType::Money, "money"),
            (FieldType::Datetime, "datetime"),
            (FieldType::Formula, "formula"),
            (FieldType::Computed, "computed"),
        ] {
            assert_eq!(serde_json::to_value(variant).unwrap(), expected);
        }
        assert_eq!(
            serde_json::from_str::<FieldType>("\"reference\"").unwrap(),
            FieldType::Reference
        );
    }

    #[test]
    fn field_type_round_trips_all_variants() {
        for variant in [
            FieldType::String,
            FieldType::Text,
            FieldType::Integer,
            FieldType::Money,
            FieldType::Date,
            FieldType::Datetime,
            FieldType::Boolean,
            FieldType::Enum,
            FieldType::Reference,
            FieldType::Array,
            FieldType::Table,
            FieldType::Json,
            FieldType::File,
            FieldType::User,
            FieldType::Company,
            FieldType::Formula,
            FieldType::Computed,
        ] {
            let json = serde_json::to_value(variant).unwrap();
            assert_eq!(serde_json::from_value::<FieldType>(json).unwrap(), variant);
        }
    }

    #[test]
    fn relation_round_trip() {
        let relation = EntityRelation {
            id: Uuid::new_v4(),
            entity_type: "invoice".to_string(),
            code: "customer".to_string(),
            target_type: "counterparty".to_string(),
            kind: RelationKind::ManyToMany,
            on_delete: OnDelete::Restrict,
        };
        let json = serde_json::to_value(&relation).unwrap();
        let back: EntityRelation = serde_json::from_value(json).unwrap();
        assert_eq!(back.code, "customer");
        assert_eq!(back.kind, RelationKind::ManyToMany);
        assert_eq!(back.on_delete, OnDelete::Restrict);
    }

    #[test]
    fn field_serde_round_trip() {
        let field = EntityField {
            id: Uuid::new_v4(),
            entity_type: "invoice".to_string(),
            code: "sum".to_string(),
            label: "Сумма".to_string(),
            data_type: FieldType::Money,
            required: true,
            is_unique: false,
            is_indexed: true,
            options: serde_json::json!({}),
            is_system: false,
            order: 1,
        };
        let json = serde_json::to_value(&field).unwrap();
        let back: EntityField = serde_json::from_value(json).unwrap();
        assert_eq!(back.data_type, FieldType::Money);
    }
}