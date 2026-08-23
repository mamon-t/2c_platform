//! Интеграционные тесты движка склада на живой MongoDB.
//!
//! Запуск: TX_TEST_MONGO=1 cargo test --test stock_engine_test
//! Прогоняют операции через executor::execute — как это будет делать
//! оркестратор/плагин. Покрывают демо-сценарий ТЗ (п.15).

use std::sync::Arc;

use app_lib::core::{CompanyId, UserId};
use app_lib::db::MongoClient;
use app_lib::events::ActorSnapshot;
use app_lib::permission_policy::PermissionPolicy;
use app_lib::tx::executor;
use app_lib::tx::{TransactionPackage, TxContext, TxOperation};
use mongodb::bson::{doc, Document};

fn mongo_enabled() -> bool {
    std::env::var("TX_TEST_MONGO").map(|v| v == "1").unwrap_or(false)
}

async fn connect() -> (MongoClient, String) {
    let uri = std::env::var("MONGODB_URI")
        .unwrap_or_else(|_| "mongodb://localhost:27017".into());
    let db_name = format!(
        "stock_test_{}_{}",
        chrono::Utc::now().timestamp_millis(),
        &uuid::Uuid::new_v4().simple().to_string()[..6],
    );
    let client = MongoClient::connect(&uri, &db_name).await.expect("mongo");
    app_lib::tx::indexes::ensure_indexes(&client).await;
    app_lib::stock::indexes::ensure_indexes(&client).await;
    (client, db_name)
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
    vec![mk("documents"), mk("stock"), mk("test")]
}

fn ctx(company: uuid::Uuid) -> TxContext {
    TxContext {
        company_id: CompanyId(company),
        actor: ActorSnapshot {
            user_id: UserId(uuid::Uuid::nil()),
            login: "stock-test".into(),
            full_name: None,
            position: None,
            company_id: CompanyId(company),
        },
        policies: policies(),
    }
}

/// Фикстуры: номенклатура-товар и две локации в objects.
async fn seed_catalog(db: &MongoClient, company: uuid::Uuid, nom_id: &str) {
    let col = db.collection::<Document>("objects");
    for (id, data) in [
        (
            nom_id.to_string(),
            doc! { "type": "item", "category": "equipment", "unit": "шт" },
        ),
        (
            format!("loc-wh-{company}"),
            doc! { "type": "warehouse", "is_active": true },
        ),
        (
            format!("loc-b-{company}"),
            doc! { "type": "warehouse", "is_active": true },
        ),
    ] {
        col.insert_one(doc! {
            "_id": id,
            "entity_type_id": "TEST",
            "kind": "catalog",
            "company_id": company.to_string(),
            "state": "active",
            "data": mongodb::bson::to_bson(&data).unwrap(),
            "version": 1i64,
        }).await.expect("фикстура");
    }
}

fn op(op_id: &str, op: &str, params: serde_json::Value) -> TxOperation {
    TxOperation { op_id: op_id.into(), op: op.into(), params }
}

async fn balances_of(db: &MongoClient, company: uuid::Uuid, loc_suffix: &str, nom: &str) -> f64 {
    let d = db
        .collection::<Document>(app_lib::stock::COL_BALANCES)
        .find_one(doc! {
            "company_id": company.to_string(),
            "location_id": if loc_suffix.is_empty() { format!("loc-{loc_suffix}-{company}") } else { format!("loc-{loc_suffix}-{company}") },
            "nomenclature_id": nom,
        })
        .await
        .unwrap()
        .unwrap_or(Document::new());
    d.get_f64("quantity").unwrap_or(0.0)
}

async fn movements_for_doc(db: &MongoClient, doc_id: &str) -> Vec<Document> {
    let mut cursor = db
        .collection::<Document>(app_lib::stock::COL_MOVEMENTS)
        .find(doc! { "doc_id": doc_id })
        .await
        .unwrap();
    let mut out = Vec::new();
    use futures::StreamExt;
    while let Some(Ok(d)) = cursor.next().await {
        out.push(d);
    }
    out
}

// ── Демо-сценарий п.15, шаги 1–3 + 5 ───────────────────────

#[tokio::test]
async fn fifo_math_and_shortage_error() {
    if !mongo_enabled() { eprintln!("SKIP: TX_TEST_MONGO=1"); return; }
    let (db, db_name) = connect().await;
    let company = uuid::Uuid::new_v4();
    seed_catalog(&db, company, "nom-1").await;
    let wh = format!("loc-wh-{company}");

    // Шаг 1–2: приход 10@100 и 5@120
    let p1 = TransactionPackage {
        idempotency_key: "s1".into(),
        required_permission: None,
        operations: vec![
            op("r1", "stock.receipt", serde_json::json!({
                "location_id": wh,
                "lines": [{"nomenclature_id": "nom-1", "qty": 10, "unit_cost": 10000}],
                "doc_kind": "receipt_doc", "doc_id": "doc-in-1",
            })),
        ],
        context: ctx(company),
        created_at: chrono::Utc::now(),
        expires_at: None,
    };
    executor::execute(&db, p1).await.expect("приход 1");

    let p2 = TransactionPackage {
        idempotency_key: "s2".into(),
        required_permission: None,
        operations: vec![
            op("r2", "stock.receipt", serde_json::json!({
                "location_id": wh,
                "lines": [{"nomenclature_id": "nom-1", "qty": 5, "unit_cost": 12000}],
                "doc_kind": "receipt_doc", "doc_id": "doc-in-2",
            })),
        ],
        context: ctx(company),
        created_at: chrono::Utc::now(),
        expires_at: None,
    };
    // Небольшая пауза, чтобы receipt_date различался для детерминированного FIFO
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    executor::execute(&db, p2).await.expect("приход 2");

    assert_eq!(balances_of(&db, company, "wh", "nom-1").await, 15.0);

    // Шаг 3: списание 12 → себестоимость 10×100 + 2×120 = 1240 (в копейках 124000)
    let p3 = TransactionPackage {
        idempotency_key: "s3".into(),
        required_permission: None,
        operations: vec![
            op("i1", "stock.issue", serde_json::json!({
                "location_id": wh,
                "lines": [{"nomenclature_id": "nom-1", "qty": 12}],
                "doc_kind": "issue_doc", "doc_id": "doc-out-1",
            })),
        ],
        context: ctx(company),
        created_at: chrono::Utc::now(),
        expires_at: None,
    };
    let r = executor::execute(&db, p3).await.expect("списание");
    let line = &r.op_results["i1"]["lines"][0];
    assert_eq!(line["total_cost"], 124000, "FIFO себестоимость 1240 ₽");
    assert_eq!(line["parts"].as_array().unwrap().len(), 2, "съедены две партии");

    assert_eq!(balances_of(&db, company, "wh", "nom-1").await, 3.0);

    // Шаг 5: списание 100 → недостаточно
    let p4 = TransactionPackage {
        idempotency_key: "s4".into(),
        required_permission: None,
        operations: vec![
            op("i2", "stock.issue", serde_json::json!({
                "location_id": wh,
                "lines": [{"nomenclature_id": "nom-1", "qty": 100}],
            })),
        ],
        context: ctx(company),
        created_at: chrono::Utc::now(),
        expires_at: None,
    };
    let err = executor::execute(&db, p4).await.expect_err("недостаточно");
    assert!(err.message.contains("Недостаточно"), "{err}");
    assert!(err.message.contains("нужно 100"), "{err}");
    assert!(err.message.contains("есть 3"), "{err}");

    // Баланс не изменился после ошибки
    assert_eq!(balances_of(&db, company, "wh", "nom-1").await, 3.0);

    db.client().database(&db_name).drop().await.expect("cleanup");
}

// ── Сторно (демо шаг 4) ────────────────────────────────────

#[tokio::test]
async fn cancel_reverses_movements() {
    if !mongo_enabled() { eprintln!("SKIP: TX_TEST_MONGO=1"); return; }
    let (db, db_name) = connect().await;
    let company = uuid::Uuid::new_v4();
    seed_catalog(&db, company, "nom-1").await;
    let wh = format!("loc-wh-{company}");

    let mk_issue = |key: &str| TransactionPackage {
        idempotency_key: key.into(),
        required_permission: None,
        operations: vec![
            op("r", "stock.receipt", serde_json::json!({
                "location_id": wh,
                "lines": [{"nomenclature_id": "nom-1", "qty": 7, "unit_cost": 5000}],
                "doc_kind": "in", "doc_id": key,
            })),
            op("i", "stock.issue", serde_json::json!({
                "location_id": wh,
                "lines": [{"nomenclature_id": "nom-1", "qty": 4}],
                "doc_kind": "out", "doc_id": "out-x",
            })),
        ],
        context: ctx(company),
        created_at: chrono::Utc::now(),
        expires_at: None,
    };

    executor::execute(&db, mk_issue("c1")).await.expect("приход+списание");
    assert_eq!(balances_of(&db, company, "wh", "nom-1").await, 3.0);

    // Сторно документа прихода: партия нетронута по qty? Нет — списание съело часть!
    // Приход создал партию 7, списание съело 4 → сторно прихода ДОЛЖНО отклониться.
    let rev = package_reverse("rev-in", "c1-rev-key", "c1", company);
    let err = executor::execute(&db, rev).await.expect_err("сторно частично съеденной партии");
    assert!(err.message.contains("уже были движения"), "{err}");
    assert_eq!(balances_of(&db, company, "wh", "nom-1").await, 3.0);

    // Сторно документа СПИСАНИЯ: партии восстанавливаются
    let rev_out = package_reverse("rev-out", "c2-rev-key", "out-x", company);
    let r = executor::execute(&db, rev_out).await.expect("сторно списания");
    assert!(r.op_results["rev"]["undone_movements"].as_i64().unwrap() >= 1);
    assert_eq!(balances_of(&db, company, "wh", "nom-1").await, 7.0);

    db.client().database(&db_name).drop().await.expect("cleanup");
}

fn package_reverse(_op_id: &str, key: &str, target_doc: &str, company: uuid::Uuid) -> TransactionPackage {
    // Компания подставляется вызывающим тестом через глобальную замену ниже
    TransactionPackage {
        idempotency_key: key.into(),
        required_permission: None,
        operations: vec![
            op("rev", "stock.reverse", serde_json::json!({ "target_doc_id": target_doc })),
        ],
        context: ctx(company),
        created_at: chrono::Utc::now(),
        expires_at: None,
    }
}

// ── Перемещение сохраняет цену (демо шаг 6) ────────────────

#[tokio::test]
async fn transfer_preserves_cost() {
    if !mongo_enabled() { eprintln!("SKIP: TX_TEST_MONGO=1"); return; }
    let (db, db_name) = connect().await;
    let company = uuid::Uuid::new_v4();
    seed_catalog(&db, company, "nom-1").await;
    let wh = format!("loc-wh-{company}");
    let wh_b = format!("loc-b-{company}");

    executor::execute(&db, TransactionPackage {
        idempotency_key: "tr-in".into(),
        required_permission: None,
        operations: vec![
            op("r", "stock.receipt", serde_json::json!({
                "location_id": wh,
                "lines": [{"nomenclature_id": "nom-1", "qty": 3, "unit_cost": 7777}],
            })),
        ],
        context: ctx(company),
        created_at: chrono::Utc::now(),
        expires_at: None,
    }).await.expect("приход");

    let r = executor::execute(&db, TransactionPackage {
        idempotency_key: "tr-move".into(),
        required_permission: None,
        operations: vec![
            op("t", "stock.transfer", serde_json::json!({
                "from_location_id": wh,
                "to_location_id": wh_b,
                "lines": [{"nomenclature_id": "nom-1", "qty": 3}],
            })),
        ],
        context: ctx(company),
        created_at: chrono::Utc::now(),
        expires_at: None,
    }).await.expect("перемещение");

    assert_eq!(balances_of(&db, company, "wh", "nom-1").await, 0.0);
    assert_eq!(balances_of(&db, company, "b", "nom-1").await, 3.0);

    // Партия на приёмнике с той же ценой
    let batch = db.collection::<Document>(app_lib::stock::COL_BATCHES)
        .find_one(doc! { "location_id": wh_b })
        .await.unwrap().expect("партия переехала");
    assert_eq!(batch.get_i64("unit_cost").unwrap(), 7777, "цена переехала вместе с товаром");
    let _ = r;

    db.client().database(&db_name).drop().await.expect("cleanup");
}

// ── Гонка списаний одной позиции ───────────────────────────

#[tokio::test]
async fn concurrent_issues_no_double_write() {
    if !mongo_enabled() { eprintln!("SKIP: TX_TEST_MONGO=1"); return; }
    let (db, db_name) = connect().await;
    let db = Arc::new(db);
    let company = uuid::Uuid::new_v4();
    seed_catalog(&db, company, "nom-1").await;
    let wh = format!("loc-wh-{company}");

    executor::execute(&db, TransactionPackage {
        idempotency_key: "seed-race".into(),
        required_permission: None,
        operations: vec![
            op("r", "stock.receipt", serde_json::json!({
                "location_id": wh,
                "lines": [{"nomenclature_id": "nom-1", "qty": 10, "unit_cost": 100}],
            })),
        ],
        context: ctx(company),
        created_at: chrono::Utc::now(),
        expires_at: None,
    }).await.expect("seed");

    let mk_issue = move || TransactionPackage {
        idempotency_key: format!("race-{}", uuid::Uuid::new_v4()),
        required_permission: None,
        operations: vec![
            op("i", "stock.issue", serde_json::json!({
                "location_id": wh,
                "lines": [{"nomenclature_id": "nom-1", "qty": 8}],
            })),
        ],
        context: ctx(company),
        created_at: chrono::Utc::now(),
        expires_at: None,
    };

    let pkg1 = mk_issue();
    let pkg2 = mk_issue();

    let db1 = db.clone();
    let t1 = tokio::spawn(async move { executor::execute(&db1, pkg1).await });
    let db2 = db.clone();
    let t2 = tokio::spawn(async move { executor::execute(&db2, pkg2).await });

    let r1 = t1.await.unwrap();
    let r2 = t2.await.unwrap();

    // Ровно одно списание должно пройти; второе — «недостаточно» или откат гонки
    let ok1 = r1.is_ok();
    let ok2 = r2.is_ok();
    let successes = usize::from(ok1) + usize::from(ok2);
    assert_eq!(successes, 1, "два списания по 8 при остатке 10 оба пройти не могут: {ok1}/{ok2}");

    let final_balance = balances_of(&db, company, "wh", "nom-1").await;
    assert_eq!(final_balance, 2.0, "остаток ровно 2, без двойного списания");

    db.client().database(&db_name).drop().await.expect("cleanup");
}
