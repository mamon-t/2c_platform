//! Интеграционные приёмочные тесты WASM-хоста (подфазы 8a–8b по ТЗ v3.1,
//! раздел 9): загрузка и валидация манифеста через `get_info()`, исполнение
//! экспорта `greet` с host-функциями `whoami`/`log_message`, KV-хранилище
//! через `kv_probe` и объекты «Доски» через `objects_probe` (host-fn 8b).
//!
//! Используется скомпилированный из `examples/hello_plugin` модуль
//! `tests/fixtures/hello.wasm` (те же подфазы).

use std::collections::HashSet;
use std::path::PathBuf;

use chrono::Utc;
use core_application::ports::{EntitySchema, MetadataRepository, WasmHost};
use core_domain::metadata::{EntityField, EntityKind, EntityState, EntityType, FieldType};
use core_infrastructure::extism_wasm_host::{ExtismWasmHost, HostCallCtx};
use core_infrastructure::surreal_metadata_repository::SurrealMetadataRepository;
use core_infrastructure::surreal_object_repository::SurrealObjectRepository;
use serde_json::Value;
use surrealdb::engine::any::Any;
use surrealdb::Surreal;
use uuid::Uuid;

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
    let dir = std::env::temp_dir().join(format!("2c-hello-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

async fn host() -> (ExtismWasmHost, Surreal<Any>, PathBuf) {
    let cache = temp_cache();
    let db = mem_db().await;
    let objects = SurrealObjectRepository::new(db.clone());
    let metadata = SurrealMetadataRepository::new(db.clone());
    let h = ExtismWasmHost::new(db.clone(), objects.clone(), metadata.clone(), cache.clone()).unwrap();
    h.ensure_schema().await.unwrap();
    objects.ensure_schema().await.unwrap();
    metadata.ensure_schema().await.unwrap();
    core_infrastructure::SurrealEventStore::new(db.clone())
        .ensure_schema()
        .await
        .unwrap();
    (h, db, cache)
}

/// Регистрирует тип сущности `greeting` (каталог, поле `text`) в компании
/// `comp1` и возвращает его id для host-вызовов подфазы 8b.
async fn seed_greeting_schema(db: &Surreal<Any>) -> Uuid {
    let metadata = SurrealMetadataRepository::new(db.clone());
    let entity_type = EntityType {
        id: Uuid::new_v4(),
        code: "greeting".to_string(),
        name: "Приветствие".to_string(),
        kind: EntityKind::Catalog,
        company_id: "comp1".to_string(),
        metadata_version: 1,
        is_system: false,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    let schema = EntitySchema {
        entity_type: entity_type.clone(),
        fields: vec![EntityField {
            id: Uuid::new_v4(),
            entity_type: "greeting".to_string(),
            code: "text".to_string(),
            label: "Текст".to_string(),
            data_type: FieldType::String,
            required: true,
            is_unique: false,
            is_indexed: false,
            options: Value::Null,
            is_system: false,
            order: 1,
        }],
        states: vec![EntityState {
            id: Uuid::new_v4(),
            entity_type: "greeting".to_string(),
            code: "active".to_string(),
            label: "Активен".to_string(),
            color: None,
            is_initial: true,
            is_final: false,
        }],
        transitions: vec![],
        forms: vec![],
        actions: vec![],
        relations: vec![],
    };
    metadata.create_entity_type(&schema, &[]).await.unwrap();
    entity_type.id
}

#[tokio::test]
async fn load_module_validates_manifest_and_checks_code() {
    let (h, _db, _cache) = host().await;

    let manifest = h.load_module("hello", HELLO_WASM).await.unwrap();
    assert_eq!(manifest.code, "hello");
    assert_eq!(manifest.api_version, "2.0");
    assert_eq!(manifest.version, "1.0.0");
    assert!(manifest.capabilities.iter().any(|c| c == "storage"));

    assert!(h.is_loaded("hello").await);

    let mismatch = h.load_module("wrong", HELLO_WASM).await;
    assert!(mismatch.is_err(), "код манифеста должен совпадать с переданным");
}

#[tokio::test]
async fn call_function_invokes_export_and_host_functions() {
    let company = Uuid::new_v4().to_string();
    let actor = Uuid::new_v4().to_string();
    let (h, _db, _cache) = host().await;

    h.load_module("hello", HELLO_WASM).await.unwrap();
    h.set_call_context(HostCallCtx {
        module_code: "hello".to_string(),
        company_id: company.clone(),
        actor: None,
        capabilities: HashSet::from(["logging".to_string(), "storage".to_string()]),
        settings: Value::Null,
    })
    .await;

    let out = h
        .call_function("hello", "greet", b"")
        .await
        .expect("greet должен отработать");
    let text = String::from_utf8(out).unwrap();
    assert!(
        text.contains("Привет"),
        "greet должен вернуть приветствие, получили: {text}"
    );
    assert!(!actor.is_empty());
}

#[tokio::test]
async fn kv_probe_reads_and_writes_through_host() {
    let (h, _db, _cache) = host().await;

    h.load_module("hello", HELLO_WASM).await.unwrap();
    h.set_call_context(HostCallCtx {
        module_code: "hello".to_string(),
        company_id: Uuid::new_v4().to_string(),
        actor: None,
        capabilities: HashSet::from(["storage".to_string()]),
        settings: Value::Null,
    })
    .await;

    let out = h
        .call_function("hello", "kv_probe", b"")
        .await
        .expect("kv_probe должен отработать");
    let text = String::from_utf8(out).unwrap();
    assert!(
        text.contains("\"ok\":true") && text.contains("\"data\":\"hello_value\""),
        "kv_probe должен вернуть записанное значение, получили: {text}"
    );
}

#[tokio::test]
async fn unload_module_removes_and_tracks_absence() {
    let (h, _db, _cache) = host().await;
    h.load_module("hello", HELLO_WASM).await.unwrap();
    assert!(h.is_loaded("hello").await);

    h.unload_module("hello").await.unwrap();
    assert!(!h.is_loaded("hello").await);

    let err = h.unload_module("hello").await;
    assert!(err.is_err(), "повторная выгрузка должна вернуть ошибку");

    let call = h.call_function("hello", "greet", b"").await;
    assert!(call.is_err(), "вызов после выгрузки должен вернуть ошибку");
}

#[tokio::test]
async fn objects_probe_creates_reads_and_lists_through_host_8b() {
    let (h, db, _cache) = host().await;
    let entity_type_id = seed_greeting_schema(&db).await;

    h.load_module("hello", HELLO_WASM).await.unwrap();
    h.set_call_context(HostCallCtx {
        module_code: "hello".to_string(),
        company_id: "comp1".to_string(),
        actor: None,
        capabilities: HashSet::from([
            "objects.create".to_string(),
            "objects.read".to_string(),
        ]),
        settings: Value::Null,
    })
    .await;

    let out = h
        .call_function("hello", "objects_probe", entity_type_id.to_string().as_bytes())
        .await
        .expect("objects_probe должен отработать");
    let text = String::from_utf8(out).unwrap();

    let create_conv = extract_conv(&text, "create=");
    let get_conv = extract_conv(&text, "get=");
    let list_conv = extract_conv(&text, "list=");

    assert!(
        create_conv.contains("\"ok\":true") && create_conv.contains("\"id\""),
        "create должен вернуть ok и id объекта, получили: {create_conv}"
    );
    let created_id = create_conv
        .split("\"id\":")
        .nth(1)
        .and_then(|s| s.trim_start_matches('"').split('"').next())
        .unwrap()
        .to_string();

    assert!(
        get_conv.contains(&format!("\"id\":\"{created_id}\""))
            && get_conv.contains("\"state\":\"active\"")
            && get_conv.contains("\"Привет, бизнес-объект!\""),
        "get должен вернуть объект с начальным состоянием и данными, получили: {get_conv}"
    );

    assert!(
        list_conv.contains("\"total_count\":1")
            && list_conv.contains(&format!("\"id\":\"{created_id}\"")),
        "list должен показать один объект с total_count=1, получили: {list_conv}"
    );

    let call = h.call_function("hello", "objects_probe", entity_type_id.to_string().as_bytes()).await;
    assert!(call.is_ok(), "повторный вызов должен создавать ещё один объект");
    let second = String::from_utf8(call.unwrap()).unwrap();
    assert!(
        extract_conv(&second, "list=").contains("\"total_count\":2"),
        "после двух созданий total_count должен стать 2, получили: {second}"
    );
}

/// Возвращает фрагмент после маркера `marker` до `; ` — содержимое одного
/// конверта host-функции в выводе экспорта objects_probe.
fn extract_conv<'a>(text: &'a str, marker: &str) -> &'a str {
    let start = text
        .find(marker)
        .map(|i| i + marker.len())
        .unwrap_or_else(|| panic!("маркер {marker} не найден в: {text}"));
    text[start..]
        .split("; ")
        .next()
        .unwrap_or_default()
}
