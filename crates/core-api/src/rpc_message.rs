//! Конверт `RpcMessage` — единый формат обмена клиента и сервера (ТЗ v3.1, §10).
//!
//! Тип-тег `type` задаёт разновидность сообщения; `rename_all = "snake_case"` —
//! значения `Command`/`Query`/`EventBatch`/`Response`/`Error`/`ServerPush`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RpcMessage {
    Command {
        id: String,
        module: String,
        action: String,
        payload: Value,
    },
    Query {
        id: String,
        module: String,
        action: String,
        payload: Value,
    },
    EventBatch {
        id: String,
        module: String,
        events: Vec<Value>,
    },
    Response {
        id: String,
        payload: Value,
    },
    Error {
        id: String,
        code: String,
        message: String,
        details: Option<Value>,
    },
    ServerPush {
        id: String,
        module: String,
        event_type: String,
        payload: Value,
    },
}