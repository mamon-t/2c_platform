//! Пример WASM-модуля для хоста платформы (подфаза 8a).
//!
//! Экспортирует `get_info()` (манифест v2), `greet()` (приветствие с вызовом
//! host-функций `whoami`/`now_ms`) и `kv_probe()` (запись/чтение KV-хранилища).
//! Собирается отдельным крейтом с целью `wasm32-unknown-unknown` и вне
//! workspace: `cargo build --release --target wasm32-unknown-unknown`.

use extism_pdk::{Error, FnResult};

#[extism_pdk::host_fn("ExtismHost")]
extern "ExtismHost" {
    fn whoami() -> String;
    fn now_ms() -> String;
    fn log_message(msg: String);
    fn kv_put(key: String, value: String) -> String;
    fn kv_get(key: String) -> String;
}

/// Манифест модуля v2 (раздел 9 ТЗ).
#[extism_pdk::plugin_fn]
pub fn get_info() -> FnResult<String> {
    let manifest = r#"{
        "code": "hello",
        "version": "1.0.0",
        "display_name": "Пример приветствия",
        "description": "Демонстрирует host-функции подфазы 8a",
        "author": "2C Platform",
        "api_version": "2.0",
        "capabilities": ["logging", "storage"],
        "commands": [{ "code": "greet", "name": "Поздороваться" }],
        "object_schemas": [],
        "print_templates": [],
        "scripts": [],
        "metadata_version": 1,
        "handles_documents": [],
        "navigation": [],
        "dependencies": []
    }"#;
    Ok(manifest.to_string())
}

/// Приветствие: использует `whoami`, `now_ms` и `log_message`.
#[extism_pdk::plugin_fn]
pub fn greet() -> FnResult<String> {
    let who = unsafe { whoami()? };
    let ts = unsafe { now_ms()? };
    let _ = unsafe { log_message(format!("greet от {who}")) };
    Ok(format!("Привет, {who}! (ms={ts})"))
}

/// Проверка KV-хранилища: пишет ключ и читает его обратно.
#[extism_pdk::plugin_fn]
pub fn kv_probe() -> FnResult<String> {
    let key = "demo_key".to_string();
    let value = serde_json::to_string("hello_value").unwrap();
    let put_conv = unsafe { kv_put(key.clone(), value)? };
    let get_conv = unsafe { kv_get(key)? };
    Ok(format!("put={put_conv}; get={get_conv}"))
}

// Фиктивный помощник, чтобы `Error` был задействован (never-type fallback не
// смешивает `!` с возвращаемым типом плагина).
#[allow(dead_code)]
fn _as_error(e: extism_pdk::Error) -> Error {
    e
}
