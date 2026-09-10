//! Интеграционные приёмочные тесты подфазы 10a (раздел 10 ТЗ v3.1):
//! транспортный конверт `RpcMessage` поверх Axum (`POST /rpc`).
//!
//! На мем-базе (`mem://`) проверяются: исполнение `core`-команд и
//! резолвинг `plugin.{module}.{action}`, идемпотентность по `request_id`,
//! маппинг `DomainError` в коды RpcMessage (включая `CONFLICT_ERROR` для OCC),
//! legacy-форма запроса и приём `EventBatch`.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use core_api::idempotency::IdempotencyStore;
use core_api::routes::ApiState;
use core_api::RpcMessage;
use core_api::router;
use core_application::command_registry::CommandRegistry;
use core_application::ports::{EventStore, ObjectRepository};
use core_domain::event::{ActorSnapshot, Event, StreamType};
use core_domain::object::{Object, ObjectKind};
use core_infrastructure::surreal_object_repository::SurrealObjectRepository;
use core_infrastructure::SurrealEventStore;
use serde_json::{Value, json};
use surrealdb::engine::any::Any;
use surrealdb::Surreal;
use tower::ServiceExt;
use uuid::Uuid;

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
    _objects: Arc<SurrealObjectRepository>,
}

async fn setup() -> Env {
    let db = mem_db().await;

    let store = Arc::new(SurrealEventStore::new(db.clone()));
    store.ensure_schema().await.unwrap();
    let objects = Arc::new(SurrealObjectRepository::new(db.clone()));
    objects.ensure_schema().await.unwrap();

    let registry = Arc::new(CommandRegistry::new());
    registry
        .register("core.sample.echo", |params: Value| async move { Ok(params) })
        .await;
    registry
        .register("plugin.hello.greet", |_: Value| async move {
            Ok(json!({"greeted": true}))
        })
        .await;

    // Идемпотентный счётчик: каждый реальный вызов инкрементирует счётчик.
    let counter = Arc::new(AtomicUsize::new(0));
    let c = counter.clone();
    registry
        .register("core.sample.counter", move |_: Value| {
            let c = c.clone();
            async move {
                let n = c.fetch_add(1, Ordering::SeqCst) + 1;
                Ok(json!({"count": n}))
            }
        })
        .await;

    // Объект: создаётся на версии 1, затем обновляется со «старой» версией 0 —
    // репозиторий возвращает `DomainError::VersionConflict`.
    let objects_conflict = objects.clone();
    registry
        .register("core.sample.conflict", move |_: Value| {
            let objects = objects_conflict.clone();
            async move {
                let now = chrono::Utc::now();
                let obj = Object {
                    id: Uuid::new_v4(),
                    entity_type: "sample".to_string(),
                    kind: ObjectKind::Custom,
                    company_id: "comp1".to_string(),
                    state: "draft".to_string(),
                    data: json!({}),
                    computed: json!({}),
                    number: None,
                    date: None,
                    parent_id: None,
                    version: 1,
                    created_by: "system".to_string(),
                    updated_by: "system".to_string(),
                    created_at: now,
                    updated_at: now,
                };
                objects.create(&obj, &[]).await?;
                let stale = Object {
                    version: 0,
                    ..obj.clone()
                };
                objects.update(&stale, &[]).await?;
                Ok(json!({}))
            }
        })
        .await;

    Env {
        store,
        registry,
        idempotency: IdempotencyStore::new(),
        _objects: objects,
    }
}

fn api_state(env: &Env) -> ApiState {
    ApiState {
        registry: env.registry.clone(),
        store: env.store.clone(),
        idempotency: env.idempotency.clone(),
    }
}

async fn post_rpc(env: &Env, body: Value) -> (StatusCode, RpcMessage) {
    let response = router(api_state(env))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/rpc")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let message = serde_json::from_slice(&bytes).unwrap();
    (status, message)
}

#[tokio::test]
async fn core_command_via_envelope_returns_response() {
    let env = setup().await;
    let (status, message) = post_rpc(
        &env,
        json!({
            "type": "command",
            "id": "r1",
            "module": "core",
            "action": "core.sample.echo",
            "payload": {"value": 42}
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        message,
        RpcMessage::Response {
            id: "r1".to_string(),
            payload: json!({"value": 42})
        }
    );
}

#[tokio::test]
async fn plugin_command_resolves_via_module_prefix() {
    let env = setup().await;
    let (status, message) = post_rpc(
        &env,
        json!({
            "type": "command",
            "id": "r2",
            "module": "hello",
            "action": "greet",
            "payload": {}
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        message,
        RpcMessage::Response {
            id: "r2".to_string(),
            payload: json!({"greeted": true})
        }
    );
}

#[tokio::test]
async fn idempotent_replay_returns_cached_result() {
    let env = setup().await;
    let body = json!({
        "type": "command",
        "id": "idem-1",
        "module": "core",
        "action": "core.sample.counter",
        "payload": {}
    });
    let (status, first) = post_rpc(&env, body.clone()).await;
    assert_eq!(status, StatusCode::OK);
    let (status, second) = post_rpc(&env, body).await;
    assert_eq!(status, StatusCode::OK);
    // Второй вызов — дубль: отдан кэш, обработчик не выполнялся повторно.
    assert_eq!(
        RpcMessage::Response {
            id: "idem-1".to_string(),
            payload: json!({"count": 1})
        },
        first
    );
    assert_eq!(first, second);
}

#[tokio::test]
async fn different_request_id_executes_again() {
    let env = setup().await;
    let counter = |id: &str| {
        json!({
            "type": "command",
            "id": id,
            "module": "core",
            "action": "core.sample.counter",
            "payload": {}
        })
    };
    let (_, first) = post_rpc(&env, counter("a")).await;
    let (_, second) = post_rpc(&env, counter("b")).await;
    assert_eq!(first, RpcMessage::Response { id: "a".to_string(), payload: json!({"count": 1}) });
    assert_eq!(second, RpcMessage::Response { id: "b".to_string(), payload: json!({"count": 2}) });
}

#[tokio::test]
async fn version_conflict_maps_to_conflict_error() {
    let env = setup().await;
    let (status, message) = post_rpc(
        &env,
        json!({
            "type": "command",
            "id": "conf",
            "module": "core",
            "action": "core.sample.conflict",
            "payload": {}
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    match message {
        RpcMessage::Error { id, code, details, .. } => {
            assert_eq!(id, "conf");
            assert_eq!(code, "CONFLICT_ERROR");
            let details = details.expect("для конфликта версий нужны детали");
            assert_eq!(details["expected_version"], "0");
            assert_eq!(details["actual_version"], "1");
        }
        other => panic!("ожидали Error, получено: {other:?}"),
    }
}

#[tokio::test]
async fn unknown_command_maps_to_not_found_error() {
    let env = setup().await;
    let (status, message) = post_rpc(
        &env,
        json!({
            "type": "command",
            "id": "miss",
            "module": "core",
            "action": "core.sample.missing",
            "payload": {}
        }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    match message {
        RpcMessage::Error { id, code, .. } => {
            assert_eq!(id, "miss");
            assert_eq!(code, "NOT_FOUND_ERROR");
        }
        other => panic!("ожидали Error, получено: {other:?}"),
    }
}

#[tokio::test]
async fn malformed_envelope_maps_to_validation_error() {
    let env = setup().await;
    let (status, message) = post_rpc(&env, json!({"type": "command", "id": "x"})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(matches!(
        message,
        RpcMessage::Error {
            code, ..
        } if code == "VALIDATION_ERROR"
    ));
}

#[tokio::test]
async fn legacy_form_is_auto_converted() {
    let env = setup().await;
    let (status, message) = post_rpc(
        &env,
        json!({"name": "core.sample.echo", "params": {"v": 1}, "id": "leg1"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        message,
        RpcMessage::Response {
            id: "leg1".to_string(),
            payload: json!({"v": 1})
        }
    );
}

#[tokio::test]
async fn event_batch_appends_to_store() {
    let env = setup().await;
    let event = Event {
        id: Uuid::new_v4(),
        stream_type: StreamType::Object,
        stream_id: "obj-1".to_string(),
        event_type: "object.created".to_string(),
        version: 0,
        payload: json!({"v": 1}),
        metadata: ActorSnapshot::system(),
        company_id: "comp1".to_string(),
        correlation_id: Uuid::new_v4().to_string(),
        causation_id: None,
        occurred_at: chrono::Utc::now(),
    };
    let (status, message) = post_rpc(
        &env,
        json!({
            "type": "event_batch",
            "id": "eb-1",
            "module": "core",
            "events": [event]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        message,
        RpcMessage::Response {
            id: "eb-1".to_string(),
            payload: json!({"appended": 1})
        }
    );
    let stream = env
        .store
        .read_stream(StreamType::Object, "obj-1")
        .await
        .unwrap();
    assert_eq!(stream.len(), 1);
    assert_eq!(stream[0].event_type, "object.created");
}

#[tokio::test]
async fn server_push_is_rejected_on_input() {
    let env = setup().await;
    let (status, message) = post_rpc(
        &env,
        json!({
            "type": "server_push",
            "id": "sp-1",
            "module": "core",
            "event_type": "company.created",
            "payload": {}
        }),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(matches!(
        message,
        RpcMessage::Error {
            code, ..
        } if code == "VALIDATION_ERROR"
    ));
}