//! Пример WASM-модуля для хоста платформы (подфазы 8a–8b).
//!
//! Экспортирует `get_info()` (манифест v2), `greet()` (приветствие с вызовом
//! host-функций `whoami`/`now_ms`), `kv_probe()` (запись/чтение KV-хранилища)
//! и `objects_probe()` (создание/чтение/список объектов «Доски» через
//! host-функции подфазы 8b). Собирается отдельным крейтом с целью
//! `wasm32-unknown-unknown` и вне workspace:
//! `cargo build --release --target wasm32-unknown-unknown`.

use extism_pdk::{Error, FnResult};

#[extism_pdk::host_fn("ExtismHost")]
extern "ExtismHost" {
    fn whoami() -> String;
    fn now_ms() -> String;
    fn log_message(msg: String);
    fn kv_put(key: String, value: String) -> String;
    fn kv_get(key: String) -> String;
    fn create_object(entity_type_id: String, data_json: String) -> String;
    fn get_object(id: String) -> String;
    fn list_objects(entity_type_id: String, limit: String) -> String;
}

/// Манифест модуля v2 (раздел 9 ТЗ).
#[extism_pdk::plugin_fn]
pub fn get_info() -> FnResult<String> {
    let manifest = r#"{
        "code": "hello",
        "version": "1.0.0",
        "display_name": "Пример приветствия",
        "description": "Демонстрирует host-функции подфаз 8a–8b",
        "author": "2C Platform",
        "api_version": "2.0",
        "capabilities": ["logging", "storage", "objects.create", "objects.read"],
        "commands": [{ "code": "greet", "name": "Поздороваться" }],
        "object_schemas": [{
            "code": "greeting",
            "name": "Приветствие",
            "kind": "catalog",
            "fields": [{ "code": "text", "name": "Текст", "kind": "string", "required": true }]
        }],
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

/// Проверка host-функций 8b: создаёт объект `greeting`, читает его по id и
/// перечисляет объекты типа. На вход получает id типа сущности (UUID).
#[extism_pdk::plugin_fn]
pub fn objects_probe(entity_type_id: String) -> FnResult<String> {
    let data = serde_json::json!({ "text": "Привет, бизнес-объект!" });
    let create_conv = unsafe { create_object(entity_type_id.clone(), data.to_string())? };
    let envelope: serde_json::Value = serde_json::from_str(&create_conv)?;
    let id = envelope["data"]["id"].as_str().unwrap_or_default().to_string();
    let get_conv = unsafe { get_object(id.clone())? };
    let list_conv = unsafe { list_objects(entity_type_id, "10".to_string())? };
    Ok(format!("create={create_conv}; get={get_conv}; list={list_conv}"))
}

// Фиктивный помощник, чтобы `Error` был задействован (never-type fallback не
// смешивает `!` с возвращаемым типом плагина).
#[allow(dead_code)]
fn _as_error(e: extism_pdk::Error) -> Error {
    e
}
