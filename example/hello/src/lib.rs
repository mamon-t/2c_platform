// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

//! hello — минимальный пример WASM-модуля 2C Platform (Plugin API v1).
//!
//! Демонстрирует три вещи:
//! 1. Манифест `get_info()` — единственный источник правды о модуле;
//!    вызывается хостом при установке и каждой загрузке.
//! 2. Конверт host-функций `{ok, data | error{code, message}}` и его
//!    развёртка (`unwrap_host`).
//! 3. Экспортируемые функции с JSON-входом/выходом через `#[plugin_fn]`.
//!
//! Сборка: `cargo build --release` (таргет задаёт .cargo/config.toml).
//! Артефакт: target/wasm32-unknown-unknown/release/hello_plugin.wasm

use extism_pdk::*;
use serde::{Deserialize, Serialize};

// ── Манифест ─────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub struct PluginFunction {
    pub name: String,
    pub label: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// Форма ModuleInfo ≥1.0 — идентична requests/trade/stock.
/// Хост десериализует в WasmModuleInfo; отсутствующие поля берутся из default.
#[derive(Serialize, Deserialize)]
pub struct ModuleInfo {
    pub name: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_version: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub handled_documents: Vec<String>,
    pub functions: Vec<PluginFunction>,
}

// ── Конверт host-функций ─────────────────────────────────────

/// Развернуть `{ok, data | error{code, message}}`.
fn unwrap_host(raw: String) -> anyhow::Result<serde_json::Value> {
    let v: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| anyhow::anyhow!("host вернул невалидный JSON: {e}"))?;
    if v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false) {
        Ok(v.get("data").cloned().unwrap_or(serde_json::Value::Null))
    } else {
        let code = v["error"]["code"].as_str().unwrap_or("UNKNOWN");
        let msg = v["error"]["message"].as_str().unwrap_or("");
        Err(anyhow::anyhow!("{code}: {msg}"))
    }
}

// ── Host functions ───────────────────────────────────────────

#[host_fn]
extern "ExtismHost" {
    fn log_message(msg: String);
    fn now_ms() -> String;
}

// ── Запросы/ответы ───────────────────────────────────────────

#[derive(Deserialize)]
pub struct EchoRequest {
    pub text: String,
}

// ── Экспортируемые функции ───────────────────────────────────

/// Эхо: возвращает текст обратно + длину. Демонстрирует логирование
/// и работу без обращения к данным платформы.
#[plugin_fn]
pub fn echo(Json(req): Json<EchoRequest>) -> FnResult<Json<serde_json::Value>> {
    let _ = unsafe { log_message(format!("[hello] echo: {} симв.", req.text.chars().count())) };

    Ok(Json(serde_json::json!({
        "echo": req.text,
    })))
}

/// Время хоста: демонстрирует host-fn с СЫРЫМ возвратом.
/// Большинство host-fn возвращают конверт {ok,data|error}, но простые
/// сервисные (`now_ms`, `log_message`) — голое значение без обёртки.
#[plugin_fn]
pub fn host_time() -> FnResult<Json<serde_json::Value>> {
    let raw = unsafe { now_ms() }?;
    let unix_ms = raw.trim().parse::<i64>().unwrap_or(0);

    Ok(Json(serde_json::json!({ "unix_ms": unix_ms })))
}

/// Обязательная самоописка модуля.
#[plugin_fn]
pub fn get_info() -> FnResult<Json<ModuleInfo>> {
    fn f(name: &str, label: &str, description: &str, props: serde_json::Value) -> PluginFunction {
        PluginFunction {
            name: name.into(),
            label: label.into(),
            description: description.into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": props,
            }),
        }
    }

    Ok(Json(ModuleInfo {
        name: "hello".into(),
        version: "0.1.0".into(),
        code: Some("hello".into()),
        author: Some("2C Platform".into()),
        description: Some(
            "Минимальный пример модуля: эхо и время хоста. Ничего не требует."
                .into(),
        ),
        api_version: Some("1.0".into()),
        // Только гранты на реально используемые host-fn.
        // RBAC-политик у модуля нет: permissions: [].
        capabilities: vec!["logging".into()],
        permissions: vec![],
        handled_documents: vec![],
        functions: vec![
            f("echo", "Эхо", "Возвращает переданный текст.",
              serde_json::json!({ "text": { "type": "string" } })),
            f("host_time", "Время хоста", "Unix-время в мс со стороны платформы.",
              serde_json::json!({})),
        ],
    }))
}
