mod commands;

use std::path::PathBuf;
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
use core_application::ports::{EventStore, WasmHost};
use core_application::CommandRegistry;
use core_application::ModuleManager;
use core_domain::error::DomainError;
use core_domain::event::{ActorSnapshot, Event, StreamType};
use core_infrastructure::extism_wasm_host::{ExtismWasmHost, HostCallCtx};
use core_infrastructure::{
    connect_db, SurrealAuditRepository, SurrealCompanyRepository, SurrealEventStore,
    SurrealMetadataRepository, SurrealModuleRepository, SurrealObjectRepository,
    SurrealPermissionPolicyRepository, SurrealRoleRepository, SurrealUserRepository,
};
use tokio::net::TcpListener;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

#[derive(Clone)]
struct AppState {
    store: Arc<SurrealEventStore>,
    registry: Arc<CommandRegistry>,
    host: Arc<ExtismWasmHost>,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    init_tracing();

    info!("2C Platform Core v3.0.1 запускается...");

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

    let cache_dir: PathBuf = std::env::var("MODULE_CACHE_DIR").map(PathBuf::from).unwrap_or_else(
        |_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home).join(".cache/2c-platform/modules")
        },
    );
    let host = Arc::new(
        ExtismWasmHost::new(db.clone(), objects.as_ref().clone(), metadata.as_ref().clone(), cache_dir.clone()).context("не удалось создать WASM-хост")?,
    );
    host.ensure_schema()
        .await
        .context("не удалось создать схему ModuleKv")?;
    info!("WASM-хост Extism готов (module_cache_dir={})", cache_dir.display());

    let app_registry = Arc::new(core_application::AppRegistry::new());
    let registry = app_registry.commands.clone();
    commands::register_phase2_commands(&registry, companies.clone(), users, roles.clone()).await;
    commands::register_phase3_commands(&registry, metadata.clone()).await;
    commands::register_phase4_commands(&registry, objects, metadata.clone()).await;
    commands::register_phase4_audit_commands(&registry, audit.clone()).await;
    commands::register_phase5_commands(
        &registry,
        roles.clone(),
        policies.clone(),
        audit.clone(),
        companies.clone(),
    )
    .await;

    let modules = Arc::new(SurrealModuleRepository::new(db.clone()));
    modules
        .ensure_schema()
        .await
        .context("не удалось создать схему модулей")?;
    let module_manager = Arc::new(ModuleManager::new(
        host.clone(),
        modules.clone(),
        app_registry.clone(),
        policies.clone(),
        metadata.clone(),
        audit.clone(),
    ));
    commands::register_phase9_module_commands(&registry, module_manager, companies.clone()).await;

    let permissions = Arc::new(PermissionManager::new(roles, policies));
    registry.attach_pipeline(audit, permissions).await;
    info!("Зарегистрировано команд ({}):", registry.list().await.len());

    let state = AppState {
        store,
        registry,
        host,
    };
    let app = Router::new()
        .route("/health", get(health))
        .route("/debug/events", post(debug_append_events))
        .route("/debug/streams/{kind}/{sid}", get(debug_read_stream))
        .route("/debug/command", post(debug_command))
        .route("/debug/modules", get(debug_modules))
        .route("/debug/module/load", post(debug_module_load))
        .route("/debug/module/invoke", post(debug_module_invoke))
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

/// Загруженные WASM-модули с их манифестами (debug-проверка Фазы 9).
async fn debug_modules(State(state): State<AppState>) -> Json<serde_json::Value> {
    let modules = state
        .host
        .list_modules()
        .await
        .into_iter()
        .map(|(code, manifest)| serde_json::json!({ "code": code, "manifest": manifest }))
        .collect::<Vec<_>>();
    Json(serde_json::json!({ "ok": true, "modules": modules }))
}

/// Загрузка модуля из base64-байтов: `{"code": "...", "wasm_base64": "..."}`.
async fn debug_module_load(
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    use base64::Engine as _;

    let code = payload
        .get("code")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "отсутствует поле 'code'".to_string()))?;
    let wasm_base64 = payload
        .get("wasm_base64")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "отсутствует поле 'wasm_base64'".to_string()))?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(wasm_base64)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("некорректный wasm_base64: {e}")))?;
    let manifest = state
        .host
        .load_module(code, &bytes)
        .await
        .map_err(debug_status)?;
    Ok(Json(serde_json::json!({
        "ok": true,
        "code": code,
        "size_bytes": bytes.len(),
        "manifest": manifest
    })))
}

/// Вызов экспортируемой функции модуля:
/// `{"module": "...", "function": "...", "input": "...", "company_id": "...",
///   "capabilities": ["..."], "actor": {...}?, "settings": {...}?}`.
async fn debug_module_invoke(
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let module = payload
        .get("module")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "отсутствует поле 'module'".to_string()))?
        .to_string();
    let function = payload
        .get("function")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "отсутствует поле 'function'".to_string()))?
        .to_string();
    let input = payload.get("input").and_then(|v| v.as_str()).unwrap_or_default();
    let company_id = payload
        .get("company_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "отсутствует поле 'company_id'".to_string()))?
        .to_string();
    let capabilities = payload
        .get("capabilities")
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|c| c.as_str().map(str::to_string))
                .collect::<std::collections::HashSet<_>>()
        })
        .unwrap_or_default();
    let actor = payload
        .get("actor")
        .cloned()
        .map(serde_json::from_value::<ActorSnapshot>)
        .transpose()
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("некорректный 'actor': {e}")))?;
    let settings = payload
        .get("settings")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    let ctx = HostCallCtx {
        module_code: module.clone(),
        company_id,
        actor,
        capabilities,
        settings,
    };
    state.host.set_call_context(ctx).await;
    let out = state
        .host
        .call_function(&module, &function, input.as_bytes())
        .await
        .map_err(debug_status)?;
    let output = String::from_utf8_lossy(&out).to_string();
    Ok(Json(serde_json::json!({
        "ok": true,
        "module": module,
        "function": function,
        "output": output
    })))
}

/// Маппинг доменных ошибок на HTTP-коды для debug-REST.
fn debug_status(e: DomainError) -> (StatusCode, String) {
    let status = if matches!(e, DomainError::NotFound(_)) {
        StatusCode::NOT_FOUND
    } else {
        StatusCode::BAD_REQUEST
    };
    (status, e.to_string())
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