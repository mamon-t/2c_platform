//! Модуль «Заявки» — WASM-плагин платформы 2C.
//!
//! Маршруты согласования и активные согласования хранятся
//! в KV-хранилище хоста (module_store), сама заявка — обычный
//! объект платформы (entity_type REQUEST).
//!
//! Ключи KV:
//! - "route:{code}"          → RequestRoute
//! - "approval:{request_id}" → RequestApproval (одна активная процедура на заявку)

mod models;

use extism_pdk::*;
use serde::{Deserialize, Serialize};

use models::{
    ApprovalStatus, ApproverType, DecideInput, RequestApproval, RequestRoute, StepState,
    StepStatus, SubmitInput,
};

// ── Host functions ─────────────────────────────────────────

#[host_fn]
extern "ExtismHost" {
    fn kv_put(key: String, value_json: String) -> String;
    fn kv_get(key: String) -> String;
    fn kv_list(prefix: String) -> String;
    fn kv_delete(key: String) -> String;
    fn get_object(id: String) -> String;
    fn transition_object(id: String, version: String, action: String) -> String;
    fn notify_user(recipient_user_id: String, subject: String, body: String) -> String;
    fn run_script(source: String, context_json: String) -> String;
    fn whoami() -> String;
    fn now_ms() -> String;
    fn module_settings() -> String;
    fn log_message(msg: String);
}

// ── Манифест ───────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub struct PluginFunction {
    pub name: String,
    pub label: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Serialize, Deserialize)]
pub struct ModuleInfo {
    pub name: String,
    pub version: String,
    pub code: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub api_version: Option<String>,
    pub capabilities: Vec<String>,
    pub permissions: Vec<String>,
    pub functions: Vec<PluginFunction>,
}

/// Политика подписи модуля (ТЗ разд. 12): submit/approve/reject — подпись обязательна.
const REQUIRE_SIGNATURE: bool = true;

// ── Helpers ────────────────────────────────────────────────

/// Разобрать конверт host-функции {ok, data | error{code,message}}.
fn unwrap_host(raw: String) -> anyhow::Result<serde_json::Value> {
    let v: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| anyhow::anyhow!("host вернул невалидный JSON: {e}"))?;
    if v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false) {
        Ok(v.get("data").cloned().unwrap_or(serde_json::Value::Null))
    } else {
        let code = v["error"]["code"].as_str().unwrap_or("UNKNOWN");
        let msg = v["error"]["message"].as_str().unwrap_or("");
        Err(anyhow::anyhow!("{code}: {msg}"))
    }
}

/// Идентичность вызывающего (из host-контекста, не из аргументов!).
#[derive(Debug, Clone, Deserialize)]
struct Caller {
    company_id: Option<String>,
    user_id: Option<String>,
    login: Option<String>,
    display_name: Option<String>,
    role_id: Option<String>,
}

fn caller() -> anyhow::Result<Caller> {
    let raw = unsafe { whoami() }?;
    let v = unwrap_host(raw)?;
    let c: Caller = serde_json::from_value(v)
        .map_err(|e| anyhow::anyhow!("INVALID_CTX: {e}"))?;
    if c.user_id.is_none() || c.company_id.is_none() {
        return Err(anyhow::anyhow!("NO_USER: пользователь не аутентифицирован"));
    }
    Ok(c)
}

fn current_ms() -> anyhow::Result<u64> {
    let raw = unsafe { now_ms() }?;
    let v = unwrap_host(raw)?;
    Ok(v.as_str().and_then(|s| s.parse().ok()).unwrap_or(0))
}

fn route_key(code: &str) -> String { format!("route:{code}") }
fn approval_key(request_id: &str) -> String { format!("approval:{request_id}") }

fn kv_put_value(key: &str, value: &impl Serialize) -> anyhow::Result<()> {
    let json = serde_json::to_string(value)?;
    unsafe { kv_put(key.to_string(), json) }?;
    Ok(())
}

fn kv_get_value<T: serde::de::DeserializeOwned>(key: &str) -> anyhow::Result<Option<T>> {
    let raw = unsafe { kv_get(key.to_string()) }?;
    let data = unwrap_host(raw)?;
    if data.get("found").and_then(|f| f.as_bool()).unwrap_or(false) {
        let value = data.get("value").cloned().unwrap_or(serde_json::Value::Null);
        Ok(Some(serde_json::from_value(value)?))
    } else {
        Ok(None)
    }
}

/// Выполнить hook-скрипт из настроек модуля.
/// `strict=true` (before_submit): ошибка скрипта прерывает операцию.
/// `strict=false` (после событий): ошибка только логируется.
fn run_hook(name: &str, strict: bool, context: serde_json::Value) -> anyhow::Result<()> {
    let settings_raw = unsafe { module_settings() }?;
    let settings = unwrap_host(settings_raw)?;
    let Some(source) = settings.get(name).and_then(|s| s.as_str()) else {
        return Ok(()); // хук не задан — норма
    };
    match unsafe { run_script(source.to_string(), context.to_string()) } {
        Ok(res_raw) => {
            if let Ok(res) = unwrap_host(res_raw) {
                let _ = unsafe { log_message(format!("[requests] hook {name} → {res}")) };
            }
            Ok(())
        }
        Err(e) => {
            if strict {
                Err(anyhow::anyhow!("Хук {name}: {e}"))
            } else {
                let _ = unsafe { log_message(format!("[requests] hook {name} failed: {e}")) };
                Ok(())
            }
        }
    }
}

/// Проверка: текущий этап назначен вызывающему?
fn step_mine(step: &StepState, c: &Caller) -> bool {
    let uid = c.user_id.as_deref().unwrap_or("");
    match step.approver_type {
        ApproverType::User => step.approver_id == uid,
        ApproverType::Role => {
            step.approver_id == c.role_id.as_deref().unwrap_or("")
        }
    }
}

fn require_signature(sig: &Option<String>, what: &str) -> anyhow::Result<String> {
    if !REQUIRE_SIGNATURE {
        return Ok(String::new());
    }
    sig.clone()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("SIGNATURE_REQUIRED: {what} требует квалифицированной подписи (выберите сертификат)"))
}

// ── Функции: маршруты ──────────────────────────────────────

#[plugin_fn]
pub fn routes_save(Json(route): Json<RequestRoute>) -> FnResult<Json<serde_json::Value>> {
    caller()?;
    if route.code.trim().is_empty() || route.name.trim().is_empty() {
        return Err(anyhow::anyhow!("VALIDATION: code и name обязательны").into());
    }
    if route.steps.is_empty() {
        return Err(anyhow::anyhow!("VALIDATION: маршрут должен содержать хотя бы один этап").into());
    }
    for (i, step) in route.steps.iter().enumerate() {
        if step.approver_id.trim().is_empty() {
            return Err(anyhow::anyhow!("VALIDATION: этап {} без утверждающего", i + 1).into());
        }
    }
    kv_put_value(&route_key(&route.code), &route)?;
    Ok(Json(serde_json::json!({ "code": route.code })))
}

#[plugin_fn]
pub fn routes_list() -> FnResult<Json<Vec<RequestRoute>>> {
    let _ = caller()?;
    let raw = unsafe { kv_list("route:".to_string()) }?;
    let data = unwrap_host(raw)?;
    let mut routes = Vec::new();
    if let Some(items) = data.get("items").and_then(|i| i.as_array()) {
        for item in items {
            if let Ok(r) = serde_json::from_value::<RequestRoute>(item.get("value").cloned().unwrap_or_default()) {
                routes.push(r);
            }
        }
    }
    routes.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Json(routes))
}

#[plugin_fn]
pub fn routes_delete(Json(input): Json<serde_json::Value>) -> FnResult<Json<serde_json::Value>> {
    caller()?;
    let code = input.get("code").and_then(|c| c.as_str())
        .ok_or_else(|| anyhow::anyhow!("VALIDATION: требуется code"))?.to_string();
    let raw = unsafe { kv_delete(route_key(&code)) }?;
    let data = unwrap_host(raw)?;
    Ok(Json(data))
}

// ── Функции: согласование ──────────────────────────────────

#[plugin_fn]
pub fn submit(Json(input): Json<SubmitInput>) -> FnResult<Json<RequestApproval>> {
    let c = caller()?;
    let user_id = c.user_id.clone().unwrap();
    let ts = current_ms()?;

    // Подпись инициатора (политика подписи)
    let signature = require_signature(&input.signature_der, "Отправка заявки")?;

    // Заявка должна существовать и быть черновиком
    let obj_raw = unsafe { get_object(input.request_id.clone()) }?;
    let obj = unwrap_host(obj_raw)?;
    let state = obj.get("state").and_then(|s| s.as_str()).unwrap_or("").to_string();
    if state != "draft" {
        return Err(anyhow::anyhow!("VALIDATION: заявку можно отправить только в статусе draft (текущий: {state})").into());
    }

    // Хук перед отправкой (strict — может отменить)
    run_hook("before_submit", true, serde_json::json!({
        "caller": { "user_id": user_id, "login": c.login },
        "request": obj,
    }))?;

    // Маршрут
    let route: RequestRoute = kv_get_value(&route_key(&input.route_code))?
        .ok_or_else(|| anyhow::anyhow!("NOT_FOUND: маршрут '{}' не найден", input.route_code))?;
    if !route.is_active {
        return Err(anyhow::anyhow!("VALIDATION: маршрут '{}' отключён", input.route_code).into());
    }

    // Повторная отправка? Активное согласование уже есть
    let key = approval_key(&input.request_id);
    if let Some(existing) = kv_get_value::<RequestApproval>(&key)? {
        if existing.status == ApprovalStatus::InProgress {
            return Err(anyhow::anyhow!("CONFLICT: заявка уже на согласовании").into());
        }
    }

    let steps: Vec<StepState> = route.steps.iter().map(|s| StepState {
        step_order: s.step_order,
        approver_type: s.approver_type.clone(),
        approver_id: s.approver_id.clone(),
        approver_name: s.approver_name.clone(),
        status: StepStatus::Pending,
        decided_at: None,
        comment: None,
        signature_der: None,
    }).collect();

    let approval = RequestApproval {
        request_id: input.request_id.clone(),
        route_code: route.code.clone(),
        route_name: route.name.clone(),
        status: ApprovalStatus::InProgress,
        current_step: 0,
        steps,
        initiator_id: user_id.clone(),
        initiator_login: c.login.clone().unwrap_or_default(),
        initiator_name: c.display_name.clone(),
        submit_signature_der: if signature.is_empty() { None } else { Some(signature) },
        submitted_at: ts,
        completed_at: None,
        last_comment: None,
    };

    kv_put_value(&key, &approval)?;

    // Уведомить первого утверждающего
    notify_current_approver(&approval)?;

    let _ = unsafe { log_message(format!(
        "[requests] заявка {} отправлена по маршруту {}", input.request_id, route.code
    )) };

    Ok(Json(approval))
}

/// Уведомить утверждающего текущего этапа.
fn notify_current_approver(a: &RequestApproval) -> anyhow::Result<()> {
    if let Some(step) = a.steps.get(a.current_step) {
        let approver_uuid = match step.approver_type {
            ApproverType::User => step.approver_id.clone(),
            ApproverType::Role => return Ok(()), // роль: рассылка позже (нужен host-fn users_by_role)
        };
        let subject = format!("Заявка ожидает согласования (этап {})", step.step_order);
        let body = format!(
            "Заявка {} отправлена {} по маршруту «{}». Требуется ваше решение.",
            a.request_id, a.initiator_login, a.route_name
        );
        let _ = unsafe { notify_user(approver_uuid, subject, body) }?;
    }
    Ok(())
}

#[plugin_fn]
pub fn approve_step(Json(input): Json<DecideInput>) -> FnResult<Json<RequestApproval>> {
    Ok(Json(decide(input, true)?))
}

#[plugin_fn]
pub fn reject_step(Json(input): Json<DecideInput>) -> FnResult<Json<RequestApproval>> {
    Ok(Json(decide(input, false)?))
}

fn decide(input: DecideInput, approve: bool) -> anyhow::Result<RequestApproval> {
    let c = caller()?;
    let ts = current_ms()?;

    // Подпись решения обязательна
    let signature = require_signature(&Some(input.signature_der.clone()),
        if approve { "Согласование" } else { "Отклонение" })?;

    let key = approval_key(&input.request_id);
    let mut a: RequestApproval = kv_get_value(&key)?
        .ok_or_else(|| anyhow::anyhow!("NOT_FOUND: активное согласование для заявки {} не найдено", input.request_id))?;

    if a.status != ApprovalStatus::InProgress {
        return Err(anyhow::anyhow!("CONFLICT: согласование уже завершено (статус: {:?})", a.status));
    }

    let idx = a.current_step;
    let step_is_mine = a.steps.get(idx).map(|s| step_mine(s, &c)).unwrap_or(false);
    if !step_is_mine {
        return Err(anyhow::anyhow!("FORBIDDEN: текущий этап назначен другому утверждающему"));
    }

    let step = &mut a.steps[idx];
    step.decided_at = Some(ts);
    step.comment = input.comment.clone();
    step.signature_der = if signature.is_empty() { None } else { Some(signature) };

    if approve {
        step.status = StepStatus::Approved;
        a.current_step = idx + 1;

        if a.current_step >= a.steps.len() {
            // Все этапы пройдены → проводим заявку (номер присвоит нумерация)
            complete_approval(&mut a, ts)?;
            run_hook("on_complete", false, serde_json::json!({ "approval": a }))?;
        } else {
            a.last_comment = input.comment.clone();
            notify_current_approver(&a)?;
            run_hook("after_approve", false, serde_json::json!({ "approval": a }))?;
        }
    } else {
        step.status = StepStatus::Rejected;
        a.status = ApprovalStatus::Rejected;
        a.completed_at = Some(ts);
        a.last_comment = input.comment.clone();

        // Уведомить инициатора
        let _ = unsafe { notify_user(
            a.initiator_id.clone(),
            "Заявка отклонена".to_string(),
            format!("Заявка {} отклонена на этапе {}. Комментарий: {}",
                a.request_id, step.step_order,
                input.comment.as_deref().unwrap_or("—")),
        )}?;

        run_hook("on_reject", false, serde_json::json!({ "approval": a }))?;
    }

    kv_put_value(&key, &a)?;
    Ok(a)
}

/// Завершение согласования: перевод заявки Draft→Posted через хост.
fn complete_approval(a: &mut RequestApproval, ts: u64) -> anyhow::Result<()> {
    // Свежие данные объекта (версия могла измениться)
    let obj_raw = unsafe { get_object(a.request_id.clone()) }?;
    let obj = unwrap_host(obj_raw)?;
    let version = obj.get("version").and_then(|v| v.as_i64()).unwrap_or(1);
    let state = obj.get("state").and_then(|v| v.as_str()).unwrap_or("");

    if state != "draft" {
        // Уже проведён/изменён вне процедуры — фиксируем факт, процесс завершаем
        a.status = ApprovalStatus::Approved;
        a.completed_at = Some(ts);
        return Ok(());
    }

    let id = obj.get("id").and_then(|v| v.as_str()).unwrap_or(&a.request_id).to_string();
    let res_raw = unsafe { transition_object(id, version.to_string(), "post".to_string()) }?;
    unwrap_host(res_raw)?;

    a.status = ApprovalStatus::Approved;
    a.completed_at = Some(ts);

    // Уведомить инициатора об успехе
    let _ = unsafe { notify_user(
        a.initiator_id.clone(),
        "Заявка согласована".to_string(),
        format!("Заявка {} полностью согласована и проведена.", a.request_id),
    )}?;

    Ok(())
}

/// Инициатор отменяет процедуру согласования (заявка остаётся черновиком).
#[plugin_fn]
pub fn cancel_request(Json(input): Json<serde_json::Value>) -> FnResult<Json<RequestApproval>> {
    let c = caller()?;
    let request_id = input.get("request_id").and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("VALIDATION: требуется request_id"))?.to_string();

    let key = approval_key(&request_id);
    let mut a: RequestApproval = kv_get_value(&key)?
        .ok_or_else(|| anyhow::anyhow!("NOT_FOUND: согласование не найдено"))?;

    if a.initiator_id != c.user_id.clone().unwrap_or_default() {
        return Err(anyhow::anyhow!("FORBIDDEN: отменить может только инициатор").into());
    }
    if a.status != ApprovalStatus::InProgress {
        return Err(anyhow::anyhow!("CONFLICT: процедура уже завершена").into());
    }

    a.status = ApprovalStatus::Cancelled;
    a.completed_at = Some(current_ms()?);

    // Пропускаем незавершённые шаги
    for step in &mut a.steps {
        if step.status == StepStatus::Pending {
            step.status = StepStatus::Skipped;
        }
    }

    kv_put_value(&key, &a)?;
    Ok(Json(a))
}

// ── Функции: чтение ────────────────────────────────────────

#[plugin_fn]
pub fn approval_get(Json(input): Json<serde_json::Value>) -> FnResult<Json<Option<RequestApproval>>> {
    let _ = caller()?;
    let request_id = input.get("request_id").and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("VALIDATION: требуется request_id"))?;
    let a: Option<RequestApproval> = kv_get_value(&approval_key(request_id))?;
    Ok(Json(a))
}

/// Согласования, где текущий этап назначен вызывающему.
#[plugin_fn]
pub fn pending_approvals() -> FnResult<Json<Vec<RequestApproval>>> {
    let c = caller()?;
    let raw = unsafe { kv_list("approval:".to_string()) }?;
    let data = unwrap_host(raw)?;

    let mut result = Vec::new();
    if let Some(items) = data.get("items").and_then(|i| i.as_array()) {
        for item in items {
            let Ok(a) = serde_json::from_value::<RequestApproval>(item.get("value").cloned().unwrap_or_default()) else {
                continue;
            };
            if a.status != ApprovalStatus::InProgress {
                continue;
            }
            if a.steps.get(a.current_step).map(|s| step_mine(s, &c)).unwrap_or(false) {
                result.push(a);
            }
        }
    }
    result.sort_by_key(|a| a.submitted_at);
    Ok(Json(result))
}

/// Все согласования компании (для вкладки «Все», фильтрация статуса на фронте).
#[plugin_fn]
pub fn all_approvals() -> FnResult<Json<Vec<RequestApproval>>> {
    let _ = caller()?;
    let raw = unsafe { kv_list("approval:".to_string()) }?;
    let data = unwrap_host(raw)?;

    let mut result = Vec::new();
    if let Some(items) = data.get("items").and_then(|i| i.as_array()) {
        for item in items {
            if let Ok(a) = serde_json::from_value::<RequestApproval>(item.get("value").cloned().unwrap_or_default()) {
                result.push(a);
            }
        }
    }
    result.sort_by_key(|a| std::cmp::Reverse(a.submitted_at));
    Ok(Json(result))
}

// ── Манифест ───────────────────────────────────────────────

#[plugin_fn]
pub fn get_info() -> FnResult<Json<ModuleInfo>> {
    fn f(name: &str, label: &str, description: &str, props: serde_json::Value, required: &[&str]) -> PluginFunction {
        PluginFunction {
            name: name.into(),
            label: label.into(),
            description: description.into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": props,
                "required": required,
            }),
        }
    }

    Ok(Json(ModuleInfo {
        name: "requests".into(),
        version: "0.1.0".into(),
        code: Some("requests".into()),
        author: Some("2C Platform".into()),
        description: Some("Заявки с маршрутами согласования, криптоподписью решений и уведомлениями.".into()),
        api_version: Some("1.0".into()),
        capabilities: vec![
            "objects.read".into(),
            "objects.update".into(),
            "storage".into(),
            "scripts".into(),
            "notifications".into(),
            "logging".into(),
        ],
        permissions: vec![
            "requests.create".into(),
            "requests.read".into(),
            "requests.read_all".into(),
            "requests.submit".into(),
            "requests.approve".into(),
            "requests.reject".into(),
            "requests.cancel".into(),
            "requests.manage_routes".into(),
        ],
        functions: vec![
            f("routes_list", "Список маршрутов", "Маршруты согласования компании.", serde_json::json!({}), &[]),
            f("routes_save", "Сохранить маршрут", "Создать/обновить маршрут согласования.", serde_json::json!({
                "route": { "type": "object", "description": "RequestRoute" }
            }), &[]),
            f("routes_delete", "Удалить маршрут", "Удаление маршрута по коду.", serde_json::json!({
                "code": { "type": "string" }
            }), &[]),
            f("submit", "Отправить на согласование", "Draft → согласование по маршруту. Подпись инициатора обязательна.", serde_json::json!({
                "request_id": { "type": "string" },
                "route_code": { "type": "string" },
                "signature_der": { "type": "string", "description": "base64 DER CMS-подписи" }
            }), &[]),
            f("approve_step", "Согласовать этап", "Решение утверждающего. Подпись обязательна.", serde_json::json!({
                "request_id": { "type": "string" },
                "comment": { "type": "string" },
                "signature_der": { "type": "string" }
            }), &[]),
            f("reject_step", "Отклонить этап", "Отклонение с комментарием. Подпись обязательна.", serde_json::json!({
                "request_id": { "type": "string" },
                "comment": { "type": "string" },
                "signature_der": { "type": "string" }
            }), &[]),
            f("cancel_request", "Отменить согласование", "Только инициатор. Заявка остаётся черновиком.", serde_json::json!({
                "request_id": { "type": "string" }
            }), &[]),
            f("approval_get", "Статус согласования", "Процедура по request_id (timeline).", serde_json::json!({
                "request_id": { "type": "string" }
            }), &[]),
            f("pending_approvals", "Мои на согласовании", "Активные согласования текущего этапа для меня.", serde_json::json!({}), &[]),
            f("all_approvals", "Все согласования", "Все процедуры компании.", serde_json::json!({}), &[]),
        ],
    }))
}
