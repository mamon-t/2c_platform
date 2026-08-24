//! IPC-команды уведомлений.

use serde::Deserialize;
use tauri::State;
use tokio::sync::Mutex;

use crate::commands::AppState;
use crate::core::middleware::CommandContext;
use crate::core::{CompanyId, UserId};
use crate::db::MongoClient;
use mongodb::bson::{doc, Document};
use crate::notify::service::NotificationStore;
use crate::notify::{
    Notification, NotificationSeverity, NotificationSubscription,
};

fn db_of(s: &AppState) -> Result<MongoClient, String> {
    s.db.clone().ok_or_else(|| "Не подключено к MongoDB".into())
}

// ── Чтение ────────────────────────────────────────────────

#[tauri::command]
pub async fn notifications_count_unread(
    state: State<'_, Mutex<AppState>>,
) -> Result<i64, String> {
    let s = state.lock().await;
    let ctx = CommandContext::extract(&s).map_err(|e| e.to_string())?;
    let uid = UserId(ctx.user._id);
    NotificationStore::count_unread(&ctx.db, &uid).await.map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
pub struct NotificationDto {
    #[serde(rename = "_id")]
    pub id: String,
    pub user_id: String,
    pub company_id: String,
    pub notification_type: String,
    pub severity: String,
    pub title: String,
    pub body: String,
    pub entity_ref: Option<crate::notify::EntityRef>,
    pub status: String,
    pub read_at: Option<String>,
    pub created_at: String,
}

// ── Подписки ──────────────────────────────────────────────

#[tauri::command]
pub async fn notification_subscriptions_list(
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<NotificationSubscription>, String> {
    let s = state.lock().await;
    let ctx = CommandContext::extract(&s).map_err(|e| e.to_string())?;
    ctx.check_permission("notifications.read").map_err(|e| e.to_string())?;
    let db = db_of(&s)?;

    let mut cursor = db.collection::<Document>("notification_subscriptions")
        .find(doc! { "user_id": ctx.user._id.to_string() })
        .await.map_err(|e| e.to_string())?;
    use futures::StreamExt;
    let mut out = Vec::new();
    while let Some(Ok(d)) = cursor.next().await {
        if let Ok(sub) = mongodb::bson::from_document::<NotificationSubscription>(d) {
            out.push(sub);
        }
    }
    Ok(out)
}

#[tauri::command]
pub async fn notification_subscriptions_upsert(
    event_type: String,
    channels: Vec<String>,
    is_muted: bool,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let s = state.lock().await;
    let ctx = CommandContext::extract(&s).map_err(|e| e.to_string())?;
    ctx.check_permission("notifications.read").map_err(|e| e.to_string())?;
    let db = db_of(&s)?;

    db.collection::<Document>("notification_subscriptions")
        .update_one(
            doc! { "user_id": ctx.user._id.to_string(), "event_type": &event_type },
            doc! { "$set": {
                "channels": channels,
                "is_muted": is_muted,
                "updated_at": mongodb::bson::DateTime::now(),
            }, "$setOnInsert": {
                "_id": uuid::Uuid::new_v4().to_string(),
                "company_id": ctx.company_id.0.to_string(),
                "user_id": ctx.user._id.to_string(),
                "event_type": &event_type,
            }},
        )
        .upsert(true)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

// ── Шаблоны (admin) ───────────────────────────────────────

#[tauri::command]
pub async fn notification_templates_list(
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<Document>, String> {
    let s = state.lock().await;
    let ctx = CommandContext::extract(&s).map_err(|e| e.to_string())?;
    ctx.check_permission("settings.manage").map_err(|e| e.to_string())?;
    let db = db_of(&s)?;

    let mut cursor = db.collection::<Document>("notification_templates")
        .find(doc! { "$or": [
            { "company_id": ctx.company_id.0.to_string() },
            { "company_id": null },
        ]})
        .await.map_err(|e| e.to_string())?;
    use futures::StreamExt;
    let mut out = Vec::new();
    while let Some(Ok(d)) = cursor.next().await { out.push(d); }
    Ok(out)
}
