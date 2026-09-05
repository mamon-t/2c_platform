use serde::{Deserialize, Serialize};

/// An intention to change the state. It may be rejected and is never persisted.
/// Mirrors `RpcMessage::Command` from the transport layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    /// Correlation ID used for idempotent delivery and tracking.
    pub id: String,
    /// Name of the WASM module that owns the action, e.g. `"warehouse"`.
    pub module: String,
    /// Name of the action, e.g. `"post_document"`.
    pub action: String,
    /// Command arguments.
    pub payload: serde_json::Value,
}