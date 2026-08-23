//! Модуль оборудования (сканеры, весы; ККМ — v0.3).
//!
//! Внутренний Rust-модуль (не WASM): прямой доступ к устройствам,
//! производительность, криптооперации ФН.
//!
//! Архитектура: конфигурации в MongoDB (`devices`, per company),
//! живые подключения — в `AppState.devices`. Данные от драйверов
//! текут через mpsc-канал в «насос», который пишет события в
//! Event Store (StreamType::Device), исполняет Rhai-обработчик из
//! settings устройства и пушит их в UI (tauri event "device-event").

pub mod service;
pub mod commands;
pub mod indexes;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, watch};

// ── Типы устройств ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DeviceKind {
    BarcodeScanner,
    Scale,
    FiscalPrinter,
    LabelPrinter,
}

impl std::fmt::Display for DeviceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = serde_json::to_string(self).unwrap_or_default();
        write!(f, "{}", s.trim_matches('"'))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionKind {
    /// HID-клавиатура: слушает фронтенд, ноль драйверов
    KeyboardWedge,
    Serial { port: String, baud: u32 },
    Tcp { host: String, port: u16 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceConfig {
    #[serde(rename = "_id")]
    pub id: String,
    pub company_id: String,
    pub kind: DeviceKind,
    pub name: String,
    pub connection: ConnectionKind,
    /// Протокольные настройки: pattern весов, scan_handler (Rhai) и т.п.
    pub settings: serde_json::Value,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Ввод для create/update.
#[derive(Debug, Clone, Deserialize)]
pub struct DeviceConfigInput {
    pub kind: DeviceKind,
    pub name: String,
    pub connection: ConnectionKind,
    #[serde(default)]
    pub settings: serde_json::Value,
    #[serde(default = "default_true")]
    pub is_active: bool,
}

fn default_true() -> bool { true }

// ── События устройств ──────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DeviceEvent {
    Scanned { device_id: String, code: String },
    Weighed { device_id: String, grams: u64, stable: bool },
    Connected { device_id: String },
    Disconnected { device_id: String },
    Error { device_id: String, message: String },
}

impl DeviceEvent {
    pub fn device_id(&self) -> &str {
        match self {
            Self::Scanned { device_id, .. }
            | Self::Weighed { device_id, .. }
            | Self::Connected { device_id }
            | Self::Disconnected { device_id }
            | Self::Error { device_id, .. } => device_id,
        }
    }

    /// Имя события для Event Store.
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::Scanned { .. } => "device.scanned",
            Self::Weighed { .. } => "device.weighed",
            Self::Connected { .. } => "device.connected",
            Self::Disconnected { .. } => "device.disconnected",
            Self::Error { .. } => "device.error",
        }
    }
}

// ── Трейт драйвера ─────────────────────────────────────────

/// Все драйверы устройств. Цикл чтения владеет насос (DeviceService):
/// драйвер лишь шлёт DeviceEvent в канал и уважает stop-сигнал.
#[async_trait::async_trait]
pub trait DeviceDriver: Send + Sync {
    async fn start(
        &self,
        tx: mpsc::Sender<DeviceEvent>,
        stop_rx: watch::Receiver<bool>,
    ) -> Result<(), String>;

    async fn test(&self) -> Result<String, String>;
}

// ── Живое подключение ──────────────────────────────────────

pub struct DeviceHandle {
    pub config: DeviceConfig,
    pub task: tokio::task::JoinHandle<()>,
    pub stop_tx: watch::Sender<bool>,
}

// ── Резерв под ККМ (v0.3) — критичные права и аудит ────────
//
// Политики точечные (каждая — отдельная строка permission_policies):
//   devices.fiscal_sell   — пробитие чека
//   devices.fiscal_refund — возврат
//   devices.fiscal_shift  — открытие/закрытие смены
//   devices.fiscal_report — отчёты (X/Z)
//
// AuditableAction резерв: FiscalizeReceipt, RefundReceipt, OpenShift,
// CloseShift — все с обязательным signature_ref (подпись ФН).
// Fiscal-команды идут строго через ctx.execute() middleware.

/// Системный actor для событий устройств.
pub fn system_actor(company_id: crate::core::CompanyId) -> crate::events::ActorSnapshot {
    crate::events::ActorSnapshot {
        user_id: crate::core::UserId(uuid::Uuid::nil()),
        login: "device".to_string(),
        full_name: None,
        position: None,
        company_id,
    }
}
