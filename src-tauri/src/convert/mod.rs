use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod plugin;
pub mod commands;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportRequest {
    pub format: String,
    pub file_data: Vec<u8>,
    pub entity_type_id: String,
    pub mapping: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub created: u32,
    pub total: u32,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportRequest {
    pub entity_type_id: String,
    pub format: String,
    pub objects: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    pub data: Vec<u8>,
    pub filename: String,
    pub content_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub path: String,
    pub formats: Vec<String>,
}
