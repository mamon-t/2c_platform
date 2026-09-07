mod commands;

use std::sync::Arc;

use anyhow::{Context, Result};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use core_application::command_registry::CommandExecutionCtx;
use core_application::permission_manager::PermissionManager;
use core_application::ports::EventStore;
use core_application::CommandRegistry;
use core_domain::event::{ActorSnapshot, Event, StreamType};
use core_infrastructure::{
    connect_db, SurrealAuditRepository, SurrealCompanyRepository, SurrealEventStore,
    SurrealMetadataRepository, SurrealObjectRepository, SurrealPermissionPolicyRepository,
    SurrealRoleRepository, SurrealUserRepository,
};
use tokio::net::TcpListener;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

#[derive(Clone)]
struct AppState {
    store: Arc<SurrealEventStore>,
    registry: Arc<CommandRegistry>,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    init_tracing();

    info!("2C Platform Core v0.1.0 запускается...");

    let db = connect_database().await?;
    let store = Arc::new(SurrealEventStore::new(db.clone()));
    store.ensure_schema().await.context("не удалось создать схему events")?;

    let companies = Arc::new(SurrealCompanyRepository::new(db.clone()));
    companies
        .ensure_schema()
        .await
        .context("не удалось создать схему companies")?;
    let users = Arc::new(SurrealUserRepository::new(db.clone()));
    users
        .ensure_schema()
        .await
        .context("не удалось создать схему users")?;
    let roles = Arc::new(SurrealRoleRepository::new(db.clone()));
    roles
        .ensure_schema()
        .await
        .context("не удалось создать схему roles")?;
    let metadata = Arc::new(SurrealMetadataRepository::new(db.clone()));
    metadata
        .ensure_schema()
        .await
        .context("не удалось создать схему метаданных")?;
    let audit = Arc::new(SurrealAuditRepository::new(db.clone()));
    audit
        .ensure_schema()
        .await
        .context("не удалось создать схему audit_log")?;
    let objects = Arc::new(SurrealObjectRepository::new(db.clone()));
    objects
        .ensure_schema()
        .await
        .context("не удалось создать схему объектов")?;
    let policies = Arc::new(SurrealPermissionPolicyRepository::new(db.clone()));
    policies
        .ensure_schema()
        .await
        .context("не удалось создать схему permission_policies")?;

    let registry = Arc::new(CommandRegistry::new());
    commands::register_phase2_commands(&registry, companies.clone(), users, roles.clone()).await;
    commands::register_phase3_commands(&registry, metadata.clone()).await;
    commands::register_phase4_commands(&registry, objects, metadata).await;
    commands::register_phase4_audit_commands(&registry, audit.clone()).await;
    commands::register_phase5_commands(
        &registry,
        roles.clone(),
        policies.clone(),
        audit.clone(),
        companies,
    )
    .await;

    let permissions = Arc::new(PermissionManager::new(roles, policies));
    registry.attach_pipeline(audit, permissions).await;
    info!("Зарегистрировано команд ({}):", registry.list().await.len());

    let state = AppState {
        store,
        registry,
    };
    let app = Router::new()
        .route("/health", get(health))
        .route("/debug/events", post(debug_append_events))
        .route("/debug/streams/{kind}/{sid}", get(debug_read_stream))
        .route("/debug/command", post(debug_command))
        .with_state(state);

    let addr = std::env::var("SERVER_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    let listener = TcpListener::bind(&addr).await?;
    info!("Сервер слушает {addr}");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("Сервер остановлен");
    Ok(())
}

async fn connect_database() -> Result<surrealdb::Surreal<surrealdb::engine::any::Any>> {
    let host = std::env::var("SURREAL_HOST").context("SURREAL_HOST не задан")?;
    let user = std::env::var("SURREAL_USER").context("SURREAL_USER не задан")?;
    let pass = std::env::var("SURREAL_PASS").context("SURREAL_PASS не задан")?;
    let ns = std::env::var("SURREAL_NS").context("SURREAL_NS не задан")?;
    let db_name = std::env::var("SURREAL_DB").context("SURREAL_DB не задан")?;

    let db = connect_db(&host, &user, &pass, &ns, &db_name)
        .await
        .with_context(|| format!("не удалось подключиться к SurrealDB {host}"))?;
    info!("SurrealDB подключена: ws://{host}, ns={ns}, db={db_name}");
    Ok(db)
}

async fn health() -> &'static str {
    "OK"
}

async fn debug_append_events(
    State(state): State<AppState>,
    Json(events): Json<Vec<Event>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut streamed = 0;
    for event in &events {
        streamed += usize::from(!event.stream_id.is_empty());
    }
    state
        .store
        .append(&events)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::json!({"appended": events.len(), "streams": streamed})))
}

async fn debug_read_stream(
    State(state): State<AppState>,
    Path((kind, sid)): Path<(String, String)>,
) -> Result<Json<Vec<Event>>, (StatusCode, String)> {
    let stream_type = parse_stream_type(&kind)?;
    state
        .store
        .read_stream(stream_type, &sid)
        .await
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

async fn debug_command(
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let name = payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "отсутствует поле 'name'".to_string()))?;
    let params = payload
        .get("params")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let actor = payload
        .get("actor")
        .cloned()
        .map(serde_json::from_value::<ActorSnapshot>)
        .transpose()
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("некорректный 'actor': {e}")))?;
    let module_code = payload
        .get("module_code")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let entity_type = payload
        .get("entity_type")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let ctx = CommandExecutionCtx {
        actor,
        module_code,
        entity_type,
    };
    let result = state
        .registry
        .execute_ctx(name, params, ctx)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    Ok(Json(serde_json::json!({ "ok": true, "command": name, "result": result })))
}

fn parse_stream_type(kind: &str) -> Result<StreamType, (StatusCode, String)> {
    StreamType::try_from(kind).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            format!("неизвестный тип потока: {kind}"),
        )
    })
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(err) = tokio::signal::ctrl_c().await {
            warn!("Не удалось установить обработчик Ctrl+C: {err}");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(err) => warn!("Не удалось установить обработчик SIGTERM: {err}"),
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
    info!("Получен сигнал завершения, останавливаемся...");
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
}