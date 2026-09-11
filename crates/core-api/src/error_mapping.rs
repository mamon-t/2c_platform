//! Сопоставление `DomainError` с `RpcMessage::Error` (ТЗ v3.1, §10).

use axum::http::StatusCode;
use core_domain::error::DomainError;
use serde_json::{json, Value};

/// Машиночитаемый код ошибки, согласованный с `DomainError::code()`.
pub fn error_code(err: &DomainError) -> &'static str {
    err.code()
}

/// HTTP-статус, соответствующий типу ошибки.
pub fn http_status(err: &DomainError) -> StatusCode {
    match err {
        DomainError::NotFound(_) => StatusCode::NOT_FOUND,
        DomainError::VersionConflict { .. } => StatusCode::CONFLICT,
        DomainError::ValidationError(_) => StatusCode::UNPROCESSABLE_ENTITY,
        DomainError::PermissionDenied(_) => StatusCode::FORBIDDEN,
        DomainError::Storage(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// HTTP-статус по машиночитаемому коду `RpcMessage::Error`.
pub fn http_status_for_code(code: &str) -> StatusCode {
    match code {
        "NOT_FOUND_ERROR" => StatusCode::NOT_FOUND,
        "CONFLICT_ERROR" => StatusCode::CONFLICT,
        "VALIDATION_ERROR" => StatusCode::UNPROCESSABLE_ENTITY,
        "PERMISSION_ERROR" => StatusCode::FORBIDDEN,
        "STORAGE_ERROR" => StatusCode::INTERNAL_SERVER_ERROR,
        _ => StatusCode::BAD_REQUEST,
    }
}

/// Дополнительные детали ошибки (заполняются для конфликта версий).
pub fn error_details(err: &DomainError) -> Option<Value> {
    match err {
        DomainError::VersionConflict { expected, actual } => Some(json!({
            "expected_version": expected.to_string(),
            "actual_version": actual.to_string(),
        })),
        _ => None,
    }
}