//! HTTP-транспорт RpcMessage (ТЗ v3.1, §10): `POST /rpc` поверх Axum.
//!
//! Принимает конверт `RpcMessage` (Command/Query/EventBatch) либо legacy-форму
//! `{"name": "...", "params": {...}}` (автоконвертация для обратной совместимости
//! с `/debug/command`). Команды модуля резолвятся как `plugin.{module}.{action}`;
//! `core`-команды исполняются под своим именем напрямую.

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use core_application::command_registry::{CommandExecutionCtx, CommandRegistry};
use core_application::ports::EventStore;
use core_domain::event::Event;
use serde_json::{Value, json};

use crate::error_mapping::{error_code, error_details, http_status};
use crate::idempotency::{IdempotencyKey, IdempotencyStore};
use crate::rpc_message::RpcMessage;

/// Состояние RPC-маршрутов.
#[derive(Clone)]
pub struct ApiState {
    pub registry: Arc<CommandRegistry>,
    pub store: Arc<dyn EventStore>,
    pub idempotency: Arc<IdempotencyStore>,
}

/// Собирает роутер `POST /rpc` с уже подставленным состоянием.
pub fn router(state: ApiState) -> Router {
    Router::new().route("/rpc", post(rpc_handler)).with_state(state)
}

/// Точка входа `POST /rpc`: парсит конверт (или legacy-форму), исполняет
/// команду и возвращает `Response`/`Error`.
pub async fn rpc_handler(
    State(state): State<ApiState>,
    Json(body): Json<Value>,
) -> (StatusCode, Json<RpcMessage>) {
    match body.get("type").and_then(Value::as_str) {
        Some(_) => {
            let parsed = serde_json::from_value::<RpcMessage>(body);
            match parsed {
                Ok(message) => handle_message(&state, message).await,
                Err(e) => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(RpcMessage::Error {
                        id: String::new(),
                        code: "VALIDATION_ERROR".to_string(),
                        message: format!("некорректный конверт RpcMessage: {e}"),
                        details: None,
                    }),
                ),
            }
        }
        None => handle_legacy(&state, &body).await,
    }
}

/// Обрабатывает сообщение в конверте RpcMessage.
async fn handle_message(state: &ApiState, message: RpcMessage) -> (StatusCode, Json<RpcMessage>) {
    match message {
        RpcMessage::Command { id, module, action, payload } => {
            run_command(state, id, module, action, payload, true).await
        }
        RpcMessage::Query { id, module, action, payload } => {
            run_command(state, id, module, action, payload, false).await
        }
        RpcMessage::EventBatch { id, module, events } => {
            if module != "core" {
                return (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(RpcMessage::Error {
                        id,
                        code: "VALIDATION_ERROR".to_string(),
                        message: "EventBatch принимается только для модуля 'core'".to_string(),
                        details: None,
                    }),
                );
            }
            let mut parsed = Vec::with_capacity(events.len());
            for event in &events {
                match serde_json::from_value::<Event>(event.clone()) {
                    Ok(e) => parsed.push(e),
                    Err(e) => {
                        return (
                            StatusCode::UNPROCESSABLE_ENTITY,
                            Json(RpcMessage::Error {
                                id,
                                code: "VALIDATION_ERROR".to_string(),
                                message: format!("некорректное событие в EventBatch: {e}"),
                                details: None,
                            }),
                        );
                    }
                }
            }
            match state.store.append(&parsed).await {
                Ok(()) => (
                    StatusCode::OK,
                    Json(RpcMessage::Response {
                        id,
                        payload: json!({"appended": parsed.len()}),
                    }),
                ),
                Err(e) => (
                    http_status(&e),
                    Json(RpcMessage::Error {
                        id,
                        code: error_code(&e).to_string(),
                        message: e.to_string(),
                        details: error_details(&e),
                    }),
                ),
            }
        }
        RpcMessage::ServerPush { id, .. } => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(RpcMessage::Error {
                id,
                code: "VALIDATION_ERROR".to_string(),
                message: "ServerPush — серверное уведомление, на вход не принимается".to_string(),
                details: None,
            }),
        ),
        RpcMessage::Response { id, .. } | RpcMessage::Error { id, .. } => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(RpcMessage::Error {
                id,
                code: "VALIDATION_ERROR".to_string(),
                message: "Response/Error — серверные сообщения, на вход не принимаются".to_string(),
                details: None,
            }),
        ),
    }
}

/// Исполняет команду и возвращает ответ с учётом идемпотентности.
async fn run_command(
    state: &ApiState,
    id: String,
    module: String,
    action: String,
    payload: Value,
    idempotent: bool,
) -> (StatusCode, Json<RpcMessage>) {
    let command_name = if module == "core" {
        action.clone()
    } else {
        format!("plugin.{module}.{action}")
    };
    let ctx = CommandExecutionCtx {
        actor: None,
        module_code: if module == "core" { None } else { Some(module) },
        entity_type: None,
    };

    let key = IdempotencyKey {
        actor_user_id: None,
        request_id: id.clone(),
    };
    if idempotent {
        if let Some(cached) = state.idempotency.get(&key).await {
            return (StatusCode::OK, Json(RpcMessage::Response { id, payload: cached }));
        }
    }

    match state.registry.execute_ctx(&command_name, payload, ctx).await {
        Ok(result) => {
            if idempotent {
                state.idempotency.put(key, result.clone()).await;
            }
            (StatusCode::OK, Json(RpcMessage::Response { id, payload: result }))
        }
        Err(e) => (
            http_status(&e),
            Json(RpcMessage::Error {
                id,
                code: error_code(&e).to_string(),
                message: e.to_string(),
                details: error_details(&e),
            }),
        ),
    }
}

/// Автоконвертация legacy-формы `{"name", "params", ...}` в Command.
async fn handle_legacy(state: &ApiState, body: &Value) -> (StatusCode, Json<RpcMessage>) {
    let name = match body.get("name").and_then(Value::as_str) {
        Some(name) => name.to_string(),
        None => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(RpcMessage::Error {
                    id: String::new(),
                    code: "VALIDATION_ERROR".to_string(),
                    message: "отсутствует поле 'name' или 'type'".to_string(),
                    details: None,
                }),
            );
        }
    };
    let id = body
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("legacy")
        .to_string();
    let payload = body.get("params").cloned().unwrap_or(Value::Null);
    let module = body
        .get("module")
        .and_then(Value::as_str)
        .unwrap_or("core")
        .to_string();
    run_command(state, id, module, name, payload, true).await
}