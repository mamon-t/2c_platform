//! Интеграционные приёмочные тесты подфазы 9b (раздел 9 ТЗ v3.1):
//! `ModuleStore` + декларативная регистрация через `ModuleManager`.
//!
//! На мем-базе (`mem://`) проверяется полный цикл жизни модуля: установка
//! (каталог `modules`, политики, схемы, команды `plugin.*`, включение для
//! компании), дубликат установки, включение/отключение для другой компании,
//! исполнение команды модуля и удаление. Фикстура — `examples/hello_plugin`.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use core_application::AppRegistry;
use core_application::ModuleManager;
use core_domain::event::StreamType;
use core_domain::module::ModuleState;
use serde_json::json;
use surrealdb::engine::any::Any;
use surrealdb::Surreal;
use uuid::Uuid;

use core_application::ports::{EventStore, ModuleRepository};
use core_infrastructure::extism_wasm_host::ExtismWasmHost;
use core_infrastructure::surreal_audit_repository::SurrealAuditRepository;
use core_infrastructure::surreal_metadata_repository::SurrealMetadataRepository;
use core_infrastructure::surreal_module_repository::SurrealModuleRepository;
use core_infrastructure::surreal_object_repository::SurrealObjectRepository;
use core_infrastructure::surreal_permission_policy_repository::SurrealPermissionPolicyRepository;
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
    let dir = std::env::temp_dir().join(format!("2c-9b-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

struct Env {
    _db: Surreal<Any>,
    manager: Arc<ModuleManager>,
    modules: Arc<SurrealModuleRepository>,
    app: Arc<AppRegistry>,
}

async fn setup() -> Env {
    let db = mem_db().await;

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

    let host = Arc::new(
        ExtismWasmHost::new(
            db.clone(),
            objects.as_ref().clone(),
            metadata.as_ref().clone(),
            store.as_ref().clone(),
            users.as_ref().clone(),
            temp_cache(),
        )
        .unwrap(),
    );
    host.ensure_schema().await.unwrap();

    let app = Arc::new(AppRegistry::new());
    let manager = Arc::new(ModuleManager::new(
        host,
        modules.clone(),
        app.clone(),
        policies,
        metadata,
        audit,
    ));

    Env {
        _db: db,
        manager,
        modules,
        app,
    }
}

#[tokio::test]
async fn install_registers_declaratively_and_enables_for_company() {
    let env = setup().await;

    let record = env
        .manager
        .install("hello", HELLO_WASM, "comp1")
        .await
        .unwrap();
    assert_eq!(record.code, "hello");
    assert_eq!(record.state, ModuleState::Installed);
    assert_eq!(record.wasm_sha256.len(), 16);
    assert_eq!(record.manifest.commands[0].code, "greet");
    assert_eq!(
        record.manifest.commands[0].required_permission.as_deref(),
        Some("hello.greet")
    );

    let catalog = env.modules.list().await.unwrap();
    assert_eq!(catalog.len(), 1);
    assert!(env.modules.is_enabled_for_company("comp1", "hello").await.unwrap());
    assert!(!env.modules.is_enabled_for_company("comp2", "hello").await.unwrap());

    assert!(env.app.commands.list().await.contains(&"plugin.hello.greet".to_string()));
    assert!(env.app.permissions.contains("hello").await);
    assert!(env.app.object_schemas.contains("hello").await);
}

#[tokio::test]
async fn duplicate_install_is_rejected() {
    let env = setup().await;

    env.manager.install("hello", HELLO_WASM, "comp1").await.unwrap();
    let err = env
        .manager
        .install("hello", HELLO_WASM, "comp1")
        .await
        .unwrap_err();
    assert!(matches!(err, core_domain::error::DomainError::ValidationError(_)));
}

#[tokio::test]
async fn plugin_command_executes_only_for_enabled_company() {
    let env = setup().await;
    env.manager.install("hello", HELLO_WASM, "comp1").await.unwrap();

    let params = json!({
        "company_id": "comp1",
        "input": "мир",
    });
    let out = env
        .app
        .commands
        .execute("plugin.hello.greet", params)
        .await
        .unwrap();
    assert!(out["output"].as_str().unwrap().contains("Привет"));

    env.manager.disable("hello", "comp1").await.unwrap();
    let params = json!({
        "company_id": "comp1",
        "input": "мир",
    });
    let err = env
        .app
        .commands
        .execute("plugin.hello.greet", params)
        .await
        .unwrap_err();
    assert!(err.contains("отключён"), "ожидали ошибку disabled, получено: {err}");
}

#[tokio::test]
async fn enable_for_second_company_then_uninstall() {
    let env = setup().await;
    env.manager.install("hello", HELLO_WASM, "comp1").await.unwrap();

    let record = env.manager.enable("hello", "comp2").await.unwrap();
    assert_eq!(record.state, ModuleState::Installed);
    assert!(env.modules.is_enabled_for_company("comp2", "hello").await.unwrap());

    env.manager.disable("hello", "comp1").await.unwrap();
    assert!(!env.modules.is_enabled_for_company("comp1", "hello").await.unwrap());
    assert!(env.modules.is_enabled_for_company("comp2", "hello").await.unwrap());

    let record = env.manager.uninstall("hello").await.unwrap();
    assert_eq!(record.state, ModuleState::Uninstalled);
    assert!(!env.app.commands.list().await.contains(&"plugin.hello.greet".to_string()));
    assert!(!env.modules.is_enabled_for_company("comp2", "hello").await.unwrap());
}

#[tokio::test]
async fn reinstall_after_uninstall_restores_module_and_tracks_events() {
    let env = setup().await;

    env.manager.install("hello", HELLO_WASM, "comp1").await.unwrap();
    let record = env.manager.uninstall("hello").await.unwrap();
    assert_eq!(record.state, ModuleState::Uninstalled);

    // Переустановка удалённого модуля — первоклассная операция.
    let record = env.manager.install("hello", HELLO_WASM, "comp1").await.unwrap();
    assert_eq!(record.state, ModuleState::Installed);
    assert!(env.modules.is_enabled_for_company("comp1", "hello").await.unwrap());
    assert!(env.app.commands.list().await.contains(&"plugin.hello.greet".to_string()));

    // Дубликат активного модуля всё ещё отклоняется.
    let err = env
        .manager
        .install("hello", HELLO_WASM, "comp1")
        .await
        .unwrap_err();
    assert!(matches!(err, core_domain::error::DomainError::ValidationError(_)));

    // После переустановки команда модуля снова исполнима.
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

    // Труба фиксирует полный цикл жизни: установка → удаление → переустановка.
    let store = core_infrastructure::SurrealEventStore::new(env._db.clone());
    let stream = store
        .read_stream(StreamType::Module, "hello")
        .await
        .unwrap();
    let types: HashSet<String> = stream.iter().map(|e| e.event_type.clone()).collect();
    for expected in [
        "module.installed",
        "module.uninstalled",
        "module.reinstalled",
    ] {
        assert!(types.contains(expected), "поток Module должен содержать {expected}");
    }
}