//! Workflow host-функции для WASM-модулей:
//! - `transition_object` (capability: `objects.update`) — проведение/отмена объекта
//! - `run_script` (capability: `scripts`) — выполнение Rhai-скрипта в песочнице
//! - `notify_user` (capability: `notifications`) — запись in-app уведомления
//!
//! Контракт Plugin SDK: единый конверт `{ok, data | error{code, message}}`.


use mongodb::bson::{doc, Document};

use super::{check_capability, err, error_response, ok_response, HostData};

fn db_or_err(hd: &HostData) -> Result<crate::db::MongoClient, String> {
    hd.db
        .clone()
        .ok_or_else(|| error_response(err::NO_DATABASE, "База данных не инициализирована"))
}

/// Разобрать контекст вызова (company + user + actor).
struct CallCtx {
    company_id: crate::core::CompanyId,
    user_id: crate::core::UserId,
    actor: crate::events::ActorSnapshot,
}

fn call_ctx(hd: &HostData) -> Result<CallCtx, String> {
    let ctx = hd.ctx.read().unwrap();

    let company_id = match ctx.company_id.as_ref() {
        Some(cid) => match uuid::Uuid::parse_str(cid) {
            Ok(uid) => crate::core::CompanyId(uid),
            Err(_) => return Err(error_response(err::INVALID_COMPANY, "Невалидный UUID компании")),
        },
        None => return Err(error_response(err::NO_COMPANY, "Компания не выбрана")),
    };

    let user_id = match ctx.user_id.as_ref() {
        Some(uid) => match uuid::Uuid::parse_str(uid) {
            Ok(u) => crate::core::UserId(u),
            Err(_) => return Err(error_response(err::INVALID_USER, "Невалидный UUID пользователя")),
        },
        None => return Err(error_response(err::NO_USER, "Пользователь не аутентифицирован")),
    };

    let actor = crate::events::ActorSnapshot {
        user_id: user_id.clone(),
        login: ctx.user_login.clone().unwrap_or_default(),
        full_name: ctx.display_name.clone(),
        position: None,
        company_id: company_id.clone(),
    };

    Ok(CallCtx { company_id, user_id, actor })
}

// ── transition_object(id, version, action) ─────────────────
//
// action: "post" (Draft→Posted, присваивается номер)
//       | "cancel" (Posted→Cancelled)

extism::host_fn!(pub transition_object_impl(user_data: HostData; id: String, version: String, action: String) -> String {
    let hd = user_data.get()?.lock().unwrap().clone();
    if let Err(e) = check_capability(&hd, "transition_object") {
        return Ok(e);
    }

    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let db = match db_or_err(&hd) { Ok(d) => d, Err(e) => return e };
            let cctx = match call_ctx(&hd) { Ok(c) => c, Err(e) => return e };

            let uuid = match uuid::Uuid::parse_str(&id) {
                Ok(u) => u,
                Err(_) => return error_response(err::INVALID_UUID, "Невалидный UUID объекта"),
            };
            let ver: i64 = match version.parse() {
                Ok(v) => v,
                Err(_) => return error_response(err::INVALID_VERSION, "Версия должна быть числом"),
            };

            let outcome = match action.as_str() {
                "post" => crate::objects::service::ObjectService::post(
                    &db, uuid, ver, cctx.user_id, cctx.actor, cctx.company_id).await,
                "cancel" => crate::objects::service::ObjectService::cancel(
                    &db, uuid, ver, cctx.user_id, cctx.actor, cctx.company_id).await,
                other => {
                    return error_response(
                        err::INVALID_ACTION,
                        &format!("Неизвестное действие '{}', допустимо post|cancel", other),
                    )
                }
            };

            match outcome {
                Ok(outcome) => {
                    let state_str = serde_json::to_string(&outcome.result.state).unwrap_or_default()
                        .trim_matches('"').to_string();
                    ok_response(serde_json::json!({
                        "id": outcome.result._id.to_string(),
                        "version": outcome.result.version,
                        "state": state_str,
                        "number": outcome.result.number,
                    }))
                }
                Err(e) => error_response(err::DB_ERROR, &e.to_string()),
            }
        })
    });
    Ok(result)
});

// ── run_script(source, context_json) ───────────────────────
//
// Выполняет Rhai-скрипт в песочнице (лимиты как в ScriptsPage).
// Контекст доступен скрипту как переменная `ctx` (JSON строка → parse_json).

extism::host_fn!(pub run_script_impl(user_data: HostData; source: String, context_json: String) -> String {
    let hd = user_data.get()?.lock().unwrap().clone();
    if let Err(e) = check_capability(&hd, "run_script") {
        return Ok(e);
    }

    // Валидация JSON контекста до запуска
    let context_value: serde_json::Value = match serde_json::from_str(&context_json) {
        Ok(v) => v,
        Err(e) => return Ok(error_response(err::INVALID_JSON, &format!("Невалидный контекст: {}", e))),
    };

    // Песочница: те же лимиты, что и в execute_rhai_script
    let sandbox = crate::rhai::Sandbox::new(10_000, 10_000_000);
    // Контекст передаётся как СТРОКОВЫЙ литерал: parse_json("{"a":1}")
    // (двойное кодирование — иначе рхай видит map-литерал и падает по синтаксису)
    let ctx_literal = serde_json::to_string(
        &serde_json::to_string(&context_value).unwrap_or_else(|_| "null".into()),
    )
    .unwrap_or_else(|_| "\"null\"".into());
    let scope_source = format!("let ctx = parse_json({});\n{}", ctx_literal, source);

    match sandbox.execute(&scope_source, "{}") {
        Ok(result) => Ok(ok_response(serde_json::json!({ "result": result }))),
        Err(e) => Ok(error_response(err::SCRIPT_FAILED, &e.to_string())),
    }
});

// ── emit_event(stream_id, event_type, payload_json) ────────
//
// Модуль пишет собственное бизнес-событие в Event Store
// (StreamType::Module, stream_id = "{module}:{stream}").
// capability: events.emit.

extism::host_fn!(pub emit_event_impl(user_data: HostData; stream_id: String, event_type: String, payload_json: String) -> String {
    let hd = user_data.get()?.lock().unwrap().clone();
    if let Err(e) = check_capability(&hd, "emit_event") {
        return Ok(e);
    }

    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let db = match db_or_err(&hd) { Ok(d) => d, Err(e) => return e };
            let cctx = match call_ctx(&hd) { Ok(c) => c, Err(e) => return e };
            let module_code = hd.module_code.clone().unwrap_or_else(|| "module".into());

            if event_type.is_empty() || stream_id.is_empty() {
                return error_response(super::err::INVALID_JSON, "stream_id и event_type обязательны");
            }
            let payload: serde_json::Value = serde_json::from_str(&payload_json)
                .unwrap_or(serde_json::Value::Null);

            let svc = crate::events::EventService::new();
            match svc.append(
                &db,
                crate::events::StreamType::Module,
                &format!("{module_code}:{stream_id}"),
                &event_type,
                payload,
                cctx.actor,
                cctx.company_id,
                None,
                None,
            ).await {
                Ok(_) => ok_response(serde_json::json!({ "emitted": true })),
                Err(e) => error_response(super::err::DB_ERROR, &e.to_string()),
            }
        })
    });
    Ok(result)
});

// ── whoami() ───────────────────────────────────────────────
//
// Идентичность вызывающего для гостя (обновляется при каждом plugin_call).
// Без capability — только чтение контекста сессии.

extism::host_fn!(pub whoami_impl(user_data: HostData;) -> String {
    let hd = user_data.get()?.lock().unwrap().clone();
    let ctx = hd.ctx.read().unwrap();
    Ok(serde_json::json!({
        "company_id": ctx.company_id,
        "user_id": ctx.user_id,
        "login": ctx.user_login,
        "display_name": ctx.display_name,
        "role_id": ctx.role_id,
    }).to_string())
});

// ── now_ms() ───────────────────────────────────────────────
//
// Текущее время в миллисекундах (гость не имеет доступа к часам).
// Без capability — безопасно.

extism::host_fn!(pub now_ms_impl(user_data: HostData;) -> String {
    let _hd = user_data.get()?.lock().unwrap().clone();
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis().to_string())
        .unwrap_or_else(|_| "0".into()))
});

// ── module_settings() ──────────────────────────────────────
//
// Настройки модуля для текущей компании (CompanyModule.settings).
// Свои настройки — без capability.

extism::host_fn!(pub module_settings_impl(user_data: HostData;) -> String {
    let hd = user_data.get()?.lock().unwrap().clone();
    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let db = match db_or_err(&hd) { Ok(d) => d, Err(e) => return e };
            let module_code = match hd.module_code.as_deref() {
                Some(c) => c,
                None => return error_response(super::err::NO_MODULE_CODE, "Модуль без кода"),
            };
            let company_str = match hd.ctx.read().unwrap().company_id.clone() {
                Some(c) => c,
                None => return error_response(super::err::NO_COMPANY, "Компания не выбрана"),
            };

            // company_modules.module_id -> modules._id по коду или имени
            let modules_col = db.collection::<Document>("modules");
            let module_doc = match modules_col
                .find_one(doc! { "$or": [doc! { "code": module_code }, doc! { "name": module_code }] })
                .await
            {
                Ok(Some(m)) => m,
                Ok(None) => return ok_response(serde_json::json!({})),
                Err(e) => return error_response(super::err::DB_ERROR, &e.to_string()),
            };
            let module_id = module_doc.get_str("_id").unwrap_or("");

            let cm_col = db.collection::<Document>("company_modules");
            match cm_col.find_one(doc! { "module_id": module_id, "company_id": &company_str }).await {
                Ok(Some(cm)) => {
                    let settings = cm.get("settings").cloned().unwrap_or(mongodb::bson::Bson::Null);
                    let value = mongodb::bson::from_bson::<serde_json::Value>(settings)
                        .unwrap_or(serde_json::json!({}));
                    ok_response(value)
                }
                Ok(None) => ok_response(serde_json::json!({})),
                Err(e) => error_response(super::err::DB_ERROR, &e.to_string()),
            }
        })
    });
    Ok(result)
});

// ── tx_begin / tx_add_op / tx_commit ───────────────────────
//
// Сборка транзакционной пачки из песочницы (capability: transactions).
// Mongo-транзакция открывается только на tx_commit. Политики прав
// загружаются на коммите по роли вызывающего (свежий снапшот).

extism::host_fn!(pub tx_begin_impl(user_data: HostData; business_key: String) -> String {
    let hd = user_data.get()?.lock().unwrap().clone();
    if let Err(e) = check_capability(&hd, "tx_begin") {
        return Ok(e);
    }

    let ctx = hd.ctx.read().unwrap();
    let company_id = match ctx.company_id.as_ref().and_then(|c| uuid::Uuid::parse_str(c).ok()) {
        Some(u) => crate::core::CompanyId(u),
        None => return Ok(error_response(err::NO_COMPANY, "Компания не выбрана")),
    };
    let actor = crate::events::ActorSnapshot {
        user_id: crate::core::UserId(
            ctx.user_id.as_ref().and_then(|u| uuid::Uuid::parse_str(u).ok()).unwrap_or_default(),
        ),
        login: ctx.user_login.clone().unwrap_or_default(),
        full_name: ctx.display_name.clone(),
        position: None,
        company_id: company_id.clone(),
    };

    if business_key.trim().is_empty() {
        return Ok(error_response(err::INVALID_JSON, "business_key обязателен"));
    }

    let handle = crate::tx::session::begin(
        business_key,
        company_id,
        actor,
        ctx.role_id.clone(),
    );
    Ok(ok_response(serde_json::json!({ "handle": handle })))
});

extism::host_fn!(pub tx_add_op_impl(user_data: HostData; handle: String, op_name: String, params_json: String) -> String {
    let hd = user_data.get()?.lock().unwrap().clone();
    if let Err(e) = check_capability(&hd, "tx_add_op") {
        return Ok(e);
    }

    match crate::tx::session::add_op(&handle, &op_name, &params_json) {
        Ok(op_id) => Ok(ok_response(serde_json::json!({ "op_id": op_id }))),
        Err(msg) => Ok(error_response("VALIDATION", &msg)),
    }
});

extism::host_fn!(pub tx_commit_impl(user_data: HostData; handle: String) -> String {
    let hd = user_data.get()?.lock().unwrap().clone();
    if let Err(e) = check_capability(&hd, "tx_commit") {
        return Ok(e);
    }

    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let db = match db_or_err(&hd) { Ok(d) => d, Err(e) => return Ok(e) };

            // Свежий снапшот политик по роли вызывающего
            let role_id = hd.ctx.read().unwrap().role_id.clone();
            let policies = load_policies(&db, role_id).await;

            let pkg = match crate::tx::session::take_and_build(&handle, policies) {
                Ok(p) => p,
                Err(msg) => return Ok(error_response("NOT_FOUND", &msg)),
            };

            let outcome = match crate::tx::executor::execute(&db, pkg).await {
                Ok(tx_result) => match serde_json::to_value(&tx_result) {
                    Ok(v) => ok_response(v),
                    Err(e) => error_response(err::INVALID_JSON, &format!("результат: {e}")),
                },
                Err(tx_err) => error_response(
                    if tx_err.failed_op.is_some() { "TX_OP_FAILED" } else { "TX_FAILED" },
                    &tx_err.to_string(),
                ),
            };
            Ok(outcome)
        })
    });
    result
});

/// Загрузить политики роли (свежие, на момент коммита).
async fn load_policies(
    db: &crate::db::MongoClient,
    role_id: Option<String>,
) -> Vec<crate::permission_policy::PermissionPolicy> {
    use crate::role::RoleService;

    let Some(rid) = role_id.and_then(|s| uuid::Uuid::parse_str(&s).ok()) else {
        return Vec::new();
    };
    match RoleService::get(db, rid).await {
        Ok(role) => RoleService::get_policies(db, &role).await.unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

// ── notify_user(recipient_user_id, subject, body) ──────────
//
// Записывает in-app уведомление в общий outbox платформы.

extism::host_fn!(pub notify_user_impl(user_data: HostData; recipient_user_id: String, subject: String, body: String) -> String {
    let hd = user_data.get()?.lock().unwrap().clone();
    if let Err(e) = check_capability(&hd, "notify_user") {
        return Ok(e);
    }

    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let db = match db_or_err(&hd) { Ok(d) => d, Err(e) => return e };
            let cctx = match call_ctx(&hd) { Ok(c) => c, Err(e) => return e };

            let recipient = match uuid::Uuid::parse_str(&recipient_user_id) {
                Ok(u) => crate::core::UserId(u),
                Err(_) => return error_response(err::INVALID_UUID, "Невалидный UUID получателя"),
            };

            let notification = crate::notify::NotificationService::new().create_outbox_entry(
                cctx.company_id,
                "module.notify",
                crate::notify::NotificationChannel::InApp,
                recipient,
                if subject.is_empty() { None } else { Some(subject) },
                body,
            );

            match crate::notify::service::NotificationStore::save(&db, &notification).await {
                Ok(id) => ok_response(serde_json::json!({ "id": id })),
                Err(e) => error_response(err::DB_ERROR, &e.to_string()),
            }
        })
    });
    Ok(result)
});
