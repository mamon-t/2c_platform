//! Интеграционные приёмочные тесты Фазы 14 (раздел 14 ТЗ v3.1):
//! первый прикладной WASM-модуль `plugin.accounting` (план счетов,
//! учётные периоды, проводки, ОСВ и баланс, интеграция с документами).
//!
//! На мем-базе (`mem://`) проверяется полный цикл управления учётом:
//! установка модуля с 13 декларативными командами, CRUD счетов и периодов,
//! проведение/сторно проводок с валидацией баланса и открытого периода,
//! атомарное проведение документа (transaction + object.create), оборотно-
//! сальдовая ведомость и баланс. Фикстура — `examples/accounting_plugin`.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use core_application::AppRegistry;
use core_application::ModuleManager;
use core_domain::event::StreamType;
use core_domain::metadata::{
    EntityField, EntityKind, EntityState, EntityType, FieldType,
};
use core_domain::module::ModuleState;
use core_domain::object::Object;
use serde_json::{json, Value};
use surrealdb::engine::any::Any;
use surrealdb::Surreal;
use uuid::Uuid;

use core_application::ports::{EntitySchema, EventStore, MetadataRepository, ObjectRepository};
use core_infrastructure::extism_wasm_host::ExtismWasmHost;
use core_infrastructure::surreal_audit_repository::SurrealAuditRepository;
use core_infrastructure::surreal_metadata_repository::SurrealMetadataRepository;
use core_infrastructure::surreal_module_repository::SurrealModuleRepository;
use core_infrastructure::surreal_object_repository::SurrealObjectRepository;
use core_infrastructure::surreal_permission_policy_repository::SurrealPermissionPolicyRepository;
use core_infrastructure::surreal_script_repository::SurrealScriptRepository;
use core_infrastructure::SurrealEventStore;

const ACCT_WASM: &[u8] = include_bytes!("fixtures/accounting.wasm");

// Компания тестов и базовые счёта/период, на которых строятся сценарии.
const COMP: &str = "comp1";

async fn mem_db() -> Surreal<Any> {
    let db = surrealdb::engine::any::connect("mem://").await.unwrap();
    db.use_ns("test")
        .use_db(Uuid::new_v4().to_string())
        .await
        .unwrap();
    db
}

fn temp_cache() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("2c-14-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

struct Env {
    _db: Surreal<Any>,
    manager: Arc<ModuleManager>,
    objects: Arc<SurrealObjectRepository>,
    metadata: Arc<SurrealMetadataRepository>,
    store: Arc<SurrealEventStore>,
    app: Arc<AppRegistry>,
}

async fn setup() -> Env {
    let db = mem_db().await;

    let store = Arc::new(SurrealEventStore::new(db.clone()));
    store.ensure_schema().await.unwrap();
    let objects = Arc::new(SurrealObjectRepository::new(db.clone()));
    objects.ensure_schema().await.unwrap();
    let metadata = Arc::new(SurrealMetadataRepository::new(db.clone()));
    metadata.ensure_schema().await.unwrap();
    let audit = Arc::new(SurrealAuditRepository::new(db.clone()));
    audit.ensure_schema().await.unwrap();
    let policies = Arc::new(SurrealPermissionPolicyRepository::new(db.clone()));
    policies.ensure_schema().await.unwrap();
    let modules = Arc::new(SurrealModuleRepository::new(db.clone()));
    modules.ensure_schema().await.unwrap();
    let users = Arc::new(core_infrastructure::SurrealUserRepository::new(db.clone()));
    users.ensure_schema().await.unwrap();

    let transactions = core_application::TransactionOrchestrator::new(objects.clone(), metadata.clone());
    let scripts = Arc::new(SurrealScriptRepository::new(db.clone()));
    scripts.ensure_schema().await.unwrap();
    let script_engine = Arc::new(
        core_infrastructure::RhaiScriptEngine::new(core_infrastructure::rhai_core_api::CoreApiShared {
            store: store.clone(),
            db: db.clone(),
            audit: audit.clone(),
            runtime: tokio::runtime::Handle::current(),
        })
        .unwrap(),
    );
    let host = Arc::new(
        ExtismWasmHost::new(
            db.clone(),
            objects.as_ref().clone(),
            metadata.as_ref().clone(),
            store.as_ref().clone(),
            users.as_ref().clone(),
            transactions,
            script_engine,
            temp_cache(),
        )
        .unwrap(),
    );
    host.ensure_schema().await.unwrap();

    let app = Arc::new(AppRegistry::new());
    let manager = Arc::new(ModuleManager::new(
        host,
        modules.clone(),
        app.clone(),
        policies,
        metadata.clone(),
        scripts,
        audit,
        temp_cache(),
    ));

    Env {
        _db: db,
        manager,
        objects,
        metadata,
        store,
        app,
    }
}

/// Дергает плагинную команду с компанией и входным JSON; возвращает `data`
/// успешного конверта. Падает с пояснением, если команда вернула `ok: false`
/// или сетевая ошибка.
async fn cmd(env: &Env, name: &str, input: Value) -> Value {
    let out = env
        .app
        .commands
        .execute(name, json!({ "company_id": COMP, "input": input.to_string() }))
        .await
        .unwrap();
    let raw = out["output"].as_str().expect("output должен быть строкой");
    let v: Value = serde_json::from_str(raw).expect("output не валидный JSON");
    assert!(v["ok"].as_bool().unwrap_or(false), "команда {name} упала: {v}");
    v["data"].clone()
}

/// Дергает плагинную команду и возвращает тело ошибки (`ok: false`).
async fn cmd_err(env: &Env, name: &str, input: Value) -> Value {
    let out = env
        .app
        .commands
        .execute(name, json!({ "company_id": COMP, "input": input.to_string() }))
        .await
        .unwrap();
    let raw = out["output"].as_str().expect("output должен быть строкой");
    let v: Value = serde_json::from_str(raw).expect("output не валидный JSON");
    assert!(!v["ok"].as_bool().unwrap_or(true), "команда {name} неожиданно успешна: {v}");
    v["error"].clone()
}

/// Открывает период и создаёт три типовых счёта. Возвращает id периода.
async fn seed_financials(env: &Env) -> String {
    cmd(env, "plugin.accounting.account.create", json!({
        "code": "10.01", "name": "Касса", "type": "asset",
    }))
    .await;
    cmd(env, "plugin.accounting.account.create", json!({
        "code": "80.01", "name": "Уставный капитал", "type": "equity",
    }))
    .await;
    cmd(env, "plugin.accounting.account.create", json!({
        "code": "90.01", "name": "Выручка", "type": "revenue",
    }))
    .await;
    let period = cmd(env, "plugin.accounting.period.open", json!({
        "year": 2026, "month": 9, "name": "Сентябрь",
    }))
    .await;
    period["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn install_registers_13_commands_schemas_and_permissions() {
    let env = setup().await;

    let record = env
        .manager
        .install("accounting", ACCT_WASM, COMP)
        .await
        .unwrap();
    assert_eq!(record.code, "accounting");
    assert_eq!(record.state, ModuleState::Installed);
    assert_eq!(record.wasm_sha256.len(), 16);
    assert_eq!(record.manifest.commands.len(), 13);

    // Декларация команд: код с точками, экспорт функции с подчёркиванием.
    for c in &record.manifest.commands {
        assert!(
            c.required_permission.is_some(),
            "команда {} должна требовать permission",
            c.code
        );
    }
    let account_create = record
        .manifest
        .commands
        .iter()
        .find(|c| c.code == "account.create")
        .expect("должна быть account.create");
    assert_eq!(account_create.function.as_deref(), Some("account_create"));

    // Политики и схемы зарегистрированы в AppRegistry.
    let commands = env.app.commands.list().await;
    for name in [
        "plugin.accounting.account.create",
        "plugin.accounting.account.list",
        "plugin.accounting.period.open",
        "plugin.accounting.entry.post",
        "plugin.accounting.entry.reverse",
        "plugin.accounting.doc.post",
        "plugin.accounting.balance.trial",
        "plugin.accounting.balance.sheet",
    ] {
        assert!(commands.contains(&name.to_string()), "нет команды {name}");
    }
    assert!(env.app.permissions.contains("accounting").await);
    assert!(env.app.object_schemas.contains("accounting").await);

    // Типы сущностей модуля созданы в метаданных для компании.
    for code in ["account", "accounting_period", "ledger_entry"] {
        env.metadata
            .get_entity_type_by_code(COMP, code)
            .await
            .expect("тип {code} должен быть зарегистрирован");
    }

    // Труба фиксирует установку модуля.
    let stream = env
        .store
        .read_stream(StreamType::Module, "accounting")
        .await
        .unwrap();
    let types: HashSet<String> = stream.iter().map(|e| e.event_type.clone()).collect();
    assert!(types.contains("module.installed"));
}

#[tokio::test]
async fn account_create_list_get_roundtrip() {
    let env = setup().await;
    env.manager.install("accounting", ACCT_WASM, COMP).await.unwrap();

    let created = cmd(&env, "plugin.accounting.account.create", json!({
        "code": "10.01", "name": "Касса", "type": "asset",
    }))
    .await;
    let id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["code"], "10.01");
    assert_eq!(created["account_type"], "asset");

    let listed = cmd(&env, "plugin.accounting.account.list", json!({})).await;
    assert_eq!(listed["total"], 1);
    let row = &listed["accounts"][0];
    assert_eq!(row["code"], "10.01");
    assert_eq!(row["name"], "Касса");
    assert_eq!(row["account_type"], "asset");
    assert_eq!(row["is_active"], true);

    let got = cmd(&env, "plugin.accounting.account.get", json!({ "id": id })).await;
    assert_eq!(got["id"], id);
    assert_eq!(got["code"], "10.01");
    assert!(got["state"].as_str().is_some());
}

#[tokio::test]
async fn account_create_rejects_duplicate_code_and_bad_type() {
    let env = setup().await;
    env.manager.install("accounting", ACCT_WASM, COMP).await.unwrap();

    cmd(&env, "plugin.accounting.account.create", json!({
        "code": "10.01", "name": "Касса", "type": "asset",
    }))
    .await;

    let err = cmd_err(&env, "plugin.accounting.account.create", json!({
        "code": "10.01", "name": "Дубликат", "type": "asset",
    }))
    .await;
    assert!(err["code"].as_str().unwrap().contains("ACCOUNTING"));

    let err = cmd_err(&env, "plugin.accounting.account.create", json!({
        "code": "19.01", "name": "Кривой", "type": "несуществующий",
    }))
    .await;
    assert!(err["code"].as_str().unwrap().contains("ACCOUNTING"));

    // Ничего лишнего не создалось.
    let listed = cmd(&env, "plugin.accounting.account.list", json!({})).await;
    assert_eq!(listed["total"], 1);
}

#[tokio::test]
async fn account_update_patches_fields_with_occ() {
    let env = setup().await;
    env.manager.install("accounting", ACCT_WASM, COMP).await.unwrap();

    let created = cmd(&env, "plugin.accounting.account.create", json!({
        "code": "10.01", "name": "Касса", "type": "asset",
    }))
    .await;
    let id = created["id"].as_str().unwrap().to_string();

    let upd = cmd(&env, "plugin.accounting.account.update", json!({
        "id": id, "version": 1, "data": { "name": "Касса операционная", "is_active": false },
    }))
    .await;
    assert_eq!(upd["id"], id);
    assert_eq!(upd["version"], 2);

    let got = cmd(&env, "plugin.accounting.account.get", json!({ "id": id })).await;
    assert_eq!(got["name"], "Касса операционная");
    assert_eq!(got["is_active"], false);
    assert_eq!(got["code"], "10.01");

    // Устаревшая версия — конфликт OCC.
    let err = cmd_err(&env, "plugin.accounting.account.update", json!({
        "id": id, "version": 1, "data": { "name": "Ещё раз" },
    }))
    .await;
    assert!(err["code"].as_str().unwrap().contains("ACCOUNTING"));
}

#[tokio::test]
async fn period_open_close_list() {
    let env = setup().await;
    env.manager.install("accounting", ACCT_WASM, COMP).await.unwrap();

    let opened = cmd(&env, "plugin.accounting.period.open", json!({
        "year": 2026, "month": 9, "name": "Сентябрь",
    }))
    .await;
    let id = opened["id"].as_str().unwrap().to_string();
    assert_eq!(opened["year"], 2026);
    assert_eq!(opened["month"], 9);
    assert_eq!(opened["status"], "open");

    // Дубликат периода отклоняется.
    let err = cmd_err(&env, "plugin.accounting.period.open", json!({
        "year": 2026, "month": 9, "name": "Повтор",
    }))
    .await;
    assert!(err["code"].as_str().unwrap().contains("ACCOUNTING"));

    let listed = cmd(&env, "plugin.accounting.period.list", json!({})).await;
    assert_eq!(listed["total"], 1);
    assert_eq!(listed["periods"][0]["status"], "open");

    let closed = cmd(&env, "plugin.accounting.period.close", json!({
        "id": id, "version": 1,
    }))
    .await;
    assert_eq!(closed["status"], "closed");

    // Закрытие уже закрытого периода — ошибка.
    let err = cmd_err(&env, "plugin.accounting.period.close", json!({
        "id": id, "version": 2,
    }))
    .await;
    assert!(err["code"].as_str().unwrap().contains("ACCOUNTING"));
}

#[tokio::test]
async fn entry_post_creates_ledger_entry_and_event() {
    let env = setup().await;
    env.manager.install("accounting", ACCT_WASM, COMP).await.unwrap();
    seed_financials(&env).await;

    let posted = cmd(&env, "plugin.accounting.entry.post", json!({
        "date": "2026-09-15",
        "description": "Поступление в кассу",
        "lines": [
            { "account": "10.01", "debit": 10000, "credit": 0 },
            { "account": "90.01", "debit": 0, "credit": 10000 },
        ],
    }))
    .await;
    let id = posted["id"].as_str().unwrap().to_string();
    assert_eq!(posted["status"], "posted");

    // Материализованная проводка создана со всеми данными.
    let got = cmd(&env, "plugin.accounting.entry.list", json!({})).await;
    assert_eq!(got["total"], 1);
    let row = &got["entries"][0];
    assert_eq!(row["id"], id);
    assert_eq!(row["status"], "posted");
    assert_eq!(row["source_type"], "manual");
    assert_eq!(row["lines"][0]["account"], "10.01");
    assert_eq!(row["lines"][0]["debit"], 10000);

    // Труба: события создания объекта и бизнес-событие проводки.
    let stream = env
        .store
        .read_stream(StreamType::Object, &id)
        .await
        .unwrap();
    let types: HashSet<String> = stream.iter().map(|e| e.event_type.clone()).collect();
    for expected in ["object.created", "ledger_entry.posted"] {
        assert!(types.contains(expected), "поток {id} должен содержать {expected}");
    }
}

#[tokio::test]
async fn entry_post_rejects_unbalanced_and_bad_lines() {
    let env = setup().await;
    env.manager.install("accounting", ACCT_WASM, COMP).await.unwrap();
    seed_financials(&env).await;

    // Дебет != кредит.
    let err = cmd_err(&env, "plugin.accounting.entry.post", json!({
        "date": "2026-09-15", "lines": [
            { "account": "10.01", "debit": 100, "credit": 0 },
            { "account": "90.01", "debit": 0, "credit": 99 },
        ],
    }))
    .await;
    assert!(err["code"].as_str().unwrap().contains("ACCOUNTING"));

    // Неизвестный счёт.
    let err = cmd_err(&env, "plugin.accounting.entry.post", json!({
        "date": "2026-09-15", "lines": [
            { "account": "99.99", "debit": 100, "credit": 0 },
            { "account": "90.01", "debit": 0, "credit": 100 },
        ],
    }))
    .await;
    assert!(err["code"].as_str().unwrap().contains("ACCOUNTING"));

    // Дата вне открытого периода.
    let err = cmd_err(&env, "plugin.accounting.entry.post", json!({
        "date": "2023-01-01", "lines": [
            { "account": "10.01", "debit": 100, "credit": 0 },
            { "account": "90.01", "debit": 0, "credit": 100 },
        ],
    }))
    .await;
    assert!(err["code"].as_str().unwrap().contains("ACCOUNTING"));

    let listed = cmd(&env, "plugin.accounting.entry.list", json!({})).await;
    assert_eq!(listed["total"], 0);
}

#[tokio::test]
async fn entry_list_filters_by_date_and_account() {
    let env = setup().await;
    env.manager.install("accounting", ACCT_WASM, COMP).await.unwrap();
    seed_financials(&env).await;

    cmd(&env, "plugin.accounting.entry.post", json!({
        "date": "2026-09-05", "lines": [
            { "account": "10.01", "debit": 1000, "credit": 0 },
            { "account": "90.01", "debit": 0, "credit": 1000 },
        ],
    }))
    .await;
    cmd(&env, "plugin.accounting.entry.post", json!({
        "date": "2026-09-20", "lines": [
            { "account": "10.01", "debit": 2000, "credit": 0 },
            { "account": "90.01", "debit": 0, "credit": 2000 },
        ],
    }))
    .await;

    let after = cmd(&env, "plugin.accounting.entry.list", json!({
        "date_from": "2026-09-15", "date_to": "2026-09-30",
    }))
    .await;
    assert_eq!(after["total"], 1);
    assert_eq!(after["entries"][0]["date"], "2026-09-20");

    let by_acct = cmd(&env, "plugin.accounting.entry.list", json!({
        "account": "10.01",
    }))
    .await;
    assert_eq!(by_acct["total"], 2);
}

#[tokio::test]
async fn entry_reverse_storno_balances_to_zero() {
    let env = setup().await;
    env.manager.install("accounting", ACCT_WASM, COMP).await.unwrap();
    seed_financials(&env).await;

    let orig = cmd(&env, "plugin.accounting.entry.post", json!({
        "date": "2026-09-15", "lines": [
            { "account": "10.01", "debit": 1000, "credit": 0 },
            { "account": "90.01", "debit": 0, "credit": 1000 },
        ],
    }))
    .await;
    let orig_id = orig["id"].as_str().unwrap().to_string();

    let rev = cmd(&env, "plugin.accounting.entry.reverse", json!({
        "id": orig_id, "reason": "Ошибочная проводка",
    }))
    .await;
    let rev_id = rev["reversal_id"].as_str().unwrap().to_string();
    assert_eq!(rev["original_id"], orig_id);
    assert_ne!(rev_id, orig_id);

    // Исходная проводка помечена сторнированной; сторно идёт обратными строками.
    let got = cmd(&env, "plugin.accounting.entry.list", json!({})).await;
    assert_eq!(got["total"], 2);
    let orig_row = got["entries"].as_array().unwrap().iter().find(|r| r["id"] == orig_id).unwrap();
    assert_eq!(orig_row["status"], "reversed");
    let rev_row = got["entries"].as_array().unwrap().iter().find(|r| r["id"] == rev_id).unwrap();
    assert_eq!(rev_row["status"], "posted");
    assert_eq!(rev_row["source_type"], "reversal");
    assert_eq!(rev_row["correlation_entry_id"], orig_id);
    assert_eq!(rev_row["lines"][0]["account"], "10.01");
    assert_eq!(rev_row["lines"][0]["credit"], 1000);

    // Повторное сторно сторнированной проводки недопустимо.
    let err = cmd_err(&env, "plugin.accounting.entry.reverse", json!({
        "id": orig_id,
    }))
    .await;
    assert!(err["code"].as_str().unwrap().contains("ACCOUNTING"));

    // Обороты исходной и сторно-проводки взаимно гасятся.
    let trial = cmd(&env, "plugin.accounting.balance.trial", json!({
        "date_from": "2026-09-01", "date_to": "2026-09-30",
    }))
    .await;
    let row10 = trial["rows"].as_array().unwrap().iter().find(|r| r["account"] == "10.01").unwrap();
    assert_eq!(row10["balance"].as_i64(), Some(0));
    let row90 = trial["rows"].as_array().unwrap().iter().find(|r| r["account"] == "90.01").unwrap();
    assert_eq!(row90["balance"].as_i64(), Some(0));
}

#[tokio::test]
async fn doc_post_posts_document_and_creates_entry_atomically() {
    let env = setup().await;
    env.manager.install("accounting", ACCT_WASM, COMP).await.unwrap();
    seed_financials(&env).await;

    // Документ-«счёт»: тип сущности + материализованный объект в draft.
    let now = chrono::Utc::now();
    let schema = EntitySchema {
        entity_type: EntityType {
            id: Uuid::new_v4(),
            code: "invoice".to_string(),
            name: "Счёт".to_string(),
            kind: EntityKind::Document,
            company_id: COMP.to_string(),
            metadata_version: 1,
            is_system: false,
            created_at: now,
            updated_at: now,
        },
        fields: vec![EntityField {
            id: Uuid::new_v4(),
            entity_type: "invoice".to_string(),
            code: "total".to_string(),
            label: "Сумма".to_string(),
            data_type: FieldType::Money,
            required: true,
            is_unique: false,
            is_indexed: false,
            options: json!({}),
            is_system: false,
            order: 1,
        }],
        states: vec![
            EntityState {
                id: Uuid::new_v4(),
                entity_type: "invoice".to_string(),
                code: "draft".to_string(),
                label: "Черновик".to_string(),
                color: None,
                is_initial: true,
                is_final: false,
            },
            EntityState {
                id: Uuid::new_v4(),
                entity_type: "invoice".to_string(),
                code: "posted".to_string(),
                label: "Проведён".to_string(),
                color: None,
                is_initial: false,
                is_final: true,
            },
        ],
        transitions: Vec::new(),
        forms: Vec::new(),
        actions: Vec::new(),
        relations: Vec::new(),
    };
    env.metadata.create_entity_type(&schema, &[]).await.unwrap();

    let invoice = Object {
        id: Uuid::new_v4(),
        entity_type: "invoice".to_string(),
        kind: EntityKind::Document,
        company_id: COMP.to_string(),
        state: "draft".to_string(),
        data: json!({ "total": 12000 }),
        computed: json!({}),
        number: None,
        date: None,
        parent_id: None,
        version: 1,
        created_by: "system".to_string(),
        updated_by: "system".to_string(),
        created_at: now,
        updated_at: now,
    };
    env.objects.create(&invoice, &[]).await.unwrap();
    let invoice_id = invoice.id.to_string();

    let posted = cmd(&env, "plugin.accounting.doc.post", json!({
        "document_id": invoice_id,
        "expected_version": 1,
        "entry": {
            "date": "2026-09-12",
            "description": "Оплата по счёту",
            "lines": [
                { "account": "10.01", "debit": 12000, "credit": 0 },
                { "account": "90.01", "debit": 0, "credit": 12000 },
            ],
        },
    }))
    .await;
    assert_eq!(posted["document_id"], invoice_id);
    assert!(!posted["entry_id"].as_str().unwrap().is_empty());
    let entry_id = posted["entry_id"].as_str().unwrap().to_string();

    // Документ проведён (state -> posted), проводка привязана source_id.
    let doc = env
        .objects
        .get(&Uuid::parse_str(&invoice_id).unwrap())
        .await
        .unwrap();
    assert_eq!(doc.state, "posted");
    assert_eq!(doc.version, 2);

    let got = cmd(&env, "plugin.accounting.entry.list", json!({})).await;
    assert_eq!(got["total"], 1);
    let row = got["entries"].as_array().unwrap().iter().find(|r| r["id"] == entry_id).unwrap();
    assert_eq!(row["source_type"], "document");
    assert_eq!(row["source_id"], invoice_id);
    assert_eq!(row["status"], "posted");

    // Атомарность: неверная версия документа откатывает и posting, и запись.
    let before = got["total"].as_u64().unwrap();
    let err = cmd_err(&env, "plugin.accounting.doc.post", json!({
        "document_id": invoice_id,
        "expected_version": 99,
        "entry": {
            "date": "2026-09-12",
            "lines": [
                { "account": "10.01", "debit": 500, "credit": 0 },
                { "account": "90.01", "debit": 0, "credit": 500 },
            ],
        },
    }))
    .await;
    assert!(err["code"].as_str().unwrap().contains("ACCOUNTING"));
    let after = cmd(&env, "plugin.accounting.entry.list", json!({})).await;
    assert_eq!(after["total"], before);
}

#[tokio::test]
async fn balance_trial_reports_turnovers_and_totals() {
    let env = setup().await;
    env.manager.install("accounting", ACCT_WASM, COMP).await.unwrap();
    seed_financials(&env).await;

    cmd(&env, "plugin.accounting.entry.post", json!({
        "date": "2026-09-10", "lines": [
            { "account": "10.01", "debit": 5000, "credit": 0 },
            { "account": "80.01", "debit": 0, "credit": 5000 },
        ],
    }))
    .await;
    cmd(&env, "plugin.accounting.entry.post", json!({
        "date": "2026-09-20", "lines": [
            { "account": "10.01", "debit": 3000, "credit": 0 },
            { "account": "90.01", "debit": 0, "credit": 3000 },
        ],
    }))
    .await;

    let trial = cmd(&env, "plugin.accounting.balance.trial", json!({
        "date_from": "2026-09-01", "date_to": "2026-09-30",
    }))
    .await;
    assert_eq!(trial["rows"].as_array().unwrap().len(), 3);

    let row10 = trial["rows"].as_array().unwrap().iter().find(|r| r["account"] == "10.01").unwrap();
    assert_eq!(row10["debit"].as_i64(), Some(8000));
    assert_eq!(row10["credit"].as_i64(), Some(0));
    assert_eq!(row10["balance"].as_i64(), Some(8000));

    let row80 = trial["rows"].as_array().unwrap().iter().find(|r| r["account"] == "80.01").unwrap();
    assert_eq!(row80["credit"].as_i64(), Some(5000));
    assert_eq!(row80["balance"].as_i64(), Some(-5000));

    // Итоги: дебетовые и кредитовые обороты сходятся.
    assert_eq!(trial["totals"]["debit"].as_i64(), Some(8000));
    assert_eq!(trial["totals"]["credit"].as_i64(), Some(8000));

    // Вне диапазона — пусто.
    let empty = cmd(&env, "plugin.accounting.balance.trial", json!({
        "date_from": "2025-01-01", "date_to": "2025-12-31",
    }))
    .await;
    assert_eq!(empty["rows"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn balance_sheet_groups_by_section() {
    let env = setup().await;
    env.manager.install("accounting", ACCT_WASM, COMP).await.unwrap();
    seed_financials(&env).await;

    cmd(&env, "plugin.accounting.entry.post", json!({
        "date": "2026-09-10", "lines": [
            { "account": "10.01", "debit": 5000, "credit": 0 },
            { "account": "80.01", "debit": 0, "credit": 5000 },
        ],
    }))
    .await;
    cmd(&env, "plugin.accounting.entry.post", json!({
        "date": "2026-09-20", "lines": [
            { "account": "10.01", "debit": 3000, "credit": 0 },
            { "account": "90.01", "debit": 0, "credit": 3000 },
        ],
    }))
    .await;

    let sheet = cmd(&env, "plugin.accounting.balance.sheet", json!({})).await;

    // Актив: деньги в кассе; пассив: уставный капитал; выручка — не в балансе.
    assert_eq!(sheet["assets"]["rows"].as_array().unwrap().len(), 1);
    let a = &sheet["assets"]["rows"][0];
    assert_eq!(a["account"], "10.01");
    assert_eq!(a["amount"].as_i64(), Some(8000));
    assert_eq!(sheet["assets"]["total"].as_i64(), Some(8000));

    assert_eq!(sheet["liabilities"]["rows"].as_array().unwrap().len(), 0);
    assert_eq!(sheet["liabilities"]["total"].as_i64(), Some(0));

    assert_eq!(sheet["equity"]["rows"].as_array().unwrap().len(), 1);
    let e = &sheet["equity"]["rows"][0];
    assert_eq!(e["account"], "80.01");
    assert_eq!(e["amount"].as_i64(), Some(5000));
    assert_eq!(sheet["equity"]["total"].as_i64(), Some(5000));
}