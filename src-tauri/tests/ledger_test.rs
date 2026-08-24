//! Интеграционные тесты учёта на живой MongoDB.
//!
//! Запуск: TX_TEST_MONGO=1 cargo test --test ledger_test

use app_lib::core::{CompanyId, UserId};
use app_lib::db::MongoClient;
use app_lib::events::ActorSnapshot;
use app_lib::permission_policy::PermissionPolicy;
use app_lib::ledger::service::{LedgerService, PostInput};
use app_lib::tx::executor;
use app_lib::ledger::PostingLine;

fn mongo_enabled() -> bool {
    std::env::var("TX_TEST_MONGO").map(|v| v == "1").unwrap_or(false)
}

async fn connect() -> (MongoClient, String) {
    let uri = std::env::var("MONGODB_URI")
        .unwrap_or_else(|_| "mongodb://localhost:27017".into());
    let db_name = format!(
        "ledger_test_{}_{}",
        chrono::Utc::now().timestamp_millis(),
        &uuid::Uuid::new_v4().simple().to_string()[..6],
    );
    let client = MongoClient::connect(&uri, &db_name).await.expect("mongo");
    app_lib::ledger::indexes::ensure_indexes(&client).await;
    (client, db_name)
}

fn test_policies() -> Vec<PermissionPolicy> {
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
    vec![mk("documents"), mk("stock"), mk("accounting")]
}

fn actor(company: uuid::Uuid) -> ActorSnapshot {
    ActorSnapshot {
        user_id: UserId(uuid::Uuid::nil()),
        login: "ledger-test".into(),
        full_name: None,
        position: None,
        company_id: CompanyId(company),
    }
}

async fn post_pair(
    db: &MongoClient,
    company: uuid::Uuid,
    key: &str,
    date: &str,
    doc_id: &str,
    debit: &str,
    credit: &str,
    amount: i64,
) -> Result<serde_json::Value, String> {
    use app_lib::tx::{TransactionPackage, TxContext, TxOperation};
    executor::execute(
        db,
        TransactionPackage {
            idempotency_key: key.into(),
            required_permission: None,
            operations: vec![TxOperation {
                op_id: "acc".into(),
                op: "accounting.post".into(),
                params: serde_json::json!({
                    "date": date,
                    "doc_kind": "SALES",
                    "doc_id": doc_id,
                    "lines": [{"debit_code": debit, "credit_code": credit, "amount": amount,
                               "nomenclature_id": "nom-1"}],
                }),
            }],
            context: TxContext { company_id: CompanyId(company), actor: actor(company), policies: test_policies() },
            created_at: chrono::Utc::now(),
            expires_at: None,
        },
    )
    .await
    .map(|r| serde_json::to_value(&r).unwrap_or_default())
    .map_err(|e| e.to_string())
}

#[tokio::test(flavor = "multi_thread")]
async fn posting_balances_and_reverse() {
    if !mongo_enabled() { eprintln!("SKIP: TX_TEST_MONGO=1"); return; }
    let (db, db_name) = connect().await;
    let company = uuid::Uuid::new_v4();
    LedgerService::ensure_default_chart(&db, &CompanyId(company)).await;

    // Постинг: Дт 62 Кт 90.1 — выручка 150000
    post_pair(&db, company, "l1", "2026-08-25", "doc-s1", "62", "90.1", 150_000)
        .await
        .expect("постинг выручки");

    // Баланс 90.1: кредитовый оборот 150000
    let bal = db.collection::<mongodb::bson::Document>(app_lib::ledger::COL_BALANCES)
        .find_one(mongodb::bson::doc! {"account_code": "90.1"})
        .await.unwrap().expect("баланс 90.1");
    assert_eq!(bal.get_i64("credit_turnover").unwrap(), 150_000);

    // Период создан и открыт
    let period = db.collection::<mongodb::bson::Document>(app_lib::ledger::COL_PERIODS)
        .find_one(mongodb::bson::doc! {"period_key": "2026-08"})
        .await.unwrap().expect("период");
    assert_eq!(period.get_bool("opened").unwrap(), true);

    // Реверс по документу
    let mut session = db.client().start_session().await.unwrap();
    session.start_transaction().await.unwrap();
    let rev = app_lib::ledger::service::LedgerService::reverse_by_doc_in_session(
        &db, &mut session, &CompanyId(company), UserId(uuid::Uuid::nil()),
        "doc-s1", Some("STORNO"), Some("doc-s1-rev"), "2026-08-26",
    ).await.expect("реверс");
    session.commit_transaction().await.unwrap();
    assert_eq!(rev["entries_reversed"], 1);

    // Оборот после сторно: 0 (зеркальная запись)
    let bal = db.collection::<mongodb::bson::Document>(app_lib::ledger::COL_BALANCES)
        .find_one(mongodb::bson::doc! {"account_code": "90.1"})
        .await.unwrap().expect("баланс");
    assert_eq!(bal.get_i64("credit_turnover").unwrap(), 150_000);
    assert_eq!(bal.get_i64("debit_turnover").unwrap(), 150_000);

    // Повторный реверс — NotFound
    let mut s2 = db.client().start_session().await.unwrap();
    s2.start_transaction().await.unwrap();
    let err = app_lib::ledger::service::LedgerService::reverse_by_doc_in_session(
        &db, &mut s2, &CompanyId(company), UserId(uuid::Uuid::nil()),
        "doc-s1", None, None, "2026-08-27",
    ).await;
    assert!(err.is_err(), "повторный реверс должен отклоняться");

    db.client().database(&db_name).drop().await.expect("cleanup");
}

#[tokio::test(flavor = "multi_thread")]
async fn closed_period_blocks_posting() {
    if !mongo_enabled() { eprintln!("SKIP: TX_TEST_MONGO=1"); return; }
    let (db, db_name) = connect().await;
    let company = uuid::Uuid::new_v4();
    LedgerService::ensure_default_chart(&db, &CompanyId(company)).await;

    // Создаём период и закрываем
    let mut sess = db.client().start_session().await.unwrap();
    sess.start_transaction().await.unwrap();
    app_lib::ledger::service::LedgerService::post_pairs_in_session(
        &db, &mut sess,
        PostInput {
            company_id: &CompanyId(company),
            created_by: UserId(uuid::Uuid::nil()),
            date: "2026-07-15",
            doc_kind: None, doc_id: None,
            lines: vec![PostingLine {

                debit_code: "51".into(), credit_code: "90.1".into(),
                amount: 100, nomenclature_id: None, description: None,
            }],
            is_reversal: false,
        },
    ).await.expect("открытие периода июль");
    sess.commit_transaction().await.unwrap();

    app_lib::ledger::service::LedgerService::set_period_state(
        &db, &CompanyId(company), 2026, 7, false, true,
    ).await.expect("закрытие");

    // Проводка в закрытый период — отказ
    let err = post_pair(&db, company, "l2", "2026-07-20", "d2", "51", "90.1", 50)
        .await
        .expect_err("должен отказать");
    assert!(err.to_string().contains("закрыт"), "{err}");

    db.client().database(&db_name).drop().await.expect("cleanup");
}
