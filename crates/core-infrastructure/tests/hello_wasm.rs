//! Интеграционные приёмочные тесты WASM-хоста (подфазы 8a–8b по ТЗ v3.1,
//! раздел 9): загрузка и валидация манифеста через `get_info()`, исполнение
//! экспорта `greet` с host-функциями `whoami`/`log_message`, KV-хранилище
//! через `kv_probe` и объекты «Доски» через `objects_probe` (host-fn 8b).
//!
//! Используется скомпилированный из `examples/hello_plugin` модуль
//! `tests/fixtures/hello.wasm` (те же подфазы).

use std::collections::HashSet;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use chrono::Utc;
use core_application::ports::{
    EntitySchema, EventStore, MetadataRepository, ObjectRepository, RoleRepository, UserRepository,
    WasmHost,
};
use core_domain::event::{ActorSnapshot, Event, StreamType};
use core_domain::metadata::{EntityField, EntityKind, EntityState, EntityType, FieldType};
use core_domain::role::Role;
use core_domain::user::{ContactChannelType, ContactPurpose, Person, User, UserContact};
use core_infrastructure::extism_wasm_host::{ExtismWasmHost, HostCallCtx};
use core_infrastructure::surreal_metadata_repository::SurrealMetadataRepository;
use core_infrastructure::surreal_object_repository::SurrealObjectRepository;
use serde_json::{json, Value};
use surrealdb::engine::any::Any;
use surrealdb::Surreal;
use uuid::Uuid;

const HELLO_WASM: &[u8] = include_bytes!("fixtures/hello.wasm");

async fn mem_db() -> Surreal<Any> {
    let db = surrealdb::engine::any::connect("mem://").await.unwrap();
    db.use_ns("test")
        .use_db(Uuid::new_v4().to_string())
        .await
        .unwrap();
    db
}

fn temp_cache() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("2c-hello-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

async fn host() -> (ExtismWasmHost, Surreal<Any>, PathBuf) {
    let cache = temp_cache();
    let db = mem_db().await;
    let objects = Arc::new(SurrealObjectRepository::new(db.clone()));
    let metadata = Arc::new(SurrealMetadataRepository::new(db.clone()));
    let events = Arc::new(core_infrastructure::SurrealEventStore::new(db.clone()));
    let users = Arc::new(core_infrastructure::SurrealUserRepository::new(db.clone()));
    let audit = Arc::new(core_infrastructure::SurrealAuditRepository::new(db.clone()));
    let transactions = core_application::TransactionOrchestrator::new(objects.clone());
    let script_engine = Arc::new(
        core_infrastructure::RhaiScriptEngine::new(core_infrastructure::rhai_core_api::CoreApiShared {
            store: events.clone(),
            db: db.clone(),
            audit: audit.clone(),
            runtime: tokio::runtime::Handle::current(),
        })
        .unwrap(),
    );
    let h = ExtismWasmHost::new(
        db.clone(),
        objects.as_ref().clone(),
        metadata.as_ref().clone(),
        events.as_ref().clone(),
        users.as_ref().clone(),
        transactions,
        script_engine,
        cache.clone(),
    )
    .unwrap();
    h.ensure_schema().await.unwrap();
    objects.ensure_schema().await.unwrap();
    metadata.ensure_schema().await.unwrap();
    events.ensure_schema().await.unwrap();
    users.ensure_schema().await.unwrap();
    audit.ensure_schema().await.unwrap();
    (h, db, cache)
}

/// Регистрирует тип сущности `greeting` (каталог, поле `text`) в компании
/// `comp1` и возвращает его id для host-вызовов подфазы 8b.
async fn seed_greeting_schema(db: &Surreal<Any>) -> Uuid {
    let metadata = SurrealMetadataRepository::new(db.clone());
    let entity_type = EntityType {
        id: Uuid::new_v4(),
        code: "greeting".to_string(),
        name: "Приветствие".to_string(),
        kind: EntityKind::Catalog,
        company_id: "comp1".to_string(),
        metadata_version: 1,
        is_system: false,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    let schema = EntitySchema {
        entity_type: entity_type.clone(),
        fields: vec![EntityField {
            id: Uuid::new_v4(),
            entity_type: "greeting".to_string(),
            code: "text".to_string(),
            label: "Текст".to_string(),
            data_type: FieldType::String,
            required: true,
            is_unique: false,
            is_indexed: false,
            options: Value::Null,
            is_system: false,
            order: 1,
        }],
        states: vec![EntityState {
            id: Uuid::new_v4(),
            entity_type: "greeting".to_string(),
            code: "active".to_string(),
            label: "Активен".to_string(),
            color: None,
            is_initial: true,
            is_final: false,
        }],
        transitions: vec![],
        forms: vec![],
        actions: vec![],
        relations: vec![],
    };
    metadata.create_entity_type(&schema, &[]).await.unwrap();
    entity_type.id
}

#[tokio::test]
async fn load_module_validates_manifest_and_checks_code() {
    let (h, _db, _cache) = host().await;

    let manifest = h.load_module("hello", HELLO_WASM).await.unwrap();
    assert_eq!(manifest.code, "hello");
    assert_eq!(manifest.api_version, "2.0");
    assert_eq!(manifest.version, "1.0.0");
    assert!(manifest.capabilities.iter().any(|c| c == "storage"));

    assert!(h.is_loaded("hello").await);

    let mismatch = h.load_module("wrong", HELLO_WASM).await;
    assert!(mismatch.is_err(), "код манифеста должен совпадать с переданным");
}

#[tokio::test]
async fn call_function_invokes_export_and_host_functions() {
    let company = Uuid::new_v4().to_string();
    let actor = Uuid::new_v4().to_string();
    let (h, _db, _cache) = host().await;

    h.load_module("hello", HELLO_WASM).await.unwrap();
    h.set_call_context(HostCallCtx {
        module_code: "hello".to_string(),
        company_id: company.clone(),
        actor: None,
        capabilities: HashSet::from(["logging".to_string(), "storage".to_string()]),
        settings: Value::Null,
    })
    .await;

    let out = h
        .call_function("hello", "greet", b"")
        .await
        .expect("greet должен отработать");
    let text = String::from_utf8(out).unwrap();
    assert!(
        text.contains("Привет"),
        "greet должен вернуть приветствие, получили: {text}"
    );
    assert!(!actor.is_empty());
}

#[tokio::test]
async fn kv_probe_reads_and_writes_through_host() {
    let (h, _db, _cache) = host().await;

    h.load_module("hello", HELLO_WASM).await.unwrap();
    h.set_call_context(HostCallCtx {
        module_code: "hello".to_string(),
        company_id: Uuid::new_v4().to_string(),
        actor: None,
        capabilities: HashSet::from(["storage".to_string()]),
        settings: Value::Null,
    })
    .await;

    let out = h
        .call_function("hello", "kv_probe", b"")
        .await
        .expect("kv_probe должен отработать");
    let text = String::from_utf8(out).unwrap();
    assert!(
        text.contains("\"ok\":true") && text.contains("\"data\":\"hello_value\""),
        "kv_probe должен вернуть записанное значение, получили: {text}"
    );
}

#[tokio::test]
async fn unload_module_removes_and_tracks_absence() {
    let (h, _db, _cache) = host().await;
    h.load_module("hello", HELLO_WASM).await.unwrap();
    assert!(h.is_loaded("hello").await);

    h.unload_module("hello").await.unwrap();
    assert!(!h.is_loaded("hello").await);

    let err = h.unload_module("hello").await;
    assert!(err.is_err(), "повторная выгрузка должна вернуть ошибку");

    let call = h.call_function("hello", "greet", b"").await;
    assert!(call.is_err(), "вызов после выгрузки должен вернуть ошибку");
}

#[tokio::test]
async fn objects_probe_creates_reads_and_lists_through_host_8b() {
    let (h, db, _cache) = host().await;
    let entity_type_id = seed_greeting_schema(&db).await;

    h.load_module("hello", HELLO_WASM).await.unwrap();
    h.set_call_context(HostCallCtx {
        module_code: "hello".to_string(),
        company_id: "comp1".to_string(),
        actor: None,
        capabilities: HashSet::from([
            "objects.create".to_string(),
            "objects.read".to_string(),
        ]),
        settings: Value::Null,
    })
    .await;

    let out = h
        .call_function("hello", "objects_probe", entity_type_id.to_string().as_bytes())
        .await
        .expect("objects_probe должен отработать");
    let text = String::from_utf8(out).unwrap();

    let create_conv = extract_conv(&text, "create=");
    let get_conv = extract_conv(&text, "get=");
    let list_conv = extract_conv(&text, "list=");

    assert!(
        create_conv.contains("\"ok\":true") && create_conv.contains("\"id\""),
        "create должен вернуть ok и id объекта, получили: {create_conv}"
    );
    let created_id = create_conv
        .split("\"id\":")
        .nth(1)
        .and_then(|s| s.trim_start_matches('"').split('"').next())
        .unwrap()
        .to_string();

    assert!(
        get_conv.contains(&format!("\"id\":\"{created_id}\""))
            && get_conv.contains("\"state\":\"active\"")
            && get_conv.contains("\"Привет, бизнес-объект!\""),
        "get должен вернуть объект с начальным состоянием и данными, получили: {get_conv}"
    );

    assert!(
        list_conv.contains("\"total_count\":1")
            && list_conv.contains(&format!("\"id\":\"{created_id}\"")),
        "list должен показать один объект с total_count=1, получили: {list_conv}"
    );

    let call = h.call_function("hello", "objects_probe", entity_type_id.to_string().as_bytes()).await;
    assert!(call.is_ok(), "повторный вызов должен создавать ещё один объект");
    let second = String::from_utf8(call.unwrap()).unwrap();
    assert!(
        extract_conv(&second, "list=").contains("\"total_count\":2"),
        "после двух созданий total_count должен стать 2, получили: {second}"
    );
}

/// Возвращает фрагмент после маркера `marker` до `; ` — содержимое одного
/// конверта host-функции в выводе экспорта objects_probe.
fn extract_conv<'a>(text: &'a str, marker: &str) -> &'a str {
    let start = text
        .find(marker)
        .map(|i| i + marker.len())
        .unwrap_or_else(|| panic!("маркер {marker} не найден в: {text}"));
    text[start..]
        .split("; ")
        .next()
        .unwrap_or_default()
}

/// Создаёт роль в компании `company_id` (UUID-строка) и возвращает её.
async fn seed_role(db: &Surreal<Any>, company_id: &str) -> Role {
    let roles = core_infrastructure::surreal_role_repository::SurrealRoleRepository::new(db.clone());
    roles.ensure_schema().await.unwrap();
    let role = Role {
        id: Uuid::new_v4(),
        company_id: Uuid::from_str(company_id).unwrap_or_default(),
        code: "accountant".to_string(),
        name: "Бухгалтер".to_string(),
        description: String::new(),
        permission_policy_codes: Vec::new(),
        is_system: false,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    roles
        .create(
            &role,
            &[Event {
                id: Uuid::new_v4(),
                stream_type: StreamType::Role,
                stream_id: role.id.to_string(),
                event_type: "role.created".to_string(),
                version: 0,
                payload: json!({}),
                metadata: ActorSnapshot::system(),
                company_id: company_id.to_string(),
                correlation_id: "corr".to_string(),
                causation_id: None,
                occurred_at: Utc::now(),
            }],
        )
        .await
        .unwrap();
    role
}

/// Создаёт пользователя с ролью, персоной и primary-email контактом.
async fn seed_user(
    db: &Surreal<Any>,
    role_id: Uuid,
    company_id: &str,
    login: &str,
    display_name: &str,
    email: &str,
) -> User {
    let users = core_infrastructure::SurrealUserRepository::new(db.clone());
    let user = User {
        id: Uuid::new_v4(),
        login: login.to_string(),
        password_hash: "x".to_string(),
        status: core_domain::user::UserStatus::Active,
        role_ids: vec![role_id.to_string()],
        failed_login_count: 0,
        locked_until: None,
        must_change_password: false,
        locale: "ru".to_string(),
        timezone: "UTC".to_string(),
        person_id: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    let person = Person {
        id: Uuid::new_v4(),
        user_id: user.id,
        last_name: String::new(),
        first_name: String::new(),
        middle_name: None,
        display_name: display_name.to_string(),
    };
    users
        .create(
            &user,
            &person,
            &[Event {
                id: Uuid::new_v4(),
                stream_type: StreamType::User,
                stream_id: user.id.to_string(),
                event_type: "user.created".to_string(),
                version: 0,
                payload: json!({}),
                metadata: ActorSnapshot::system(),
                company_id: company_id.to_string(),
                correlation_id: "corr".to_string(),
                causation_id: None,
                occurred_at: Utc::now(),
            }],
        )
        .await
        .unwrap();
    users
        .add_contact(
            &UserContact {
                id: Uuid::new_v4(),
                user_id: user.id,
                channel_type: ContactChannelType::Email,
                value: email.to_string(),
                is_primary: true,
                is_verified: true,
                purposes: vec![ContactPurpose::Login],
            },
            &[Event {
                id: Uuid::new_v4(),
                stream_type: StreamType::User,
                stream_id: user.id.to_string(),
                event_type: "user.contact_added".to_string(),
                version: 0,
                payload: json!({}),
                metadata: ActorSnapshot::system(),
                company_id: company_id.to_string(),
                correlation_id: "corr".to_string(),
                causation_id: None,
                occurred_at: Utc::now(),
            }],
        )
        .await
        .unwrap();
    user
}

#[tokio::test]
async fn host_8b_envelope_codes_follow_spec() {
    let (h, _db, _cache) = host().await;

    h.load_module("hello", HELLO_WASM).await.unwrap();
    h.set_call_context(HostCallCtx {
        module_code: "hello".to_string(),
        company_id: "comp1".to_string(),
        actor: None,
        capabilities: HashSet::from(["objects.read".to_string()]),
        settings: Value::Null,
    })
    .await;

    // Не-UUID аргумент → INVALID_UUID (Приложение №6 ТЗ).
    let out = h
        .call_function("hello", "get_probe", b"not-a-uuid")
        .await
        .unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(
        text.contains("\"code\":\"INVALID_UUID\""),
        "некорректный id должен дать INVALID_UUID, получено: {text}"
    );

    // Несуществующий тип сущности → NOT_FOUND.
    let ghost = Uuid::new_v4();
    let out = h
        .call_function("hello", "list_probe", ghost.to_string().as_bytes())
        .await
        .unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(
        text.contains("\"code\":\"NOT_FOUND\""),
        "несуществующий тип должен дать NOT_FOUND, получено: {text}"
    );
}

#[tokio::test]
async fn host_8b_update_object_applies_occ_and_reports_conflict() {
    let (h, db, _cache) = host().await;
    let entity_type_id = seed_greeting_schema(&db).await;

    h.load_module("hello", HELLO_WASM).await.unwrap();
    h.set_call_context(HostCallCtx {
        module_code: "hello".to_string(),
        company_id: "comp1".to_string(),
        actor: None,
        capabilities: HashSet::from([
            "objects.create".to_string(),
            "objects.read".to_string(),
            "objects.update".to_string(),
        ]),
        settings: Value::Null,
    })
    .await;

    let out = h
        .call_function("hello", "objects_probe", entity_type_id.to_string().as_bytes())
        .await
        .unwrap();
    let text = String::from_utf8(out).unwrap();
    let created_id = extract_conv(&text, "create=")
        .split("\"id\":")
        .nth(1)
        .and_then(|s| s.trim_start_matches('"').split('"').next())
        .unwrap()
        .to_string();

    // Первое обновление с version=1 успешно: объект переходит на версию 2.
    let req = serde_json::json!({ "id": created_id, "data": { "text": "обновлено" }, "version": 1 });
    let out = h
        .call_function("hello", "update_probe", req.to_string().as_bytes())
        .await
        .unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(
        text.contains("\"ok\":true") && text.contains("\"version\":2"),
        "первое обновление должно пройти и поднять версию до 2, получено: {text}"
    );

    // Второе обновление с той же устаревшей версией 1 → CONFLICT_ERROR.
    let out = h
        .call_function("hello", "update_probe", req.to_string().as_bytes())
        .await
        .unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(
        text.contains("\"code\":\"CONFLICT_ERROR\""),
        "повторное обновление со старой версией должно дать CONFLICT_ERROR, получено: {text}"
    );
}

#[tokio::test]
async fn emit_event_writes_to_event_store() {
    let (h, db, _cache) = host().await;
    let events = core_infrastructure::SurrealEventStore::new(db.clone());

    h.load_module("hello", HELLO_WASM).await.unwrap();
    h.set_call_context(HostCallCtx {
        module_code: "hello".to_string(),
        company_id: "comp1".to_string(),
        actor: None,
        capabilities: HashSet::from(["events.emit".to_string()]),
        settings: Value::Null,
    })
    .await;

    let out = h
        .call_function("hello", "events_probe", b"")
        .await
        .expect("events_probe должен отработать");
    let conv = String::from_utf8(out).unwrap();
    assert!(
        conv.contains("\"ok\":true") && conv.contains("\"event_id\""),
        "emit_event должен вернуть ok и event_id, получено: {conv}"
    );

    let stream = events
        .read_stream(StreamType::Object, "11111111-2222-3333-4444-555555555555")
        .await
        .unwrap();
    assert_eq!(stream.len(), 1, "в потоке объекта должно быть ровно одно событие");
    assert_eq!(stream[0].stream_type, StreamType::Object);
    assert_eq!(stream[0].event_type, "hello.event.emitted");
    assert_eq!(stream[0].company_id, "comp1");
    assert_eq!(stream[0].metadata.login, "system");
    assert_eq!(stream[0].payload["source"], "events_probe");
}

#[tokio::test]
async fn emit_event_requires_capability() {
    let (h, _db, _cache) = host().await;

    h.load_module("hello", HELLO_WASM).await.unwrap();
    h.set_call_context(HostCallCtx {
        module_code: "hello".to_string(),
        company_id: "comp1".to_string(),
        actor: None,
        capabilities: HashSet::from(["logging".to_string()]),
        settings: Value::Null,
    })
    .await;

    let out = h
        .call_function("hello", "events_probe", b"")
        .await
        .unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(
        text.contains("\"code\":\"CAPABILITY_DENIED\""),
        "без capability events.emit должен быть CAPABILITY_DENIED, получено: {text}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn stub_workflow_and_signature_host_fns_return_spec_envelopes() {
    let (h, _db, _cache) = host().await;

    h.load_module("hello", HELLO_WASM).await.unwrap();
    h.set_call_context(HostCallCtx {
        module_code: "hello".to_string(),
        company_id: "comp1".to_string(),
        actor: None,
        capabilities: HashSet::from([
            "scripts".to_string(),
            "notifications".to_string(),
            "signature".to_string(),
        ]),
        settings: Value::Null,
    })
    .await;

    let out = h
        .call_function("hello", "stubs_probe", b"")
        .await
        .expect("stubs_probe должен отработать");
    let text = String::from_utf8(out).unwrap();

    let script_conv = extract_conv(&text, "script=");
    let notify_conv = extract_conv(&text, "notify=");
    let users_conv = extract_conv(&text, "users=");
    let sigreq_conv = extract_conv(&text, "sigreq=");
    let cms_conv = extract_conv(&text, "cms=");

    assert!(
        script_conv.contains("\"ok\":true"),
        "run_script должен успешно выполнить print(1) через Rhai, получено: {script_conv}"
    );
    assert!(
        notify_conv.contains("\"ok\":true") && notify_conv.contains("\"queued\":true"),
        "notify_user должен вернуть queued=true, получено: {notify_conv}"
    );
    assert!(
        users_conv.contains("\"ok\":true") && users_conv.contains("\"users\":[]"),
        "users_by_role должен вернуть пустой список, получено: {users_conv}"
    );
    assert!(
        sigreq_conv.contains("\"ok\":true") && sigreq_conv.contains("\"required\":false"),
        "signature_required должен вернуть required=false, получено: {sigreq_conv}"
    );
    assert!(
        cms_conv.contains("\"ok\":true") && cms_conv.contains("\"valid\":true"),
        "cms_verify должен вернуть valid=true, получено: {cms_conv}"
    );
}

async fn seed_role_and_users(db: &Surreal<Any>) -> (String, Uuid, Vec<(String, String, String)>) {
    let company = Uuid::new_v4().to_string();
    let role = seed_role(db, &company).await;
    seed_user(db, role.id, &company, "alice", "Петрова Алиса", "alice@example.test").await;
    seed_user(db, role.id, &company, "bob", "Иванов Боб", "bob@example.test").await;
    (
        company,
        role.id,
        vec![
            ("alice".to_string(), "Петрова Алиса".to_string(), "alice@example.test".to_string()),
            ("bob".to_string(), "Иванов Боб".to_string(), "bob@example.test".to_string()),
        ],
    )
}

async fn users_payload(h: &ExtismWasmHost, role_id: Uuid) -> Value {
    let out = h
        .call_function("hello", "users_probe", role_id.to_string().as_bytes())
        .await
        .expect("users_probe должен отработать");
    let text = String::from_utf8(out).unwrap();
    let envelope: Value = serde_json::from_str(&text).unwrap();
    assert!(
        envelope["ok"].as_bool().unwrap_or(false),
        "users_by_role должен вернуть ok, получено: {text}"
    );
    envelope["data"]["users"].clone()
}

#[tokio::test]
async fn users_by_role_9d_returns_users_with_person_and_email() {
    let (h, db, _cache) = host().await;
    h.load_module("hello", HELLO_WASM).await.unwrap();

    let (company_id, role_id, expected) = seed_role_and_users(&db).await;
    h.set_call_context(HostCallCtx {
        module_code: "hello".to_string(),
        company_id,
        actor: None,
        capabilities: HashSet::from(["notifications".to_string()]),
        settings: Value::Null,
    })
    .await;

    let users = users_payload(&h, role_id).await;
    assert_eq!(users.as_array().map(Vec::len), Some(2));
    for (login, full_name, email) in expected {
        let found = users.as_array().unwrap().iter().any(|u| {
            u["login"] == login && u["full_name"] == full_name && u["email"] == email
        });
        assert!(found, "не найден пользователь {login} в {users}");
    }
}

#[tokio::test]
async fn users_by_role_9d_denies_without_notifications_capability() {
    let (h, db, _cache) = host().await;
    h.load_module("hello", HELLO_WASM).await.unwrap();

    let (company_id, role_id, _) = seed_role_and_users(&db).await;
    h.set_call_context(HostCallCtx {
        module_code: "hello".to_string(),
        company_id,
        actor: None,
        capabilities: HashSet::from(["logging".to_string()]),
        settings: Value::Null,
    })
    .await;

    let out = h
        .call_function("hello", "users_probe", role_id.to_string().as_bytes())
        .await
        .unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(
        text.contains("\"code\":\"CAPABILITY_DENIED\""),
        "без capability notifications должен быть CAPABILITY_DENIED, получено: {text}"
    );
}

#[tokio::test]
async fn users_by_role_9d_missing_role_returns_empty_list() {
    let (h, db, _cache) = host().await;
    h.load_module("hello", HELLO_WASM).await.unwrap();
    // Создаём таблицы roles/users (сид произвольной роли чинит схему).
    seed_role(&db, &Uuid::new_v4().to_string()).await;
    h.set_call_context(HostCallCtx {
        module_code: "hello".to_string(),
        company_id: Uuid::new_v4().to_string(),
        actor: None,
        capabilities: HashSet::from(["notifications".to_string()]),
        settings: Value::Null,
    })
    .await;

    let users = users_payload(&h, Uuid::new_v4()).await;
    assert!(
        users.as_array().map(Vec::is_empty).unwrap_or(false),
        "несуществующая роль → пустой список, получено: {users}"
    );
}

#[tokio::test]
async fn users_by_role_9d_foreign_company_role_returns_empty_list() {
    let (h, db, _cache) = host().await;
    h.load_module("hello", HELLO_WASM).await.unwrap();

    seed_role_and_users(&db).await;
    // Ищем по реальной роли, но в чужой компании — роль не должна быть видна.
    let role = seed_role(&db, &Uuid::new_v4().to_string()).await;
    h.set_call_context(HostCallCtx {
        module_code: "hello".to_string(),
        company_id: Uuid::new_v4().to_string(),
        actor: None,
        capabilities: HashSet::from(["notifications".to_string()]),
        settings: Value::Null,
    })
    .await;

    let users = users_payload(&h, role.id).await;
    assert!(
        users.as_array().map(Vec::is_empty).unwrap_or(false),
        "роль чужой компании → пустой список, получено: {users}"
    );
}

// ---------------------------------------------------------------------------
// Подфаза 9d: транзакционная оркестрация (tx_begin/tx_add_op/tx_commit).
// ---------------------------------------------------------------------------

use core_domain::object::{Object, ObjectKind};

/// Регистрирует тип сущности `document` (проводка: draft → posted/cancelled)
/// в компании `comp1` и возвращает его id для объектов подфазы 9d.
async fn seed_document_schema(db: &Surreal<Any>) -> Uuid {
    let metadata = SurrealMetadataRepository::new(db.clone());
    let s = |code: &str, label: &str, initial: bool| EntityState {
        id: Uuid::new_v4(),
        entity_type: "document".to_string(),
        code: code.to_string(),
        label: label.to_string(),
        color: None,
        is_initial: initial,
        is_final: code == "posted" || code == "cancelled",
    };
    let entity_type = EntityType {
        id: Uuid::new_v4(),
        code: "document".to_string(),
        name: "Документ".to_string(),
        kind: EntityKind::Document,
        company_id: "comp1".to_string(),
        metadata_version: 1,
        is_system: false,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    let schema = EntitySchema {
        entity_type: entity_type.clone(),
        fields: vec![EntityField {
            id: Uuid::new_v4(),
            entity_type: "document".to_string(),
            code: "sum".to_string(),
            label: "Сумма".to_string(),
            data_type: FieldType::Money,
            required: false,
            is_unique: false,
            is_indexed: false,
            options: Value::Null,
            is_system: false,
            order: 1,
        }],
        states: vec![s("draft", "Черновик", true), s("posted", "Проведён", false), s("cancelled", "Отменён", false)],
        transitions: vec![],
        forms: vec![],
        actions: vec![],
        relations: vec![],
    };
    metadata.create_entity_type(&schema, &[]).await.unwrap();
    entity_type.id
}

/// Создаёт объект `document` состояния `draft` v1 в компании `comp1`.
async fn create_document(db: &Surreal<Any>, entity_type: &str) -> Object {
    let objects = SurrealObjectRepository::new(db.clone());
    let obj = Object {
        id: Uuid::new_v4(),
        entity_type: entity_type.to_string(),
        kind: ObjectKind::Document,
        company_id: "comp1".to_string(),
        state: "draft".to_string(),
        data: json!({ "sum": 100 }),
        computed: Value::Null,
        number: None,
        date: None,
        parent_id: None,
        version: 1,
        created_by: "system".to_string(),
        updated_by: "system".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    objects
        .create(
            &obj,
            &[Event {
                id: Uuid::new_v4(),
                stream_type: StreamType::Object,
                stream_id: obj.id.to_string(),
                event_type: "object.created".to_string(),
                version: 0,
                payload: json!(obj),
                metadata: ActorSnapshot::system(),
                company_id: "comp1".to_string(),
                correlation_id: "corr-tx".to_string(),
                causation_id: None,
                occurred_at: Utc::now(),
            }],
        )
        .await
        .unwrap();
    obj
}

/// Извлекает JSON-конверт из вывода экспорта: `outer` — весь вывод, `name` —
/// ключ с сырым конвертом (строкой). Возвращает распарсенный конверт.
fn export_conv(outer: &Value, name: &str) -> Value {
    serde_json::from_str::<Value>(outer[name].as_str().expect(name)).expect(name)
}

fn conv_err_code(conv: &Value) -> String {
    conv["error"]["code"].as_str().unwrap_or_default().to_string()
}

fn assert_conv_ok(conv: &Value, what: &str) {
    assert!(
        conv["ok"] == Value::Bool(true),
        "конверт {what} должен быть ok, получено: {conv}"
    );
}

#[tokio::test]
async fn tx_9d_commit_posts_object_atomically() {
    let (h, db, _cache) = host().await;
    h.load_module("hello", HELLO_WASM).await.unwrap();
    h.set_call_context(HostCallCtx {
        module_code: "hello".to_string(),
        company_id: "comp1".to_string(),
        actor: None,
        capabilities: HashSet::from(["transactions".to_string()]),
        settings: Value::Null,
    })
    .await;

    seed_document_schema(&db).await;
    let obj = create_document(&db, "document").await;

    let out = h
        .call_function(
            "hello",
            "tx_probe",
            json!({ "object_id": obj.id, "expected_version": 1 })
                .to_string()
                .as_bytes(),
        )
        .await
        .unwrap();
    let parsed: Value = serde_json::from_str(&String::from_utf8(out).unwrap()).unwrap();

    let begin = export_conv(&parsed, "begin");
    assert_conv_ok(&begin, "tx_begin");
    assert_eq!(begin["data"]["operations_count"], json!(0));
    let add_op = export_conv(&parsed, "add_op");
    assert_conv_ok(&add_op, "tx_add_op");
    let commit = export_conv(&parsed, "commit");
    assert_conv_ok(&commit, "tx_commit");
    assert_eq!(commit["data"]["committed"], json!(true));

    let objects = SurrealObjectRepository::new(db.clone());
    let posted = objects.get(&obj.id).await.unwrap();
    assert_eq!(posted.state, "posted", "объект должен стать проведённым");
    assert_eq!(posted.version, 2, "версия должна вырасти до 2");
}

#[tokio::test]
async fn tx_9d_ref_binding_resolves_params_from_previous_op() {
    let (h, db, _cache) = host().await;
    h.load_module("hello", HELLO_WASM).await.unwrap();
    h.set_call_context(HostCallCtx {
        module_code: "hello".to_string(),
        company_id: "comp1".to_string(),
        actor: None,
        capabilities: HashSet::from(["transactions".to_string()]),
        settings: Value::Null,
    })
    .await;

    seed_document_schema(&db).await;
    let obj = create_document(&db, "document").await;

    let out = h
        .call_function(
            "hello",
            "tx_ref_probe",
            json!({ "object_id": obj.id, "expected_version": 1 })
                .to_string()
                .as_bytes(),
        )
        .await
        .unwrap();
    let parsed: Value = serde_json::from_str(&String::from_utf8(out).unwrap()).unwrap();

    assert_conv_ok(&export_conv(&parsed, "begin"), "tx_begin");
    let noop = export_conv(&parsed, "noop");
    assert_conv_ok(&noop, "test.noop");
    assert!(noop["data"]["op_id"].is_string());
    assert_conv_ok(&export_conv(&parsed, "add_op"), "tx_add_op");
    assert_conv_ok(&export_conv(&parsed, "commit"), "tx_commit");

    let objects = SurrealObjectRepository::new(db.clone());
    let posted = objects.get(&obj.id).await.unwrap();
    assert_eq!(posted.state, "posted", "$ref → object.post должен провести объект");
    assert_eq!(posted.version, 2);
}

#[tokio::test]
async fn tx_9d_idempotent_begin_returns_same_handle() {
    let (h, _db, _cache) = host().await;
    h.load_module("hello", HELLO_WASM).await.unwrap();
    h.set_call_context(HostCallCtx {
        module_code: "hello".to_string(),
        company_id: "comp1".to_string(),
        actor: None,
        capabilities: HashSet::from(["transactions".to_string()]),
        settings: Value::Null,
    })
    .await;

    let out = h
        .call_function("hello", "tx_idem_probe", b"")
        .await
        .unwrap();
    let parsed: Value = serde_json::from_str(&String::from_utf8(out).unwrap()).unwrap();

    let first = export_conv(&parsed, "first");
    let second = export_conv(&parsed, "second");
    assert_conv_ok(&first, "первый tx_begin");
    assert_conv_ok(&second, "второй tx_begin");
    assert_conv_ok(&export_conv(&parsed, "noop"), "noop между begin");

    assert_eq!(
        first["data"]["handle"], second["data"]["handle"],
        "повторный begin с тем же business_key должен вернуть тот же handle"
    );
    assert_eq!(first["data"]["operations_count"], json!(0));
    assert_eq!(
        second["data"]["operations_count"], json!(1),
        "при повторе пачки оркестратор должен сообщить о добавленной операции"
    );
}

#[tokio::test]
async fn tx_9d_requires_transactions_capability() {
    let (h, _db, _cache) = host().await;
    h.load_module("hello", HELLO_WASM).await.unwrap();
    h.set_call_context(HostCallCtx {
        module_code: "hello".to_string(),
        company_id: "comp1".to_string(),
        actor: None,
        capabilities: HashSet::from(["objects.read".to_string()]),
        settings: Value::Null,
    })
    .await;

    let out = h
        .call_function(
            "hello",
            "tx_probe",
            json!({ "object_id": Uuid::new_v4(), "expected_version": 1 })
                .to_string()
                .as_bytes(),
        )
        .await
        .unwrap();
    let parsed: Value = serde_json::from_str(&String::from_utf8(out).unwrap()).unwrap();

    let begin = export_conv(&parsed, "begin");
    assert_eq!(
        begin["ok"], Value::Bool(false),
        "begin без capability transactions должен отклоняться"
    );
    assert_eq!(conv_err_code(&begin), "CAPABILITY_DENIED");
}

#[tokio::test]
async fn tx_9d_stale_version_conflicts_at_commit() {
    let (h, db, _cache) = host().await;
    h.load_module("hello", HELLO_WASM).await.unwrap();
    h.set_call_context(HostCallCtx {
        module_code: "hello".to_string(),
        company_id: "comp1".to_string(),
        actor: None,
        capabilities: HashSet::from(["transactions".to_string()]),
        settings: Value::Null,
    })
    .await;

    seed_document_schema(&db).await;
    let obj = create_document(&db, "document").await;

    // Внешний писатель доводит объект до v2 до транзакции.
    let objects = SurrealObjectRepository::new(db.clone());
    let mut moved = obj.clone();
    moved.state = "posted".to_string();
    objects
        .update(
            &moved,
            &[Event {
                id: Uuid::new_v4(),
                stream_type: StreamType::Object,
                stream_id: obj.id.to_string(),
                event_type: "object.posted".to_string(),
                version: 0,
                payload: json!(moved),
                metadata: ActorSnapshot::system(),
                company_id: "comp1".to_string(),
                correlation_id: "corr-ext".to_string(),
                causation_id: None,
                occurred_at: Utc::now(),
            }],
        )
        .await
        .unwrap();

    let out = h
        .call_function(
            "hello",
            "tx_probe",
            json!({ "object_id": obj.id, "expected_version": 1 })
                .to_string()
                .as_bytes(),
        )
        .await
        .unwrap();
    let parsed: Value = serde_json::from_str(&String::from_utf8(out).unwrap()).unwrap();

    let commit = export_conv(&parsed, "commit");
    assert_eq!(
        commit["ok"], Value::Bool(false),
        "commit при устаревшей версии должен откатиться и вернуть ошибку"
    );
    assert_eq!(conv_err_code(&commit), "CONFLICT_ERROR");

    let after = objects.get(&obj.id).await.unwrap();
    assert_eq!(after.version, 2, "неудачная транзакция не должна менять объект");
    assert_eq!(after.state, "posted");
}
