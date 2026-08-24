// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

//! DeviceService: CRUD конфигураций + живые подключения + насос событий.

use std::sync::Arc;

use futures::StreamExt;
use mongodb::bson::{doc, Document};
use tokio::sync::{mpsc, watch};

use crate::core::{CompanyId, PlatformError, PlatformResult};
use crate::db::MongoClient;
use crate::events::{EventService, StreamType};

use super::{DeviceConfig, DeviceConfigInput, DeviceDriver, DeviceEvent, DeviceKind};

pub const COLLECTION: &str = "devices";

pub struct PortInfo {
    pub path: String,
    pub description: String,
}

pub struct DeviceService;

impl DeviceService {
    // ── CRUD ───────────────────────────────────────────────

    pub async fn list(db: &MongoClient, company_id: &CompanyId) -> PlatformResult<Vec<DeviceConfig>> {
        let col = db.collection::<Document>(COLLECTION);
        let mut cursor = col
            .find(doc! { "company_id": company_id.0.to_string() })
            .sort(doc! { "name": 1 })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;

        let mut result = Vec::new();
        while let Some(Ok(d)) = cursor.next().await {
            if let Some(c) = deserialize_device(&d) {
                result.push(c);
            }
        }
        Ok(result)
    }

    pub async fn get(db: &MongoClient, id: &str) -> PlatformResult<DeviceConfig> {
        let col = db.collection::<Document>(COLLECTION);
        let d = col
            .find_one(doc! { "_id": id })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?
            .ok_or_else(|| PlatformError::NotFound(format!("Устройство {id} не найдено")))?;
        deserialize_device(&d).ok_or_else(|| PlatformError::Internal("Ошибка десериализации устройства".into()))
    }

    pub async fn save(
        db: &MongoClient,
        company_id: &CompanyId,
        id: Option<String>,
        input: DeviceConfigInput,
    ) -> PlatformResult<DeviceConfig> {
        if input.name.trim().is_empty() {
            return Err(PlatformError::Validation("Укажите название устройства".into()));
        }

        let now = chrono::Utc::now();
        let cfg = match id {
            Some(id) => {
                let mut existing = Self::get(db, &id).await?;
                if existing.company_id != company_id.0.to_string() {
                    return Err(PlatformError::PermissionDenied("Устройство другой компании".into()));
                }
                existing.kind = input.kind;
                existing.name = input.name;
                existing.connection = input.connection;
                existing.settings = input.settings;
                existing.is_active = input.is_active;
                existing.updated_at = now;
                existing
            }
            None => DeviceConfig {
                id: uuid::Uuid::new_v4().to_string(),
                company_id: company_id.0.to_string(),
                kind: input.kind,
                name: input.name,
                connection: input.connection,
                settings: input.settings,
                is_active: input.is_active,
                created_at: now,
                updated_at: now,
            },
        };

        let col = db.collection::<Document>(COLLECTION);
        col.replace_one(doc! { "_id": &cfg.id }, serialize_device(&cfg))
            .upsert(true)
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(cfg)
    }

    pub async fn delete(db: &MongoClient, company_id: &CompanyId, id: &str) -> PlatformResult<()> {
        let cfg = Self::get(db, id).await?;
        if cfg.company_id != company_id.0.to_string() {
            return Err(PlatformError::PermissionDenied("Устройство другой компании".into()));
        }
        db.collection::<Document>(COLLECTION)
            .delete_one(doc! { "_id": id })
            .await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(())
    }

    // ── Порты ──────────────────────────────────────────────

    pub fn list_ports() -> Vec<PortInfo> {
        match serialport::available_ports() {
            Ok(ports) => ports
                .into_iter()
                .map(|p| match p.port_type {
                    serialport::SerialPortType::UsbPort(info) => PortInfo {
                        path: p.port_name,
                        description: info.product.unwrap_or_else(|| "USB устройство".into()),
                    },
                    serialport::SerialPortType::PciPort => PortInfo { path: p.port_name, description: "PCI".into() },
                    _ => PortInfo { path: p.port_name, description: "Последовательный порт".into() },
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    // ── Драйверы ───────────────────────────────────────────

    pub fn build_driver(cfg: &DeviceConfig) -> Result<Arc<dyn DeviceDriver>, String> {
        use super::ConnectionKind as CK;
        match (&cfg.kind, &cfg.connection) {
            (DeviceKind::BarcodeScanner, CK::Serial { port, baud }) => {
                Ok(Arc::new(crate::devices::scanner::SerialScanner::new(port.clone(), *baud)))
            }
            (DeviceKind::Scale, CK::Serial { port, baud }) => {
                let pattern = cfg.settings.get("pattern").and_then(|p| p.as_str());
                let unit = cfg.settings.get("unit").and_then(|u| u.as_str());
                Ok(Arc::new(crate::devices::scale::SerialScale::new(
                    port.clone(),
                    *baud,
                    pattern,
                    unit,
                )?))
            }
            (DeviceKind::BarcodeScanner, CK::KeyboardWedge) | (DeviceKind::Scale, CK::KeyboardWedge) => {
                Err("KeyboardWedge не требует подключения (слушает фронтенд)".into())
            }
            (_, CK::Tcp { .. }) => Err("TCP-подключения пока не поддерживаются".into()),
            _ => Err("Такое сочетание типа устройства и подключения не поддерживается".into()),
        }
    }

    // ── Насос событий ──────────────────────────────────────

    /// Запустить цикл обработки DeviceEvent → Event Store + Rhai-handler + UI-push.
    pub fn spawn_pump(
        app: tauri::AppHandle,
        db: MongoClient,
        company_id: CompanyId,
        settings: serde_json::Value,
        mut rx: mpsc::Receiver<DeviceEvent>,
        mut stop_rx: watch::Receiver<bool>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    ev = rx.recv() => {
                        match ev {
                            Some(ev) => Self::handle_event(&app, &db, &company_id, &settings, ev).await,
                            None => break,
                        }
                    }
                    _ = stop_rx.changed() => {
                        if *stop_rx.borrow() { break; }
                    }
                }
            }
        })
    }

    async fn handle_event(
        app: &tauri::AppHandle,
        db: &MongoClient,
        company_id: &CompanyId,
        settings: &serde_json::Value,
        ev: DeviceEvent,
    ) {
        // 1. «Труба»
        if let Err(e) = write_device_event(db, company_id, &ev).await {
            tracing::warn!("[devices] EventStore {}: {e}", ev.event_type());
        }

        // Сохранить уведомление для критичных событий (error/disconnect)
        let ev_type = ev.event_type();
        if ev_type.contains("error") || ev_type.contains("disconnect") || ev_type.contains("connected") {
            let n_doc = doc! {
                "company_id": company_id.0.to_string(),
                "user_id": ev.device_id(),
                "notification_type": ev_type,
                "severity": if ev_type.contains("error") { "warning" } else { "info" },
                "title": format!("Устройство {}", ev.device_id()),
                "body": format!("{:?}", ev),
                "status": "delivered",
                "created_at": mongodb::bson::DateTime::now(),
            };
            if let Err(e) = db.collection::<Document>("notifications")
                .insert_one(n_doc).await {
                tracing::warn!("[devices] notification: {e}");
            }
        }

        // 2. Rhai-обработчик из настроек устройства (опционально)
        if let Some(handler) = settings.get("scan_handler").and_then(|h| h.as_str()) {
            let ctx = serde_json::json!({ "event": &ev });
            let src = format!(
                "let ctx = parse_json({});\n{}",
                serde_json::to_string(&ctx).unwrap_or_else(|_| "null".into()),
                handler
            );
            let sandbox = crate::rhai::Sandbox::new(5_000, 5_000_000);
            if let Err(e) = sandbox.execute(&src, ev.event_type()) {
                tracing::warn!("[devices] scan_handler: {e}");
            }
        }

        // 3. Push в UI
        use tauri::Emitter;
        let payload = serde_json::json!({ "device_id": ev.device_id(), "event": &ev });
        if let Err(e) = app.emit("device-event", payload) {
            tracing::warn!("[devices] emit: {e}");
        }
    }

    /// Тест подключения/приёма данных.
    pub async fn test_driver(cfg: &DeviceConfig) -> Result<String, String> {
        match (&cfg.kind, &cfg.connection) {
            (_, super::ConnectionKind::KeyboardWedge) => {
                Ok("KeyboardWedge готов: отсканируйте код в тестовом поле".into())
            }
            (DeviceKind::BarcodeScanner, super::ConnectionKind::Serial { port, baud }) => {
                crate::devices::scanner::SerialScanner::new(port.clone(), *baud)
                    .test()
                    .await
            }
            (DeviceKind::Scale, super::ConnectionKind::Serial { port, baud }) => {
                let pattern = cfg.settings.get("pattern").and_then(|p| p.as_str());
                let unit = cfg.settings.get("unit").and_then(|u| u.as_str());
                crate::devices::scale::SerialScale::new(port.clone(), *baud, pattern, unit)
                    .map_err(|e| e)?
                    .test()
                    .await
            }
            _ => Err("Тест для этого сочетания не поддерживается".into()),
        }
    }
}

/// Записать событие устройства в Event Store (системный actor).
pub async fn write_device_event(
    db: &MongoClient,
    company_id: &CompanyId,
    ev: &DeviceEvent,
) -> PlatformResult<()> {
    let svc = EventService::new();
    let payload = serde_json::to_value(ev).unwrap_or_default();
    svc.append(
        db,
        StreamType::Device,
        ev.device_id(),
        ev.event_type(),
        payload,
        super::system_actor(company_id.clone()),
        company_id.clone(),
        None,
        None,
    )
    .await
    .map(|_| ())
}

// ── Сериализация (ручная: enum со структурными вариантами) ──

fn serialize_device(c: &DeviceConfig) -> Document {
    let conn_doc = match &c.connection {
        super::ConnectionKind::KeyboardWedge => doc! { "kind": "keyboard_wedge" },
        super::ConnectionKind::Serial { port, baud } => doc! { "kind": "serial", "port": port, "baud": *baud as i64 },
        super::ConnectionKind::Tcp { host, port } => doc! { "kind": "tcp", "host": host, "port": *port as i64 },
    };
    doc! {
        "_id": &c.id,
        "company_id": &c.company_id,
        "kind": c.kind.to_string(),
        "name": &c.name,
        "connection": conn_doc,
        "settings": mongodb::bson::to_bson(&c.settings).unwrap_or_default(),
        "is_active": c.is_active,
        "created_at": mongodb::bson::DateTime::from_millis(c.created_at.timestamp_millis()),
        "updated_at": mongodb::bson::DateTime::from_millis(c.updated_at.timestamp_millis()),
    }
}

fn deserialize_device(d: &Document) -> Option<DeviceConfig> {
    let id = d.get_str("_id").ok()?.to_string();
    let company_id = d.get_str("company_id").ok()?.to_string();
    let kind = match d.get_str("kind").ok()? {
        "barcode_scanner" | "BarcodeScanner" => DeviceKind::BarcodeScanner,
        "scale" | "Scale" => DeviceKind::Scale,
        "fiscal_printer" | "FiscalPrinter" => DeviceKind::FiscalPrinter,
        "label_printer" | "LabelPrinter" => DeviceKind::LabelPrinter,
        _ => return None,
    };
    let conn = d.get_document("connection").ok()?;
    let connection = match conn.get_str("kind").ok()? {
        "keyboard_wedge" => super::ConnectionKind::KeyboardWedge,
        "serial" => super::ConnectionKind::Serial {
            port: conn.get_str("port").ok()?.to_string(),
            baud: conn.get_i32("baud").unwrap_or(9600) as u32,
        },
        "tcp" => super::ConnectionKind::Tcp {
            host: conn.get_str("host").ok()?.to_string(),
            port: conn.get_i32("port").unwrap_or(0) as u16,
        },
        _ => return None,
    };
    let settings = d
        .get("settings")
        .cloned()
        .map(|b| mongodb::bson::from_bson::<serde_json::Value>(b).unwrap_or_default())
        .unwrap_or_default();

    let ts = |k: &str| {
        d.get_datetime(k)
            .ok()
            .and_then(|v| chrono::DateTime::from_timestamp_millis(v.timestamp_millis()))
            .unwrap_or_else(chrono::Utc::now)
    };

    Some(DeviceConfig {
        id,
        company_id,
        kind,
        name: d.get_str("name").ok()?.to_string(),
        connection,
        settings,
        is_active: d.get_bool("is_active").unwrap_or(false),
        created_at: ts("created_at"),
        updated_at: ts("updated_at"),
    })
}
