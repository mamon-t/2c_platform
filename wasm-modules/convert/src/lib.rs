// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

use extism_pdk::*;
use serde::{Deserialize, Serialize};

mod csv_fmt;
mod json_fmt;
mod yaml_fmt;
mod xml_fmt;

// ── Plugin self-description ────────────────────────────────

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
    pub functions: Vec<PluginFunction>,
}

// ── Request / Result types ─────────────────────────────────

#[derive(Serialize, Deserialize)]
pub struct ImportRequest {
    pub format: String,
    pub file_data: Vec<u8>,
    pub entity_type_id: String,
    pub mapping: Option<std::collections::HashMap<String, String>>,
}

#[derive(Serialize, Deserialize)]
pub struct ImportResult {
    pub created: u32,
    pub total: u32,
    pub errors: Vec<String>,
    pub objects: Vec<serde_json::Value>,
}

#[derive(Serialize, Deserialize)]
pub struct ExportRequest {
    pub entity_type_id: String,
    pub format: String,
}

#[derive(Serialize, Deserialize)]
pub struct ExportResult {
    pub data: Vec<u8>,
    pub filename: String,
    pub content_type: String,
}

// ── Internal helpers ───────────────────────────────────────

fn parse_file(req: &ImportRequest) -> anyhow::Result<Vec<serde_json::Value>> {
    match req.format.as_str() {
        "csv" => csv_fmt::parse(&req.file_data, req.mapping.as_ref()),
        "json" => json_fmt::parse(&req.file_data),
        "yaml" => yaml_fmt::parse(&req.file_data),
        "xml" => xml_fmt::parse(&req.file_data),
        f => Err(anyhow::anyhow!("Unsupported format: {}", f)),
    }
}

fn export_file(format: &str, objects: &[serde_json::Value]) -> anyhow::Result<ExportResult> {
    match format {
        "csv" => csv_fmt::export(objects),
        "json" => json_fmt::export(objects),
        "yaml" => yaml_fmt::export(objects),
        "xml" => xml_fmt::export(objects),
        f => Err(anyhow::anyhow!("Unsupported format: {}", f)),
    }
}

// ── Host functions ─────────────────────────────────────────

#[host_fn]
extern "ExtismHost" {
    fn create_object(entity_type_id: String, data: String) -> String;
    fn list_objects(entity_type_id: String, limit: String) -> String;
    fn log_message(msg: String);
}

// ── Exported functions ─────────────────────────────────────

#[plugin_fn]
pub fn import_data(Json(req): Json<ImportRequest>) -> FnResult<Json<ImportResult>> {
    let _ = unsafe { log_message(format!(
        "[convert] import: format={}, entity={}, file_bytes={}",
        req.format, req.entity_type_id, req.file_data.len()
    )) };

    let objects = parse_file(&req)?;

    let mut created = 0u32;
    let mut errors = Vec::new();
    let mut result_objects = Vec::new();

    for obj in &objects {
        let data_str = serde_json::to_string(obj).unwrap_or_default();
        let call_result = unsafe { create_object(req.entity_type_id.clone(), data_str) };

        match call_result {
            Ok(response) => {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&response) {
                    if parsed.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                        created += 1;
                        result_objects.push(parsed);
                    } else if let Some(err) = parsed.get("error").and_then(|v| v.as_str()) {
                        errors.push(err.to_string());
                    }
                } else {
                    created += 1;
                }
            }
            Err(e) => {
                errors.push(format!("Host error: {}", e));
            }
        }
    }

    let _ = unsafe { log_message(format!(
        "[convert] import done: created={}/total={}", created, objects.len()
    )) };

    Ok(Json(ImportResult {
        created,
        total: objects.len() as u32,
        errors,
        objects: result_objects,
    }))
}

#[plugin_fn]
pub fn export_data(Json(req): Json<ExportRequest>) -> FnResult<Json<ExportResult>> {
    let _ = unsafe { log_message(format!(
        "[convert] export: entity={}, format={}", req.entity_type_id, req.format
    )) };

    let objects_json = unsafe {
        list_objects(req.entity_type_id.clone(), "500".into())
    }?;

    let objects: Vec<serde_json::Value> = serde_json::from_str(&objects_json)
        .map_err(|e| anyhow::anyhow!("Parse list_objects response: {}", e))?;

    let _ = unsafe { log_message(format!(
        "[convert] export: got {} objects from host", objects.len()
    )) };

    let result = export_file(&req.format, &objects)?;

    let _ = unsafe { log_message(format!(
        "[convert] export done: {} bytes, filename={}", result.data.len(), result.filename
    )) };

    Ok(Json(result))
}

#[plugin_fn]
pub fn get_info() -> FnResult<Json<ModuleInfo>> {
    Ok(Json(ModuleInfo {
        name: "convert".into(),
        version: "0.1.0".into(),
        functions: vec![
            PluginFunction {
                name: "import_data".into(),
                label: "Импорт данных".into(),
                description: "Загрузка CSV/JSON/YAML/XML в систему. Файл парсится, каждый объект создаётся через create_object.".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "format": {
                            "type": "string",
                            "enum": ["csv", "json", "yaml", "xml"],
                            "description": "Формат входного файла"
                        },
                        "file_data": {
                            "type": "array",
                            "items": { "type": "integer" },
                            "description": "Байты файла (Array.from(Uint8Array))"
                        },
                        "entity_type_id": {
                            "type": "string",
                            "description": "UUID типа сущности для создаваемых объектов"
                        },
                        "mapping": {
                            "type": "object",
                            "description": "Маппинг колонок/ключей → кодов полей ({\"col_name\": \"field_code\"})"
                        }
                    },
                    "required": ["format", "file_data", "entity_type_id"]
                }),
            },
            PluginFunction {
                name: "export_data".into(),
                label: "Экспорт данных".into(),
                description: "Выгрузка объектов из системы в файл. Объекты запрашиваются через list_objects.".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "format": {
                            "type": "string",
                            "enum": ["csv", "json", "yaml", "xml"],
                            "description": "Формат выходного файла"
                        },
                        "entity_type_id": {
                            "type": "string",
                            "description": "UUID типа сущности для экспортируемых объектов"
                        }
                    },
                    "required": ["format", "entity_type_id"]
                }),
            },
        ],
    }))
}
