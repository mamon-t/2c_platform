// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

pub mod renderer;
pub mod seed;
pub mod service;
pub mod commands;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ── Enum ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PaperFormat {
    A4,
    A5,
    Letter,
}

impl PaperFormat {
    pub fn css(&self) -> &'static str {
        match self {
            PaperFormat::A4 => "size: A4",
            PaperFormat::A5 => "size: A5",
            PaperFormat::Letter => "size: Letter",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Orientation {
    Portrait,
    Landscape,
}

// ── Print Template ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrintMargins {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

impl Default for PrintMargins {
    fn default() -> Self {
        Self { top: 20.0, right: 15.0, bottom: 20.0, left: 15.0 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrintTemplate {
    pub _id: crate::core::Id,
    pub code: String,
    pub name: String,
    pub entity_type: String,
    pub form_code: String,
    pub template_body: String,
    pub css_styles: String,
    pub paper_format: PaperFormat,
    pub orientation: Orientation,
    pub margins: PrintMargins,
    pub is_default: bool,
    pub is_active: bool,
    pub version: i32,
    pub valid_from: Option<DateTime<Utc>>,
    pub valid_to: Option<DateTime<Utc>>,
    pub company_id: Option<String>,
    pub before_print_script: Option<String>,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ── Input types ────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct CreatePrintTemplateInput {
    pub code: String,
    pub name: String,
    pub entity_type: String,
    pub form_code: String,
    pub template_body: String,
    pub css_styles: Option<String>,
    pub paper_format: Option<PaperFormat>,
    pub orientation: Option<Orientation>,
    pub margins: Option<PrintMargins>,
    pub is_default: Option<bool>,
    pub before_print_script: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdatePrintTemplateInput {
    pub name: Option<String>,
    pub template_body: Option<String>,
    pub css_styles: Option<String>,
    pub paper_format: Option<PaperFormat>,
    pub orientation: Option<Orientation>,
    pub margins: Option<PrintMargins>,
    pub is_default: Option<bool>,
    pub is_active: Option<bool>,
    pub before_print_script: Option<String>,
}

// ── View Model ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct PrintInfo {
    pub print_date: String,
    pub page_number: i32,
    pub total_pages: i32,
    pub watermark: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrintContext {
    pub object: serde_json::Value,
    pub entity_type: serde_json::Value,
    pub company: serde_json::Value,
    pub parent: Option<serde_json::Value>,
    pub computed: serde_json::Value,
    pub print_info: PrintInfo,
}
