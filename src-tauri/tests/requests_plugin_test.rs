//! Интеграционные тесты WASM-плагина «Заявки».
//!
//! Загружается НАСТОЯЩИЙ собранный requests_plugin.wasm, а host-функции
//! заменены моками поверх общей обвязки Harness: KV в памяти, фикстуры
//! объектов, переключаемый пользователь, запись нотификаций/событий,
//! реальный Rhai для хуков.
//!
//! Запуск: сначала собрать плагин (wasm-modules/requests), затем
//!   cargo test --test requests_plugin
//! Если .wasm не найден — тесты пропускаются.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use extism::{Manifest, Plugin, PluginBuilder, UserData, Wasm, PTR};
use serde_json::{json, Value};

// ── Пути и идентификаторы ──────────────────────────────────

const WASM_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../wasm-modules/requests/target/wasm32-unknown-unknown/release/requests_plugin.wasm"
);

const U1: &str = "11111111-1111-1111-1111-111111111111"; // инициатор
const U2: &str = "22222222-2222-2222-2222-222222222222"; // утверждающий 1
const U3: &str = "33333333-3333-3333-3333-333333333333"; // утверждающий 2

// ── Обвязка ────────────────────────────────────────────────

#[derive(Clone, Default)]
struct Harness {
    /// Логические ключи KV → значение (JSON строка)
    kv: HashMap<String, String>,
    /// Объекты: id → {id, state, version, data}
    objects: HashMap<String, Value>,
    /// Вызовы transition_object: (id, version, action)
    transitions: Vec<(String, String, String)>,
    /// Уведомления: (recipient, subject, body)
    notifications: Vec<(String, String, String)>,
    /// События: (stream_id, event_type, payload)
    events: Vec<(String, String, Value)>,
    /// Текущий пользователь для whoami()
    current_user: Value,
    /// Дополнительные роли (role_ids) текущего пользователя
    extra_role_ids: Vec<String>,
    /// Настройки модуля (хуки)
    settings: Value,
    /// Принудительная ошибка следующего run_script
    script_fail: Option<String>,
    /// Принудительный результат следующего cms_verify
    verify_invalid: bool,
    /// Зафиксированные вызовы cms_verify: (data, sig)
    cms_calls: Vec<(String, String)>,
    clock: u64,
}

impl Harness {
    fn new(caller_user: &str, role: Option<&str>) -> Self {
        Self {
            current_user: json!({
                "company_id": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
                "user_id": caller_user,
                "login": format!("user_{}", &caller_user[..2]),
                "display_name": "Тестовый пользователь",
                "role_id": role,
            }),
            ..Default::default()
        }
    }

    fn add_draft_object(&mut self, id: &str) {
        self.objects.insert(id.to_string(), json!({
            "id": id, "state": "draft", "version": 1, "data": {"title": "Тестовая заявка"},
        }));
    }
}

fn envelope_ok(data: Value) -> String {
    json!({ "ok": true, "data": data }).to_string()
}

fn envelope_err(code: &str, msg: &str) -> String {
    json!({ "ok": false, "error": {"code": code, "message": msg} }).to_string()
}

/// Реальный прогон Rhai (как в хосте): ctx инжектится через parse_json.
fn run_rhai(context: &str, source: &str) -> Result<String, String> {
    let engine = rhai::Engine::new();
    let ctx_value: Value = context.parse().unwrap_or(Value::Null);
    // Двойное кодирование: аргумент parse_json — строковый литерал с JSON
    let ctx_literal = serde_json::to_string(
        &serde_json::to_string(&ctx_value).unwrap_or_else(|_| "null".into()),
    )
    .unwrap_or_else(|_| "\"null\"".into());
    let src = format!("let ctx = parse_json({});\n{}", ctx_literal, source);
    let result: rhai::Dynamic = engine
        .eval(&src)
        .map_err(|e| format!("SCRIPT_FAILED: {e}"))?;
    Ok(result.to_string())
}

// ── Мок-host-функции ───────────────────────────────────────

type H = Arc<Mutex<Harness>>;

/// Клонировать Arc из UserData и залочить наш внутренний мьютекс.
macro_rules! clone_h {
    ($ud:expr) => {{
        let hd: H = $ud.get()?.lock().unwrap().clone();
        hd
    }};
}

extism::host_fn!(pub mock_kv_put(user_data: H; key: String, value_json: String) -> String {
    let hd = clone_h!(user_data);
    hd.lock().unwrap().kv.insert(key, value_json);
    Ok(envelope_ok(json!({})))
});

extism::host_fn!(pub mock_kv_get(user_data: H; key: String) -> String {
    let hd = clone_h!(user_data);
    let h = hd.lock().unwrap();
    match h.kv.get(&key) {
        Some(v) => Ok(envelope_ok(json!({"found": true, "value": serde_json::from_str::<Value>(v).unwrap_or(Value::Null)}))),
        None => Ok(envelope_ok(json!({"found": false, "value": null}))),
    }
});

extism::host_fn!(pub mock_kv_list(user_data: H; prefix: String) -> String {
    let hd = clone_h!(user_data);
    let h = hd.lock().unwrap();
    let items: Vec<Value> = h.kv.iter()
        .filter(|(k, _)| k.starts_with(&prefix))
        .map(|(k, v)| json!({
            "key": k.trim_start_matches(&prefix).to_string(),
            "value": serde_json::from_str::<Value>(v).unwrap_or(Value::Null),
        }))
        .collect();
    Ok(envelope_ok(json!({ "items": items, "count": items.len() })))
});

extism::host_fn!(pub mock_kv_delete(user_data: H; key: String) -> String {
    let hd = clone_h!(user_data);
    let removed = hd.lock().unwrap().kv.remove(&key).is_some();
    Ok(envelope_ok(json!({ "deleted": if removed {1} else {0} })))
});

extism::host_fn!(pub mock_get_object(user_data: H; id: String) -> String {
    let hd = clone_h!(user_data);
    let h = hd.lock().unwrap();
    match h.objects.get(&id) {
        Some(o) => Ok(envelope_ok(o.clone())),
        None => Ok(envelope_err("NOT_FOUND", &format!("Объект {id} не найден"))),
    }
});

extism::host_fn!(pub mock_transition(user_data: H; id: String, version: String, action: String) -> String {
    let hd = clone_h!(user_data);
    let mut h = hd.lock().unwrap();
    h.transitions.push((id.clone(), version.clone(), action.clone()));
    if let Some(obj) = h.objects.get_mut(&id) {
        obj["state"] = json!(if action == "post" { "posted" } else { "cancelled" });
        obj["version"] = json!(version.parse::<i64>().unwrap_or(1) + 1);
        obj["number"] = json!(format!("MAIN-REQ-{id}"));
    }
    Ok(envelope_ok(json!({"id": id, "version": 2, "state": "posted", "number": "N1"})))
});

extism::host_fn!(pub mock_whoami(user_data: H;) -> String {
    let hd = clone_h!(user_data);
    let mut u = hd.lock().unwrap().current_user.clone();
    if !hd.lock().unwrap().extra_role_ids.is_empty() {
        let ids = hd.lock().unwrap().extra_role_ids.clone();
        if let Some(obj) = u.as_object_mut() {
            obj.insert("role_ids".into(), serde_json::json!(ids));
        }
    }
    Ok(envelope_ok(u))
});

extism::host_fn!(pub mock_now_ms(user_data: H;) -> String {
    let hd = clone_h!(user_data);
    let mut h = hd.lock().unwrap();
    h.clock += 1000;
    Ok(envelope_ok(json!(h.clock.to_string())))
});

extism::host_fn!(pub mock_module_settings(user_data: H;) -> String {
    let hd = clone_h!(user_data);
    let s = hd.lock().unwrap().settings.clone();
    Ok(envelope_ok(s))
});

extism::host_fn!(pub mock_run_script(user_data: H; source: String, context_json: String) -> String {
    let fail: Option<String> = {
        let hd = clone_h!(user_data);
        let f = hd.lock().unwrap().script_fail.clone();
        f
    };
    if let Some(msg) = fail {
        return Ok(envelope_err("SCRIPT_FAILED", &msg));
    }
    match run_rhai(&context_json, &source) {
        Ok(result) => Ok(envelope_ok(json!({ "result": result }))),
        Err(e) => Ok(envelope_err("SCRIPT_FAILED", &e)),
    }
});

extism::host_fn!(pub mock_cms_verify(user_data: H; data_b64: String, sig_b64: String) -> String {
    let hd = clone_h!(user_data);
    let mut h = hd.lock().unwrap();
    h.cms_calls.push((data_b64.clone(), sig_b64.clone()));
    if h.verify_invalid {
        Ok(envelope_ok(serde_json::json!({ "valid": false, "message": "подпись не соответствует данным" })))
    } else {
        Ok(envelope_ok(serde_json::json!({
            "valid": true,
            "signer_subject": "CN=Test Signer",
            "signer_sha1": "aabbccdd00112233445566778899aabbccddeeff",
        })))
    }
});

extism::host_fn!(pub mock_notify(user_data: H; recipient: String, subject: String, body: String) -> String {
    let hd = clone_h!(user_data);
    hd.lock().unwrap().notifications.push((recipient, subject, body));
    Ok(envelope_ok(json!({ "id": "n1" })))
});

extism::host_fn!(pub mock_emit_event(user_data: H; stream_id: String, event_type: String, payload_json: String) -> String {
    let payload = serde_json::from_str::<Value>(&payload_json).unwrap_or(Value::Null);
    let hd = clone_h!(user_data);
    hd.lock().unwrap().events.push((stream_id, event_type, payload));
    Ok(envelope_ok(json!({ "emitted": true })))
});

extism::host_fn!(pub mock_log(_user_data: H; msg: String) -> String {
    eprintln!("[guest] {msg}");
    Ok(envelope_ok(json!({})))
});

// ── Тестовое приложение ────────────────────────────────────

struct TestApp {
    plugin: Plugin,
    harness: H,
}

impl TestApp {
    fn new(caller: &str, role: Option<&str>) -> Option<Self> {
        let wasm = match std::fs::read(WASM_PATH) {
            Ok(b) => b,
            Err(_) => return None, // плагин не собран — пропускаем тесты
        };
        let harness: H = Arc::new(Mutex::new(Harness::new(caller, role)));
        let manifest = Manifest::new([Wasm::data(wasm)]);
        let plugin = PluginBuilder::new(&manifest)
            .with_function("kv_put", [PTR, PTR], [PTR], UserData::new(harness.clone()), mock_kv_put)
            .with_function("kv_get", [PTR], [PTR], UserData::new(harness.clone()), mock_kv_get)
            .with_function("kv_list", [PTR], [PTR], UserData::new(harness.clone()), mock_kv_list)
            .with_function("kv_delete", [PTR], [PTR], UserData::new(harness.clone()), mock_kv_delete)
            .with_function("get_object", [PTR], [PTR], UserData::new(harness.clone()), mock_get_object)
            .with_function("transition_object", [PTR, PTR, PTR], [PTR], UserData::new(harness.clone()), mock_transition)
            .with_function("whoami", [], [PTR], UserData::new(harness.clone()), mock_whoami)
            .with_function("now_ms", [], [PTR], UserData::new(harness.clone()), mock_now_ms)
            .with_function("module_settings", [], [PTR], UserData::new(harness.clone()), mock_module_settings)
            .with_function("run_script", [PTR, PTR], [PTR], UserData::new(harness.clone()), mock_run_script)
            .with_function("notify_user", [PTR, PTR, PTR], [PTR], UserData::new(harness.clone()), mock_notify)
            .with_function("emit_event", [PTR, PTR, PTR], [PTR], UserData::new(harness.clone()), mock_emit_event)
            .with_function("cms_verify", [PTR, PTR], [PTR], UserData::new(harness.clone()), mock_cms_verify)
            .with_function("log_message", [PTR], [], UserData::new(harness.clone()), mock_log)
            .with_fuel_limit(50_000_000)
            .build()
            .expect("плагин должен загрузиться");
        Some(Self { plugin, harness })
    }

    /// Вызвать функцию госта, вернуть результат или паниковать.
    /// Гость возвращает данные НАПРЯМУЮ (Json<T>); конверт {ok,data}
    /// используется только на границе host→guest, не для выходов плагина.
    #[track_caller]
    fn call(&mut self, function: &str, args: Value) -> Value {
        match self.plugin.call::<&[u8], String>(function, args.to_string().as_bytes()) {
            Ok(out) => serde_json::from_str(&out)
                .unwrap_or_else(|_| Value::String(out.clone())),
            Err(e) => panic!("{function}: гость упал: {e}"),
        }
    }

    /// Вызвать функцию, ожидая ошибку госта (? пробрасывает anyhow::Error
    /// с нашим текстом "CODE: message"); вернуть текст ошибки.
    #[track_caller]
    fn call_err(&mut self, function: &str, args: Value) -> String {
        match self.plugin.call::<&[u8], String>(function, args.to_string().as_bytes()) {
            Ok(out) => panic!("{function}: ожидали ошибку, получили ok: {out}"),
            Err(e) => e.to_string(),
        }
    }

    fn switch_user(&self, user: &str, role: Option<&str>) {
        let mut h = self.harness.lock().unwrap();
        h.current_user["user_id"] = json!(user);
        h.current_user["role_id"] = role.map(|r| serde_json::json!(r)).unwrap_or(Value::Null);
    }

    /// Сохранить маршрут через сам плагин (двойная польза: тестируем routes_save).
    fn save_route(&mut self, code: &str, requires_signature: bool, approvers: &[&str]) {
        let steps: Vec<Value> = approvers
            .iter()
            .enumerate()
            .map(|(i, u)| json!({
                "step_order": i + 1,
                "approver_type": "user",
                "approver_id": u,
                "timeout_hours": 0,
                "is_required": true,
            }))
            .collect();
        self.call("routes_save", json!({
            "code": code,
            "name": format!("Маршрут {code}"),
            "steps": steps,
            "requires_signature": requires_signature,
            "is_active": true,
        }));
    }

    fn seed_object(&self, id: &str) {
        self.harness.lock().unwrap().add_draft_object(id);
    }
}

// ── Вспомогательные сценарии ───────────────────────────────

fn app_unsigned_route() -> TestApp {
    let mut app = TestApp::new(U1, None).expect("wasm не найден");
    app.save_route("SIMPLE", false, &[U2]);
    app.seed_object("req-1");
    app
}

fn approve_current(app: &mut TestApp, request_id: &str, comment: &str) {
    app.call("approve_step", json!({
        "request_id": request_id,
        "comment": comment,
        "signature_der": "",
    }));
}

// ── Тесты ──────────────────────────────────────────────────

#[test]
#[allow(dead_code)]
fn skipped_when_wasm_missing() {
    if std::fs::metadata(WASM_PATH).is_err() {
        eprintln!("SKIP: {WASM_PATH} не найден — сначала соберите плагин");
    }
}

#[test]
fn routes_save_validation() {
    let mut app = TestApp::new(U1, None).unwrap();

    let err = app.call_err("routes_save", json!({"code": "", "name": "", "steps": []}));
    assert!(err.contains("VALIDATION"), "{err}");

    let err = app.call_err("routes_save", json!({"code": "X", "name": "N", "steps": []}));
    assert!(err.contains("хотя бы один этап"), "{err}");

    let err = app.call_err("routes_save", json!({
        "code": "X", "name": "N",
        "steps": [{"step_order": 1, "approver_type": "user", "approver_id": ""}],
    }));
    assert!(err.contains("без утверждающего"), "{err}");
}

#[test]
fn submit_happy_path_unsigned() {
    let mut app = app_unsigned_route();

    let a = app.call("submit", json!({"request_id": "req-1", "route_code": "SIMPLE"}));

    assert_eq!(a["status"], "in_progress");
    assert_eq!(a["current_step"], 0);
    assert_eq!(a["requires_signature"], false);
    assert_eq!(a["initiator_id"], U1);

    let h = app.harness.lock().unwrap();
    // Событие submitted
    assert!(h.events.iter().any(|(_, t, p)| t == "request.submitted"
        && p["request_id"] == "req-1"
        && p["route_code"] == "SIMPLE"));
    // Уведомление первому утверждающему
    assert!(h.notifications.iter().any(|(to, _, _)| to == U2));
    // Подпись не требовалась — поле пустое
    assert!(a["submit_signature_der"].is_null());
}

#[test]
fn submit_rejects_non_draft_and_duplicate() {
    let mut app = app_unsigned_route();

    // Не черновик
    app.harness.lock().unwrap().objects.get_mut("req-1").unwrap()["state"] = json!("posted");
    let err = app.call_err("submit", json!({"request_id": "req-1", "route_code": "SIMPLE"}));
    assert!(err.contains("только в статусе draft"), "{err}");

    // Дубликат активного согласования
    app.seed_object("req-2");
    app.call("submit", json!({"request_id": "req-2", "route_code": "SIMPLE"}));
    let err = app.call_err("submit", json!({"request_id": "req-2", "route_code": "SIMPLE"}));
    assert!(err.contains("CONFLICT"), "{err}");
}

#[test]
fn signed_route_requires_certificate() {
    let mut app = TestApp::new(U1, None).unwrap();
    app.save_route("SIGNED", true, &[U2]);
    app.seed_object("req-s");

    // Без подписи — отказ
    let err = app.call_err("submit", json!({"request_id": "req-s", "route_code": "SIGNED"}));
    assert!(err.contains("SIGNATURE_REQUIRED"), "{err}");

    // С подписью — ок, DER сохранён
    let a = app.call("submit", json!({
        "request_id": "req-s", "route_code": "SIGNED",
        "signature_der": "TUlNRUQtREVS",
    }));
    assert_eq!(a["submit_signature_der"], "TUlNRUQtREVS");

    // Решение тоже требует подпись
    app.switch_user(U2, None);
    let err = app.call_err("approve_step", json!({"request_id": "req-s", "comment": "", "signature_der": ""}));
    assert!(err.contains("SIGNATURE_REQUIRED"), "{err}");
}

#[test]
fn approve_by_wrong_user_forbidden() {
    let mut app = app_unsigned_route();
    app.call("submit", json!({"request_id": "req-1", "route_code": "SIMPLE"}));

    // Не тот пользователь
    app.switch_user(U3, None);
    let err = app.call_err("approve_step", json!({"request_id": "req-1", "comment": "", "signature_der": ""}));
    assert!(err.contains("FORBIDDEN"), "{err}");

    // Правильный — работает
    app.switch_user(U2, None);
    approve_current(&mut app, "req-1", "ок");
    let a = app.call("approval_get", json!({"request_id": "req-1"}));
    assert_eq!(a["status"], "approved");
}

#[test]
fn full_chain_posts_object_and_emits_events() {
    let mut app = app_unsigned_route();
    app.call("submit", json!({"request_id": "req-1", "route_code": "SIMPLE"}));

    app.switch_user(U2, None);
    approve_current(&mut app, "req-1", "согласовано");

    let h = app.harness.lock().unwrap();

    // Заявка проведена через transition_object
    assert!(h.transitions.iter().any(|(id, _, act)| id == "req-1" && act == "post"));
    assert_eq!(h.objects["req-1"]["state"], "posted");

    // Полная цепочка событий
    let types: Vec<&str> = h.events.iter().map(|(_, t, _)| t.as_str()).collect();
    assert!(types.contains(&"request.submitted"));
    assert!(types.contains(&"request.step_approved"));
    assert!(types.contains(&"request.completed"));

    // Инициатор уведомлён об успехе
    assert!(h.notifications.iter().any(|(to, subj, _)| to == U1 && subj.contains("согласована")));
}

#[test]
fn rejection_stops_process_and_notifies_initiator() {
    let mut app = app_unsigned_route();
    app.call("submit", json!({"request_id": "req-1", "route_code": "SIMPLE"}));

    app.switch_user(U2, None);
    let a = app.call("reject_step", json!({"request_id": "req-1", "comment": "не согласовано", "signature_der": ""}));
    assert_eq!(a["status"], "rejected");

    let a = app.call("approval_get", json!({"request_id": "req-1"}));
    assert_eq!(a["status"], "rejected");
    assert!(!a["completed_at"].is_null());

    let h = app.harness.lock().unwrap();
    assert!(h.events.iter().any(|(_, t, _)| t == "request.rejected"));
    assert!(h.notifications.iter().any(|(to, subj, body)| to == U1 && subj.contains("отклонена") && body.contains("не согласовано")));
    // Проведения быть не должно
    assert!(h.transitions.is_empty());
}

#[test]
fn cancel_only_by_initiator() {
    let mut app = app_unsigned_route();
    app.call("submit", json!({"request_id": "req-1", "route_code": "SIMPLE"}));

    app.switch_user(U2, None);
    let err = app.call_err("cancel_request", json!({"request_id": "req-1"}));
    assert!(err.contains("FORBIDDEN"), "{err}");

    app.switch_user(U1, None);
    let a = app.call("cancel_request", json!({"request_id": "req-1"}));
    assert_eq!(a["status"], "cancelled");
    // Все незавершённые шаги помечены skipped
    assert_eq!(a["steps"][0]["status"], "skipped");
}

#[test]
fn before_submit_hook_can_abort() {
    let mut app = app_unsigned_route();
    app.harness.lock().unwrap().settings = json!({
        "before_submit": r#"if ctx.request.data.title != "особая" { throw "нужна особая заявка"; }"#
    });

    let err = app.call_err("submit", json!({"request_id": "req-1", "route_code": "SIMPLE"}));
    assert!(err.contains("before_submit") || err.contains("нужна особая"), "{err}");

    // Согласование НЕ создано
    let a = app.call("approval_get", json!({"request_id": "req-1"}));
    assert!(a.is_null(), "процесс не должен был создаться: {a}");
}

#[test]
fn pending_approvals_filtered_by_assignee() {
    let mut app = app_unsigned_route();
    app.call("submit", json!({"request_id": "req-1", "route_code": "SIMPLE"}));

    app.switch_user(U3, None);
    let for_u3 = app.call("pending_approvals", json!({}));
    assert_eq!(for_u3.as_array().map(Vec::len), Some(0));

    app.switch_user(U2, None);
    let for_u2 = app.call("pending_approvals", json!({}));
    assert_eq!(for_u2.as_array().map(|a| a.len()), Some(1));
    assert_eq!(for_u2[0]["request_id"], "req-1");
}

#[test]
fn role_based_approval() {
    let mut app = TestApp::new(U1, None).unwrap();
    let steps = vec![json!({
        "step_order": 1, "approver_type": "role", "approver_id": "ROLE_FIN",
        "timeout_hours": 0, "is_required": true,
    })];
    app.call("routes_save", json!({
        "code": "FIN", "name": "Финансовый", "steps": steps,
        "requires_signature": false, "is_active": true,
    }));
    app.seed_object("req-f");
    app.call("submit", json!({"request_id": "req-f", "route_code": "FIN"}));

    // Пользователь с другой ролью — нельзя
    app.switch_user(U2, Some("ROLE_HR"));
    let err = app.call_err("approve_step", json!({"request_id": "req-f", "comment": "", "signature_der": ""}));
    assert!(err.contains("FORBIDDEN"), "{err}");

    // С нужной ролью — можно
    app.switch_user(U2, Some("ROLE_FIN"));
    approve_current(&mut app, "req-f", "финансы ок");
    let a = app.call("approval_get", json!({"request_id": "req-f"}));
    assert_eq!(a["status"], "approved");
}

#[test]
fn multi_step_chain_advances_correctly() {
    let mut app = TestApp::new(U1, None).unwrap();
    app.save_route("TWO", false, &[U2, U3]);
    app.seed_object("req-m");
    app.call("submit", json!({"request_id": "req-m", "route_code": "TWO"}));

    // Первый этап
    app.switch_user(U2, None);
    approve_current(&mut app, "req-m", "шаг 1");

    let a = app.call("approval_get", json!({"request_id": "req-m"}));
    assert_eq!(a["status"], "in_progress");
    assert_eq!(a["current_step"], 1);

    let h = app.harness.lock().unwrap();
    assert!(h.notifications.iter().any(|(to, _, _)| to == U3)); // следующий утверждён уведомлён
    assert!(h.transitions.is_empty()); // ещё рано проводить
    drop(h);

    // Второй этап → завершение
    app.switch_user(U3, None);
    approve_current(&mut app, "req-m", "шаг 2");

    let a = app.call("approval_get", json!({"request_id": "req-m"}));
    assert_eq!(a["status"], "approved");
    let h = app.harness.lock().unwrap();
    assert_eq!(h.transitions.len(), 1);
    let types: Vec<&str> = h.events.iter().map(|(_, t, _)| t.as_str()).collect();
    assert_eq!(types.iter().filter(|t| **t == "request.step_approved").count(), 2);
    assert!(types.contains(&"request.completed"));
}

#[test]
fn routes_lifecycle() {
    let mut app = TestApp::new(U1, None).unwrap();
    app.save_route("A", false, &[U2]);
    app.save_route("B", true, &[U2]);

    let list = app.call("routes_list", json!({}));
    let codes: Vec<&str> = list.as_array().unwrap().iter().map(|r| r["code"].as_str().unwrap()).collect();
    assert!(codes.contains(&"A") && codes.contains(&"B"));

    // Обновление маршрута A (upsert по коду)
    app.save_route("A", true, &[U2, U3]);
    let list = app.call("routes_list", json!({}));
    assert_eq!(list.as_array().unwrap().len(), 2, "upsert не должен плодить дубли");

    let d = app.call("routes_delete", json!({"code": "A"}));
    assert_eq!(d["deleted"], 1);
    let list = app.call("routes_list", json!({}));
    assert_eq!(list.as_array().unwrap().len(), 1);
}

#[test]
fn approval_get_unknown_returns_null() {
    let mut app = TestApp::new(U1, None).unwrap();
    let a = app.call("approval_get", json!({"request_id": "nope"}));
    assert!(a.is_null());
}


// ── RQ1: верификация подписи и слепок ──────────────────────

#[test]
fn invalid_signature_aborts_and_records_nothing() {
    let mut app = TestApp::new(U1, None).unwrap();
    app.save_route("SIGNED", true, &[U2]);
    app.seed_object("req-sig");

    // Подписанная отправка проходит и фиксирует слепок
    let a = app.call("submit", json!({
        "request_id": "req-sig", "route_code": "SIGNED",
        "signature_der": "QUJDREVG",
    }));
    assert_eq!(a["submit_verified"], true);
    assert_eq!(
        a["submitted_payload_sha256"].as_str().map(|s| s.len()),
        Some(64),
        "sha256 слепка обязателен"
    );
    let payload = a["submitted_payload"].as_str().unwrap().to_string();
    assert!(payload.starts_with("requests.submit|req-sig|1|draft"), "{payload}");

    // Ломаем верификацию решения
    app.switch_user(U2, None);
    app.harness.lock().unwrap().verify_invalid = true;
    let err = app.call_err("approve_step", json!({
        "request_id": "req-sig", "comment": "", "signature_der": "QUJDREVG",
    }));
    assert!(err.contains("SIGNATURE_INVALID"), "{err}");

    // Решение НЕ записано: шаг остался pending, процесс жив
    app.harness.lock().unwrap().verify_invalid = false;
    let a = app.call("approval_get", json!({"request_id": "req-sig"}));
    assert_eq!(a["status"], "in_progress");
    assert_eq!(a["steps"][0]["status"], "pending");
    assert_eq!(a["steps"][0]["verified"], false);

    // После восстановления верификации решение проходит С ПОДПИСЬЮ
    let a = app.call("approve_step", json!({
        "request_id": "req-sig", "comment": "", "signature_der": "QUJDREVG",
    }));
    let step = &a["steps"][0];
    assert_eq!(step["status"], "approved");
    assert_eq!(step["verified"], true);
    assert_eq!(step["payload_sha256"].as_str().map(|s| s.len()), Some(64));
    assert_eq!(step["signer_subject"], "CN=Test Signer");
    assert!(step["signed_payload"]
        .as_str()
        .unwrap()
        .starts_with("requests.decide|req-sig|approve|"));
}

#[test]
fn unsigned_route_rejects_unexpected_signature() {
    let mut app = TestApp::new(U1, None).unwrap();
    app.save_route("SIMPLE", false, &[U2]);
    app.seed_object("req-u");

    let err = app.call_err("submit", json!({
        "request_id": "req-u", "route_code": "SIMPLE",
        "signature_der": "QUJD",
    }));
    assert!(err.contains("CONTRACT"), "{err}");
}

// ── RQ3: мультироли ────────────────────────────────────────

#[test]
fn multi_role_approver_intersection() {
    // U2 — начальник склада (primary ROLE_WH), по совместительству
    // финдиректор (ROLE_FIN в role_ids). Этап назначен на ROLE_FIN.
    let mut app = TestApp::new(U1, None).unwrap();

    let steps = vec![json!({
        "step_order": 1, "approver_type": "role", "approver_id": "ROLE_FIN",
        "timeout_hours": 0, "is_required": true,
    })];
    app.call("routes_save", json!({
        "code": "FIN", "name": "Фин", "steps": steps,
        "requires_signature": false, "is_active": true,
    }));
    app.seed_object("req-mr");
    app.call("submit", json!({"request_id": "req-mr", "route_code": "FIN"}));

    // Primary-роль НЕ совпадает, но роль есть в role_ids → может согласовать
    app.switch_user(U2, None);
    {
        let mut h = app.harness.lock().unwrap();
        h.current_user["role_id"] = json!("ROLE_WH");
        h.extra_role_ids = vec!["ROLE_WH".to_string(), "ROLE_FIN".to_string()];
    }

    let pend = app.call("pending_approvals", json!({}));
    assert_eq!(pend.as_array().unwrap().len(), 1, "этап виден по второй роли");

    let a = app.call("approve_step", json!({
        "request_id": "req-mr", "comment": "по совместительству", "signature_der": "",
    }));
    assert_eq!(a["status"], "approved");

    // Пользователь вообще без нужной роли — не видит и не согласовывает
    let mut app2 = TestApp::new(U3, None).unwrap();
    app2.harness.lock().unwrap().extra_role_ids =
        vec!["ROLE_HR".to_string()];
    let pend = app2.call("pending_approvals", json!({}));
    assert_eq!(pend.as_array().unwrap().len(), 0);
}
