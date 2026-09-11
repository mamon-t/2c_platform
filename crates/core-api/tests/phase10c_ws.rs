//! Интеграционные приёмочные тесты подфазы 10c (раздел 10 ТЗ v3.1):
//! WebSocket-транспорт для Command/Query/EventBatch и серверные уведомления
//! `ServerPush` через `PushHub`.
//!
//! Тесты поднимают live-сервер Axum на ephemeral-порту и общаются с ним по
//! `ws://` через `tokio-tungstenite`.

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::routing::get;
use core_api::idempotency::IdempotencyStore;
use core_api::routes::ApiState;
use core_api::{JwtConfig, JwtTokenManager, PushHub, RpcMessage, ws_handler};
use core_application::command_registry::CommandRegistry;
use core_application::ports::{EventStore, TokenManager};
use core_domain::event::{ActorSnapshot, Event, StreamType};
use core_infrastructure::SurrealEventStore;
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use surrealdb::engine::any::Any;
use surrealdb::Surreal;
use tokio::net::TcpListener;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

const SECRET: &str = "test-secret";

async fn mem_db() -> Surreal<Any> {
    let db = surrealdb::engine::any::connect("mem://").await.unwrap();
    db.use_ns("test")
        .use_db(Uuid::new_v4().to_string())
        .await
        .unwrap();
    db
}

struct Env {
    store: Arc<SurrealEventStore>,
    registry: Arc<CommandRegistry>,
    idempotency: Arc<IdempotencyStore>,
    tokens: Arc<dyn TokenManager>,
    pushes: Arc<PushHub>,
}

async fn setup() -> Env {
    let db = mem_db().await;

    let store = Arc::new(SurrealEventStore::new(db.clone()));
    store.ensure_schema().await.unwrap();

    let registry = Arc::new(CommandRegistry::new());
    registry
        .register("core.sample.echo", |params: Value| async move { Ok(params) })
        .await;

    Env {
        store,
        registry,
        idempotency: IdempotencyStore::new(),
        tokens: Arc::new(JwtTokenManager::new(JwtConfig {
            secret: SECRET.to_string(),
            access_ttl: Duration::from_secs(600),
        })),
        pushes: PushHub::new(),
    }
}

fn api_state(env: &Env) -> ApiState {
    ApiState {
        registry: env.registry.clone(),
        store: env.store.clone(),
        idempotency: env.idempotency.clone(),
        tokens: env.tokens.clone(),
        pushes: env.pushes.clone(),
    }
}

/// Поднимает live-сервер на случайном порту и возвращает URL `ws://` + handle.
async fn spawn_server(env: &Env) -> (String, tokio::task::JoinHandle<()>) {
    let router = Router::new()
        .route("/ws", get(ws_handler).with_state(api_state(env)));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("ws://{addr}/ws");
    let handle = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (url, handle)
}

/// Открывает WebSocket-сессию и возвращает приёмник сообщений.
async fn connect(url: &str) -> tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
> {
    let (socket, _) = connect_async(url).await.unwrap();
    socket
}

/// Читает одно текстовое сообщение и десериализует его в `RpcMessage`.
async fn recv_rpc(
    ws: &mut (impl futures_util::Stream<
        Item = Result<Message, tokio_tungstenite::tungstenite::Error>,
    > + Unpin),
) -> RpcMessage {
    let frame = tokio::time::timeout(Duration::from_secs(5), ws.next())
        .await
        .expect("таймаут ожидания сообщения по WebSocket")
        .expect("канал WebSocket закрыт")
        .unwrap();
    match frame {
        Message::Text(text) => serde_json::from_str(&text).unwrap(),
        other => panic!("ожидалось Text-сообщение, получено: {other:?}"),
    }
}

#[tokio::test]
async fn echo_command_over_websocket_returns_response() {
    let env = setup().await;
    let (url, server) = spawn_server(&env).await;
    let mut ws = connect(&url).await;

    ws.send(Message::Text(
        json!({
            "type": "command",
            "id": "w1",
            "module": "core",
            "action": "core.sample.echo",
            "payload": {"v": 1}
        })
        .to_string(),
    ))
    .await
    .unwrap();

    let rpc = recv_rpc(&mut ws).await;
    assert_eq!(
        rpc,
        RpcMessage::Response {
            id: "w1".to_string(),
            payload: json!({"v": 1})
        }
    );

    let _ = ws.close(None).await;
    server.abort();
}

#[tokio::test]
async fn push_hub_delivers_server_push_to_ws_client() {
    let env = setup().await;
    let (url, server) = spawn_server(&env).await;

    // Два клиента подключаются заранее; пушим после установки обеих сессий.
    let _client_a = connect(&url).await;
    let mut client_b = connect(&url).await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    env.pushes.push("core", "company.created", json!({"code": "X"}));

    let rpc = recv_rpc(&mut client_b).await;
    match rpc {
        RpcMessage::ServerPush {
            id,
            module,
            event_type,
            payload,
        } => {
            assert!(!id.is_empty());
            assert_eq!(module, "core");
            assert_eq!(event_type, "company.created");
            assert_eq!(payload, json!({"code": "X"}));
        }
        other => panic!("ожидался ServerPush, получено: {other:?}"),
    }

    server.abort();
}

#[tokio::test]
async fn event_batch_over_websocket_appends_to_store() {
    let env = setup().await;
    let (url, server) = spawn_server(&env).await;
    let mut ws = connect(&url).await;

    let event = Event {
        id: Uuid::new_v4(),
        stream_type: StreamType::Object,
        stream_id: "obj-ws".to_string(),
        event_type: "object.created".to_string(),
        version: 0,
        payload: json!({"v": 1}),
        metadata: ActorSnapshot::system(),
        company_id: "comp1".to_string(),
        correlation_id: Uuid::new_v4().to_string(),
        causation_id: None,
        occurred_at: chrono::Utc::now(),
    };
    ws.send(Message::Text(
        json!({
            "type": "event_batch",
            "id": "eb-ws",
            "module": "core",
            "events": [event]
        })
        .to_string(),
    ))
    .await
    .unwrap();

    let rpc = recv_rpc(&mut ws).await;
    assert_eq!(
        rpc,
        RpcMessage::Response {
            id: "eb-ws".to_string(),
            payload: json!({"appended": 1})
        }
    );

    let stream = env
        .store
        .read_stream(StreamType::Object, "obj-ws")
        .await
        .unwrap();
    assert_eq!(stream.len(), 1);
    assert_eq!(stream[0].event_type, "object.created");

    let _ = ws.close(None).await;
    server.abort();
}