//! Транспортный слой: конверт RpcMessage, REST- и WebSocket-эндпоинты поверх Axum.

pub mod error_mapping;
pub mod idempotency;
pub mod routes;
pub mod rpc_message;

pub use idempotency::{IdempotencyKey, IdempotencyStore};
pub use routes::{ApiState, router};
pub use rpc_message::RpcMessage;