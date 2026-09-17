use crate::types::Version;

/// Ошибки доменного уровня, сопоставляемые транспортным слоем с кодами
/// `RpcMessage::Error`. Сообщения предназначены для пользователя и потому написаны на русском.
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("Объект не найден: {0}")]
    NotFound(String),

    #[error("Конфликт версий: ожидалась {expected}, актуальная {actual}")]
    VersionConflict { expected: Version, actual: Version },

    #[error("Невалидные данные: {0}")]
    ValidationError(String),

    /// Ошибка выполнения/компиляции скрипта с позицией в исходнике (line/column).
    #[error("Ошибка скрипта: {message}")]
    ScriptFailure {
        message: String,
        line: Option<u32>,
        column: Option<u32>,
    },

    #[error("Недостаточно прав: {0}")]
    PermissionDenied(String),

    #[error("Ошибка хранилища: {0}")]
    Storage(String),
}

impl DomainError {
    /// Машиночитаемый код, согласованный с `RpcMessage::Error.code`.
    pub fn code(&self) -> &'static str {
        match self {
            DomainError::NotFound(_) => "NOT_FOUND_ERROR",
            DomainError::VersionConflict { .. } => "CONFLICT_ERROR",
            DomainError::ValidationError(_) => "VALIDATION_ERROR",
            DomainError::ScriptFailure { .. } => "VALIDATION_ERROR",
            DomainError::PermissionDenied(_) => "PERMISSION_ERROR",
            DomainError::Storage(_) => "STORAGE_ERROR",
        }
    }
}