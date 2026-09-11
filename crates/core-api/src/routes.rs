//! HTTP-транспорт RpcMessage (ТЗ v3.1, §10): `POST /rpc` поверх Axum.
//!
//! Принимает конверт `RpcMessage` (Command/Query/EventBatch) либо legacy-форму
//! `{"name": "...", "params": {...}}` (автоконвертация для обратной совместимости
//! с `/debug/command`). Команды модуля резолвятся как `plugin.{module}.{action}`;
//! `core`-команды исполняются под своим именем напрямую.
//!
//! Обработка сообщений вынесена в [`process`], общий для HTTP (`/rpc`) и
//! WebSocket (`/ws`, подфаза 10c).

use std::sync::Arc;

use axum::extract::State;
use axum::http::header::AUTHORIZATION;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use core_application::command_registry::{CommandExecutionCtx, CommandRegistry};
use core_application::ports::{EventStore, TokenManager};
use core_domain::event::{ActorSnapshot, Event};
use serde_json::{Value, json};

use crate::error_mapping::{
    error_code, error_details, http_status_for_code,
};
use crate::idempotency::{IdempotencyKey, IdempotencyStore};
use crate::push_hub::PushHub;
use crate::rpc_message::RpcMessage;

/// Состояние RPC-маршрутов и WebSocket.
#[derive(Clone)]
pub struct ApiState {
    pub registry: Arc<CommandRegistry>,
    pub store: Arc<dyn EventStore>,
    pub idempotency: Arc<IdempotencyStore>,
    pub tokens: Arc<dyn TokenManager>,
    pub pushes: Arc<PushHub>,
}

/// Собирает роутер `POST /rpc` с уже подставленным состоянием.
pub fn router(state: ApiState) -> Router {
    Router::new().route("/rpc", post(rpc_handler)).with_state(state)
}

/// Точка входа `POST /rpc`: парсит конверт (или legacy-форму), исполняет
/// команду и возвращает `Response`/`Error` с HTTP-статусом.
pub async fn rpc_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> (StatusCode, Json<RpcMessage>) {
    let actor = resolve_actor(&state, &headers);
    match body.get("type").and_then(Value::as_str) {
        Some(_) => match serde_json::from_value::<RpcMessage>(body) {
            Ok(message) => respond(process(&state, &actor, message).await),
            Err(e) => respond(RpcMessage::Error {
                id: String::new(),
                code: "VALIDATION_ERROR".to_string(),
                message: format!("некорректный конверт RpcMessage: {e}"),
                details: None,
            }),
        },
        None => handle_legacy(&state, &actor, &body).await,
    }
}

/// Резолвит исполнителя из заголовка `Authorization: Bearer <jwt>`.
/// Без валидного токена возвращает анонимного актора, чтобы пайплайн RBAC
/// мог отказать ему в командах с `required_permission`.
fn resolve_actor(state: &ApiState, headers: &HeaderMap) -> ActorSnapshot {
    let token = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "));
    match token {
        Some(token) => state.tokens.parse(token).unwrap_or_else(|_| ActorSnapshot::anonymous()),
        None => ActorSnapshot::anonymous(),
    }
}

/// Исполняет любое входящее `RpcMessage` и возвращает `Response`/`Error`.
/// Общая логика для HTTP и WebSocket.
pub async fn process(state: &ApiState, actor: &ActorSnapshot, message: RpcMessage) -> RpcMessage {
    match message {
        RpcMessage::Command { id, module, action, payload } => {
            run_command(state, actor, id, module, action, payload, true).await
        }
        RpcMessage::Query { id, module, action, payload } => {
            run_command(state, actor, id, module, action, payload, false).await
        }
        RpcMessage::EventBatch { id, module, events } => {
            event_batch(state, id, module, events).await
        }
        RpcMessage::ServerPush { id, .. } => RpcMessage::Error {
            id,
            code: "VALIDATION_ERROR".to_string(),
            message: "ServerPush — серверное уведомление, на вход не принимается".to_string(),
            details: None,
        },
        RpcMessage::Response { id, .. } | RpcMessage::Error { id, .. } => RpcMessage::Error {
            id,
            code: "VALIDATION_ERROR".to_string(),
            message: "Response/Error — серверные сообщения, на вход не принимаются".to_string(),
            details: None,
        },
    }
}

/// Исполняет команду и возвращает ответ с учётом идемпотентности.
async fn run_command(
    state: &ApiState,
    actor: &ActorSnapshot,
    id: String,
    module: String,
    action: String,
    payload: Value,
    idempotent: bool,
) -> RpcMessage {
    let command_name = if module == "core" {
        action.clone()
    } else {
        format!("plugin.{module}.{action}")
    };
    let ctx = CommandExecutionCtx {
        actor: Some(actor.clone()),
        module_code: if module == "core" { None } else { Some(module) },
        entity_type: None,
    };

    let key = IdempotencyKey {
        actor_user_id: actor.user_id.map(|id| id.to_string()),
        request_id: id.clone(),
    };
    if idempotent {
        if let Some(cached) = state.idempotency.get(&key).await {
            return RpcMessage::Response { id, payload: cached };
        }
    }

    match state.registry.execute_ctx(&command_name, payload, ctx).await {
        Ok(result) => {
            if idempotent {
                state.idempotency.put(key, result.clone()).await;
            }
            RpcMessage::Response { id, payload: result }
        }
        Err(e) => RpcMessage::Error {
            id,
            code: error_code(&e).to_string(),
            message: e.to_string(),
            details: error_details(&e),
        },
    }
}

/// Применяет пачку событий к Трубе (event store).
async fn event_batch(
    state: &ApiState,
    id: String,
    module: String,
    events: Vec<Value>,
) -> RpcMessage {
    if module != "core" {
        return RpcMessage::Error {
            id,
            code: "VALIDATION_ERROR".to_string(),
            message: "EventBatch принимается только для модуля 'core'".to_string(),
            details: None,
        };
    }
    let mut parsed = Vec::with_capacity(events.len());
    for event in &events {
        match serde_json::from_value::<Event>(event.clone()) {
            Ok(e) => parsed.push(e),
            Err(e) => {
                return RpcMessage::Error {
                    id,
                    code: "VALIDATION_ERROR".to_string(),
                    message: format!("некорректное событие в EventBatch: {e}"),
                    details: None,
                };
            }
        }
    }
    match state.store.append(&parsed).await {
        Ok(()) => RpcMessage::Response {
            id,
            payload: json!({"appended": parsed.len()}),
        },
        Err(e) => RpcMessage::Error {
            id,
            code: error_code(&e).to_string(),
            message: e.to_string(),
            details: error_details(&e),
        },
    }
}

/// Автоконвертация legacy-формы `{"name", "params", ...}` в Command.
async fn handle_legacy(
    state: &ApiState,
    actor: &ActorSnapshot,
    body: &Value,
) -> (StatusCode, Json<RpcMessage>) {
    let name = match body.get("name").and_then(Value::as_str) {
        Some(name) => name.to_string(),
        None => {
            return respond(RpcMessage::Error {
                id: String::new(),
                code: "VALIDATION_ERROR".to_string(),
                message: "отсутствует поле 'name' или 'type'".to_string(),
                details: None,
            });
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
    respond(run_command(state, actor, id, module, name, payload, true).await)
}

/// Сопоставляет `Response`/`Error` с HTTP-статусом по коду ошибки.
fn respond(message: RpcMessage) -> (StatusCode, Json<RpcMessage>) {
    let status = match &message {
        RpcMessage::Response { .. } => StatusCode::OK,
        RpcMessage::Error { code, .. } => http_status_for_code(code),
        _ => StatusCode::OK,
    };
    (status, Json(message))
}