//! Пример из example/hello против реального хост-контракта Plugin API v1.
//! Гвардия совместимости: если меняется конверт/манифест — падает здесь.
//! Запуск: cargo test --test example_hello_test (MongoDB не нужен)

use std::sync::{Arc, RwLock};

use app_lib::plugin_manager as pm;
use app_lib::plugin_manager::workflow as wf;
use app_lib::plugin_manager::{HostData, PluginContext};
use extism::{Manifest, PluginBuilder, UserData, Wasm, PTR};

const WASM_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../example/hello/target/wasm32-unknown-unknown/release/hello_plugin.wasm"
);

fn mk_plugin() -> extism::Plugin {
    let wasm = std::fs::read(WASM_PATH).expect("соберите пример: cd example/hello && cargo build --release");
    let ctx = Arc::new(RwLock::new(PluginContext {
        company_id: Some(uuid::Uuid::new_v4().to_string()),
        user_id: Some(uuid::Uuid::new_v4().to_string()),
        user_login: Some("example-test".into()),
        display_name: None,
        role_id: None,
        role_ids: vec![],
    }));
    let host = HostData {
        db: None, // hello не трогает БД
        ctx,
        module_code: Some("hello".into()),
        capabilities: vec!["logging".into()],
    };
    PluginBuilder::new(&Manifest::new([Wasm::data(wasm)]))
        .with_function("log_message", [PTR], [], UserData::new(host.clone()), pm::log_message_impl)
        .with_function("now_ms", [], [PTR], UserData::new(host.clone()), wf::now_ms_impl)
        .with_fuel_limit(10_000_000)
        .build()
        .expect("hello загружается")
}

#[test]
fn hello_manifest_matches_api_v1() {
    let mut p = mk_plugin();
    let out = p.call::<&[u8], String>("get_info", b"").expect("get_info");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();

    assert_eq!(v["name"], "hello");
    assert_eq!(v["code"], "hello");
    assert_eq!(v["api_version"], "1.0");
    assert_eq!(v["capabilities"], serde_json::json!(["logging"]));
    assert_eq!(v["permissions"], serde_json::json!([]));
    assert_eq!(v["handled_documents"], serde_json::json!([]));
    let fns = v["functions"].as_array().unwrap();
    assert!(fns.iter().any(|f| f["name"] == "echo"));
}

#[test]
fn hello_echo_works() {
    let mut p = mk_plugin();
    let out = p.call::<&[u8], String>(
        "echo",
        serde_json::json!({"text": "привет"}).to_string().as_bytes(),
    ).expect("echo");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["echo"], "привет", "эхо возвращает текст: {v}");
}

#[test]
fn hello_host_time_unwraps_envelope() {
    let mut p = mk_plugin();
    let out = p.call::<&[u8], String>("host_time", b"").expect("host_time");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let ms = v["unix_ms"].as_i64().expect("unix_ms числом");
    assert!(ms > 1_700_000_000_000, "правдоподобное unix-время: {ms}");
}
