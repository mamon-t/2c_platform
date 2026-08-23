//! Интеграционные тесты tx_exec на ЖИВОЙ MongoDB (replica set).
//!
//! Запуск:
//!   TX_TEST_MONGO=1 cargo test --test tx_executor_test
//!
//! Используется отдельная БД `tx_test_<run>` с полной очисткой в конце.
//! Без флага тесты пропускаются.

use std::collections::HashMap;
use std::sync::Arc;

use app_lib::core::{CompanyId, UserId};
use app_lib::db::MongoClient;
use app_lib::events::ActorSnapshot;
use app_lib::permission_policy::{PermissionPolicy, PermissionPolicyService};
use app_lib::tx::executor;
use app_lib::tx::{TransactionPackage, TxContext, TxOperation};
use mongodb::bson::{doc, Document};

// ── Инфраструктура ─────────────────────────────────────────

fn mongo_enabled() -> bool {
    std::env::var("TX_TEST_MONGO").map(|v| v == "1").unwrap_or(false)
}

async fn connect() -> (MongoClient, String) {
    let uri = std::env::var("MONGODB_URI")
        .unwrap_or_else(|_| "mongodb://localhost:27017".into());
    let db_name = format!(
        "tx_test_{}_{}",
        chrono::Utc::now().timestamp_millis(),
        &uuid::Uuid::new_v4().simple().to_string()[..6],
    );
    let client = MongoClient::connect(&uri, &db_name)
        .await
        .expect("подключение к MongoDB");
    client.clone();
    app_lib::tx::indexes::ensure_indexes(&client).await;
    (client, db_name)
}

fn all_access_policies() -> Vec<PermissionPolicy> {
    // Один wildcard-политики достаточно: documents.* и test.*
    let mk = |subsystem: &str| PermissionPolicy {
        _id: uuid::Uuid::new_v4(),
        code: format!("test.{subsystem}"),
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
    vec![mk("documents"), mk("test")]
}

fn ctx(company_id: uuid::Uuid, policies: Vec<PermissionPolicy>) -> TxContext {
    TxContext {
        company_id: CompanyId(company_id),
        actor: ActorSnapshot {
            user_id: UserId(uuid::Uuid::nil()),
            login: "tx-test".into(),
            full_name: None,
            position: None,
            company_id: CompanyId(company_id),
        },
        policies,
    }
}

/// Фикстура объекта-документа прямо в коллекции objects.
async fn seed_draft_object(db: &MongoClient, company_id: &CompanyId, id: uuid::Uuid) {
    let rec = doc! {
        "_id": id.to_string(),
        "entity_type_id": "test-type",
        "kind": "document",
        "company_id": company_id.0.to_string(),
        "state": "draft",
        "data": {},
        "version": 1i64,
        "created_by": uuid::Uuid::nil().to_string(),
        "updated_by": uuid::Uuid::nil().to_string(),
        "created_at": mongodb::bson::DateTime::now(),
        "updated_at": mongodb::bson::DateTime::now(),
    };
    db.collection::<Document>("objects")
        .insert_one(rec)
        .await
        .expect("фикстура объекта");
}

async fn object_state(db: &MongoClient, id: uuid::Uuid) -> (String, i64, Option<String>) {
    let d = db
        .collection::<Document>("objects")
        .find_one(doc! { "_id": id.to_string() })
        .await
        .expect("find")
        .expect("объект существует");
    (
        d.get_str("state").unwrap_or("").to_string(),
        d.get_i64("version").unwrap_or(1),
        d.get_str("number").ok().map(String::from),
    )
}

fn package(
    key: &str,
    company: uuid::Uuid,
    policies: Vec<PermissionPolicy>,
    ops: Vec<TxOperation>,
) -> TransactionPackage {
    TransactionPackage {
        idempotency_key: key.into(),
        required_permission: None,
        operations: ops,
        context: ctx(company, policies),
        created_at: chrono::Utc::now(),
        expires_at: None,
    }
}

const OP_POST: &str = r#"{"object_id":"OBJ","expected_version":1}"#;

fn post_op(object_id: uuid::Uuid, op_id: &str) -> TxOperation {
    TxOperation {
        op_id: op_id.into(),
        op: "object.post".into(),
        params: serde_json::json!({
            "object_id": object_id.to_string(),
            "expected_version": 1,
        }),
    }
}

// ── Тесты ──────────────────────────────────────────────────

#[tokio::test]
async fn idempotent_replay_does_not_repost() {
    if !mongo_enabled() {
        eprintln!("SKIP: задайте TX_TEST_MONGO=1 для интеграционных тестов tx_exec");
        return;
    }
    let (db, db_name) = connect().await;
    let company = uuid::Uuid::new_v4();
    let obj = uuid::Uuid::new_v4();
    seed_draft_object(&db, &CompanyId(company), obj).await;

    let pkg = package(
        "replay-case",
        company,
        all_access_policies(),
        vec![
            TxOperation { op_id: "noop1".into(), op: "test.noop".into(), params: serde_json::json!({"x": 1}) },
            post_op(obj, "post1"),
        ],
    );

    // Первый запуск: пост прошёл
    let r1 = executor::execute(&db, pkg.clone()).await.expect("первый execute");
    assert_eq!(r1.op_results["post1"]["state"], "posted");
    let (_, v_after_first, _) = object_state(&db, obj).await;

    // Второй запуск с тем же ключом: идемпотентный повтор
    let r2 = executor::execute(&db, pkg).await.expect("повтор");
    assert_eq!(r2.op_results["post1"]["state"], "posted");

    let (_, v_after_replay, _) = object_state(&db, obj).await;
    assert_eq!(v_after_first, 2, "после первого поста версия = 2");
    assert_eq!(v_after_replay, 2, "повтор НЕ должен был снова провести объект");

    // Результаты идентичны (номер тот же)
    assert_eq!(r1.op_results["post1"], r2.op_results["post1"]);

    db.client()
        .database(&db_name)
        .drop()
        .await
        .expect("cleanup");
}

#[tokio::test]
async fn concurrent_same_key_single_application() {
    if !mongo_enabled() {
        eprintln!("SKIP: задайте TX_TEST_MONGO=1");
        return;
    }
    let (db, db_name) = connect().await;
    let db = Arc::new(db);
    let company = uuid::Uuid::new_v4();
    let obj = uuid::Uuid::new_v4();
    seed_draft_object(&db, &CompanyId(company), obj).await;

    let mk_pkg = move || {
        package(
            "race-case",
            company,
            all_access_policies(),
            vec![post_op(obj, "post1")],
        )
    };

    // Оба старта одновременно
    let db1 = db.clone();
    let t1 = tokio::spawn(async move { executor::execute(&db1, mk_pkg()).await });
    let db2 = db.clone();
    let t2 = tokio::spawn(async move { executor::execute(&db2, mk_pkg()).await });

    let r1 = t1.await.unwrap();
    let r2 = t2.await.unwrap();

    // Оба вызывающих получают успех с одинаковым результатом
    let a = r1.expect("первый вызов");
    let b = r2.expect("второй вызов");
    assert_eq!(a.op_results["post1"], b.op_results["post1"]);

    // Применение ровно одно; журнал — одна запись
    let (_, version, number) = object_state(&db, obj).await;
    assert_eq!(version, 2);
    assert!(number.is_some(), "номер присвоен один раз");

    let journal_count = db
        .collection::<Document>(app_lib::tx::journal::COLLECTION)
        .count_documents(doc! { "idempotency_key": "race-case", "company_id": company.to_string() })
        .await
        .unwrap();
    assert_eq!(journal_count, 1, "запись журнала должна быть одна");

    db.client().database(&db_name).drop().await.expect("cleanup");
}

#[tokio::test]
async fn unknown_op_rolls_back_everything() {
    if !mongo_enabled() {
        eprintln!("SKIP: задайте TX_TEST_MONGO=1");
        return;
    }
    let (db, db_name) = connect().await;
    let company = uuid::Uuid::new_v4();
    let obj = uuid::Uuid::new_v4();
    seed_draft_object(&db, &CompanyId(company), obj).await;

    let pkg = package(
        "rollback-case",
        company,
        all_access_policies(),
        vec![
            post_op(obj, "post1"),
            TxOperation { op_id: "boom".into(), op: "no.such.op".into(), params: serde_json::json!({}) },
        ],
    );

    let err = executor::execute(&db, pkg).await.expect_err("должна быть ошибка");
    assert_eq!(err.failed_op.as_deref(), Some("boom"));

    // Откат: объект остался черновиком v1
    let (state, version, _) = object_state(&db, obj).await;
    assert_eq!((state.as_str(), version), ("draft", 1));

    // Журнала нет
    let n = db
        .collection::<Document>(app_lib::tx::journal::COLLECTION)
        .count_documents(doc! { "idempotency_key": "rollback-case" })
        .await
        .unwrap();
    assert_eq!(n, 0);

    db.client().database(&db_name).drop().await.expect("cleanup");
}

#[tokio::test]
async fn ref_chain_feeds_post_params() {
    if !mongo_enabled() {
        eprintln!("SKIP: задайте TX_TEST_MONGO=1");
        return;
    }
    let (db, db_name) = connect().await;
    let company = uuid::Uuid::new_v4();
    let obj = uuid::Uuid::new_v4();
    seed_draft_object(&db, &CompanyId(company), obj).await;

    // noop выдаёт object_id+версию → post берёт их по $ref
    let pkg = package(
        "ref-chain",
        company,
        all_access_policies(),
        vec![
            TxOperation {
                op_id: "src".into(),
                op: "test.noop".into(),
                params: serde_json::json!({
                    "object_id": obj.to_string(),
                    "expected_version": 1,
                }),
            },
            TxOperation {
                op_id: "post1".into(),
                op: "object.post".into(),
                params: serde_json::json!({
                    "object_id": {"$ref": "src.object_id"},
                    "expected_version": {"$ref": "src.expected_version"},
                }),
            },
        ],
    );

    let r = executor::execute(&db, pkg).await.expect("цепочка $ref");
    assert_eq!(r.op_results["post1"]["state"], "posted");
    let (_, version, _) = object_state(&db, obj).await;
    assert_eq!(version, 2);

    db.client().database(&db_name).drop().await.expect("cleanup");
}

#[tokio::test]
async fn permission_denied_without_policy() {
    if !mongo_enabled() {
        eprintln!("SKIP: задайте TX_TEST_MONGO=1");
        return;
    }
    let (db, db_name) = connect().await;
    let company = uuid::Uuid::new_v4();
    let obj = uuid::Uuid::new_v4();
    seed_draft_object(&db, &CompanyId(company), obj).await;

    // Пустые политики → deny-by-default на обработчике documents.approve
    let pkg = package("perm-case", company, vec![], vec![post_op(obj, "post1")]);

    let err = executor::execute(&db, pkg).await.expect_err("deny-by-default");
    assert_eq!(err.failed_op.as_deref(), Some("post1"));
    assert!(err.message.contains("Доступ запрещён") || err.message.contains("права"), "{err}");

    let (state, _, _) = object_state(&db, obj).await;
    assert_eq!(state, "draft");

    db.client().database(&db_name).drop().await.expect("cleanup");
}

#[allow(dead_code)]
fn _keep_imports(_: HashMap<String, String>, _: &PermissionPolicyService) {}
