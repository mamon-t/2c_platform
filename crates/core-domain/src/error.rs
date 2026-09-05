use crate::types::Version;

/// Domain-level errors mapped to `RpcMessage::Error` codes by the transport
/// layer. Messages are user-facing and therefore written in Russian.
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("Объект не найден: {0}")]
    NotFound(String),

    #[error("Конфликт версий: ожидалась {expected}, актуальная {actual}")]
    VersionConflict { expected: Version, actual: Version },

    #[error("Невалидные данные: {0}")]
    ValidationError(String),

    #[error("Недостаточно прав: {0}")]
    PermissionDenied(String),

    #[error("Ошибка хранилища: {0}")]
    Storage(String),
}

impl DomainError {
    /// Machine-readable code consistent with `RpcMessage::Error.code`.
    pub fn code(&self) -> &'static str {
        match self {
            DomainError::NotFound(_) => "NOT_FOUND_ERROR",
            DomainError::VersionConflict { .. } => "CONFLICT_ERROR",
            DomainError::ValidationError(_) => "VALIDATION_ERROR",
            DomainError::PermissionDenied(_) => "PERMISSION_ERROR",
            DomainError::Storage(_) => "STORAGE_ERROR",
        }
    }
}