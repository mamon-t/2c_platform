//! KV-хранилище для WASM-модулей (capability: `storage`).
//!
//! Контракт Plugin SDK (единый для всех host-функций):
//! - успех:  `{"ok": true,  "data": ...}`
//! - ошибка: `{"ok": false, "error": {"code": "...", "message": "..."}}`
//!
//! Изоляция данных: ключи автоматически неймспейсятся хостом.
//! Полный ключ в БД = `{company_id}:{module_code}:{key}`.
//! Модуль оперирует только своим срезом — доступ к чужим данным
//! невозможен даже при попытке подменить ключ.
//!
//! # Контракты функций
//!
//! | Функция | Аргументы | Результат data |
//! |---|---|---|
//! | `kv_put(key, value_json)` | value — JSON строка | `{"key": "..."}` |
//! | `kv_get(key)` | | `{"found": bool, "value": any}` |
//! | `kv_list(prefix)` | prefix может быть "" | `{"items": [{"key", "value"}]}` |
//! | `kv_delete(key)` | | `{"deleted": 0|1}` |

use mongodb::bson::{doc, Document};
use crate::audit::service::AuditService as _;

use super::{check_capability, err, error_response, ok_response, HostData};

pub const COLLECTION_MODULE_STORE: &str = "module_store";


/// Аудит записи/удаления KV (actor из контекста плагина). Warn-and-forget.
fn audit_kv(hd: &HostData, action: crate::audit::AuditableAction, ns_key_full: &str) {
    let ctx = hd.ctx.read().unwrap();
    let Some(cid) = ctx.company_id.as_ref().and_then(|c| uuid::Uuid::parse_str(c).ok()) else {
        return;
    };
    let uid = crate::core::UserId(
        ctx.user_id.as_ref().and_then(|u| uuid::Uuid::parse_str(u).ok()).unwrap_or_default(),
    );
    let entry = crate::audit::AuditEntry::new(
        action,
        uid,
        crate::core::CompanyId(cid),
        Some(ns_key_full.to_string()),
        hd.module_code.clone(),
        None,
        None,
        None,
        None,
        None,
        None,
    );
    if let Some(db) = &hd.db {
        let db = db.clone();
        let fut = async move {
            if let Err(e) = crate::audit::service::MongoAuditService::new().log(&db, entry).await {
                tracing::warn!("[kv audit] {e}");
            }
        };
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(fut);
        });
    }
}

/// Собрать неймспейс-ключ из контекста плагина.
/// Ошибка → готовый error_response.
fn ns_key(hd: &HostData) -> Result<String, String> {
    let ctx = hd.ctx.read().unwrap();
    let company = ctx.company_id.as_deref().ok_or_else(|| {
        error_response(err::NO_COMPANY, "Компания не выбрана")
    })?;
    let module_code = hd.module_code.as_deref().ok_or_else(|| {
        error_response(err::NO_MODULE_CODE, "Модуль без кода")
    })?;
    Ok(format!("{}:{}:", company, module_code))
}

fn db_or_err(hd: &HostData) -> Result<crate::db::MongoClient, String> {
    hd.db
        .clone()
        .ok_or_else(|| error_response(err::NO_DATABASE, "База данных не инициализирована"))
}

// ── kv_put(key, value_json) ────────────────────────────────

extism::host_fn!(pub kv_put_impl(user_data: HostData; key: String, value_json: String) -> String {
    let hd = user_data.get()?.lock().unwrap().clone();
    if let Err(e) = check_capability(&hd, "kv_put") {
        return Ok(e);
    }

    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let db = match db_or_err(&hd) { Ok(d) => d, Err(e) => return e };
            let prefix = match ns_key(&hd) { Ok(k) => k, Err(e) => return e };

            let value: serde_json::Value = match serde_json::from_str(&value_json) {
                Ok(v) => v,
                Err(e) => return error_response(err::INVALID_JSON, &format!("Невалидный JSON: {}", e)),
            };
            let value_bson = match mongodb::bson::to_bson(&value) {
                Ok(b) => b,
                Err(e) => return error_response(err::INVALID_JSON, &format!("BSON сериализация: {}", e)),
            };

            let ctx = hd.ctx.read().unwrap();
            let full_key = format!("{}{}", prefix, key);

            let set = doc! {
                "$set": {
                    "ns_key": &full_key,
                    "company_id": ctx.company_id.clone().unwrap_or_default(),
                    "module_code": hd.module_code.clone().unwrap_or_default(),
                    "key": &key,
                    "value": value_bson,
                    "updated_at": mongodb::bson::Bson::DateTime(mongodb::bson::DateTime::now()),
                }
            };

            let col = db.collection::<Document>(COLLECTION_MODULE_STORE);
            match col.update_one(doc! { "ns_key": &full_key }, set).upsert(true).await {
                Ok(_) => {
                    audit_kv(&hd, crate::audit::AuditableAction::ModuleKvPut, &full_key);
                    ok_response(serde_json::json!({ "key": key }))
                }
                Err(e) => error_response(err::DB_ERROR, &e.to_string()),
            }
        })
    });
    Ok(result)
});


// ── kv_put_if_absent(key, value_json) ──────────────────────

extism::host_fn!(pub kv_put_if_absent_impl(user_data: HostData; key: String, value_json: String) -> String {
    let hd = user_data.get()?.lock().unwrap().clone();
    if let Err(e) = check_capability(&hd, "kv_put_if_absent") {
        return Ok(e);
    }

    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let db = match db_or_err(&hd) { Ok(d) => d, Err(e) => return e };
            let prefix = match ns_key(&hd) { Ok(k) => k, Err(e) => return e };
            let ctx = hd.ctx.read().unwrap();
            let full_key = format!("{}{}", prefix, key);

            let value: serde_json::Value = match serde_json::from_str(&value_json) {
                Ok(v) => v,
                Err(e) => return error_response(err::INVALID_JSON, &format!("Невалидный JSON: {e}")),
            };
            let value_bson = match mongodb::bson::to_bson(&value) {
                Ok(b) => b,
                Err(e) => return error_response(err::INVALID_JSON, &format!("BSON: {e}")),
            };

            let rec = doc! {
                "_id": uuid::Uuid::new_v4().to_string(),
                "ns_key": &full_key,
                "company_id": ctx.company_id.clone().unwrap_or_default(),
                "module_code": hd.module_code.clone().unwrap_or_default(),
                "key": &key,
                "value": value_bson,
                "updated_at": mongodb::bson::DateTime::now(),
            };

            // Атомарность гонки: чистая вставка + уникальный индекс.
            // E11000 → ключ уже занят параллельным вызовом.
            let res = db.collection::<Document>(COLLECTION_MODULE_STORE)
                .insert_one(rec)
                .await;

            match res {
                Ok(_) => {
                    audit_kv(&hd, crate::audit::AuditableAction::ModuleKvPut, &full_key);
                    ok_response(serde_json::json!({ "created": true }))
                }
                Err(e) if e.to_string().contains("E11000")
                       || e.to_string().contains("duplicate key") => {
                    ok_response(serde_json::json!({ "created": false }))
                }
                Err(e) => error_response(err::DB_ERROR, &e.to_string()),
            }
        })
    });
    Ok(result)
});

// ── kv_get(key) ────────────────────────────────────────────

extism::host_fn!(pub kv_get_impl(user_data: HostData; key: String) -> String {
    let hd = user_data.get()?.lock().unwrap().clone();
    if let Err(e) = check_capability(&hd, "kv_get") {
        return Ok(e);
    }

    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let db = match db_or_err(&hd) { Ok(d) => d, Err(e) => return e };
            let prefix = match ns_key(&hd) { Ok(k) => k, Err(e) => return e };
            let full_key = format!("{}{}", prefix, key);

            let col = db.collection::<Document>(COLLECTION_MODULE_STORE);
            match col.find_one(doc! { "ns_key": &full_key }).await {
                Ok(Some(doc)) => {
                    let value = doc.get("value").cloned()
                        .map(|b| mongodb::bson::from_bson::<serde_json::Value>(b).unwrap_or(serde_json::Value::Null))
                        .unwrap_or(serde_json::Value::Null);
                    ok_response(serde_json::json!({ "found": true, "value": value }))
                }
                Ok(None) => ok_response(serde_json::json!({ "found": false, "value": null })),
                Err(e) => error_response(err::DB_ERROR, &e.to_string()),
            }
        })
    });
    Ok(result)
});

// ── kv_list(prefix) ────────────────────────────────────────

const KV_LIST_LIMIT: usize = 500;

extism::host_fn!(pub kv_list_impl(user_data: HostData; prefix: String) -> String {
    let hd = user_data.get()?.lock().unwrap().clone();
    if let Err(e) = check_capability(&hd, "kv_list") {
        return Ok(e);
    }

    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let db = match db_or_err(&hd) { Ok(d) => d, Err(e) => return e };
            let ns_prefix = match ns_key(&hd) { Ok(k) => k, Err(e) => return e };
            let full_prefix = format!("{}{}", ns_prefix, prefix);

            // Регулярка с экранированием спецсимволов префикса
            let escaped = regex_escape(&full_prefix);

            let col = db.collection::<Document>(COLLECTION_MODULE_STORE);
            let mut cursor = match col
                .find(doc! { "ns_key": { "$regex": &escaped } })
                .limit(KV_LIST_LIMIT as i64)
                .await
            {
                Ok(c) => c,
                Err(e) => return error_response(err::DB_ERROR, &e.to_string()),
            };

            let mut items = Vec::new();
            while let Some(Ok(doc)) = cursor.next().await {
                let key = doc.get_str("key").unwrap_or("").to_string();
                let value = doc.get("value").cloned()
                    .map(|b| mongodb::bson::from_bson::<serde_json::Value>(b).unwrap_or(serde_json::Value::Null))
                    .unwrap_or(serde_json::Value::Null);
                items.push(serde_json::json!({ "key": key, "value": value }));
            }

            ok_response(serde_json::json!({ "items": items, "count": items.len() }))
        })
    });
    Ok(result)
});

// ── kv_delete(key) ─────────────────────────────────────────

extism::host_fn!(pub kv_delete_impl(user_data: HostData; key: String) -> String {
    let hd = user_data.get()?.lock().unwrap().clone();
    if let Err(e) = check_capability(&hd, "kv_delete") {
        return Ok(e);
    }

    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let db = match db_or_err(&hd) { Ok(d) => d, Err(e) => return e };
            let prefix = match ns_key(&hd) { Ok(k) => k, Err(e) => return e };
            let full_key = format!("{}{}", prefix, key);

            let col = db.collection::<Document>(COLLECTION_MODULE_STORE);
            match col.delete_one(doc! { "ns_key": &full_key }).await {
                Ok(res) => {
                    if res.deleted_count > 0 {
                        audit_kv(&hd, crate::audit::AuditableAction::ModuleKvDelete, &full_key);
                    }
                    ok_response(serde_json::json!({ "deleted": res.deleted_count }))
                }
                Err(e) => error_response(err::DB_ERROR, &e.to_string()),
            }
        })
    });
    Ok(result)
});

// ── Helpers ────────────────────────────────────────────────

use futures::StreamExt;

/// Экранировать спецсимволы регулярных выражений.
fn regex_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if "\\.^$*+?()[]{}|".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}
