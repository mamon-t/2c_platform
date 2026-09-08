//! Интеграционные приёмочные тесты WASM-хоста (подфаза 8a по ТЗ v3.1, раздел 9):
//! загрузка и валидация манифеста через `get_info()`, исполнение экспорта `greet`
//! с host-функциями `whoami`/`log_message`, KV-хранилище через `kv_probe`.
//!
//! Используется скомпилированный из `examples/hello_plugin` модуль
//! `tests/fixtures/hello.wasm` (та же подфаза).

use std::collections::HashSet;
use std::path::PathBuf;

use core_application::ports::WasmHost;
use core_infrastructure::extism_wasm_host::{ExtismWasmHost, HostCallCtx};
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

async fn host() -> (ExtismWasmHost, PathBuf) {
    let cache = temp_cache();
    let h = ExtismWasmHost::new(mem_db().await, cache.clone()).unwrap();
    h.ensure_schema().await.unwrap();
    (h, cache)
}

#[tokio::test]
async fn load_module_validates_manifest_and_checks_code() {
    let (h, _cache) = host().await;

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
    let (h, _cache) = host().await;

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
    let (h, _cache) = host().await;

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
    let (h, _cache) = host().await;
    h.load_module("hello", HELLO_WASM).await.unwrap();
    assert!(h.is_loaded("hello").await);

    h.unload_module("hello").await.unwrap();
    assert!(!h.is_loaded("hello").await);

    let err = h.unload_module("hello").await;
    assert!(err.is_err(), "повторная выгрузка должна вернуть ошибку");

    let call = h.call_function("hello", "greet", b"").await;
    assert!(call.is_err(), "вызов после выгрузки должен вернуть ошибку");
}
