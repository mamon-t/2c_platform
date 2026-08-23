//! IPC-команды модуля оборудования.
//!
//! Права: devices.read (чтение), devices.manage (конфигурация/подключение/тест),
//! devices.use (использование сканов в документах). Каждая мутация — аудит.


use tauri::{AppHandle, State};
use tokio::sync::Mutex;

use crate::audit::AuditableAction;
use crate::commands::AppState;
use crate::core::middleware::CommandContext;

use super::service::DeviceService;
use super::{DeviceConfig, DeviceConfigInput, DeviceEvent, DeviceHandle};

/// Найти активное wedge-устройство компании или синтетический id.
async fn wedge_device_id(ctx: &CommandContext) -> String {
    let configs = DeviceService::list(&ctx.db, &ctx.company_id).await.unwrap_or_default();
    configs
        .into_iter()
        .find(|c| {
            c.is_active
                && matches!(c.kind, crate::devices::DeviceKind::BarcodeScanner)
                && matches!(c.connection, crate::devices::ConnectionKind::KeyboardWedge)
        })
        .map(|c| c.id)
        .unwrap_or_else(|| "wedge".to_string())
}

// ── Чтение ─────────────────────────────────────────────────

#[tauri::command]
pub async fn devices_list(state: State<'_, Mutex<AppState>>) -> Result<Vec<DeviceListItem>, String> {
    let state = state.lock().await;
    let ctx = CommandContext::extract(&state).map_err(|e| e.to_string())?;
    ctx.check_permission("devices.read").map_err(|e| e.to_string())?;
    let configs = DeviceService::list(&ctx.db, &ctx.company_id).await.map_err(|e| e.to_string())?;
    Ok(configs
        .into_iter()
        .map(|config| {
            let connected = state.devices.contains_key(&config.id);
            DeviceListItem { connected, config }
        })
        .collect())
}

/// Элемент списка: конфигурация + флаг живого подключения.
#[derive(serde::Serialize)]
pub struct DeviceListItem {
    pub connected: bool,
    #[serde(flatten)]
    pub config: DeviceConfig,
}

#[tauri::command]
pub async fn devices_get(id: String, state: State<'_, Mutex<AppState>>) -> Result<DeviceConfig, String> {
    let state = state.lock().await;
    let ctx = CommandContext::extract(&state).map_err(|e| e.to_string())?;
    ctx.check_permission("devices.read").map_err(|e| e.to_string())?;
    let cfg = DeviceService::get(&ctx.db, &id).await.map_err(|e| e.to_string())?;
    if cfg.company_id != ctx.company_id.0.to_string() {
        return Err("Доступ запрещён: устройство другой компании".into());
    }
    Ok(cfg)
}

// ── Конфигурация (manage) ──────────────────────────────────

#[tauri::command]
pub async fn devices_save(
    id: Option<String>,
    input: DeviceConfigInput,
    state: State<'_, Mutex<AppState>>,
) -> Result<DeviceConfig, String> {
    let state = state.lock().await;
    let ctx = CommandContext::extract(&state).map_err(|e| e.to_string())?;
    ctx.check_permission("devices.manage").map_err(|e| e.to_string())?;

    let is_new = id.is_none();
    let cfg = DeviceService::save(&ctx.db, &ctx.company_id, id, input)
        .await
        .map_err(|e| e.to_string())?;
    let _ = is_new;
    crate::audit_log!(state, ctx.db, AuditableAction::ConfigureDevice,
        target_id = cfg.id.clone());
    Ok(cfg)
}

#[tauri::command]
pub async fn devices_delete(id: String, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let mut state = state.lock().await;
    let ctx = CommandContext::extract(&state).map_err(|e| e.to_string())?;
    ctx.check_permission("devices.manage").map_err(|e| e.to_string())?;

    // Если подключено — останавливаем
    if let Some(handle) = state.devices.remove(&id) {
        let _ = handle.stop_tx.send(true);
        handle.task.abort();
        tracing::info!("[devices] остановлено при удалении: {id}");
    }

    DeviceService::delete(&ctx.db, &ctx.company_id, &id)
        .await
        .map_err(|e| e.to_string())?;
    crate::audit_log!(state, ctx.db, AuditableAction::ConfigureDevice,
        target_id = id);
    Ok(())
}

// ── Подключение (manage) ───────────────────────────────────

#[tauri::command]
pub async fn devices_connect(
    id: String,
    app: AppHandle,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let (db, company_id, cfg) = {
        let s = state.lock().await;
        let ctx = CommandContext::extract(&s).map_err(|e| e.to_string())?;
        ctx.check_permission("devices.manage").map_err(|e| e.to_string())?;
        let db_ref = s.db.as_ref().ok_or("Нет БД")?;
        let cfg = DeviceService::get(db_ref, &id).await.map_err(|e| e.to_string())?;
        if cfg.company_id != ctx.company_id.0.to_string() {
            return Err("Доступ запрещён: устройство другой компании".into());
        }
        (s.db.clone().ok_or("Нет БД")?, ctx.company_id.clone(), cfg)
    }; // лок отпущен до всей тяжёлой работы

    let driver = DeviceService::build_driver(&cfg)?;

    let already = {
        let s = state.lock().await;
        s.devices.contains_key(&id)
    };
    if already {
        return Err(format!("Устройство {id} уже подключено"));
    }

    let (tx, rx) = tokio::sync::mpsc::channel(256);
    let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
    driver.start(tx, stop_rx.clone()).await.map_err(|e| e.to_string())?;

    let task = DeviceService::spawn_pump(app, db, company_id, cfg.settings.clone(), rx, stop_rx);

    let mut s = state.lock().await;
    s.devices.insert(id.clone(), DeviceHandle { config: cfg, task, stop_tx });

    crate::audit_log!(s, get_db_ref(&s), AuditableAction::ConnectDevice, target_id = id);
    Ok(())
}

#[tauri::command]
pub async fn devices_disconnect(id: String, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let mut state = state.lock().await;
    let ctx = CommandContext::extract(&state).map_err(|e| e.to_string())?;
    ctx.check_permission("devices.manage").map_err(|e| e.to_string())?;

    let handle = state
        .devices
        .remove(&id)
        .ok_or_else(|| format!("Устройство {id} не подключено"))?;
    let _ = handle.stop_tx.send(true);
    handle.task.abort();

    crate::audit_log!(state, get_db_ref(&state), AuditableAction::DisconnectDevice, target_id = id);
    Ok(())
}

/// Тестовое подключение / ожидание данных.
#[tauri::command]
pub async fn devices_test(id: String, state: State<'_, Mutex<AppState>>) -> Result<String, String> {
    let state = state.lock().await;
    let ctx = CommandContext::extract(&state).map_err(|e| e.to_string())?;
    ctx.check_permission("devices.manage").map_err(|e| e.to_string())?;

    let cfg = DeviceService::get(&ctx.db, &id).await.map_err(|e| e.to_string())?;
    if cfg.company_id != ctx.company_id.0.to_string() {
        return Err("Доступ запрещён: устройство другой компании".into());
    }

    let result = DeviceService::test_driver(&cfg).await;
    let _ = &result;
    crate::audit_log!(state, ctx.db, AuditableAction::TestDevice,
        target_id = id);
    result
}

// ── Порты ──────────────────────────────────────────────────

#[derive(serde::Serialize)]
pub struct PortDto {
    pub path: String,
    pub description: String,
}

#[tauri::command]
pub async fn devices_list_ports(state: State<'_, Mutex<AppState>>) -> Result<Vec<PortDto>, String> {
    let state = state.lock().await;
    let ctx = CommandContext::extract(&state).map_err(|e| e.to_string())?;
    ctx.check_permission("devices.manage").map_err(|e| e.to_string())?;
    Ok(DeviceService::list_ports()
        .into_iter()
        .map(|p| PortDto { path: p.path, description: p.description })
        .collect())
}

// ── Keyboard wedge (devices.use) ───────────────────────────

/// Скан из «клавиатурного» сканера: фронт ловит ввод в barcode-поле
/// и присылает готовый код. Событие идёт в ту же «Трубу» + UI-push.
#[tauri::command]
pub async fn devices_wedge_scan(code: String, app: AppHandle, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let state = state.lock().await;
    let ctx = CommandContext::extract(&state).map_err(|e| e.to_string())?;
    ctx.check_permission("devices.use").map_err(|e| e.to_string())?;

    let code = code.trim().to_string();
    if code.len() < 4 {
        return Err("Слишком короткий код".into());
    }

    let device_id = wedge_device_id(&ctx).await;
    let ev = DeviceEvent::Scanned { device_id, code };

    crate::devices::service::write_device_event(&ctx.db, &ctx.company_id, &ev)
        .await
        .map_err(|e| e.to_string())?;

    use tauri::Emitter;
    let _ = app.emit("device-event", serde_json::json!({ "device_id": ev.device_id(), "event": &ev }));
    Ok(())
}

/// Доступ к MongoClient для audit_log! внутри команд с уже отпущенным контекстом.
fn get_db_ref(s: &AppState) -> crate::db::MongoClient {
    s.db.clone().expect("БД не подключена")
}
