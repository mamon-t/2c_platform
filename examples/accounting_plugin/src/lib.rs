//! WASM-модуль `plugin.accounting` — управленческий учёт (Фаза 14 ТЗ v3.1).
//!
//! Экспортирует `get_info()` (манифест v2) и 13 команд: план счетов
//! (`account.create/update/list/get`), учётные периоды
//! (`period.open/close/list`), журнал проводок (`entry.post/list/reverse`),
//! интеграцию с документами (`doc.post`) и отчёты (`balance.trial/sheet`).
//! Все сущности ведутся объектами платформы (Доска) через host-функции
//! `objects.*` и `metadata.read`; `doc.post` проводит документ и создаёт
//! проводку одной транзакцией (`transactions`). Балансы вычисляются на лету
//! из проведённых проводок. Собирается отдельным крейтом с целью
//! `wasm32-unknown-unknown` вне workspace:
//! `cargo build --release --target wasm32-unknown-unknown`.

use extism_pdk::{Error, FnResult};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

/// Импорты host-функций платформы в namespace `ExtismHost`.
mod host {
    #[extism_pdk::host_fn("ExtismHost")]
    extern "ExtismHost" {
        pub fn get_entity_type_by_code(code: String) -> String;
        pub fn create_object(entity_type_id: String, data_json: String) -> String;
        pub fn get_object(id: String) -> String;
        pub fn update_object(id: String, data_json: String, version: String) -> String;
        pub fn list_objects(entity_type_id: String, limit: String) -> String;
        pub fn emit_event(stream_id: String, event_type: String, payload_json: String) -> String;
        pub fn tx_begin(business_key: String) -> String;
        pub fn tx_add_op(handle: String, op_type: String, params_json: String) -> String;
        pub fn tx_commit(handle: String) -> String;
    }
}

/// Допустимые типы счетов управленческого плана (ТЗ §14).
const ACCOUNT_TYPES: &[&str] = &[
    "asset",
    "liability",
    "equity",
    "revenue",
    "expense",
    "off_balance",
];

/// Допустимая погрешность сверки дебета и кредита в проводке.
const BALANCE_EPS: f64 = 0.001;

/// Манифест модуля v2 (раздел 9 ТЗ). Имена WASM-экспортов команд заданы
/// полем `function` (подчёркиванием), registry-имена команд — манифест-кодом
/// с точками (`plugin.accounting.<code>`).
#[extism_pdk::plugin_fn]
pub fn get_info() -> FnResult<String> {
    let manifest = r#"{
        "code": "accounting",
        "version": "1.0.0",
        "display_name": "Управленческий учёт",
        "description": "План счетов, учётные периоды, проводки, ОСВ и баланс",
        "author": "2C Platform",
        "api_version": "2.0",
        "capabilities": ["objects.create", "objects.read", "objects.update",
            "events.emit", "transactions", "metadata.read"],
        "commands": [{
            "code": "account.create",
            "name": "Создать счёт",
            "function": "account_create",
            "required_permission": "accounting.manage"
        }, {
            "code": "account.update",
            "name": "Изменить счёт",
            "function": "account_update",
            "required_permission": "accounting.manage"
        }, {
            "code": "account.list",
            "name": "Список счетов",
            "function": "account_list",
            "required_permission": "accounting.read"
        }, {
            "code": "account.get",
            "name": "Карточка счёта",
            "function": "account_get",
            "required_permission": "accounting.read"
        }, {
            "code": "period.open",
            "name": "Открыть учётный период",
            "function": "period_open",
            "required_permission": "accounting.manage"
        }, {
            "code": "period.close",
            "name": "Закрыть учётный период",
            "function": "period_close",
            "required_permission": "accounting.manage"
        }, {
            "code": "period.list",
            "name": "Список учётных периодов",
            "function": "period_list",
            "required_permission": "accounting.read"
        }, {
            "code": "entry.post",
            "name": "Провести проводку",
            "function": "entry_post",
            "required_permission": "accounting.post"
        }, {
            "code": "entry.list",
            "name": "Журнал проводок",
            "function": "entry_list",
            "required_permission": "accounting.read"
        }, {
            "code": "entry.reverse",
            "name": "Сторно проводки",
            "function": "entry_reverse",
            "required_permission": "accounting.post"
        }, {
            "code": "doc.post",
            "name": "Провести документ с проводкой",
            "function": "doc_post",
            "required_permission": "accounting.post"
        }, {
            "code": "balance.trial",
            "name": "Оборотно-сальдовая ведомость",
            "function": "balance_trial",
            "required_permission": "accounting.read"
        }, {
            "code": "balance.sheet",
            "name": "Баланс",
            "function": "balance_sheet",
            "required_permission": "accounting.read"
        }],
        "permissions": [{
            "code": "accounting.manage",
            "description": "Управление планом счетов и учётными периодами",
            "scope_type": { "module": "accounting" },
            "record_access": "owned",
            "actions": [{
                "entity_type": "account",
                "actions": ["create", "update", "read"],
                "compose_action": false
            }, {
                "entity_type": "accounting_period",
                "actions": ["create", "update", "read"],
                "compose_action": false
            }]
        }, {
            "code": "accounting.read",
            "description": "Чтение счетов, периодов и журнала проводок",
            "scope_type": { "module": "accounting" },
            "record_access": "owned",
            "actions": [{
                "entity_type": "account",
                "actions": ["read"],
                "compose_action": false
            }, {
                "entity_type": "accounting_period",
                "actions": ["read"],
                "compose_action": false
            }, {
                "entity_type": "ledger_entry",
                "actions": ["read"],
                "compose_action": false
            }]
        }, {
            "code": "accounting.post",
            "description": "Проведение и сторно проводок",
            "scope_type": { "module": "accounting" },
            "record_access": "owned",
            "actions": [{
                "entity_type": "ledger_entry",
                "actions": ["create", "update", "read"],
                "compose_action": false
            }]
        }],
        "object_schemas": [{
            "code": "account",
            "name": "Счёт плана счетов",
            "kind": "catalog",
            "fields": [
                { "code": "code", "name": "Код счёта", "kind": "string", "required": true },
                { "code": "name", "name": "Наименование", "kind": "string", "required": true },
                { "code": "account_type", "name": "Тип счёта", "kind": "enum", "required": true, "options": ["asset", "liability", "equity", "revenue", "expense", "off_balance"] },
                { "code": "is_active", "name": "Активен", "kind": "boolean", "required": true }
            ]
        }, {
            "code": "accounting_period",
            "name": "Учётный период",
            "kind": "catalog",
            "fields": [
                { "code": "year", "name": "Год", "kind": "integer", "required": true },
                { "code": "month", "name": "Месяц", "kind": "integer", "required": true },
                { "code": "name", "name": "Наименование", "kind": "string" },
                { "code": "status", "name": "Статус", "kind": "enum", "required": true, "options": ["open", "closed"] }
            ]
        }, {
            "code": "ledger_entry",
            "name": "Проводка",
            "kind": "document",
            "fields": [
                { "code": "date", "name": "Дата", "kind": "date", "required": true },
                { "code": "description", "name": "Описание", "kind": "text" },
                { "code": "lines", "name": "Строки проводки", "kind": "table", "required": true },
                { "code": "status", "name": "Статус", "kind": "enum", "required": true, "options": ["draft", "posted", "reversed"] },
                { "code": "source_type", "name": "Тип источника", "kind": "string" },
                { "code": "source_id", "name": "Идентификатор источника", "kind": "string" },
                { "code": "correlation_entry_id", "name": "Сторнируемая проводка", "kind": "string" }
            ]
        }],
        "print_templates": [],
        "scripts": [],
        "metadata_version": 1,
        "handles_documents": [],
        "navigation": [],
        "dependencies": []
    }"#;
    Ok(manifest.to_string())
}

/// Обёртка выполнения команды: вход — JSON-строка, выход — JSON-конверт
/// `{ok, data|error}` (список ошибок приложения).
fn exec(input: String, f: impl FnOnce(Value) -> Result<Value, String>) -> String {
    let result = match serde_json::from_str::<Value>(&input) {
        Ok(req) => f(req),
        Err(e) => Err(format!("невалидный JSON входа: {e}")),
    };
    match result {
        Ok(data) => json!({ "ok": true, "data": data }).to_string(),
        Err(message) => {
            json!({ "ok": false, "error": { "code": "ACCOUNTING_ERROR", "message": message } })
                .to_string()
        }
    }
}

/// Разворачивает конверт host-функции (Приложение №6 ТЗ).
fn require_ok(conv: &str) -> Result<Value, String> {
    let v: Value = serde_json::from_str(conv)
        .map_err(|e| format!("невалидный конверт host-функции: {e}"))?;
    if v["ok"].as_bool().unwrap_or(false) {
        Ok(v.get("data").cloned().unwrap_or(Value::Null))
    } else {
        let code = v["error"]["code"].as_str().unwrap_or("HOST_ERROR");
        let msg = v["error"]["message"].as_str().unwrap_or("ошибка host-функции");
        Err(format!("{code}: {msg}"))
    }
}

/// Возвращает id типа сущности по его коду (компания — из сессии хоста).
fn entity_type_id(code: &str) -> Result<String, String> {
    let conv = (unsafe { host::get_entity_type_by_code(code.to_string()) })
        .map_err(|e| format!("host get_entity_type_by_code: {e}"))?;
    let data = require_ok(&conv)?;
    data["id"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| format!("get_entity_type_by_code('{code}'): нет id типа сущности"))
}

/// Перечисляет объекты типа сущности (через `objects.read`).
fn list_type(type_id: &str, limit: u32) -> Result<Vec<Value>, String> {
    let conv = (unsafe { host::list_objects(type_id.to_string(), limit.to_string()) })
        .map_err(|e| format!("host list_objects: {e}"))?;
    let data = require_ok(&conv)?;
    Ok(data
        .get("objects")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}

/// Создаёт объект (через `objects.create`).
fn host_create_object(type_id: &str, data: &Value) -> Result<Value, String> {
    let conv = (unsafe { host::create_object(type_id.to_string(), data.to_string()) })
        .map_err(|e| format!("host create_object: {e}"))?;
    require_ok(&conv)
}

/// Обновляет объект (через `objects.update`).
fn host_update_object(id: &str, data: &Value, version: u64) -> Result<Value, String> {
    let conv = (unsafe {
        host::update_object(id.to_string(), data.to_string(), version.to_string())
    })
    .map_err(|e| format!("host update_object: {e}"))?;
    require_ok(&conv)
}

/// Читает объект (через `objects.read`), возвращает `None`, если не найден.
fn host_get_object(id: &str) -> Result<Option<Value>, String> {
    let conv = (unsafe { host::get_object(id.to_string()) })
        .map_err(|e| format!("host get_object: {e}"))?;
    let data = require_ok(&conv)?;
    if data.is_null() {
        Ok(None)
    } else {
        Ok(Some(data))
    }
}

/// Контекст команды: id типов сущностей модуля и кэш счетов и периодов.
struct Ctx {
    account_type_id: String,
    period_type_id: String,
    entry_type_id: String,
    accounts: Vec<Value>,
    periods: Vec<Value>,
}

/// Собирает контекст: раскрывает коды типов сущностей в id и загружает
/// счета и периоды компании одной командой.
fn ctx() -> Result<Ctx, String> {
    let account_type_id = entity_type_id("account")?;
    let period_type_id = entity_type_id("accounting_period")?;
    let entry_type_id = entity_type_id("ledger_entry")?;
    let accounts = list_type(&account_type_id, 500)?;
    let periods = list_type(&period_type_id, 500)?;
    Ok(Ctx {
        account_type_id,
        period_type_id,
        entry_type_id,
        accounts,
        periods,
    })
}

fn str_param(v: &Value, key: &str) -> Result<String, String> {
    v[key]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| format!("нет строкового поля '{key}'"))
}

fn u64_param(v: &Value, key: &str) -> Result<u64, String> {
    v[key]
        .as_u64()
        .ok_or_else(|| format!("нет беззнакового целого поля '{key}'"))
}

/// Проверяет строки проводки: существование и активность счетов, отсутствие
/// отрицательных сумм, равенство дебета и кредита.
fn validate_entry_lines(ctx: &Ctx, req: &Value) -> Result<(), String> {
    let lines = req["lines"]
        .as_array()
        .ok_or("поле 'lines' должно быть массивом")?;
    if lines.is_empty() {
        return Err("проводка должна содержать хотя бы одну строку".to_string());
    }
    let mut debit_total = 0.0;
    let mut credit_total = 0.0;
    for (i, line) in lines.iter().enumerate() {
        let code = line["account"]
            .as_str()
            .ok_or_else(|| format!("строка {i}: нет поля 'account'"))?;
        let account = ctx
            .accounts
            .iter()
            .find(|a| a["data"]["code"].as_str() == Some(code))
            .ok_or_else(|| format!("счёт '{code}' не найден"))?;
        let is_active = account["data"]["is_active"].as_bool().unwrap_or(true);
        if !is_active {
            return Err(format!("счёт '{code}' деактивирован"));
        }
        let debit = line["debit"].as_f64().unwrap_or(0.0);
        let credit = line["credit"].as_f64().unwrap_or(0.0);
        if debit < 0.0 || credit < 0.0 {
            return Err(format!("строка {i}: отрицательные суммы недопустимы"));
        }
        debit_total += debit;
        credit_total += credit;
    }
    if (debit_total - credit_total).abs() > BALANCE_EPS {
        return Err(format!(
            "суммы дебета ({debit_total}) и кредита ({credit_total}) не сходятся"
        ));
    }
    if debit_total <= 0.0 {
        return Err("сумма проводки должна быть положительной".to_string());
    }
    Ok(())
}

/// Проверяет наличие открытого учётного периода для даты `YYYY-MM-DD`.
fn ensure_open_period(ctx: &Ctx, date: &str) -> Result<(), String> {
    let bin = date.as_bytes();
    if bin.len() < 7 {
        return Err(format!("дата '{date}' не в формате YYYY-MM-DD"));
    }
    let year: i64 = std::str::from_utf8(&bin[0..4])
        .ok()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| format!("дата '{date}' не в формате YYYY-MM-DD"))?;
    let month: i64 = std::str::from_utf8(&bin[5..7])
        .ok()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| format!("дата '{date}' не в формате YYYY-MM-DD"))?;
    let open = ctx.periods.iter().any(|p| {
        p["data"]["status"].as_str() == Some("open")
            && p["data"]["year"].as_i64() == Some(year)
            && p["data"]["month"].as_i64() == Some(month)
    });
    if !open {
        return Err(format!("нет открытого учётного периода для {date}"));
    }
    Ok(())
}

/// Возвращает свёрнутое представление объекта счёта.
fn account_row(obj: &Value) -> Value {
    json!({
        "id": obj["id"],
        "code": obj["data"]["code"],
        "name": obj["data"]["name"],
        "account_type": obj["data"]["account_type"],
        "is_active": obj["data"]["is_active"],
        "state": obj["state"],
    })
}

/// `account.create` — создаёт счёт плана счетов.
fn account_create_impl(req: Value) -> Result<Value, String> {
    let code = str_param(&req, "code")?;
    let name = str_param(&req, "name")?;
    let account_type = str_param(&req, "type")?;
    if !ACCOUNT_TYPES.contains(&account_type.as_str()) {
        return Err(format!(
            "недопустимый тип счёта '{account_type}'; допустимо: {}",
            ACCOUNT_TYPES.join(", ")
        ));
    }
    let is_active = req.get("is_active").and_then(Value::as_bool).unwrap_or(true);
    let ctx = ctx()?;
    if ctx
        .accounts
        .iter()
        .any(|a| a["data"]["code"].as_str() == Some(code.as_str()))
    {
        return Err(format!("счёт с кодом '{code}' уже существует"));
    }
    let data = json!({
        "code": code,
        "name": name,
        "account_type": account_type,
        "is_active": is_active,
    });
    let created = host_create_object(&ctx.account_type_id, &data)?;
    let id = created["id"].as_str().unwrap_or_default().to_string();
    let _ = (unsafe {
        host::emit_event(
            id.clone(),
            "account.created".to_string(),
            json!({ "code": code, "account_type": account_type }).to_string(),
        )
    })
    .map_err(|e| format!("host emit_event: {e}"))?;
    Ok(json!({ "id": id, "code": code, "account_type": account_type }))
}

/// `account.update` — изменяет счёт (частичное обновление данных).
fn account_update_impl(req: Value) -> Result<Value, String> {
    let id = str_param(&req, "id")?;
    let version = u64_param(&req, "version")?;
    let upd = req.get("data").cloned().unwrap_or_else(|| json!({}));
    let mut partial: Map<String, Value> = Map::new();
    for key in ["code", "name", "is_active"] {
        if let Some(v) = upd.get(key) {
            partial.insert(key.to_string(), v.clone());
        }
    }
    if let Some(t) = upd.get("type").or_else(|| upd.get("account_type")) {
        let t = t
            .as_str()
            .ok_or_else(|| "тип счёта должен быть строкой".to_string())?;
        if !ACCOUNT_TYPES.contains(&t) {
            return Err(format!(
                "недопустимый тип счёта '{t}'; допустимо: {}",
                ACCOUNT_TYPES.join(", ")
            ));
        }
        partial.insert("account_type".to_string(), json!(t));
    }
    if partial.is_empty() {
        return Err("нет полей для изменения в 'data'".to_string());
    }
    let data = host_update_object(&id, &json!(partial), version)?;
    Ok(json!({ "id": data["id"], "version": data["version"] }))
}

/// `account.list` — список счетов плана.
fn account_list_impl(_req: Value) -> Result<Value, String> {
    let ctx = ctx()?;
    let rows: Vec<Value> = ctx.accounts.iter().map(account_row).collect();
    Ok(json!({ "accounts": rows, "total": rows.len() }))
}

/// `account.get` — карточка счёта.
fn account_get_impl(req: Value) -> Result<Value, String> {
    let id = str_param(&req, "id")?;
    let obj = host_get_object(&id)?.ok_or_else(|| format!("счёт '{id}' не найден"))?;
    Ok(account_row(&obj))
}

/// `period.open` — открывает учётный период `(year, month)`.
fn period_open_impl(req: Value) -> Result<Value, String> {
    let year = req["year"]
        .as_i64()
        .ok_or_else(|| "нет целочисленного поля 'year'".to_string())?;
    let month = req["month"]
        .as_i64()
        .ok_or_else(|| "нет целочисленного поля 'month'".to_string())?;
    if !(1..=12).contains(&month) {
        return Err(format!("месяц вне диапазона 1..12: {month}"));
    }
    let name = req.get("name").and_then(Value::as_str).unwrap_or("").to_string();
    let ctx = ctx()?;
    if ctx.periods.iter().any(|p| {
        p["data"]["year"].as_i64() == Some(year) && p["data"]["month"].as_i64() == Some(month)
    }) {
        return Err(format!("учётный период {year}-{month:02} уже существует"));
    }
    let data = json!({
        "year": year,
        "month": month,
        "name": name,
        "status": "open",
    });
    let created = host_create_object(&ctx.period_type_id, &data)?;
    let id = created["id"].as_str().unwrap_or_default().to_string();
    Ok(json!({ "id": id, "year": year, "month": month, "status": "open" }))
}

/// `period.close` — закрывает учётный период.
fn period_close_impl(req: Value) -> Result<Value, String> {
    let id = str_param(&req, "id")?;
    let version = u64_param(&req, "version")?;
    let obj = host_get_object(&id)?.ok_or_else(|| format!("период '{id}' не найден"))?;
    if obj["data"]["status"].as_str() != Some("open") {
        return Err("период не открыт — закрывать нечего".to_string());
    }
    let data = host_update_object(&id, &json!({ "status": "closed" }), version)?;
    Ok(json!({ "id": data["id"], "version": data["version"], "status": "closed" }))
}

/// `period.list` — список учётных периодов.
fn period_list_impl(_req: Value) -> Result<Value, String> {
    let ctx = ctx()?;
    let mut rows: Vec<Value> = ctx
        .periods
        .iter()
        .map(|p| {
            json!({
                "id": p["id"],
                "year": p["data"]["year"],
                "month": p["data"]["month"],
                "name": p["data"]["name"],
                "status": p["data"]["status"],
            })
        })
        .collect();
    rows.sort_by(|a, b| {
        (a["year"].as_i64(), a["month"].as_i64())
            .cmp(&(b["year"].as_i64(), b["month"].as_i64()))
    });
    Ok(json!({ "periods": rows, "total": rows.len() }))
}

/// `entry.post` — проводит бухгалтерскую проводку: проверяет счета, период и
/// баланс, создаёт объект `ledger_entry` и эмитирует событие.
fn entry_post_impl(req: Value) -> Result<Value, String> {
    let date = str_param(&req, "date")?;
    let description = req
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let lines = req
        .get("lines")
        .cloned()
        .ok_or_else(|| "нет поля 'lines'".to_string())?;
    let ctx = ctx()?;
    validate_entry_lines(&ctx, &req)?;
    ensure_open_period(&ctx, &date)?;
    let data = json!({
        "date": date,
        "description": description,
        "lines": lines,
        "status": "posted",
        "source_type": "manual",
        "source_id": Value::Null,
        "correlation_entry_id": Value::Null,
    });
    let created = host_create_object(&ctx.entry_type_id, &data)?;
    let id = created["id"].as_str().unwrap_or_default().to_string();
    let _ = (unsafe {
        host::emit_event(
            id.clone(),
            "ledger_entry.posted".to_string(),
            json!({ "date": date, "lines": lines }).to_string(),
        )
    })
    .map_err(|e| format!("host emit_event: {e}"))?;
    Ok(json!({ "id": id, "status": "posted" }))
}

/// `entry.list` — журнал проводок с фильтрами `date_from`, `date_to`,
/// `account` (по коду счёта в строках).
fn entry_list_impl(req: Value) -> Result<Value, String> {
    let entry_type_id = entity_type_id("ledger_entry")?;
    let entries = list_type(&entry_type_id, 500)?;
    let date_from = req["date_from"].as_str();
    let date_to = req["date_to"].as_str();
    let account = req["account"].as_str();
    let mut rows = Vec::new();
    for entry in &entries {
        let data = &entry["data"];
        let date = data["date"].as_str().unwrap_or("");
        if let Some(f) = date_from {
            if date < f {
                continue;
            }
        }
        if let Some(t) = date_to {
            if date > t {
                continue;
            }
        }
        if let Some(acc) = account {
            let has = data["lines"].as_array().map_or(false, |ls| {
                ls.iter().any(|l| l["account"].as_str() == Some(acc))
            });
            if !has {
                continue;
            }
        }
        rows.push(json!({
            "id": entry["id"],
            "date": date,
            "description": data["description"],
            "status": data["status"],
            "lines": data["lines"],
            "source_type": data["source_type"],
            "source_id": data["source_id"],
            "correlation_entry_id": data["correlation_entry_id"],
        }));
    }
    rows.sort_by(|a, b| a["date"].as_str().cmp(&b["date"].as_str()));
    Ok(json!({ "entries": rows, "total": rows.len() }))
}

/// `entry.reverse` — сторнирует проведённую проводку: создаёт обратную запись
/// (дебет/кредит меняются местами) и помечает исходную как `reversed`
/// (исторические записи не удаляются).
fn entry_reverse_impl(req: Value) -> Result<Value, String> {
    let id = str_param(&req, "id")?;
    let reason = req
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or("сторно")
        .to_string();
    let entry_type_id = entity_type_id("ledger_entry")?;
    let obj = host_get_object(&id)?.ok_or_else(|| format!("проводка '{id}' не найдена"))?;
    if obj["data"]["status"].as_str() != Some("posted") {
        return Err("сторнировать можно только проведённую проводку".to_string());
    }
    let version = obj["version"].as_u64().unwrap_or(0);
    let date = obj["data"]["date"].as_str().unwrap_or("").to_string();
    let description = obj["data"]["description"].as_str().unwrap_or("").to_string();
    let lines = obj["data"]["lines"].clone();
    let reversed: Vec<Value> = lines
        .as_array()
        .map(|ls| {
            ls.iter()
                .map(|l| {
                    json!({
                        "account": l["account"],
                        "debit": l["credit"].clone(),
                        "credit": l["debit"].clone(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let reversal_data = json!({
        "date": date,
        "description": description,
        "lines": reversed,
        "status": "posted",
        "source_type": "reversal",
        "source_id": Value::Null,
        "correlation_entry_id": id,
    });
    let created = host_create_object(&entry_type_id, &reversal_data)?;
    let reversal_id = created["id"].as_str().unwrap_or_default().to_string();
    let _ = host_update_object(&id, &json!({ "status": "reversed" }), version)?;
    let _ = (unsafe {
        host::emit_event(
            id.clone(),
            "ledger_entry.reversed".to_string(),
            json!({ "reversal_id": reversal_id, "reason": reason }).to_string(),
        )
    })
    .map_err(|e| format!("host emit_event: {e}"))?;
    Ok(json!({ "original_id": id, "reversal_id": reversal_id }))
}

/// `doc.post` — проводит документ и атомарно создаёт проводку одной
/// транзакцией (post документа + create записи журнала).
fn doc_post_impl(req: Value) -> Result<Value, String> {
    let document_id = str_param(&req, "document_id")?;
    let expected_version = u64_param(&req, "expected_version")?;
    let entry = req
        .get("entry")
        .cloned()
        .ok_or_else(|| "нет поля 'entry'".to_string())?;
    let date = entry["date"]
        .as_str()
        .ok_or_else(|| "entry.date обязателен".to_string())?
        .to_string();
    let description = entry
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let ctx = ctx()?;
    validate_entry_lines(&ctx, &entry)?;
    ensure_open_period(&ctx, &date)?;
    let entry_data = json!({
        "date": date,
        "description": description,
        "lines": entry["lines"],
        "status": "posted",
        "source_type": "document",
        "source_id": document_id,
        "correlation_entry_id": Value::Null,
    });
    let begin_conv = (unsafe { host::tx_begin(format!("doc:post:{document_id}")) })
        .map_err(|e| format!("host tx_begin: {e}"))?;
    let begin = require_ok(&begin_conv)?;
    let handle = begin["handle"]
        .as_str()
        .ok_or_else(|| "tx_begin: нет handle".to_string())?
        .to_string();
    let post_params = json!({ "id": document_id, "expected_version": expected_version });
    let _ = (unsafe {
        host::tx_add_op(
            handle.clone(),
            "object.post".to_string(),
            post_params.to_string(),
        )
    })
    .map_err(|e| format!("host tx_add_op: {e}"))?;
    let create_params = json!({ "entity_type": "ledger_entry", "data": entry_data });
    let _ = (unsafe {
        host::tx_add_op(
            handle.clone(),
            "object.create".to_string(),
            create_params.to_string(),
        )
    })
    .map_err(|e| format!("host tx_add_op: {e}"))?;
    let _commit = require_ok(&(unsafe { host::tx_commit(handle.clone()) })
        .map_err(|e| format!("host tx_commit: {e}"))?)?;
    // commit возвращает только {committed:true}; результаты операций не
    // раскрываются — находим созданную проводку по source_id == document_id.
    let entry_type_id = entity_type_id("ledger_entry")?;
    let entries = list_type(&entry_type_id, 100)?;
    let entry_id = entries
        .iter()
        .rev()
        .find(|e| e["data"]["source_id"].as_str() == Some(&document_id))
        .and_then(|e| e["id"].as_str())
        .unwrap_or_default()
        .to_string();
    Ok(json!({ "document_id": document_id, "entry_id": entry_id }))
}

/// `balance.trial` — оборотно-сальдовая ведомость по счетам (проведённые
/// проводки, обороты дебета/кредита и сальдо).
fn balance_trial_impl(req: Value) -> Result<Value, String> {
    let date_from = req["date_from"].as_str();
    let date_to = req["date_to"].as_str();
    let entry_type_id = entity_type_id("ledger_entry")?;
    let entries = list_type(&entry_type_id, 500)?;
    let mut debit_by_account: BTreeMap<String, f64> = BTreeMap::new();
    let mut credit_by_account: BTreeMap<String, f64> = BTreeMap::new();
    for entry in &entries {
        if !matches!(entry["data"]["status"].as_str(), Some("posted") | Some("reversed")) {
            continue;
        }
        let date = entry["data"]["date"].as_str().unwrap_or("");
        if let Some(f) = date_from {
            if date < f {
                continue;
            }
        }
        if let Some(t) = date_to {
            if date > t {
                continue;
            }
        }
        if let Some(lines) = entry["data"]["lines"].as_array() {
            for line in lines {
                let code = line["account"].as_str().unwrap_or_default();
                if code.is_empty() {
                    continue;
                }
                let debit = line["debit"].as_f64().unwrap_or(0.0);
                let credit = line["credit"].as_f64().unwrap_or(0.0);
                *debit_by_account.entry(code.to_string()).or_insert(0.0) += debit;
                *credit_by_account.entry(code.to_string()).or_insert(0.0) += credit;
            }
        }
    }
    let codes: std::collections::BTreeSet<&String> = debit_by_account
        .keys()
        .chain(credit_by_account.keys())
        .collect();
    let mut rows = Vec::new();
    let mut total_debit = 0.0;
    let mut total_credit = 0.0;
    for code in &codes {
        let debit = debit_by_account.get(*code).copied().unwrap_or(0.0);
        let credit = credit_by_account.get(*code).copied().unwrap_or(0.0);
        total_debit += debit;
        total_credit += credit;
        rows.push(json!({
            "account": code,
            "debit": debit,
            "credit": credit,
            "balance": debit - credit,
        }));
    }
    Ok(json!({
        "rows": rows,
        "total_accounts": rows.len(),
        "totals": { "debit": total_debit, "credit": total_credit },
    }))
}

/// `balance.sheet` — баланс по группам счетов (активы, обязательства,
/// собственный капитал), сальдо берётся закрытым на конец текущих данных.
fn balance_sheet_impl(_req: Value) -> Result<Value, String> {
    let ctx = ctx()?;
    let entry_type_id = entity_type_id("ledger_entry")?;
    let entries = list_type(&entry_type_id, 500)?;
    let mut balance_by_account: BTreeMap<String, f64> = BTreeMap::new();
    for entry in &entries {
        if !matches!(entry["data"]["status"].as_str(), Some("posted") | Some("reversed")) {
            continue;
        }
        if let Some(lines) = entry["data"]["lines"].as_array() {
            for line in lines {
                let code = line["account"].as_str().unwrap_or_default();
                if code.is_empty() {
                    continue;
                }
                let debit = line["debit"].as_f64().unwrap_or(0.0);
                let credit = line["credit"].as_f64().unwrap_or(0.0);
                *balance_by_account.entry(code.to_string()).or_insert(0.0) += debit - credit;
            }
        }
    }
    let mut assets = Vec::new();
    let mut liabilities = Vec::new();
    let mut equity = Vec::new();
    let mut total_assets = 0.0;
    let mut total_liabilities = 0.0;
    let mut total_equity = 0.0;
    for account in &ctx.accounts {
        let code = account["data"]["code"].as_str().unwrap_or_default();
        let account_type = account["data"]["account_type"].as_str().unwrap_or_default();
        let balance = balance_by_account.get(code).copied().unwrap_or(0.0);
        let amount = if matches!(account_type, "asset") {
            balance
        } else {
            -balance
        };
        if amount.abs() < BALANCE_EPS {
            continue;
        }
        let row = json!({ "account": code, "amount": amount });
        match account_type {
            "asset" => {
                total_assets += amount;
                assets.push(row);
            }
            "liability" => {
                total_liabilities += amount;
                liabilities.push(row);
            }
            "equity" => {
                total_equity += amount;
                equity.push(row);
            }
            _ => {}
        }
    }
    Ok(json!({
        "assets": { "rows": assets, "total": total_assets },
        "liabilities": { "rows": liabilities, "total": total_liabilities },
        "equity": { "rows": equity, "total": total_equity },
    }))
}

#[extism_pdk::plugin_fn]
pub fn account_create(input: String) -> FnResult<String> {
    Ok(exec(input, account_create_impl))
}

#[extism_pdk::plugin_fn]
pub fn account_update(input: String) -> FnResult<String> {
    Ok(exec(input, account_update_impl))
}

#[extism_pdk::plugin_fn]
pub fn account_list(input: String) -> FnResult<String> {
    Ok(exec(input, account_list_impl))
}

#[extism_pdk::plugin_fn]
pub fn account_get(input: String) -> FnResult<String> {
    Ok(exec(input, account_get_impl))
}

#[extism_pdk::plugin_fn]
pub fn period_open(input: String) -> FnResult<String> {
    Ok(exec(input, period_open_impl))
}

#[extism_pdk::plugin_fn]
pub fn period_close(input: String) -> FnResult<String> {
    Ok(exec(input, period_close_impl))
}

#[extism_pdk::plugin_fn]
pub fn period_list(input: String) -> FnResult<String> {
    Ok(exec(input, period_list_impl))
}

#[extism_pdk::plugin_fn]
pub fn entry_post(input: String) -> FnResult<String> {
    Ok(exec(input, entry_post_impl))
}

#[extism_pdk::plugin_fn]
pub fn entry_list(input: String) -> FnResult<String> {
    Ok(exec(input, entry_list_impl))
}

#[extism_pdk::plugin_fn]
pub fn entry_reverse(input: String) -> FnResult<String> {
    Ok(exec(input, entry_reverse_impl))
}

#[extism_pdk::plugin_fn]
pub fn doc_post(input: String) -> FnResult<String> {
    Ok(exec(input, doc_post_impl))
}

#[extism_pdk::plugin_fn]
pub fn balance_trial(input: String) -> FnResult<String> {
    Ok(exec(input, balance_trial_impl))
}

#[extism_pdk::plugin_fn]
pub fn balance_sheet(input: String) -> FnResult<String> {
    Ok(exec(input, balance_sheet_impl))
}

// Фиктивный помощник, чтобы `Error` был задействован (never-type fallback не
// смешивает `!` с возвращаемым типом плагина).
#[allow(dead_code)]
fn _as_error(e: extism_pdk::Error) -> Error {
    e
}