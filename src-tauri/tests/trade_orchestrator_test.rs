//! E2E демо п.11 ТЗ: торговля реальным wasm на живой MongoDB.
//!
//! Запуск: TX_TEST_MONGO=1 cargo test --test trade_orchestrator_test

use std::sync::{Arc, RwLock};

use app_lib::core::{CompanyId, UserId};
use app_lib::db::MongoClient;
use app_lib::events::ActorSnapshot;
use app_lib::permission_policy::PermissionPolicy;
use app_lib::plugin_manager as pm;
use app_lib::plugin_manager::workflow as wf;
use app_lib::plugin_manager::{HostData, PluginContext};
use app_lib::tx::executor;
use app_lib::tx::{TransactionPackage, TxContext, TxOperation};
use extism::{Manifest, PluginBuilder, UserData, Wasm, PTR};
use mongodb::bson::{doc, Document};

const WASM_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../wasm-modules/trade/target/wasm32-unknown-unknown/release/trade_plugin.wasm"
);

fn mongo_enabled() -> bool {
    std::env::var("TX_TEST_MONGO").map(|v| v == "1").unwrap_or(false)
}

async fn connect() -> (MongoClient, String) {
    let uri = std::env::var("MONGODB_URI")
        .unwrap_or_else(|_| "mongodb://localhost:27017".into());
    let db_name = format!(
        "trade_e2e_{}_{}",
        chrono::Utc::now().timestamp_millis(),
        &uuid::Uuid::new_v4().simple().to_string()[..6],
    );
    let client = MongoClient::connect(&uri, &db_name).await.expect("mongo");
    app_lib::tx::indexes::ensure_indexes(&client).await;
    app_lib::stock::indexes::ensure_indexes(&client).await;
    app_lib::ledger::indexes::ensure_indexes(&client).await;
    (client, db_name)
}

/// Очистка тестовой БД: drop коллекций по одной.
/// dropDatabase запрещён пользователю Atlas с ролью readWrite (AtlasError 8000).
async fn drop_collections(client: &mongodb::Client, db_name: &str) {
    let db = client.database(db_name);
    let names = match db.list_collection_names().await {
        Ok(n) => n,
        Err(_) => return,
    };
    for name in names {
        let _ = db.collection::<Document>(&name).drop().await;
    }
}


fn policies() -> Vec<PermissionPolicy> {
    let mk = |subsystem: &str| PermissionPolicy {
        _id: uuid::Uuid::new_v4(),
        code: format!("t.{subsystem}"),
        name: subsystem.into(),
        description: None,
        scope_type: "subsystem".into(),
        subsystem_code: subsystem.into(),
        entity_type: None,
        actions: vec!["*".into()],
        record_scope: "company".into(),
        deny: false,
        priority: 100,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    vec![mk("documents"), mk("stock"), mk("accounting"), mk("trade")]
}

fn ctx(company: uuid::Uuid) -> TxContext {
    TxContext {
        company_id: CompanyId(company),
        actor: ActorSnapshot {
            user_id: UserId(uuid::Uuid::nil()),
            login: "trade-e2e".into(),
            full_name: None,
            position: None,
            company_id: CompanyId(company),
        },
        policies: policies(),
    }
}

/// Создать объект-документ в objects.
async fn seed_doc(db: &MongoClient, company: uuid::Uuid, et_id: &str, id: &str, data: serde_json::Value) {
    db.collection::<Document>("objects").insert_one(doc! {
        "_id": id, "entity_type_id": et_id, "kind": "document",
        "company_id": company.to_string(), "state": "draft",
        "data": mongodb::bson::to_bson(&data).unwrap(), "version": 1i64,
        "created_by": uuid::Uuid::nil().to_string(),
        "updated_by": uuid::Uuid::nil().to_string(),
        "created_at": mongodb::bson::DateTime::now(),
        "updated_at": mongodb::bson::DateTime::now(),
    }).await.expect("seed doc");
}

/// Зарегистрировать entity_type с кодом → UUID.
async fn seed_entity_type(db: &MongoClient, code: &str) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    db.collection::<Document>("entity_types").insert_one(doc! {
        "_id": &id, "code": code, "name": code, "kind": "document",
    }).await.expect("entity type");
    id
}

struct Orch {
    plugin: extism::Plugin,
}

impl Orch {
    fn new(db: &MongoClient, company: uuid::Uuid, role: uuid::Uuid, user: uuid::Uuid) -> Self {
        let wasm = std::fs::read(WASM_PATH).expect("соберите wasm-modules/trade");
        let ctx = Arc::new(RwLock::new(PluginContext {
            company_id: Some(company.to_string()),
            user_id: Some(user.to_string()),
            user_login: Some("trade-e2e".into()),
            display_name: None,
            role_id: Some(role.to_string()),
            role_ids: vec![role.to_string()],
        }));
        let host = HostData {
            db: Some(db.clone()),
            ctx,
            module_code: Some("trade".into()),
            capabilities: vec![
                "transactions".into(), "objects.read".into(), "metadata.read".into(), "logging".into(),
            ],
        };
        let manifest = Manifest::new([Wasm::data(wasm)]);
        let plugin = PluginBuilder::new(&manifest)
            .with_function("get_object",      [PTR], [PTR], UserData::new(host.clone()), pm::get_object_impl)
            .with_function("get_entity_type", [PTR], [PTR], UserData::new(host.clone()), pm::get_entity_type_impl)
            .with_function("module_settings", [],    [PTR], UserData::new(host.clone()), wf::module_settings_impl)
            .with_function("tx_begin",        [PTR], [PTR], UserData::new(host.clone()), wf::tx_begin_impl)
            .with_function("tx_add_op",       [PTR, PTR, PTR], [PTR], UserData::new(host.clone()), wf::tx_add_op_impl)
            .with_function("tx_commit",       [PTR], [PTR], UserData::new(host.clone()), wf::tx_commit_impl)
            .with_function("emit_event",      [PTR, PTR, PTR], [PTR], UserData::new(host.clone()), wf::emit_event_impl)
            .with_function("notify_user", [PTR, PTR, PTR], [PTR], UserData::new(host.clone()), wf::notify_user_impl)
            .with_function("log_message",     [PTR], [], UserData::new(host.clone()), pm::log_message_impl)
            .with_function("stock_doc_cost",  [PTR], [PTR], UserData::new(host.clone()), wf::stock_doc_cost_impl)
            .with_fuel_limit(50_000_000)
            .build()
            .expect("плагин загружается");
        Self { plugin }
    }

    fn call(&mut self, function: &str, args: serde_json::Value) -> Result<serde_json::Value, String> {
        match self.plugin.call::<&[u8], String>(function, args.to_string().as_bytes()) {
            Ok(out) => Ok(serde_json::from_str(&out).unwrap_or(serde_json::Value::Null)),
            Err(e) => Err(e.to_string()),
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn demo_scenario_full_cycle() {
    if !mongo_enabled() { eprintln!("SKIP: TX_TEST_MONGO=1"); return; }
    let (db, db_name) = connect().await;
    let company = uuid::Uuid::new_v4();
    let role = uuid::Uuid::new_v4();

    // Seed роль
    let mk = |subsystem: &str| PermissionPolicy {
        _id: uuid::Uuid::new_v4(),
        code: format!("t.{subsystem}"),
        name: subsystem.into(),
        description: None,
        scope_type: "subsystem".into(),
        subsystem_code: subsystem.into(),
        entity_type: None,
        actions: vec!["*".into()],
        record_scope: "company".into(),
        deny: false,
        priority: 100,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    let p_stock = mk("stock");
    let p_docs = mk("documents");
    let p_acc = mk("accounting");
    let col_p = db.collection::<Document>("permission_policies");
    col_p.insert_one(mongodb::bson::to_document(&p_stock).unwrap()).await.ok();
    col_p.insert_one(mongodb::bson::to_document(&p_docs).unwrap()).await.ok();
    col_p.insert_one(mongodb::bson::to_document(&p_acc).unwrap()).await.ok();
    // Seed план счетов
    app_lib::ledger::service::LedgerService::ensure_default_chart(&db, &app_lib::core::CompanyId(company)).await;

    db.collection::<Document>("roles").insert_one(doc! {
        "_id": role.to_string(), "company_id": company.to_string(),
        "code": "TRADE_TEST", "name": "Test",
        "permission_policy_ids": [p_stock._id.to_string(), p_docs._id.to_string(), p_acc._id.to_string()],
        "created_at": chrono::Utc::now().to_rfc3339(),
        "updated_at": chrono::Utc::now().to_rfc3339(),
    }).await.expect("role");

    // Seed entity types для документов торговли
    let pur_et = seed_entity_type(&db, "PURCHASE").await;
    let sal_et = seed_entity_type(&db, "SALES").await;

    // UUID для документов
    let pur1_id = uuid::Uuid::new_v4().to_string();
    let pur2_id = uuid::Uuid::new_v4().to_string();
    let salfail_id = uuid::Uuid::new_v4().to_string();
    let salok_id = uuid::Uuid::new_v4().to_string();

    // Seed номенклатуры и склада
    let wh = format!("loc-wh-{company}");
    db.collection::<Document>("objects").insert_many(vec![
        doc! { "_id": "nom-t1", "entity_type_id": "NOM", "kind": "catalog",
               "company_id": company.to_string(), "state": "active",
               "data": { "type": "item" }, "version": 1i64 },
        doc! { "_id": &wh, "entity_type_id": "LOC", "kind": "catalog",
               "company_id": company.to_string(), "state": "active",
               "data": { "type": "warehouse" }, "version": 1i64 },
    ]).await.expect("fixtures");

    let mut orch = Orch::new(&db, company, role, uuid::Uuid::nil());

    // ── Шаг 2: Поступление 10 @ 100₽ ──
    seed_doc(&db, company, &pur_et, &pur1_id, serde_json::json!({
        "warehouse_id": wh, "supplier_id": "sup-1",
        "lines": [{"nomenclature_id": "nom-t1", "qty": 10, "price": 100.0}],
        "total": 1000,
    })).await;

    let out = orch.call("on_post", serde_json::json!({"id": pur1_id}))
        .expect("поступление on_post");
    assert_eq!(out["posted"], true);

    // Проверка остатков через executor
    {
        use app_lib::stock::engine::EngineCtx;
        let mut sess = db.client().start_session().await.unwrap();
        let mut ectx = EngineCtx {
            db: &db, session: &mut sess,
            company_id: CompanyId(company), actor: ActorSnapshot {
                user_id: UserId(uuid::Uuid::nil()), login: "v".into(),
                full_name: None, position: None, company_id: CompanyId(company),
            },
        };
        let bal = app_lib::stock::engine::balances(&mut ectx, Some(&wh), Some("nom-t1")).await.unwrap();
        let qty = bal["balances"][0]["quantity"].as_f64().unwrap();
        assert_eq!(qty, 10.0, "остаток после прихода");
    }

    // Проводка закупки существует
    let postings = db.collection::<Document>(app_lib::ledger::COL_ENTRIES)
        .count_documents(doc! { "doc_id": &pur1_id }).await.unwrap();
    assert!(postings >= 1, "закупочная проводка Дт41 Кт60 должна быть");

    // ── Шаг 3: Реализация 12 — недостаточно ──
    seed_doc(&db, company, &sal_et, &salfail_id, serde_json::json!({
        "warehouse_id": wh, "customer_id": "cust-1",
        "lines": [{"nomenclature_id": "nom-t1", "qty": 12, "price": 200.0}],
        "total": 2400,
    })).await;
    let err = orch.call("on_post", serde_json::json!({"id": salfail_id}))
        .expect_err("недостаточно");
    assert!(err.contains("Недостаточно") || err.contains("недостаточно"), "{err}");

    // ── Шаг 4: Поступление ещё 5 @ 120₽ ──
    seed_doc(&db, company, &pur_et, &pur2_id, serde_json::json!({
        "warehouse_id": wh, "supplier_id": "sup-1",
        "lines": [{"nomenclature_id": "nom-t1", "qty": 5, "price": 120.0}],
        "total": 600,
    })).await;
    orch.call("on_post", serde_json::json!({"id": pur2_id})).expect("приход 2");

    // ── Шаг 5: Реализация 12 → себестоимость FIFO 1240₽ ──
    seed_doc(&db, company, &sal_et, &salok_id, serde_json::json!({
        "warehouse_id": wh, "customer_id": "cust-1",
        "lines": [{"nomenclature_id": "nom-t1", "qty": 12, "price": 200.0}],
        "total": 2400,
    })).await;
    let out = orch.call("on_post", serde_json::json!({"id": salok_id}))
        .expect("реализация");
    assert_eq!(out["posted"], true);
    // COGS проводка строится из результата stock.issue через $ref
    // ({op}.total_cost) — себестоимость живёт только в движениях склада.
    // Остаток = 15 - 12 = 3
    {
        use app_lib::stock::engine::EngineCtx;
        let mut sess = db.client().start_session().await.unwrap();
        let mut ectx = EngineCtx {
            db: &db, session: &mut sess,
            company_id: CompanyId(company), actor: ActorSnapshot {
                user_id: UserId(uuid::Uuid::nil()), login: "v".into(),
                full_name: None, position: None, company_id: CompanyId(company),
            },
        };
        let bal = app_lib::stock::engine::balances(&mut ectx, Some(&wh), Some("nom-t1")).await.unwrap();
        assert_eq!(bal["balances"][0]["quantity"].as_f64().unwrap(), 3.0);
    }

    // ── Шаг 7: Отмена реализации (сторно) ──
    let out = orch.call("on_cancel", serde_json::json!({"id": salok_id}));
    assert!(out.is_ok(), "отмена реализации: {out:?}");

    // Остаток вернулся: 3 + 12 = 15
    {
        use app_lib::stock::engine::EngineCtx;
        let mut sess = db.client().start_session().await.unwrap();
        let mut ectx = EngineCtx {
            db: &db, session: &mut sess,
            company_id: CompanyId(company), actor: ActorSnapshot {
                user_id: UserId(uuid::Uuid::nil()), login: "v".into(),
                full_name: None, position: None, company_id: CompanyId(company),
            },
        };
        let bal = app_lib::stock::engine::balances(&mut ectx, Some(&wh), Some("nom-t1")).await.unwrap();
        assert_eq!(bal["balances"][0]["quantity"].as_f64().unwrap(), 15.0, "сторно вернул остаток");
    }

    drop_collections(&db.client().clone(), &db_name).await;
}

// ════════════════════════════════════════════════════════════
// Себестоимость как проекция движений (line_ref → doc_cost)
// ════════════════════════════════════════════════════════════

use app_lib::stock::engine as stock_engine;

/// Реализация двумя строками из разных партий: себестоимость каждой строки
/// читается из движений по (doc_id, line_ref).
#[tokio::test(flavor = "multi_thread")]
async fn sales_two_lines_cost_per_line() {
    if !mongo_enabled() { eprintln!("SKIP: TX_TEST_MONGO=1"); return; }
    let (db, db_name) = connect().await;
    let company = uuid::Uuid::new_v4();
    let role = uuid::Uuid::new_v4();

    let mk = |subsystem: &str| PermissionPolicy {
        _id: uuid::Uuid::new_v4(),
        code: format!("t.{subsystem}"), name: subsystem.into(), description: None,
        scope_type: "subsystem".into(), subsystem_code: subsystem.into(),
        entity_type: None, actions: vec!["*".into()], record_scope: "company".into(),
        deny: false, priority: 100,
        created_at: chrono::Utc::now(), updated_at: chrono::Utc::now(),
    };
    let p_stock = mk("stock"); let p_docs = mk("documents"); let p_acc = mk("accounting");
    let col_p = db.collection::<Document>("permission_policies");
    col_p.insert_one(mongodb::bson::to_document(&p_stock).unwrap()).await.ok();
    col_p.insert_one(mongodb::bson::to_document(&p_docs).unwrap()).await.ok();
    col_p.insert_one(mongodb::bson::to_document(&p_acc).unwrap()).await.ok();
    // Роль с привязкой политик — tx_begin резолвит права через role_ids
    db.collection::<Document>("roles").insert_one(doc! {
        "_id": role.to_string(), "company_id": company.to_string(),
        "code": "TRADE_TEST", "name": "Test",
        "permission_policy_ids": [p_stock._id.to_string(), p_docs._id.to_string(), p_acc._id.to_string()],
        "created_at": chrono::Utc::now().to_rfc3339(),
        "updated_at": chrono::Utc::now().to_rfc3339(),
    }).await.expect("role");
    // План счетов по умолчанию (41/60/90.x...) для accounting.post
    app_lib::ledger::service::LedgerService::ensure_default_chart(&db, &CompanyId(company)).await;

    let mut orch = Orch::new(&db, company, role, uuid::Uuid::nil());

    let wh = format!("loc-wh-{company}");
    db.collection::<Document>("objects").insert_many(vec![
        doc! { "_id": "nom-a", "entity_type_id": "NOM", "kind": "catalog",
               "company_id": company.to_string(), "state": "active",
               "data": { "type": "item" }, "version": 1i64 },
        doc! { "_id": "nom-b", "entity_type_id": "NOM", "kind": "catalog",
               "company_id": company.to_string(), "state": "active",
               "data": { "type": "item" }, "version": 1i64 },
        doc! { "_id": &wh, "entity_type_id": "LOC", "kind": "catalog",
               "company_id": company.to_string(), "state": "active",
               "data": { "type": "warehouse" }, "version": 1i64 },
    ]).await.expect("fixtures");

    // Партия A: 2 шт @ 100₽ (unit_cost 10000 коп)
    let pur_a = uuid::Uuid::new_v4().to_string();
    seed_doc(&db, company, "PURCHASE_ET", &pur_a, serde_json::json!({
        "warehouse_id": wh, "lines": [{"nomenclature_id": "nom-a", "qty": 2, "price": 100.0}], "total": 200,
    })).await;
    // Нужен entity_type PURCHASE — засеиваем и пересеедим документ
    let pur_et = seed_entity_type(&db, "PURCHASE").await;
    let sal_et = seed_entity_type(&db, "SALES").await;
    let ret_et = seed_entity_type(&db, "CUSTOMER_RETURN").await;
    db.collection::<Document>("objects").update_one(
        doc! { "_id": &pur_a },
        doc! { "$set": { "entity_type_id": &pur_et } },
    ).await.unwrap();

    orch.call("on_post", serde_json::json!({"id": pur_a})).expect("приход A");

    // Партия B: 3 шт @ 200₽ (unit_cost 20000 коп)
    let pur_b = uuid::Uuid::new_v4().to_string();
    seed_doc(&db, company, &pur_et, &pur_b, serde_json::json!({
        "warehouse_id": wh, "lines": [{"nomenclature_id": "nom-b", "qty": 3, "price": 200.0}], "total": 600,
    })).await;
    orch.call("on_post", serde_json::json!({"id": pur_b})).expect("приход B");

    // Реализация: строка L1 = 2×nom-a, строка L2 = 3×nom-b
    let sales_id = uuid::Uuid::new_v4().to_string();
    seed_doc(&db, company, &sal_et, &sales_id, serde_json::json!({
        "warehouse_id": wh,
        "lines": [
            {"line_id": "L1", "nomenclature_id": "nom-a", "qty": 2, "price": 150.0},
            {"line_id": "L2", "nomenclature_id": "nom-b", "qty": 3, "price": 300.0},
        ],
        "total": 1200,
    })).await;
    let out = orch.call("on_post", serde_json::json!({"id": sales_id})).expect("реализация");
    assert_eq!(out["posted"], true);

    // Себестоимость по строкам из движений
    let cost = stock_engine::doc_cost(&db, &CompanyId(company), &sales_id).await.unwrap();
    let lines = cost["lines"].as_array().unwrap();
    assert_eq!(lines.len(), 2, "две строки: {cost}");
    let l1 = lines.iter().find(|l| l["line_ref"] == "L1").unwrap();
    let l2 = lines.iter().find(|l| l["line_ref"] == "L2").unwrap();
    assert_eq!(l1["nomenclature_id"], "nom-a");
    assert_eq!(l1["qty"].as_f64().unwrap(), 2.0);
    assert_eq!(l1["cost"].as_i64().unwrap(), 20_000, "строка L1: 2×10000коп");
    assert_eq!(l2["cost"].as_i64().unwrap(), 60_000, "строка L2: 3×20000коп");
    assert_eq!(cost["total_cost"].as_i64().unwrap(), 80_000);

    // COGS проводка через $ref появилась
    let cogs = db.collection::<Document>(app_lib::ledger::COL_ENTRIES)
        .count_documents(doc! { "doc_id": &sales_id, "debit_code": "90.2" }).await.unwrap();
    assert_eq!(cogs, 1, "COGS Дт90.2 Кт41 от $ref total_cost");

    drop_collections(&db.client().clone(), &db_name).await;
}

/// Частичный возврат от покупателя: себестоимость возвращаемого количества =
/// средняя по списаниям строки исходной реализации. Одна цифра на склад
/// (партия возврата) и бухгалтерию (Дт41 Кт90.2).
#[tokio::test(flavor = "multi_thread")]
async fn partial_customer_return_avg_line_cost() {
    if !mongo_enabled() { eprintln!("SKIP: TX_TEST_MONGO=1"); return; }
    let (db, db_name) = connect().await;
    let company = uuid::Uuid::new_v4();
    let role = uuid::Uuid::new_v4();

    let mk = |subsystem: &str| PermissionPolicy {
        _id: uuid::Uuid::new_v4(),
        code: format!("t.{subsystem}"), name: subsystem.into(), description: None,
        scope_type: "subsystem".into(), subsystem_code: subsystem.into(),
        entity_type: None, actions: vec!["*".into()], record_scope: "company".into(),
        deny: false, priority: 100,
        created_at: chrono::Utc::now(), updated_at: chrono::Utc::now(),
    };
    let p_stock = mk("stock"); let p_docs = mk("documents"); let p_acc = mk("accounting");
    let col_p = db.collection::<Document>("permission_policies");
    col_p.insert_one(mongodb::bson::to_document(&p_stock).unwrap()).await.ok();
    col_p.insert_one(mongodb::bson::to_document(&p_docs).unwrap()).await.ok();
    col_p.insert_one(mongodb::bson::to_document(&p_acc).unwrap()).await.ok();
    // Роль с привязкой политик — tx_begin резолвит права через role_ids
    db.collection::<Document>("roles").insert_one(doc! {
        "_id": role.to_string(), "company_id": company.to_string(),
        "code": "TRADE_TEST", "name": "Test",
        "permission_policy_ids": [p_stock._id.to_string(), p_docs._id.to_string(), p_acc._id.to_string()],
        "created_at": chrono::Utc::now().to_rfc3339(),
        "updated_at": chrono::Utc::now().to_rfc3339(),
    }).await.expect("role");
    // План счетов по умолчанию (41/60/90.x...) для accounting.post
    app_lib::ledger::service::LedgerService::ensure_default_chart(&db, &CompanyId(company)).await;

    let mut orch = Orch::new(&db, company, role, uuid::Uuid::nil());

    let wh = format!("loc-wh-{company}");
    let pur_et = seed_entity_type(&db, "PURCHASE").await;
    let sal_et = seed_entity_type(&db, "SALES").await;
    let ret_et = seed_entity_type(&db, "CUSTOMER_RETURN").await;

    db.collection::<Document>("objects").insert_many(vec![
        doc! { "_id": "nom-r", "entity_type_id": "NOM", "kind": "catalog",
               "company_id": company.to_string(), "state": "active",
               "data": { "type": "item" }, "version": 1i64 },
        doc! { "_id": &wh, "entity_type_id": "LOC", "kind": "catalog",
               "company_id": company.to_string(), "state": "active",
               "data": { "type": "warehouse" }, "version": 1i64 },
    ]).await.expect("fixtures");

    // Приход 4 @ 250₽ (25000 коп/шт) и реализация 4 шт одной строкой L1
    let pur = uuid::Uuid::new_v4().to_string();
    seed_doc(&db, company, &pur_et, &pur, serde_json::json!({
        "warehouse_id": wh, "lines": [{"nomenclature_id": "nom-r", "qty": 4, "price": 250.0}], "total": 1000,
    })).await;
    orch.call("on_post", serde_json::json!({"id": pur})).expect("приход");

    let sales_id = uuid::Uuid::new_v4().to_string();
    seed_doc(&db, company, &sal_et, &sales_id, serde_json::json!({
        "warehouse_id": wh,
        "lines": [{"line_id": "L1", "nomenclature_id": "nom-r", "qty": 4, "price": 400.0}],
        "total": 1600,
    })).await;
    orch.call("on_post", serde_json::json!({"id": sales_id})).expect("реализация");

    // Частичный возврат 1 шт по source_sales_id
    let ret_id = uuid::Uuid::new_v4().to_string();
    seed_doc(&db, company, &ret_et, &ret_id, serde_json::json!({
        "warehouse_id": wh, "source_sales_id": sales_id,
        "lines": [{"line_id": "R1", "nomenclature_id": "nom-r", "qty": 1, "price": 400.0}],
        "total": -400,
    })).await;
    let out = orch.call("on_post", serde_json::json!({"id": ret_id}));
    assert!(out.is_ok(), "возврат: {out:?}");

    // Партия возврата: unit_cost = 25000/4×... = средняя строки (100000/4=25000)
    let batch = db.collection::<Document>(app_lib::stock::COL_BATCHES)
        .find_one(doc! { "source_doc_id": &ret_id }).await.unwrap()
        .expect("партия возврата");
    assert_eq!(batch.get_i64("unit_cost").unwrap(), 25_000, "средняя строки L1");

    // Бухгалтерия возврата: та же цифра в товарной части (Дт41 Кт90.2)
    let goods_entry = db.collection::<Document>(app_lib::ledger::COL_ENTRIES)
        .find_one(doc! { "doc_id": &ret_id, "debit_code": "41", "credit_code": "90.2" })
        .await.unwrap().expect("проводка Дт41 Кт90.2");
    assert_eq!(goods_entry.get_i64("amount").unwrap(), 25_000, "одна цифра склад/бухгалтерия");

    drop_collections(&db.client().clone(), &db_name).await;
}

/// Отмена реализации: движения помечены reversed → себестоимость исчезает.
#[tokio::test(flavor = "multi_thread")]
async fn cancel_sales_cost_dissapears() {
    if !mongo_enabled() { eprintln!("SKIP: TX_TEST_MONGO=1"); return; }
    let (db, db_name) = connect().await;
    let company = uuid::Uuid::new_v4();
    let role = uuid::Uuid::new_v4();

    let mk = |subsystem: &str| PermissionPolicy {
        _id: uuid::Uuid::new_v4(),
        code: format!("t.{subsystem}"), name: subsystem.into(), description: None,
        scope_type: "subsystem".into(), subsystem_code: subsystem.into(),
        entity_type: None, actions: vec!["*".into()], record_scope: "company".into(),
        deny: false, priority: 100,
        created_at: chrono::Utc::now(), updated_at: chrono::Utc::now(),
    };
    let p_stock = mk("stock"); let p_docs = mk("documents"); let p_acc = mk("accounting");
    let col_p = db.collection::<Document>("permission_policies");
    col_p.insert_one(mongodb::bson::to_document(&p_stock).unwrap()).await.ok();
    col_p.insert_one(mongodb::bson::to_document(&p_docs).unwrap()).await.ok();
    col_p.insert_one(mongodb::bson::to_document(&p_acc).unwrap()).await.ok();
    // Роль с привязкой политик — tx_begin резолвит права через role_ids
    db.collection::<Document>("roles").insert_one(doc! {
        "_id": role.to_string(), "company_id": company.to_string(),
        "code": "TRADE_TEST", "name": "Test",
        "permission_policy_ids": [p_stock._id.to_string(), p_docs._id.to_string(), p_acc._id.to_string()],
        "created_at": chrono::Utc::now().to_rfc3339(),
        "updated_at": chrono::Utc::now().to_rfc3339(),
    }).await.expect("role");
    // План счетов по умолчанию (41/60/90.x...) для accounting.post
    app_lib::ledger::service::LedgerService::ensure_default_chart(&db, &CompanyId(company)).await;

    let mut orch = Orch::new(&db, company, role, uuid::Uuid::nil());

    let wh = format!("loc-wh-{company}");
    let pur_et = seed_entity_type(&db, "PURCHASE").await;
    let sal_et = seed_entity_type(&db, "SALES").await;

    db.collection::<Document>("objects").insert_many(vec![
        doc! { "_id": "nom-c", "entity_type_id": "NOM", "kind": "catalog",
               "company_id": company.to_string(), "state": "active",
               "data": { "type": "item" }, "version": 1i64 },
        doc! { "_id": &wh, "entity_type_id": "LOC", "kind": "catalog",
               "company_id": company.to_string(), "state": "active",
               "data": { "type": "warehouse" }, "version": 1i64 },
    ]).await.expect("fixtures");

    let pur = uuid::Uuid::new_v4().to_string();
    seed_doc(&db, company, &pur_et, &pur, serde_json::json!({
        "warehouse_id": wh, "lines": [{"nomenclature_id": "nom-c", "qty": 5, "price": 100.0}], "total": 500,
    })).await;
    orch.call("on_post", serde_json::json!({"id": pur})).expect("приход");

    let sales_id = uuid::Uuid::new_v4().to_string();
    seed_doc(&db, company, &sal_et, &sales_id, serde_json::json!({
        "warehouse_id": wh,
        "lines": [{"line_id": "L9", "nomenclature_id": "nom-c", "qty": 5, "price": 200.0}],
        "total": 1000,
    })).await;
    orch.call("on_post", serde_json::json!({"id": sales_id})).expect("реализация");

    let before = stock_engine::doc_cost(&db, &CompanyId(company), &sales_id).await.unwrap();
    assert_eq!(before["total_cost"].as_i64().unwrap(), 50_000);

    orch.call("on_cancel", serde_json::json!({"id": sales_id}))
        .expect("отмена реализации");

    let after = stock_engine::doc_cost(&db, &CompanyId(company), &sales_id).await.unwrap();
    assert_eq!(after["total_cost"].as_i64().unwrap(), 0, "проекция исчезла");
    assert_eq!(after["lines"].as_array().unwrap().len(), 0);

    drop_collections(&db.client().clone(), &db_name).await;
}
