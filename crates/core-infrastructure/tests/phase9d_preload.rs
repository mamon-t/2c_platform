//! Интеграционные приёмочные тесты подфазы 9d (раздел 9 ТЗ v3.1):
//! `preload_company_modules` — предзагрузка установленных модулей при старте
//! сервера через `ModuleManager::preload_all`.
//!
//! На мем-базе (`mem://`) проверяется: загрузка модуля из кэша
//! `{cache_dir}/{code}-{wasm_sha256}.wasm` и декларативная регистрация для
//! всех включивших его компаний, идемпотентность повторного вызова и
//! нефатальность отсутствия кэш-файла (ошибка попадает в `PreloadReport`).

use std::path::PathBuf;
use std::sync::Arc;

use core_application::AppRegistry;
use core_application::ModuleManager;
use serde_json::json;
use surrealdb::engine::any::Any;
use surrealdb::Surreal;
use uuid::Uuid;

use core_application::ports::WasmHost;
use core_infrastructure::extism_wasm_host::ExtismWasmHost;
use core_infrastructure::surreal_audit_repository::SurrealAuditRepository;
use core_infrastructure::surreal_metadata_repository::SurrealMetadataRepository;
use core_infrastructure::surreal_module_repository::SurrealModuleRepository;
use core_infrastructure::surreal_object_repository::SurrealObjectRepository;
use core_infrastructure::surreal_permission_policy_repository::SurrealPermissionPolicyRepository;
use core_infrastructure::surreal_script_repository::SurrealScriptRepository;
use core_infrastructure::SurrealEventStore;

const HELLO_WASM: &[u8] = include_bytes!("fixtures/hello.wasm");

async fn mem_db() -> Surreal<Any> {
    let db = surrealdb::engine::any::connect("mem://").await.unwrap();
    db.use_ns("test")
        .use_db(Uuid::new_v4().to_string())
        .await
        .unwrap();
    db
}

fn temp_cache() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("2c-9d-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

struct Env {
    _db: Surreal<Any>,
    manager: Arc<ModuleManager>,
    app: Arc<AppRegistry>,
    host: Arc<ExtismWasmHost>,
    cache: PathBuf,
}

async fn setup() -> Env {
    let db = mem_db().await;
    let cache = temp_cache();

    let store = Arc::new(SurrealEventStore::new(db.clone()));
    store.ensure_schema().await.unwrap();
    let objects = Arc::new(SurrealObjectRepository::new(db.clone()));
    objects.ensure_schema().await.unwrap();
    let metadata = Arc::new(SurrealMetadataRepository::new(db.clone()));
    metadata.ensure_schema().await.unwrap();
    let audit = Arc::new(SurrealAuditRepository::new(db.clone()));
    audit.ensure_schema().await.unwrap();
    let policies = Arc::new(SurrealPermissionPolicyRepository::new(db.clone()));
    policies.ensure_schema().await.unwrap();
    let modules = Arc::new(SurrealModuleRepository::new(db.clone()));
    modules.ensure_schema().await.unwrap();
    let users = Arc::new(core_infrastructure::SurrealUserRepository::new(db.clone()));
    users.ensure_schema().await.unwrap();

    let transactions = core_application::TransactionOrchestrator::new(objects.clone(), metadata.clone());
    let scripts = Arc::new(SurrealScriptRepository::new(db.clone()));
    scripts.ensure_schema().await.unwrap();
    let script_engine = Arc::new(
        core_infrastructure::RhaiScriptEngine::new(core_infrastructure::rhai_core_api::CoreApiShared {
            store: store.clone(),
            db: db.clone(),
            audit: audit.clone(),
            runtime: tokio::runtime::Handle::current(),
        })
        .unwrap(),
    );
    let host = Arc::new(
        ExtismWasmHost::new(
            db.clone(),
            objects.as_ref().clone(),
            metadata.as_ref().clone(),
            store.as_ref().clone(),
            users.as_ref().clone(),
            transactions,
            script_engine,
            cache.clone(),
        )
        .unwrap(),
    );
    host.ensure_schema().await.unwrap();

    let app = Arc::new(AppRegistry::new());
    let manager = Arc::new(ModuleManager::new(
        host.clone(),
        modules.clone(),
        app.clone(),
        policies,
        metadata,
        scripts,
        audit,
        cache.clone(),
    ));

    Env {
        _db: db,
        manager,
        app,
        host,
        cache,
    }
}

fn cache_wasm_path(env: &Env, wasm_sha256: &str) -> PathBuf {
    env.cache.join(format!("hello-{wasm_sha256}.wasm"))
}

#[tokio::test]
async fn preload_all_loads_and_registers_installed_modules() {
    let env = setup().await;

    env.manager.install("hello", HELLO_WASM, "comp1").await.unwrap();
    env.manager.enable("hello", "comp2").await.unwrap();
    assert!(env.host.is_loaded("hello").await);

    let report = env.manager.preload_all().await.unwrap();
    assert_eq!(report.modules_loaded, 1, "модуль должен быть обработан");
    assert_eq!(report.companies_affected, 2, "учтено включение для двух компаний");
    assert!(report.errors.is_empty(), "ошибок быть не должно: {:?}", report.errors);

    assert!(env.host.is_loaded("hello").await);
    assert!(env.app.commands.list().await.contains(&"plugin.hello.greet".to_string()));
    assert!(env.app.object_schemas.contains("hello").await);
}

#[tokio::test]
async fn preload_all_is_idempotent() {
    let env = setup().await;

    env.manager.install("hello", HELLO_WASM, "comp1").await.unwrap();
    env.manager.enable("hello", "comp2").await.unwrap();

    let first = env.manager.preload_all().await.unwrap();
    let commands_before = env.app.commands.list().await.len();

    let second = env.manager.preload_all().await.unwrap();
    assert_eq!(first.modules_loaded, second.modules_loaded);
    assert_eq!(first.companies_affected, second.companies_affected);
    assert!(second.errors.is_empty());

    let commands_after = env.app.commands.list().await.len();
    assert_eq!(
        commands_before, commands_after,
        "повторная регистрация не должна плодить дубли команд"
    );
}

#[tokio::test]
async fn preload_all_reports_missing_cache_without_crashing() {
    let env = setup().await;

    let record = env.manager.install("hello", HELLO_WASM, "comp1").await.unwrap();
    let path = cache_wasm_path(&env, &record.wasm_sha256);
    assert!(path.exists(), "кэш-файл должен быть записан хостом");

    // Имитация потери кэша после штампа: модуль выгружаем из памяти.
    std::fs::remove_file(&path).unwrap();
    env.host.unload_module("hello").await.unwrap();
    assert!(!env.host.is_loaded("hello").await);

    let report = env.manager.preload_all().await.unwrap();
    assert_eq!(report.modules_loaded, 0, "без кэша модуль не загружается");
    assert!(
        report.errors.iter().any(|e| e.contains("кэш недоступен")),
        "ожидали ошибку о недоступном кэше: {:?}",
        report.errors
    );

    // Восстановление кэша и повторный preload — модуль загружается снова.
    std::fs::write(&path, HELLO_WASM).unwrap();
    let retry = env.manager.preload_all().await.unwrap();
    assert_eq!(retry.modules_loaded, 1, "после восстановления кэша preload успешен");
    assert!(retry.errors.is_empty(), "ошибок быть не должно: {:?}", retry.errors);
    assert!(env.host.is_loaded("hello").await);

    // Команда модуля исполнима после предзагрузки.
    let out = env
        .app
        .commands
        .execute(
            "plugin.hello.greet",
            json!({ "company_id": "comp1", "input": "мир" }),
        )
        .await
        .unwrap();
    assert!(out["output"].as_str().unwrap().contains("Привет"));
}