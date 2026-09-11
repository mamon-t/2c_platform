//! Транспортный слой: конверт RpcMessage, REST- и WebSocket-эндпоинты поверх Axum.

pub mod error_mapping;
pub mod idempotency;
pub mod push_hub;
pub mod routes;
pub mod rpc_message;
pub mod token;
pub mod ws;

pub use idempotency::{IdempotencyKey, IdempotencyStore};
pub use push_hub::PushHub;
pub use routes::{ApiState, process, router};
pub use rpc_message::RpcMessage;
pub use token::{JwtConfig, JwtTokenManager};
pub use ws::{ws_handler, WsQuery};