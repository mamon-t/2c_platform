// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

use crate::core::{FieldKind, PlatformError};
use crate::meta::EntityField;
use serde_json::Value;

/// Ошибка валидации конкретного поля
#[derive(Debug, Clone)]
pub struct FieldValidationError {
    pub field_code: String,
    pub field_name: String,
    pub message: String,
}

impl std::fmt::Display for FieldValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Поле '{}': {}", self.field_name, self.message)
    }
}

/// Валидация data объекта против метамодели entity_fields
pub fn validate_data(
    data: &Value,
    fields: &[EntityField],
    is_update: bool,
) -> Result<(), PlatformError> {
    let obj = match data {
        Value::Object(map) => map,
        _ => return Err(PlatformError::Validation("Data должен быть объектом".into())),
    };

    let mut errors: Vec<FieldValidationError> = Vec::new();

    for field in fields {
        let value = obj.get(&field.code);

        // Required check
        if field.is_required {
            match value {
                None | Some(Value::Null) => {
                    errors.push(FieldValidationError {
                        field_code: field.code.clone(),
                        field_name: field.name.clone(),
                        message: "обязательное поле".into(),
                    });
                    continue;
                }
                _ => {}
            }
        }

        // Readonly check (только при update)
        if is_update && field.is_readonly {
            if value.is_some() {
                errors.push(FieldValidationError {
                    field_code: field.code.clone(),
                    field_name: field.name.clone(),
                    message: "поле только для чтения".into(),
                });
                continue;
            }
        }

        // Если поле отсутствует или null — пропускаем (required уже проверили)
        let val = match value {
            Some(v) if !v.is_null() => v,
            _ => continue,
        };

        // Type + business-валидация
        if let Err(msg) = validate_field_value(val, &field) {
            errors.push(FieldValidationError {
                field_code: field.code.clone(),
                field_name: field.name.clone(),
                message: msg,
            });
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        let msg: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
        Err(PlatformError::Validation(msg.join("; ")))
    }
}

/// Валидация одного значения поля
fn validate_field_value(val: &Value, field: &EntityField) -> Result<(), String> {
    match &field.field_kind {
        FieldKind::String => {
            if !val.is_string() && !val.is_number() && !val.is_boolean() {
                return Err("ожидается строка или число".into());
            }
        }
        FieldKind::Text => {
            if !val.is_string() {
                return Err("ожидается строка".into());
            }
        }
        FieldKind::Integer => {
            if !val.is_i64() && !val.is_u64() {
                // Пытаемся распарсить строку как число
                if let Some(s) = val.as_str() {
                    s.parse::<i64>().map_err(|_| "ожидается целое число".to_string())?;
                } else if val.is_f64() {
                    // ok, целочисленное значение
                } else {
                    return Err("ожидается целое число".into());
                }
            }
        }
        FieldKind::Money => {
            if !val.is_number() && !val.is_string() {
                return Err("ожидается денежная сумма (число или строка)".into());
            }
            if let Some(s) = val.as_str() {
                s.parse::<f64>().map_err(|_| "невалидное денежное значение".to_string())?;
            }
        }
        FieldKind::Date => {
            if let Some(s) = val.as_str() {
                // Простая проверка формата YYYY-MM-DD
                if s.len() != 10 || !s.chars().nth(4).map(|c| c == '-').unwrap_or(false) ||
                   !s.chars().nth(7).map(|c| c == '-').unwrap_or(false) {
                    return Err("ожидается формат даты YYYY-MM-DD".into());
                }
                s.parse::<chrono::NaiveDate>()
                    .map_err(|_| "невалидная дата".to_string())?;
            } else {
                return Err("ожидается строка даты".into());
            }
        }
        FieldKind::DateTime => {
            if let Some(s) = val.as_str() {
                chrono::DateTime::parse_from_rfc3339(s)
                    .map_err(|_| "ожидается формат ISO 8601 (DateTime)".to_string())?;
            } else {
                return Err("ожидается строка даты/времени".into());
            }
        }
        FieldKind::Boolean => {
            if !val.is_boolean() {
                return Err("ожидается boolean".into());
            }
        }
        FieldKind::Enum => {
            if let Some(s) = val.as_str() {
                if let Some(ref allowed) = field.enum_values {
                    if !allowed.contains(&s.to_string()) {
                        return Err(format!("допустимые значения: {}", allowed.join(", ")));
                    }
                }
            } else {
                return Err("ожидается строковое значение перечисления".into());
            }
        }
        FieldKind::Reference => {
            // Reference — UUID или null
            if let Some(s) = val.as_str() {
                uuid::Uuid::parse_str(s)
                    .map_err(|_| "ожидается UUID ссылки".to_string())?;
            } else if !val.is_null() {
                return Err("ожидается UUID или null".into());
            }
        }
        FieldKind::Array => {
            if !val.is_array() {
                return Err("ожидается массив".into());
            }
        }
        FieldKind::Table => {
            if let Some(arr) = val.as_array() {
                for (i, item) in arr.iter().enumerate() {
                    if !item.is_object() {
                        return Err(format!("таблица: строка {} — ожидается объект", i + 1));
                    }
                }
            } else {
                return Err("ожидается массив объектов (таблица)".into());
            }
        }
        FieldKind::Json => {
            // Любой валидный JSON — уже валиден, если дошли сюда
        }
        FieldKind::File => {
            // null или объект с метаданными файла
            if !val.is_null() && !val.is_object() {
                return Err("ожидается объект файла или null".into());
            }
        }
        FieldKind::User => {
            if let Some(s) = val.as_str() {
                uuid::Uuid::parse_str(s)
                    .map_err(|_| "ожидается UUID пользователя".to_string())?;
            } else if !val.is_null() {
                return Err("ожидается UUID пользователя или null".into());
            }
        }
        FieldKind::Company => {
            if let Some(s) = val.as_str() {
                uuid::Uuid::parse_str(s)
                    .map_err(|_| "ожидается UUID компании".to_string())?;
            } else if !val.is_null() {
                return Err("ожидается UUID компании или null".into());
            }
        }
        FieldKind::Formula | FieldKind::Computed => {
            // Вычисляемые поля — пропускаем (на запись не приходят)
        }
    }
    Ok(())
}
