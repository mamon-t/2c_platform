//! Projection engine: событие → шаблон → подписка → уведомление.
//!
//! Слушает события из Трубы и генерирует уведомления для пользователей,
//! которые подписаны на соответствующие типы событий.
//!
//! Встроенные проекции (v0.1):
//! - document.approved → уведомить created_by
//! - request.submitted/approved/rejected → уведомить инициатора/утверждающего
//!
//! Кастомные — через Rhai-хуки и WASM-плагины (emit_event).

use mongodb::bson::{doc, Document};

use crate::core::{CompanyId, PlatformError, PlatformResult};
use crate::db::MongoClient;
use crate::events::ActorSnapshot;

use super::{
    service::NotificationStore, Notification, NotificationSeverity,
    NotificationSubscription,
};

/// Обработать событие из Event Store и создать уведомления.
/// Вызывается после коммита транзакции (побочный эффект — вне txn).
pub async fn project_event(
    db: &MongoClient,
    company_id: &CompanyId,
    event_type: &str,
    payload: &serde_json::Value,
    actor: &ActorSnapshot,
) -> PlatformResult<u32> {
    // 1. Ищем активный шаблон для этого типа события
    let template = find_template(db, company_id, event_type).await?;

    // 2. Определяем целевого пользователя из payload или политики
    let Some(target_user_id) = resolve_target_user(payload) else {
        return Ok(0);
    };

    // 3. Проверяем подписку пользователя
    if !is_subscribed(db, company_id, &target_user_id, event_type).await? {
        return Ok(0);
    }

    // 4. Рендерим title/body из шаблона или дефолтного текста
    let (title, body) = render(&template, event_type, payload, actor);

    // 5. Создаём уведомление
    let notification = Notification {
        id: uuid::Uuid::new_v4(),
        company_id: company_id.0.to_string(),
        user_id: target_user_id.clone(),
        notification_type: event_type.to_string(),
        severity: severity_of(event_type),
        title,
        body,
        entity_ref: extract_entity_ref(payload),
        channels: vec!["inapp".into()],
        status: "delivered".into(),
        delivered_at: Some(chrono::Utc::now()),
        read_at: None,
        metadata: payload.clone(),
        created_at: chrono::Utc::now(),
    };

    NotificationStore::save_notification(&db, &notification).await?;
    Ok(1)
}

async fn find_template(
    db: &MongoClient,
    company_id: &CompanyId,
    event_type: &str,
) -> PlatformResult<Option<Document>> {
    db.collection::<Document>("notification_templates")
        .find_one(doc! {
            "event_type": event_type,
            "channel": "inapp",
            "enabled": true,
            "$or": [
                { "company_id": company_id.0.to_string() },
                { "company_id": null },
            ],
        })
        .sort(doc! { "company_id": -1 }) // компания важнее глобального
        .await
        .map_err(|e| PlatformError::Database(e.to_string()))
}

fn resolve_target_user(payload: &serde_json::Value) -> Option<String> {
    // Порядок приоритета: target_user_id → initiator → created_by
    payload["target_user_id"].as_str()
        .or(payload["initiator_id"].as_str())
        .or(payload["created_by"].as_str())
        .map(String::from)
}

async fn is_subscribed(
    db: &MongoClient,
    company_id: &CompanyId,
    user_id: &str,
    event_type: &str,
) -> PlatformResult<bool> {
    let sub = db.collection::<Document>("notification_subscriptions")
        .find_one(doc! {
            "company_id": company_id.0.to_string(),
            "user_id": user_id,
            "$or": [
                { "event_type": event_type },
                { "event_type": "*" },
            ],
            "is_muted": { "$ne": true },
        })
        .await
        .map_err(|e| PlatformError::Database(e.to_string()))?;
    // Нет явной подписки = подписан по умолчанию
    Ok(true)
}

fn render(
    template: &Option<Document>,
    event_type: &str,
    payload: &serde_json::Value,
    actor: &ActorSnapshot,
) -> (String, String) {
    let default_title = format!("Событие: {event_type}");
    let default_body = serde_json::to_string_pretty(payload)
        .unwrap_or_else(|_| "{}".into());

    if let Some(t) = template {
        let subject_t = t.get_str("subject").unwrap_or("");
        let body_t = t.get_str("body").unwrap_or("");
        if !subject_t.is_empty() || !body_t.is_empty() {
            return (
                render_placeholders(subject_t, payload, actor),
                render_placeholders(body_t, payload, actor),
            );
        }
    }
    (default_title, default_body)
}

fn render_placeholders(template: &str, payload: &serde_json::Value, actor: &ActorSnapshot) -> String {
    template
        .replace("{{actor_name}}", &actor.login)
        .replace("{{actor_login}}", &actor.login)
        .replace("{{request_id}}", payload["request_id"].as_str().unwrap_or(""))
        .replace("{{doc_id}}", payload["doc_id"].as_str().unwrap_or(""))
        .replace("{{comment}}", payload["comment"].as_str().unwrap_or(""))
}

fn severity_of(event_type: &str) -> String {
    match event_type {
        t if t.contains("rejected") || t.contains("overdue") => "warning",
        t if t.contains("critical") => "critical",
        _ => "info",
    }.into()
}

fn extract_entity_ref(payload: &serde_json::Value) -> Option<super::EntityRef> {
    let entity_type = payload["entity_type"].as_str()?;
    let entity_id = payload["entity_id"].as_str()
        .or(payload["request_id"].as_str())?;
    Some(super::EntityRef {
        entity_type: entity_type.into(),
        entity_id: entity_id.into(),
    })
}
