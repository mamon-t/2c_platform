// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

//! IPC-команды сообщений.
//!
//! Право: trade.read — любой пользователь модуля торговли может общаться.

use mongodb::bson::{doc, Document};
use serde::Deserialize;
use tauri::State;
use tokio::sync::Mutex;

use crate::commands::AppState;
use crate::core::middleware::CommandContext;
use crate::db::MongoClient;

fn db_of(s: &AppState) -> Result<MongoClient, String> {
    s.db.clone().ok_or_else(|| "Не подключено к MongoDB".into())
}

// ── Комнаты ────────────────────────────────────────────────

#[tauri::command]
pub async fn messaging_rooms_list(
    room_type: Option<String>,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<serde_json::Value>, String> {
    let s = state.lock().await;
    let ctx = CommandContext::extract(&s).map_err(|e| e.to_string())?;
    ctx.check_permission("trade.read").map_err(|e| e.to_string())?;
    let uid = ctx.user._id.to_string();
    let db = ctx.db.clone();
    drop(s);
    super::service::MessagingService::list_rooms(&db, &ctx.company_id, &uid, room_type.as_deref())
        .await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn messaging_rooms_create(
    title: String,
    member_ids: Vec<String>,
    entity_ref: Option<serde_json::Value>,
    state: State<'_, Mutex<AppState>>,
) -> Result<Document, String> {
    let s = state.lock().await;
    let ctx = CommandContext::extract(&s).map_err(|e| e.to_string())?;
    let creator = ctx.user._id.to_string();
    let db = ctx.db.clone();
    drop(s);
    super::service::MessagingService::create_group_room(
        &db, &ctx.company_id, &title, member_ids, &creator, entity_ref,
    ).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn messaging_rooms_archive(
    room_id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let s = state.lock().await;
    let ctx = CommandContext::extract(&s).map_err(|e| e.to_string())?;
    ctx.check_permission("trade.read").map_err(|e| e.to_string())?;
    let db = db_of(&s)?;
    super::service::MessagingService::archive_room(&db, &ctx.company_id, &room_id, &ctx.user._id.to_string())
        .await.map_err(|e| e.to_string())
}

// ── Сообщения ──────────────────────────────────────────────

#[derive(serde::Deserialize)]
pub struct SendMessageInput {
    pub room_id: String,
    pub content: String,
    #[serde(default)]
    pub reply_to: Option<String>,
}

#[tauri::command]
pub async fn messaging_messages_send(
    input: SendMessageInput,
    state: State<'_, Mutex<AppState>>,
) -> Result<serde_json::Value, String> {
    let s = state.lock().await;
    let ctx = CommandContext::extract(&s).map_err(|e| e.to_string())?;
    ctx.check_permission("trade.read").map_err(|e| e.to_string())?;
    let author = ctx.user._id.to_string();
    let db = ctx.db.clone();
    drop(s);

    let msg = super::service::MessagingService::send_message(
        &db, &ctx.company_id, &input.room_id, &author, &input.content, input.reply_to.as_deref(),
    ).await.map_err(|e| e.to_string())?;
    serde_json::to_value(&msg).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn messaging_messages_list(
    room_id: String,
    limit: Option<i64>,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<serde_json::Value>, String> {
    let s = state.lock().await;
    let ctx = CommandContext::extract(&s).map_err(|e| e.to_string())?;
    let db = ctx.db.clone();
    drop(s);
    let msgs = super::service::MessagingService::list_messages(
        &db, &ctx.company_id, &room_id, limit.unwrap_or(100),
    ).await.map_err(|e| e.to_string())?;
    Ok(msgs.into_iter().map(|m| serde_json::to_value(&m).unwrap_or_default()).collect())
}

#[tauri::command]
pub async fn messaging_messages_edit(
    message_id: String,
    content: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let s = state.lock().await;
    let ctx = CommandContext::extract(&s).map_err(|e| e.to_string())?;
    ctx.check_permission("trade.read").map_err(|e| e.to_string())?;
    let author = ctx.user._id.to_string();
    let db = ctx.db.clone();
    drop(s);
    super::service::MessagingService::edit_message(
        &db, &ctx.company_id, &message_id, &author, &content
    ).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn messaging_messages_delete(
    message_id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let s = state.lock().await;
    let ctx = CommandContext::extract(&s).map_err(|e| e.to_string())?;
    ctx.check_permission("trade.read").map_err(|e| e.to_string())?;
    let author = ctx.user._id.to_string();
    let db = ctx.db.clone();
    drop(s);
    super::service::MessagingService::delete_message(
        &db, &ctx.company_id, &message_id, &author
    ).await.map_err(|e| e.to_string())
}

// ── Прочтения ──────────────────────────────────────────────

#[tauri::command]
pub async fn messaging_reads_update(
    room_id: String,
    last_message_id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let s = state.lock().await;
    let ctx = CommandContext::extract(&s).map_err(|e| e.to_string())?;
    ctx.check_permission("trade.read").map_err(|e| e.to_string())?;
    let uid = crate::core::UserId(ctx.user._id);
    let company = ctx.company_id.clone();
    let db = ctx.db.clone();
    drop(s);
    super::service::MessagingService::update_read(&db, &company, &room_id, &uid, &last_message_id)
        .await.map_err(|e| e.to_string())
}
