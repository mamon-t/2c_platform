use std::sync::Arc;

use anyhow::{Context, Result};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use core_application::ports::EventStore;
use core_domain::event::{Event, StreamType};
use core_infrastructure::SurrealEventStore;
use tokio::net::TcpListener;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

#[derive(Clone)]
struct AppState {
    store: Arc<SurrealEventStore>,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    init_tracing();

    info!("2C Platform Core v0.1.0 запускается...");

    let store = connect_event_store().await?;

    let state = AppState {
        store: Arc::new(store),
    };
    let app = Router::new()
        .route("/health", get(health))
        .route("/debug/events", post(debug_append_events))
        .route("/debug/streams/{kind}/{sid}", get(debug_read_stream))
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

async fn connect_event_store() -> Result<SurrealEventStore> {
    let host = std::env::var("SURREAL_HOST").context("SURREAL_HOST не задан")?;
    let user = std::env::var("SURREAL_USER").context("SURREAL_USER не задан")?;
    let pass = std::env::var("SURREAL_PASS").context("SURREAL_PASS не задан")?;
    let ns = std::env::var("SURREAL_NS").context("SURREAL_NS не задан")?;
    let db_name = std::env::var("SURREAL_DB").context("SURREAL_DB не задан")?;

    let store = SurrealEventStore::connect(&host, &user, &pass, &ns, &db_name)
        .await
        .with_context(|| format!("не удалось подключиться к SurrealDB {host}"))?;
    store
        .ensure_schema()
        .await
        .context("не удалось создать схему events")?;
    info!("SurrealDB подключена: ws://{host}, ns={ns}, db={db_name}");
    Ok(store)
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

fn parse_stream_type(kind: &str) -> Result<StreamType, (StatusCode, String)> {
    match kind {
        "object" => Ok(StreamType::Object),
        "user" => Ok(StreamType::User),
        "module" => Ok(StreamType::Module),
        _ => Err((
            StatusCode::BAD_REQUEST,
            format!("неизвестный тип потока: {kind}"),
        )),
    }
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