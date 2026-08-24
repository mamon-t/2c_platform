//! E2E оркестратора склада: настоящий stock_plugin.wasm проводит
//! документ через tx-сессию против живой MongoDB.
//!
//! Запуск: TX_TEST_MONGO=1 cargo test --test stock_orchestrator_test

use std::sync::{Arc, RwLock};

use app_lib::core::{CompanyId, UserId};
use app_lib::db::MongoClient;
use app_lib::events::ActorSnapshot;
use app_lib::permission_policy::PermissionPolicy;
use app_lib::plugin_manager as pm;
use app_lib::plugin_manager::workflow as wf;
use app_lib::plugin_manager::{HostData, PluginContext};
use extism::{Manifest, PluginBuilder, UserData, Wasm, PTR};

const WASM_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../wasm-modules/stock/target/wasm32-unknown-unknown/release/stock_plugin.wasm"
);

fn mongo_enabled() -> bool {
    std::env::var("TX_TEST_MONGO").map(|v| v == "1").unwrap_or(false)
}

async fn connect() -> (MongoClient, String) {
    let uri = std::env::var("MONGODB_URI")
        .unwrap_or_else(|_| "mongodb://localhost:27017".into());
    let db_name = format!(
        "stock_orch_{}_{}",
        chrono::Utc::now().timestamp_millis(),
        &uuid::Uuid::new_v4().simple().to_string()[..6],
    );
    let client = MongoClient::connect(&uri, &db_name).await.expect("mongo");
    app_lib::tx::indexes::ensure_indexes(&client).await;
    app_lib::stock::indexes::ensure_indexes(&client).await;
    (client, db_name)
}

fn seed_policies() -> Vec<PermissionPolicy> {
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
    vec![mk("documents"), mk("stock"), mk("test")]
}

/// Роль со звёздочными политиками stock/documents.
async fn seed_role(db: &MongoClient, company: uuid::Uuid) -> uuid::Uuid {
    let mk = |subsystem: &str| PermissionPolicy {
        _id: uuid::Uuid::new_v4(),
        code: format!("r.{subsystem}"),
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
    let col = db.collection::<mongodb::bson::Document>("permission_policies");
    for p in [&p_stock, &p_docs] {
        col.insert_one(mongodb::bson::to_document(p).unwrap())
            .await
            .expect("policy");
    }
    let role_id = uuid::Uuid::new_v4();
    db.collection::<mongodb::bson::Document>("roles")
        .insert_one(doc! {
            "_id": role_id.to_string(),
            "company_id": company.to_string(),
            "code": "STOCK_TEST",
            "name": "Тестовая роль",
            "permission_policy_ids": [p_stock._id.to_string(), p_docs._id.to_string()],
            "created_at": chrono::Utc::now().to_rfc3339(),
            "updated_at": chrono::Utc::now().to_rfc3339(),
        })
        .await
        .expect("role");
    role_id
}

use mongodb::bson::{doc, Document};

/// Фикстуры: товар, два места учёта и документ MOVE в objects.
async fn seed_fixtures(db: &MongoClient, company: uuid::Uuid) -> (String, String, String, String) {
    let col = db.collection::<mongodb::bson::Document>("objects");
    let put = |id: String, et: &str, kind: &str, state: &str, data: serde_json::Value| {
        doc! {
            "_id": id,
            "entity_type_id": et,
            "kind": kind,
            "company_id": company.to_string(),
            "state": state,
            "data": mongodb::bson::to_bson(&data).unwrap(),
            "version": 1i64,
            "created_by": uuid::Uuid::nil().to_string(),
            "updated_by": uuid::Uuid::nil().to_string(),
            "created_at": chrono::Utc::now().to_rfc3339(),
            "updated_at": chrono::Utc::now().to_rfc3339(),
        }
    };
    col.insert_one(put("nom-x".into(), "NOMENCLATURE", "catalog", "active",
        serde_json::json!({"type":"item","unit":"шт"}))).await.expect("nom");
    col.insert_one(put(format!("loc-wh-{company}"), "STOCK_LOCATION", "catalog", "active",
        serde_json::json!({"type":"warehouse"}))).await.expect("wh");
    col.insert_one(put(format!("loc-b-{company}"), "STOCK_LOCATION", "catalog", "active",
        serde_json::json!({"type":"warehouse"}))).await.expect("b");

    let doc_id = uuid::Uuid::new_v4().to_string();
    col.insert_one(put(doc_id.clone(), "MOVE_ID", "document", "draft",
        serde_json::json!({
            "from_location_id": format!("loc-wh-{company}"),
            "to_location_id": format!("loc-b-{company}"),
            "lines": [{"nomenclature_id": "nom-x", "qty": 5}],
        })))
        .await
        .expect("документ");
    // Тип сущности MOVE с настоящим UUID (плагин резолвит код по id)
    let move_type_id = uuid::Uuid::new_v4().to_string();
    db.collection::<mongodb::bson::Document>("entity_types")
        .insert_one(doc! {
            "_id": &move_type_id,
            "code": "MOVE",
            "name": "Перемещение",
            "kind": "document",
        })
        .await
        .expect("entity type");
    db.collection::<mongodb::bson::Document>("objects")
        .update_one(doc! { "_id": &doc_id }, doc! { "$set": { "entity_type_id": &move_type_id } })
        .await
        .expect("fix et");

    (doc_id, "nom-x".into(), format!("loc-wh-{company}"), format!("loc-b-{company}"))
}

struct Orch {
    plugin: extism::Plugin,
}

impl Orch {
    fn new(db: &MongoClient, company: uuid::Uuid, role: uuid::Uuid, user: uuid::Uuid) -> Self {
        let wasm = std::fs::read(WASM_PATH).expect("соберите wasm-modules/stock");
        let ctx = Arc::new(RwLock::new(PluginContext {
            company_id: Some(company.to_string()),
            user_id: Some(user.to_string()),
            user_login: Some("orch-test".into()),
            display_name: None,
            role_id: Some(role.to_string()),
            role_ids: vec![role.to_string()],
        }));
        let host = HostData {
            db: Some(db.clone()),
            ctx,
            module_code: Some("stock".into()),
            capabilities: vec![
                "transactions".into(),
                "objects.read".into(),
                "metadata.read".into(),
                "signature".into(),
                "logging".into(),
            ],
        };
        let manifest = Manifest::new([Wasm::data(wasm)]);
        let plugin = PluginBuilder::new(&manifest)
            .with_function("get_object",   [PTR], [PTR], UserData::new(host.clone()), pm::get_object_impl)
            .with_function("get_entity_type", [PTR], [PTR], UserData::new(host.clone()), pm::get_entity_type_impl)
            .with_function("tx_begin",     [PTR], [PTR], UserData::new(host.clone()), wf::tx_begin_impl)
            .with_function("tx_add_op",    [PTR, PTR, PTR], [PTR], UserData::new(host.clone()), wf::tx_add_op_impl)
            .with_function("tx_commit",    [PTR], [PTR], UserData::new(host.clone()), wf::tx_commit_impl)
            .with_function("signature_required", [PTR, PTR, PTR], [PTR], UserData::new(host.clone()), wf::signature_required_impl)
            .with_function("log_message",  [PTR], [], UserData::new(host.clone()), pm::log_message_impl)
            .with_fuel_limit(50_000_000)
            .build()
            .expect("плагин загружается");
        Self { plugin }
    }

    fn call(&mut self, function: &str, args: serde_json::Value) -> serde_json::Value {
        let out = self
            .plugin
            .call::<&[u8], String>(function, args.to_string().as_bytes())
            .unwrap_or_else(|e| panic!("{function}: {e}"));
        serde_json::from_str(&out).expect("конверт")
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn plugin_post_and_cancel_move_atomically() {
    if !mongo_enabled() { eprintln!("SKIP: TX_TEST_MONGO=1"); return; }
    let (db, db_name) = connect().await;
    let company = uuid::Uuid::new_v4();
    let role = seed_role(&db, company).await;
    let user = uuid::Uuid::new_v4();
    let (doc_id, nom, wh, wh_b) = seed_fixtures(&db, company).await;

    // Приход 5 шт на основной склад напрямую через executor (как будто была приёмка)
    use app_lib::tx::{TransactionPackage, TxOperation, TxContext};
    app_lib::tx::executor::execute(&db, TransactionPackage {
        idempotency_key: "seed-receipt".into(),
        required_permission: None,
        operations: vec![TxOperation {
            op_id: "r".into(),
            op: "stock.receipt".into(),
            params: serde_json::json!({
                "location_id": wh,
                "lines": [{"nomenclature_id": nom, "qty": 5, "unit_cost": 20000}],
            }),
        }],
        context: TxContext { company_id: CompanyId(company), actor: ActorSnapshot {
            user_id: UserId(uuid::Uuid::nil()), login: "t".into(), full_name: None,
            position: None, company_id: CompanyId(company),
        }, policies: seed_policies() },
        created_at: chrono::Utc::now(),
        expires_at: None,
    }).await.expect("приход");

    // ── Плагин проводит документ ──
    let mut orch = Orch::new(&db, company, role, user);
    // Гость возвращает данные НАПРЯМУЮ (конверт — только host→guest)
    let out = orch.call("on_post", serde_json::json!({"id": doc_id}));
    assert_eq!(out["posted"], true, "on_post: {out}");

    // Документ проведён, остатки переехали
    let doc = db.collection::<Document>("objects")
        .find_one(doc! { "_id": &doc_id }).await.unwrap().expect("doc");
    assert_eq!(doc.get_str("state").unwrap(), "posted");
    assert!(doc.get_i64("version").unwrap() >= 2);

    let bal = |loc: String| {
        let db = db.clone();
        let nom = nom.to_string();
        async move {
            db.collection::<Document>(app_lib::stock::COL_BALANCES)
                .find_one(doc! { "location_id": loc, "nomenclature_id": nom })
                .await.unwrap().map(|d| d.get_f64("quantity").unwrap_or(0.0)).unwrap_or(0.0)
        }
    };
    assert_eq!(bal(wh.clone()).await, 0.0);
    assert_eq!(bal(wh_b.clone()).await, 5.0);

    // Партия на приёмнике сохранила цену прихода
    let batch = db.collection::<Document>(app_lib::stock::COL_BATCHES)
        .find_one(doc! { "location_id": &wh_b }).await.unwrap().expect("batch");
    assert_eq!(batch.get_i64("unit_cost").unwrap(), 20000);

    // ── Плагин отменяет документ (сторно) ──
    let ver = doc.get_i64("version").unwrap();
    let out = orch.call("on_cancel", serde_json::json!({"id": doc_id, "expected_version": ver}));
    assert_eq!(out["cancelled"], true, "on_cancel: {out}");

    let doc = db.collection::<Document>("objects")
        .find_one(doc! { "_id": &doc_id }).await.unwrap().expect("doc");
    assert_eq!(doc.get_str("state").unwrap(), "cancelled");

    // Остатки вернулись на исходный склад, с приёмника ушли
    assert_eq!(bal(wh.clone()).await, 5.0);
    assert_eq!(bal(wh_b.clone()).await, 0.0);

    db.client().database(&db_name).drop().await.expect("cleanup");
}

// ── Политики подписи: оборудование да, канцтовары нет ──────

#[tokio::test(flavor = "multi_thread")]
async fn signature_policy_by_category() {
    if !mongo_enabled() { eprintln!("SKIP: TX_TEST_MONGO=1"); return; }
    let (db, db_name) = connect().await;
    let company = uuid::Uuid::new_v4();
    let role = seed_role(&db, company).await;
    let _user = uuid::Uuid::new_v4();

    // Номенклатура: монитор (equipment) и степлер (канцтовары)
    let col = db.collection::<Document>("objects");
    for (id, cat) in [("nom-monitor", "equipment"), ("nom-stapler", "stationery")] {
        col.insert_one(doc! {
            "_id": id, "entity_type_id": "NOM", "kind": "catalog",
            "company_id": company.to_string(), "state": "active",
            "data": { "type": "item", "category": cat },
            "version": 1i64,
        }).await.expect("номенклатура");
    }
    // Локации
    for id in ["loc-wh", "loc-iv"] {
        col.insert_one(doc! {
            "_id": id, "entity_type_id": "NOM", "kind": "catalog",
            "company_id": company.to_string(), "state": "active",
            "data": { "type": if id.contains("iv") { "custodian" } else { "warehouse" } },
            "version": 1i64,
        }).await.expect("локация");
    }

    // Политика: выдача оборудования — подпись обязательна
    db.collection::<Document>(app_lib::stock::signature::COLLECTION)
        .insert_one(doc! {
            "_id": uuid::Uuid::new_v4().to_string(),
            "company_id": company.to_string(),
            "module": "stock",
            "action": "handover.post",
            "name": "Оборудование под подпись",
            "condition": { "nomenclature_category": "equipment" },
            "required": true,
        }).await.expect("политика");

    let mk_handover = |doc_id: &str, nom: &str| {
        serde_json::json!({ "id": doc_id })
    };
    async fn seed_doc(db: &MongoClient, doc_id: &str, data: serde_json::Value) -> String {
        let et_id = uuid::Uuid::new_v4().to_string();
        let col = db.collection::<Document>("objects");
        db.collection::<Document>("entity_types").insert_one(doc! {
            "_id": &et_id, "code": "MOVE_X", "name": "X", "kind": "document",
        }).await.ok();
        col.insert_one(doc! {
            "_id": doc_id, "entity_type_id": &et_id, "kind": "document",
            "company_id": "x", "state": "draft", "data":
                mongodb::bson::to_bson(&data).unwrap(), "version": 1i64,
        }).await.expect("документ");
        // company_id поправим реальным ниже через параметр? упрощаем: тест использует один company
        doc_id.to_string()
    }
    let _ = mk_handover;

    // Оценка напрямую через сервис (без wasm): оборудование → required
    let eval = |data: serde_json::Value| {
        let db = db.clone();
        async move {
            app_lib::stock::signature::SignatureService::evaluate(
                &db, &CompanyId(company), "stock", "handover.post", &data,
            ).await.unwrap()
        }
    };
    assert!(eval(serde_json::json!({"lines":[{"nomenclature_id":"nom-monitor"}]})).await);
    assert!(!eval(serde_json::json!({"lines":[{"nomenclature_id":"nom-stapler"}]})).await);
    // Смешанная строка: есть оборудование → требуется
    assert!(
        eval(serde_json::json!({"lines":[
            {"nomenclature_id":"nom-stapler"},{"nomenclature_id":"nom-monitor"}
        ]})).await
    );
    // Нет строк → условие не выполнено
    assert!(!eval(serde_json::json!({"lines":[]})).await);

    db.client().database(&db_name).drop().await.expect("cleanup");
}
