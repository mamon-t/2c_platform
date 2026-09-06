use serde::{Deserialize, Serialize};

/// Намерение изменить состояние. Может быть отклонено и никогда не сохраняется.
/// Отражает `RpcMessage::Command` из транспортного слоя.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    /// Correlation ID, используемый для идемпотентной доставки и отслеживания.
    pub id: String,
    /// Имя WASM-модуля, которому принадлежит действие, например `"warehouse"`.
    pub module: String,
    /// Имя действия, например `"post_document"`.
    pub action: String,
    /// Аргументы команды.
    pub payload: serde_json::Value,
}