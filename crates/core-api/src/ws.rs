//! WebSocket-транспорт RpcMessage (ТЗ v3.1, §10): `GET /ws`.
//!
//! Дуплексный RPC поверх WebSocket: клиенты отправляют `Command`/`Query`/`EventBatch`
//! текстовыми JSON-фреймами и получают `Response`/`Error` на каждый запрос,
//! а также принимают `ServerPush` от хаба уведомлений.

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::response::Response;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::{broadcast, mpsc};

use crate::routes::{ApiState, process};
use crate::rpc_message::RpcMessage;

/// Параметры WebSocket-подключения.
#[derive(Debug, Deserialize, Default)]
pub struct WsQuery {
    /// JWT-токен аутентификации.
    pub token: Option<String>,
}

/// Эндпоинт апгрейда на WebSocket.
pub async fn ws_handler(
    State(state): State<ApiState>,
    ws: WebSocketUpgrade,
    Query(params): Query<WsQuery>,
) -> Response {
    let actor = params
        .token
        .as_deref()
        .and_then(|t| state.tokens.parse(t).ok())
        .unwrap_or_else(core_domain::event::ActorSnapshot::anonymous);
    ws.on_upgrade(move |socket| ws_session(state, socket, actor))
}

/// Логика одной WebSocket-сессии.
///
/// - Входящие текстовые фреймы парсятся как `RpcMessage` (Command/Query/EventBatch);
///   некорректные сообщения получают `Error` с кодом `VALIDATION_ERROR`.
/// - Ответы (`Response`/`Error`) и серверные уведомления (`ServerPush`) шлются
///   через канал исходящих сообщений.
async fn ws_session(state: ApiState, socket: WebSocket, actor: core_domain::event::ActorSnapshot) {
    let (socket_tx, mut socket_rx) = socket.split();
    let mut push_rx: broadcast::Receiver<RpcMessage> = state.pushes.subscribe();
    let (out_tx, mut out_rx) = mpsc::channel::<Message>(64);

    // Задача-писатель: выбирает между ServerPush и исходящими ответами.
    let writer = {
        let mut socket_tx = socket_tx;
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    msg = push_rx.recv() => {
                        match msg {
                            Ok(rpc) => {
                                if let Ok(text) = serde_json::to_string(&rpc) {
                                    if socket_tx.send(Message::Text(text.into())).await.is_err() {
                                        break;
                                    }
                                }
                            }
                            Err(broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(broadcast::error::RecvError::Closed) => break,
                        }
                    }
                    Some(msg) = out_rx.recv() => {
                        if socket_tx.send(msg).await.is_err() {
                            break;
                        }
                    }
                    else => break,
                }
            }
        })
    };

    // Читатель: обрабатывает входящие фреймы.
    while let Some(msg_result) = socket_rx.next().await {
        let msg = match msg_result {
            Ok(m) => m,
            Err(_) => break,
        };
        match msg {
            Message::Text(text) => {
                let response = match serde_json::from_str::<RpcMessage>(&text) {
                    Ok(rpc) => process(&state, &actor, rpc).await,
                    Err(e) => RpcMessage::Error {
                        id: String::new(),
                        code: "VALIDATION_ERROR".to_string(),
                        message: format!("некорректный RpcMessage: {e}"),
                        details: None,
                    },
                };
                if let Ok(text) = serde_json::to_string(&response) {
                    let _ = out_tx.send(Message::Text(text.into())).await;
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    drop(out_tx);
    let _ = writer.await;
}