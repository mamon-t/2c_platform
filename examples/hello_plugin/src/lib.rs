//! Пример WASM-модуля для хоста платформы (подфазы 8a–8b, 9c и 9d).
//!
//! Экспортирует `get_info()` (манифест v2), `greet()` (приветствие с вызовом
//! host-функций `whoami`/`now_ms`), `kv_probe()` (запись/чтение KV-хранилища),
//! `objects_probe()` (создание/чтение/список объектов «Доски» через
//! host-функции подфазы 8b), пробы подфазы 9c: `events_probe()`
//! (эмиссия события через `emit_event`), `run_script()` (заглушка Rhai)
//! и `stubs_probe()` (заглушки уведомлений и подписей), а также пробу подфазы
//! 9d `users_probe()` (`users_by_role`). Собирается отдельным
//! крейтом с целью `wasm32-unknown-unknown` и вне workspace:
//! `cargo build --release --target wasm32-unknown-unknown`. Экспортирует также
//! `type_by_code_probe()` (резолв типа сущности по коду) и демонстрирует
//! поле `function` манифеста (команда `echo` выполняет экспорт `greet`).

use extism_pdk::{Error, FnResult};

/// Импорты host-функций платформы в namespace `ExtismHost`. Функции требуют
/// `pub`, чтобы быть видимыми из модуля плагина (макрос генерирует обёртки
/// с видимостью объявленной foreign-функции).
mod host {
    #[extism_pdk::host_fn("ExtismHost")]
    extern "ExtismHost" {
        pub fn whoami() -> String;
        pub fn now_ms() -> String;
        pub fn log_message(msg: String);
        pub fn kv_put(key: String, value: String) -> String;
        pub fn kv_get(key: String) -> String;
        pub fn create_object(entity_type_id: String, data_json: String) -> String;
        pub fn get_object(id: String) -> String;
        pub fn list_objects(entity_type_id: String, limit: String) -> String;
        pub fn get_entity_type_by_code(code: String) -> String;
        pub fn update_object(id: String, data_json: String, version: String) -> String;
        pub fn emit_event(stream_id: String, event_type: String, payload_json: String) -> String;
        pub fn run_script(source: String, ctx_json: String) -> String;
        pub fn notify_user(recipient_user_id: String, subject: String, body: String) -> String;
        pub fn users_by_role(role_id: String) -> String;
        pub fn signature_required(
            module_code: String,
            action: String,
            object_id: String,
        ) -> String;
        pub fn cms_verify(data_b64: String, sig_b64: String) -> String;
        pub fn tx_begin(business_key: String) -> String;
        pub fn tx_add_op(handle: String, op_type: String, params_json: String) -> String;
        pub fn tx_commit(handle: String) -> String;
    }
}

/// Манифест модуля v2 (раздел 9 ТЗ).
#[extism_pdk::plugin_fn]
pub fn get_info() -> FnResult<String> {
    let manifest = r#"{
        "code": "hello",
        "version": "1.0.0",
        "display_name": "Пример приветствия",
        "description": "Демонстрирует host-функции подфаз 8a–8b и 9c",
        "author": "2C Platform",
        "api_version": "2.0",
        "capabilities": ["logging", "storage", "objects.create", "objects.read", "objects.update",
            "events.emit", "scripts", "notifications", "signature", "transactions"],
        "commands": [{
            "code": "greet",
            "name": "Поздороваться",
            "required_permission": "hello.greet"
        }, {
            "code": "events_probe",
            "name": "Проба emit_event"
        }, {
            "code": "run_script",
            "name": "Проба run_script"
        }, {
            "code": "tx_probe",
            "name": "Проба транзакций"
        }, {
            "code": "echo",
            "name": "Эхо (выполняет функцию greet)",
            "function": "greet",
            "required_permission": "hello.greet"
        }],
        "permissions": [{
            "code": "hello.greet",
            "description": "Право поздороваться через модуль hello",
            "scope_type": { "module": "hello" },
            "record_access": "owned",
            "actions": [{
                "entity_type": "greeting",
                "actions": ["create", "read"],
                "compose_action": false
            }]
        }],
        "object_schemas": [{
            "code": "greeting",
            "name": "Приветствие",
            "kind": "catalog",
            "fields": [{ "code": "text", "name": "Текст", "kind": "string", "required": true }]
        }],
        "print_templates": [],
        "scripts": [{
            "code": "hello.double",
            "name": "Удвоить amount",
            "script_type": "formula",
            "source": "ctx.object.amount * 2",
            "entity_type": "greeting",
            "description": "Демонстрационный скрипт-формула: удваивает ctx.object.amount"
        }],
        "metadata_version": 1,
        "handles_documents": [],
        "navigation": [],
        "dependencies": []
    }"#;
    Ok(manifest.to_string())
}

/// Приветствие: использует `whoami`, `now_ms` и `log_message`.
#[extism_pdk::plugin_fn]
pub fn greet() -> FnResult<String> {
    let who = unsafe { host::whoami()? };
    let ts = unsafe { host::now_ms()? };
    let _ = unsafe { host::log_message(format!("greet от {who}")) };
    Ok(format!("Привет, {who}! (ms={ts})"))
}

/// Проверка KV-хранилища: пишет ключ и читает его обратно.
#[extism_pdk::plugin_fn]
pub fn kv_probe() -> FnResult<String> {
    let key = "demo_key".to_string();
    let value = serde_json::to_string("hello_value").unwrap();
    let put_conv = unsafe { host::kv_put(key.clone(), value)? };
    let get_conv = unsafe { host::kv_get(key)? };
    Ok(format!("put={put_conv}; get={get_conv}"))
}

/// Проверка host-функций 8b: создаёт объект `greeting`, читает его по id и
/// перечисляет объекты типа. На вход получает id типа сущности (UUID).
#[extism_pdk::plugin_fn]
pub fn objects_probe(entity_type_id: String) -> FnResult<String> {
    let data = serde_json::json!({ "text": "Привет, бизнес-объект!" });
    let create_conv = unsafe { host::create_object(entity_type_id.clone(), data.to_string())? };
    let envelope: serde_json::Value = serde_json::from_str(&create_conv)?;
    let id = envelope["data"]["id"].as_str().unwrap_or_default().to_string();
    let get_conv = unsafe { host::get_object(id.clone())? };
    let list_conv = unsafe { host::list_objects(entity_type_id, "10".to_string())? };
    Ok(format!("create={create_conv}; get={get_conv}; list={list_conv}"))
}

/// Проверка OCC в `update_object`: вход — JSON-запрос
/// `{"id": ..., "data": {...}, "version": N}`; возвращает конверт хоста.
#[extism_pdk::plugin_fn]
pub fn update_probe(request: String) -> FnResult<String> {
    let req: serde_json::Value = serde_json::from_str(&request)?;
    let id = req["id"].as_str().unwrap_or_default().to_string();
    let data = req["data"].clone().to_string();
    let version = req["version"].as_u64().unwrap_or(0).to_string();
    let conv = unsafe { host::update_object(id, data, version)? };
    Ok(conv)
}

/// Прозрачная проба `get_object`: возвращает конверт хоста как есть.
#[extism_pdk::plugin_fn]
pub fn get_probe(id: String) -> FnResult<String> {
    let conv = unsafe { host::get_object(id)? };
    Ok(conv)
}

/// Прозрачная проба `list_objects`: возвращает конверт хоста как есть.
#[extism_pdk::plugin_fn]
pub fn list_probe(entity_type_id: String) -> FnResult<String> {
    let conv = unsafe { host::list_objects(entity_type_id, "10".to_string())? };
    Ok(conv)
}

/// Прозрачная проба `get_entity_type_by_code`: по коду типа сущности через
/// host-функцию (capability `metadata.read`). Возвращает конверт хоста как есть.
#[extism_pdk::plugin_fn]
pub fn type_by_code_probe(code: String) -> FnResult<String> {
    let conv = unsafe { host::get_entity_type_by_code(code)? };
    Ok(conv)
}

/// Проба подфазы 9c: эмитирует событие на поток объекта через `emit_event`
/// (capability `events.emit`). Возвращает конверт хоста как есть.
#[extism_pdk::plugin_fn]
pub fn events_probe() -> FnResult<String> {
    let stream_id = "11111111-2222-3333-4444-555555555555".to_string();
    let payload = serde_json::json!({
        "text": "Событие из hello через emit_event",
        "source": "events_probe"
    });
    let conv = unsafe {
        host::emit_event(
            stream_id,
            "hello.event.emitted".to_string(),
            payload.to_string(),
        )?
    };
    Ok(conv)
}

/// Проба host-функции `run_script` (capability `scripts`): выполняет
/// переданный источник через Rhai-движок с демонстрационным контекстом
/// `{object: {amount: 210}}` (позволяет проверить скрипт `hello.double`).
#[extism_pdk::plugin_fn]
pub fn run_script(source: String) -> FnResult<String> {
    let ctx = "{\"object\":{\"amount\":210}}".to_string();
    let conv = unsafe { host::run_script(source, ctx)? };
    Ok(conv)
}

/// Проба заглушек 9c: уведомления (`notify_user`, `users_by_role`) и подписи
/// (`signature_required`, `cms_verify`). Возвращает конверты через маркеры
/// для тестового разбора.
#[extism_pdk::plugin_fn]
pub fn stubs_probe() -> FnResult<String> {
    let script_conv = unsafe { host::run_script("log_info(\"hello-run-script\");".to_string(), "{}".to_string())? };
    let notify_conv = unsafe {
        host::notify_user(
            "00000000-0000-0000-0000-000000000001".to_string(),
            "тема".to_string(),
            "текст уведомления".to_string(),
        )?
    };
    let users_conv = unsafe {
        host::users_by_role("00000000-0000-0000-0000-000000000002".to_string())?
    };
    let sigreq_conv = unsafe {
        host::signature_required(
            "hello".to_string(),
            "object.sign".to_string(),
            "00000000-0000-0000-0000-000000000003".to_string(),
        )?
    };
    let cms_conv = unsafe { host::cms_verify("ZGF0YQ==".to_string(), "c2ln".to_string())? };
    Ok(format!(
        "script={script_conv}; notify={notify_conv}; users={users_conv}; sigreq={sigreq_conv}; cms={cms_conv}"
    ))
}

/// Проба подфазы 9d: `users_by_role` для заданной роли. Возвращает конверт
/// хоста как есть.
#[extism_pdk::plugin_fn]
pub fn users_probe(role_id: String) -> FnResult<String> {
    let conv = unsafe { host::users_by_role(role_id)? };
    Ok(conv)
}

/// Проба транзакций подфазы 9d: `tx_begin` → `tx_add_op(object.post)` →
/// `tx_commit`. Вход — JSON `{"object_id": "...", "expected_version": N}`.
/// Возвращает JSON с сырыми конвертами хоста для тестового разбора.
#[extism_pdk::plugin_fn]
pub fn tx_probe(request: String) -> FnResult<String> {
    let req: serde_json::Value = serde_json::from_str(&request)?;
    let object_id = req["object_id"].as_str().unwrap_or_default().to_string();
    let expected_version = req["expected_version"].as_u64().unwrap_or(0);

    let begin_conv = unsafe { host::tx_begin("tx-probe-key".to_string())? };
    let begin: serde_json::Value = serde_json::from_str(&begin_conv)?;
    let handle = begin["data"]["handle"].as_str().unwrap_or_default().to_string();

    let post_params = serde_json::json!({
        "id": object_id,
        "expected_version": expected_version,
    });
    let add_op_conv = unsafe {
        host::tx_add_op(handle.clone(), "object.post".to_string(), post_params.to_string())?
    };
    let commit_conv = unsafe { host::tx_commit(handle.clone())? };

    Ok(serde_json::json!({
        "begin": begin_conv,
        "add_op": add_op_conv,
        "commit": commit_conv,
    })
    .to_string())
}

/// Проба $ref-связывания подфазы 9d: `test.noop` отражает параметры, а
/// `object.post` берёт id и версию через `{"$ref": "<op_id>.params.…"}`.
/// Вход — JSON `{"object_id": "...", "expected_version": N}`. Возвращает
/// JSON с сырыми конвертами хоста.
#[extism_pdk::plugin_fn]
pub fn tx_ref_probe(request: String) -> FnResult<String> {
    let req: serde_json::Value = serde_json::from_str(&request)?;
    let object_id = req["object_id"].as_str().unwrap_or_default().to_string();
    let expected_version = req["expected_version"].as_u64().unwrap_or(0);

    let begin_conv = unsafe { host::tx_begin("tx-ref-probe-key".to_string())? };
    let begin: serde_json::Value = serde_json::from_str(&begin_conv)?;
    let handle = begin["data"]["handle"].as_str().unwrap_or_default().to_string();

    let noop_params = serde_json::json!({
        "target_id": object_id,
        "expected_version": expected_version,
    });
    let noop_conv = unsafe {
        host::tx_add_op(handle.clone(), "test.noop".to_string(), noop_params.to_string())?
    };
    let noop: serde_json::Value = serde_json::from_str(&noop_conv)?;
    let op1_id = noop["data"]["op_id"].as_str().unwrap_or_default().to_string();

    let post_params = serde_json::json!({
        "id": { "$ref": format!("{op1_id}.params.target_id") },
        "expected_version": { "$ref": format!("{op1_id}.params.expected_version") },
    });
    let add_op_conv = unsafe {
        host::tx_add_op(handle.clone(), "object.post".to_string(), post_params.to_string())?
    };
    let commit_conv = unsafe { host::tx_commit(handle.clone())? };

    Ok(serde_json::json!({
        "begin": begin_conv,
        "noop": noop_conv,
        "add_op": add_op_conv,
        "commit": commit_conv,
    })
    .to_string())
}

/// Проба идемпотентности подфазы 9d: два `tx_begin` с одинаковым
/// business_key должны вернуть один и тот же handle.
#[extism_pdk::plugin_fn]
pub fn tx_idem_probe() -> FnResult<String> {
    let first_conv = unsafe { host::tx_begin("tx-idem-probe-key".to_string())? };
    let first: serde_json::Value = serde_json::from_str(&first_conv)?;
    let handle = first["data"]["handle"].as_str().unwrap_or_default().to_string();
    let noop_conv = unsafe {
        host::tx_add_op(handle.clone(), "test.noop".to_string(), "{}".to_string())?
    };
    let second_conv = unsafe { host::tx_begin("tx-idem-probe-key".to_string())? };
    Ok(serde_json::json!({
        "first": first_conv,
        "noop": noop_conv,
        "second": second_conv,
    })
    .to_string())
}

// Фиктивный помощник, чтобы `Error` был задействован (never-type fallback не
// смешивает `!` с возвращаемым типом плагина).
#[allow(dead_code)]
fn _as_error(e: extism_pdk::Error) -> Error {
    e
}