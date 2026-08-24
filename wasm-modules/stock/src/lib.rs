//! Модуль «Склад» — оркестратор складских документов.
//!
//! Движок остатков живёт на хосте (реестр tx_exec: stock.receipt/issue/
//! transfer/handover/handover_return/count/reverse). Этот плагин —
//! ОРКЕСТРАТОР: при проведении документа собирает короткую пачку
//! [складские операции…, object.post] через tx-сессию и коммитит
//! одной транзакцией. Отмена — [stock.reverse, object.cancel].
//!
//! Манифест декларирует handles_documents: хост делегирует post_object/
//! cancel_object этих типов сюда (on_post / on_cancel).

use extism_pdk::*;
use serde::{Deserialize, Serialize};

// ── Host functions ─────────────────────────────────────────

#[host_fn]
extern "ExtismHost" {
    fn get_object(id: String) -> String;
    fn get_entity_type(id: String) -> String;
    fn tx_begin(business_key: String) -> String;
    fn tx_add_op(handle: String, op_name: String, params_json: String) -> String;
    fn tx_commit(handle: String) -> String;
    fn log_message(msg: String);
}

// ── Конверт host-вызовов ───────────────────────────────────

fn unwrap_host(raw: String) -> anyhow::Result<serde_json::Value> {
    let v: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| anyhow::anyhow!("host невалидный JSON: {e}"))?;
    if v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false) {
        Ok(v.get("data").cloned().unwrap_or(serde_json::Value::Null))
    } else {
        let code = v["error"]["code"].as_str().unwrap_or("UNKNOWN");
        let msg = v["error"]["message"].as_str().unwrap_or("");
        Err(anyhow::anyhow!("{code}: {msg}"))
    }
}

// ── Манифест ───────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub struct PluginFunction {
    pub name: String,
    pub label: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Serialize, Deserialize)]
pub struct ModuleInfo {
    pub name: String,
    pub version: String,
    pub code: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub api_version: Option<String>,
    pub capabilities: Vec<String>,
    pub permissions: Vec<String>,
    /// Типы документов, чьё проведение оркестрирует модуль.
    pub handled_documents: Vec<String>,
    pub functions: Vec<PluginFunction>,
}

#[plugin_fn]
pub fn get_info() -> FnResult<Json<ModuleInfo>> {
    fn f(name: &str, label: &str, description: &str) -> PluginFunction {
        PluginFunction {
            name: name.into(),
            label: label.into(),
            description: description.into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": { "id": {"type": "string"} },
                "required": ["id"],
            }),
        }
    }

    Ok(Json(ModuleInfo {
        name: "stock".into(),
        version: "0.1.0".into(),
        code: Some("stock".into()),
        author: Some("2C Platform".into()),
        description: Some(
            "Склад: перемещения, инвентаризация, выдача/возврат под отчёт. \
             Проведение документов атомарно с движением остатков через tx_exec."
                .into(),
        ),
        api_version: Some("1.0".into()),
        capabilities: vec![
            "objects.read".into(),
            "objects.update".into(),
            "metadata.read".into(),
            "transactions".into(),
            "logging".into(),
        ],
        permissions: vec![
            "stock.read".into(),
            "stock.use".into(),
        ],
        handled_documents: vec![
            "MOVE".into(),
            "COUNT".into(),
            "HANDOVER".into(),
            "HANDOVER_RETURN".into(),
        ],
        functions: vec![
            f("on_post", "Проведение", "Пачка: складская операция + object.post."),
            f("on_cancel", "Отмена", "Пачка: stock.reverse + object.cancel."),
        ],
    }))
}

// ── Оркестрация ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct DocInput {
    pub id: String,
    #[serde(default)]
    pub expected_version: Option<i64>,
}

/// Прочитать документ и код его типа сущности.
fn load_doc(id: &str) -> anyhow::Result<(serde_json::Value, String)> {
    let raw = unsafe { get_object(id.to_string()) }?;
    let obj = unwrap_host(raw)?;
    let et_id = obj["entity_type_id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("документ без entity_type_id"))?
        .to_string();
    let et_raw = unsafe { get_entity_type(et_id.clone()) }?;
    let et = unwrap_host(et_raw)?;
    let code = et["code"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("тип сущности без кода"))?
        .to_string();
    Ok((obj, code))
}

/// Добавить операцию в пачку, вернуть op_id.
fn add(handle: &str, op: &str, params: serde_json::Value) -> anyhow::Result<String> {
    let raw = unsafe { tx_add_op(
        handle.to_string(),
        op.to_string(),
        params.to_string(),
    ) }?;
    let data = unwrap_host(raw)?;
    Ok(data["op_id"].as_str().unwrap_or("?").to_string())
}

#[plugin_fn]
pub fn on_post(Json(input): Json<DocInput>) -> FnResult<Json<serde_json::Value>> {
    let (doc, type_code) = load_doc(&input.id)?;
    let data = doc.get("data").cloned().unwrap_or_default();

    let handle_raw = unsafe { tx_begin(format!("post-{}", input.id)) }?;
    let handle = unwrap_host(handle_raw)?;
    let h = handle["handle"].as_str().ok_or_else(|| anyhow::anyhow!("tx_begin без handle"))?;

    // Складская операция по типу документа
    match type_code.as_str() {
        "MOVE" => {
            add(h, "stock.transfer", serde_json::json!({
                "from_location_id": data["from_location_id"],
                "to_location_id": data["to_location_id"],
                "lines": data["lines"],
                "doc_kind": "MOVE",
                "doc_id": input.id,
            }))?;
        }
        "HANDOVER" => {
            add(h, "stock.handover", serde_json::json!({
                "from_location_id": data["from_location_id"],
                "to_location_id": data["to_location_id"],
                "lines": data["lines"],
                "responsible_user_id": data["responsible_user_id"],
                "expected_return_date": data["expected_return_date"],
                "doc_kind": "HANDOVER",
                "doc_id": input.id,
            }))?;
        }
        "COUNT" => {
            add(h, "stock.count", serde_json::json!({
                "location_id": data["location_id"],
                "lines": data["lines"],
                "doc_kind": "COUNT",
                "doc_id": input.id,
            }))?;
        }
        other => {
            return Err(anyhow::anyhow!(
                "VALIDATION: тип {other} не оркестрируется модулем склада"
            ).into());
        }
    }

    // Переход документа — в той же пачке
    let ver = input.expected_version.unwrap_or_else(|| {
        doc["version"].as_i64().unwrap_or(1)
    });
    add(h, "object.post", serde_json::json!({
        "object_id": input.id,
        "expected_version": ver,
    }))?;

    let commit_raw = unsafe { tx_commit(h.to_string()) }?;
    let result = unwrap_host(commit_raw)?;

    let _ = unsafe { log_message(format!(
        "[stock] документ {} проведён атомарно", input.id
    )) };

    Ok(Json(serde_json::json!({
        "posted": true,
        "request_id": input.id,
        "op_results": result["op_results"],
    })))
}

/// Отмена проведённого складского документа: сторно движений + object.cancel.
#[plugin_fn]
pub fn on_cancel(Json(input): Json<DocInput>) -> FnResult<Json<serde_json::Value>> {
    let (_doc, _code) = load_doc(&input.id)?;

    let handle_raw = unsafe { tx_begin(format!("cancel-{}", input.id)) }?;
    let handle = unwrap_host(handle_raw)?;
    let h = handle["handle"].as_str().ok_or_else(|| anyhow::anyhow!("tx_begin без handle"))?;

    add(h, "stock.reverse", serde_json::json!({ "target_doc_id": input.id }))?;

    let ver = input.expected_version.unwrap_or(0);
    add(h, "object.cancel", serde_json::json!({
        "object_id": input.id,
        "expected_version": ver,
    }))?;

    let commit_raw = unsafe { tx_commit(h.to_string()) }?;
    let result = unwrap_host(commit_raw)?;

    let _ = unsafe { log_message(format!(
        "[stock] документ {} отменён (сторно выполнено)", input.id
    )) };

    Ok(Json(serde_json::json!({
        "cancelled": true,
        "request_id": input.id,
        "op_results": result["op_results"],
    })))
}
