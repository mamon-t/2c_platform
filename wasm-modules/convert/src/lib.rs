use extism_pdk::*;
use serde::{Deserialize, Serialize};

mod csv_fmt;
mod json_fmt;
mod yaml_fmt;
mod xml_fmt;

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
    pub objects: Vec<serde_json::Value>,
}

#[derive(Serialize, Deserialize)]
pub struct ExportResult {
    pub data: Vec<u8>,
    pub filename: String,
    pub content_type: String,
}

#[derive(Serialize, Deserialize)]
pub struct ModuleInfo {
    pub name: String,
    pub version: String,
    pub formats: Vec<String>,
}

fn parse_file(req: &ImportRequest) -> anyhow::Result<Vec<serde_json::Value>> {
    match req.format.as_str() {
        "csv" => csv_fmt::parse(&req.file_data, req.mapping.as_ref()),
        "json" => json_fmt::parse(&req.file_data),
        "yaml" => yaml_fmt::parse(&req.file_data),
        "xml" => xml_fmt::parse(&req.file_data),
        f => Err(anyhow::anyhow!("Unsupported format: {}", f)),
    }
}

fn export_file(req: &ExportRequest) -> anyhow::Result<ExportResult> {
    match req.format.as_str() {
        "csv" => csv_fmt::export(&req.objects),
        "json" => json_fmt::export(&req.objects),
        "yaml" => yaml_fmt::export(&req.objects),
        "xml" => xml_fmt::export(&req.objects),
        f => Err(anyhow::anyhow!("Unsupported format: {}", f)),
    }
}

#[host_fn]
extern "ExtismHost" {
    fn create_object(entity_type_id: String, data: String) -> String;
    fn log_message(msg: String);
}

#[plugin_fn]
pub fn import_data(Json(req): Json<ImportRequest>) -> FnResult<Json<ImportResult>> {
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

    Ok(Json(ImportResult {
        created,
        total: objects.len() as u32,
        errors,
        objects: result_objects,
    }))
}

#[plugin_fn]
pub fn export_data(Json(req): Json<ExportRequest>) -> FnResult<Json<ExportResult>> {
    let result = export_file(&req)?;
    Ok(Json(result))
}

#[plugin_fn]
pub fn get_info() -> FnResult<Json<ModuleInfo>> {
    Ok(Json(ModuleInfo {
        name: "convert".into(),
        version: "0.1.0".into(),
        formats: vec!["csv".into(), "json".into(), "yaml".into(), "xml".into()],
    }))
}
