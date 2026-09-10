//! Транзакционная оркестрация из WASM-модулей (подфаза 9d, раздел 9 ТЗ v3.1).
//!
//! `TransactionOrchestrator` держит активные пачки операций модуля
//! (`tx_begin`/`tx_add_op`/`tx_commit`) с идемпотентностью по `business_key`,
//! `$ref`-связыванием результатов предыдущих операций и атомарным коммитом
//! всей пачки одним вызовом `ObjectRepository::update_batch`. Ошибка любой
//! операции откатывает пачку целиком, а handle транзакции остаётся активным
//! для повтора; протухшие пачки вычищаются фоновым сборщиком (TTL 5 минут).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use core_domain::error::DomainError;
use core_domain::event::{ActorSnapshot, Event, StreamType};
use core_domain::object::Object;
use serde_json::{json, Map, Value};
use tokio::runtime::Handle;
use tokio::sync::RwLock;
use tokio::time::MissedTickBehavior;
use uuid::Uuid;

use crate::ports::ObjectRepository;

/// TTL активной транзакции: раньше пачки считаются «зависшими» и вычищаются GC.
const TX_TTL: Duration = Duration::from_secs(5 * 60);
/// Период обхода сборщика мусора активных транзакций.
const TX_GC_INTERVAL: Duration = Duration::from_secs(60);
/// Максимальная глубина вложенности `$ref`-резолвинга.
const MAX_REF_DEPTH: usize = 10;

/// Статус транзакции.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionStatus {
    Active,
    Committed,
    RolledBack,
}

/// Ожидающая операция пачки: тип, параметры и вычисленный результат
/// (заполняется после успешного выполнения для `$ref`-ссылок).
#[derive(Debug, Clone)]
pub struct PendingOperation {
    pub op_id: Uuid,
    pub op_type: String,
    pub params: Value,
    pub result: Option<Value>,
}

/// Активная транзакция модуля. `company_id` и `actor` фиксируются в момент
/// `begin`; при коммите объекты обязаны принадлежать этой компании.
#[derive(Debug, Clone)]
pub struct ActiveTransaction {
    pub handle: Uuid,
    pub business_key: String,
    pub company_id: String,
    pub actor: Option<ActorSnapshot>,
    pub operations: Vec<PendingOperation>,
    pub status: TransactionStatus,
    pub created_at: DateTime<Utc>,
}

/// Отчёт об очистке протухших транзакций (для логов и тестов).
#[derive(Debug, Clone, Default)]
pub struct GcReport {
    pub pruned: usize,
    pub remaining: usize,
}

/// Оркестратор транзакций WASM-модулей.
pub struct TransactionOrchestrator {
    active_transactions: RwLock<HashMap<Uuid, ActiveTransaction>>,
    object_repo: Arc<dyn ObjectRepository>,
}

impl TransactionOrchestrator {
    /// Создаёт оркестратор и, если вызван внутри tokio-runtime, запускает
    /// фоновую задачу сборщика протухших транзакций (60 c, TTL 5 минут).
    /// Вне runtime GC пропускается — его можно отработать вручную через
    /// `prune_expired`.
    pub fn new(object_repo: Arc<dyn ObjectRepository>) -> Arc<Self> {
        let orch = Arc::new(Self {
            active_transactions: RwLock::new(HashMap::new()),
            object_repo,
        });
        if let Ok(handle) = Handle::try_current() {
            let weak = Arc::downgrade(&orch);
            handle.spawn(async move {
                let mut ticker = tokio::time::interval(TX_GC_INTERVAL);
                ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
                loop {
                    ticker.tick().await;
                    match weak.upgrade() {
                        Some(orch) => {
                            orch.prune_expired().await;
                        }
                        None => break,
                    }
                }
            });
        }
        orch
    }

    /// Удаляет транзакции, созданные раньше `now - TX_TTL`; возвращает отчёт.
    /// Публичен для тестов и явного вызова в дополнение к фоновому GC.
    pub async fn prune_expired(&self) -> GcReport {
        let cutoff = Utc::now() - chrono::Duration::from_std(TX_TTL).unwrap_or_default();
        let mut guard = self.active_transactions.write().await;
        let before = guard.len();
        guard.retain(|_, tx| tx.created_at >= cutoff);
        GcReport {
            pruned: before - guard.len(),
            remaining: guard.len(),
        }
    }

    /// Открывает новую транзакцию. При непустом `business_key` возвращает ту же
    /// пачку, если такая (компания + business_key + исполнитель) уже активна —
    /// тогда второй элемент кортежа содержит число уже добавленных операций.
    /// Пустой `business_key` идемпотентностью не обладает: каждый `begin`
    /// создаёт новую пачку.
    pub async fn begin(
        &self,
        business_key: &str,
        company_id: &str,
        actor: Option<ActorSnapshot>,
    ) -> Result<(Uuid, usize), DomainError> {
        if !business_key.is_empty() {
            let guard = self.active_transactions.read().await;
            for tx in guard.values() {
                let same_actor = match (&actor, &tx.actor) {
                    (Some(a), Some(b)) => a.login == b.login,
                    (None, None) => true,
                    _ => false,
                };
                if tx.business_key == business_key
                    && tx.company_id == company_id
                    && tx.status == TransactionStatus::Active
                    && same_actor
                {
                    return Ok((tx.handle, tx.operations.len()));
                }
            }
        }
        let tx = ActiveTransaction {
            handle: Uuid::new_v4(),
            business_key: business_key.to_string(),
            company_id: company_id.to_string(),
            actor,
            operations: Vec::new(),
            status: TransactionStatus::Active,
            created_at: Utc::now(),
        };
        let handle = tx.handle;
        self.active_transactions.write().await.insert(handle, tx);
        Ok((handle, 0))
    }

    /// Добавляет операцию в пачку. `op_type` валидируется сразу: допустимы
    /// `object.post`, `object.cancel` и `test.noop`; неизвестный тип отклоняется
    /// немедленно (`DomainError::ValidationError` → `INVALID_ACTION`).
    ///
    /// # Errors
    ///
    /// `ValidationError`, если handle неизвестен/не активен или `op_type`
    /// не входит в допустимый набор.
    pub async fn add_op(
        &self,
        handle: Uuid,
        op_type: &str,
        params: Value,
    ) -> Result<Uuid, DomainError> {
        if !matches!(op_type, "object.post" | "object.cancel" | "test.noop") {
            return Err(DomainError::ValidationError(format!(
                "Неизвестная операция транзакции '{op_type}'"
            )));
        }
        let mut guard = self.active_transactions.write().await;
        let tx = guard.get_mut(&handle).ok_or_else(|| {
            DomainError::ValidationError(format!("Транзакция {handle} не найдена"))
        })?;
        if tx.status != TransactionStatus::Active {
            return Err(DomainError::ValidationError(format!(
                "Транзакция {handle} уже завершена"
            )));
        }
        let op = PendingOperation {
            op_id: Uuid::new_v4(),
            op_type: op_type.to_string(),
            params,
            result: None,
        };
        let op_id = op.op_id;
        tx.operations.push(op);
        Ok(op_id)
    }

    /// Коммитит пачку: последовательно резолвит `$ref` и выполняет операции,
    /// после чего атомарно записывает все изменения объектов одним вызовом
    /// `update_batch`. При ошибке любой операции пачка откатывается целиком,
    /// а handle остаётся активным в карте для повтора.
    ///
    /// # Errors
    ///
    /// `ValidationError` для неизвестного handle, неверных параметров операции,
    /// `$ref`-ссылки или объекта не из компании транзакции; `NotFound` для
    /// отсутствующего объекта; `VersionConflict` — конфликт версий OCC.
    pub async fn commit(&self, handle: Uuid) -> Result<(), DomainError> {
        let mut guard = self.active_transactions.write().await;
        let mut tx = match guard.remove(&handle) {
            Some(tx) => tx,
            None => {
                return Err(DomainError::ValidationError(format!(
                    "Транзакция {handle} не найдена"
                )));
            }
        };

        let (mut tx, outcome) = async move {
            let outcome: Result<(), DomainError> = async {
                let mut batch: Vec<(Object, Vec<Event>)> = Vec::new();
                let mut idx = 0;
                while idx < tx.operations.len() {
                    let params =
                        self.resolve_refs(tx.operations[idx].params.clone(), &tx.operations, 0)?;
                    let result = match tx.operations[idx].op_type.as_str() {
                        "test.noop" => json!({
                            "ok": true,
                            "params": params,
                            "noop_id": tx.operations[idx].op_id,
                        }),
                        "object.post" | "object.cancel" => {
                            let target = if tx.operations[idx].op_type == "object.post" {
                                "posted"
                            } else {
                                "cancelled"
                            };
                            let event_type = if target == "posted" {
                                "object.posted"
                            } else {
                                "object.cancelled"
                            };
                            let (obj, event) =
                                self.prepare_object_op(&tx, &params, target, event_type).await?;
                            let result = json!({
                                "id": obj.id,
                                "version": obj.version + 1,
                                "state": target,
                            });
                            batch.push((obj, vec![event]));
                            result
                        }
                        _ => unreachable!("op_type валидирован в add_op"),
                    };
                    tx.operations[idx].result = Some(result);
                    idx += 1;
                }

                if !batch.is_empty() {
                    self.object_repo.update_batch(&batch).await?;
                }
                Ok(())
            }
            .await;
            if outcome.is_ok() {
                tx.status = TransactionStatus::Committed;
            }
            (tx, outcome)
        }
        .await;

        match outcome {
            Ok(()) => Ok(()),
            Err(e) => {
                tx.status = TransactionStatus::Active;
                guard.insert(handle, tx);
                Err(e)
            }
        }
    }

    /// Готовит объект для операции `object.post`/`object.cancel`: проверяет
    /// принадлежность компании транзакции, применяет целевое состояние и
    /// формирует событие «Трубы».
    async fn prepare_object_op(
        &self,
        tx: &ActiveTransaction,
        params: &Value,
        target: &str,
        event_type: &str,
    ) -> Result<(Object, Event), DomainError> {
        let id = params.get("id").and_then(Value::as_str).ok_or_else(|| {
            DomainError::ValidationError(format!("Операция {target}: нет обязательного поля 'id'"))
        })?;
        let id = Uuid::parse_str(id).map_err(|_| {
            DomainError::ValidationError(format!("Операция {target}: 'id' не является UUID"))
        })?;
        let expected_version = params.get("expected_version").and_then(Value::as_u64);

        let current = self
            .object_repo
            .get(&id)
            .await
            .map_err(|_| DomainError::NotFound(format!("Объект {id} не найден")))?;
        if current.company_id != tx.company_id {
            return Err(DomainError::ValidationError(format!(
                "Объект {id} не принадлежит компании {} транзакции",
                tx.company_id
            )));
        }
        let version = expected_version.unwrap_or(current.version);
        let mut obj = current.clone();
        obj.state = target.to_string();
        obj.version = version;
        obj.updated_by = tx
            .actor
            .as_ref()
            .map(|a| a.login.clone())
            .unwrap_or_else(|| "system".to_string());
        obj.updated_at = Utc::now();

        let event = Event {
            id: Uuid::new_v4(),
            stream_type: StreamType::Object,
            stream_id: obj.id.to_string(),
            event_type: event_type.to_string(),
            version: 0,
            payload: json!(obj),
            metadata: tx.actor.clone().unwrap_or_else(ActorSnapshot::system),
            company_id: tx.company_id.clone(),
            correlation_id: Uuid::new_v4().to_string(),
            causation_id: None,
            occurred_at: Utc::now(),
        };
        Ok((obj, event))
    }

    /// Резолвит `$ref`-ссылки вида `{"$ref": "<op_id>.<dot.path>"}` в значение
    /// из результата ранее выполненной операции. Заменяется весь узел, после
    /// чего резолвинг рекурсивно продолжается внутри подставленного значения.
    ///
    /// # Errors
    ///
    /// `ValidationError` при превышении `MAX_REF_DEPTH`, некорректной ссылке
    /// на отсутствующую операцию, поле или ещё не выполненную операцию.
    fn resolve_refs(
        &self,
        value: Value,
        ops: &[PendingOperation],
        depth: usize,
    ) -> Result<Value, DomainError> {
        if depth > MAX_REF_DEPTH {
            return Err(DomainError::ValidationError(
                "Превышен лимит глубины $ref-резолвинга".to_string(),
            ));
        }
        match value {
            Value::Object(map) => {
                if let Some(Value::String(raw)) = map.get("$ref") {
                    let (op_str, path) = raw.split_once('.').ok_or_else(|| {
                        DomainError::ValidationError(format!(
                            "Некорректная $ref-ссылка '{raw}': ожидается '<op_id>.<путь>'"
                        ))
                    })?;
                    let op_id = Uuid::parse_str(op_str).map_err(|_| {
                        DomainError::ValidationError(format!(
                            "Некорректная $ref-ссылка '{raw}': '{op_str}' не UUID"
                        ))
                    })?;
                    let op = ops.iter().find(|o| o.op_id == op_id).ok_or_else(|| {
                        DomainError::ValidationError(format!(
                            "$ref ссылается на отсутствующую операцию '{op_str}'"
                        ))
                    })?;
                    let mut cur = op.result.clone().ok_or_else(|| {
                        DomainError::ValidationError(format!(
                            "$ref ссылается на ещё не выполненную операцию '{op_str}'"
                        ))
                    })?;
                    for part in path.split('.') {
                        cur = cur.get(part).cloned().ok_or_else(|| {
                            DomainError::ValidationError(format!(
                                "$ref: поле '{part}' не найдено в результате операции '{op_str}'"
                            ))
                        })?;
                    }
                    return self.resolve_refs(cur, ops, depth + 1);
                }
                let mut out = Map::new();
                for (k, v) in map {
                    out.insert(k, self.resolve_refs(v, ops, depth + 1)?);
                }
                Ok(Value::Object(out))
            }
            Value::Array(arr) => {
                let mut out = Vec::with_capacity(arr.len());
                for v in arr {
                    out.push(self.resolve_refs(v, ops, depth + 1)?);
                }
                Ok(Value::Array(out))
            }
            other => Ok(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::BoxFuture;
    use core_domain::object::{ObjectKind, ObjectSnapshot};

    /// Памятный репозиторий объектов для тестов оркестратора: имитирует OCC
    /// `update_batch` (сбой при несовпадении версий) и `get`.
    struct MemObjectRepo {
        objects: RwLock<HashMap<Uuid, Object>>,
    }

    impl MemObjectRepo {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                objects: RwLock::new(HashMap::new()),
            })
        }

        async fn seed(&self, obj: Object) {
            self.objects.write().await.insert(obj.id, obj);
        }
    }

    impl ObjectRepository for MemObjectRepo {
        fn get_with_version(
            &self,
            id: &Uuid,
        ) -> BoxFuture<'_, Result<(Object, u64), DomainError>> {
            let id = *id;
            Box::pin(async move {
                let obj = self
                    .objects
                    .read()
                    .await
                    .get(&id)
                    .cloned()
                    .ok_or_else(|| DomainError::NotFound(format!("{id} not found")))?;
                Ok((obj.clone(), obj.version))
            })
        }
        fn get(&self, id: &Uuid) -> BoxFuture<'_, Result<Object, DomainError>> {
            let id = *id;
            Box::pin(async move {
                self.objects
                    .read()
                    .await
                    .get(&id)
                    .cloned()
                    .ok_or_else(|| DomainError::NotFound(format!("{id} not found")))
            })
        }
        fn create(
            &self,
            _obj: &Object,
            _events: &[Event],
        ) -> BoxFuture<'_, Result<Object, DomainError>> {
            Box::pin(async move {
                Err(DomainError::ValidationError("not used".to_string()))
            })
        }
        fn update(
            &self,
            _obj: &Object,
            _events: &[Event],
        ) -> BoxFuture<'_, Result<Object, DomainError>> {
            Box::pin(async move {
                Err(DomainError::ValidationError("not used".to_string()))
            })
        }
        fn update_batch(
            &self,
            ops: &[(Object, Vec<Event>)],
        ) -> BoxFuture<'_, Result<Vec<Object>, DomainError>> {
            let ops = ops.to_vec();
            Box::pin(async move {
                let mut guard = self.objects.write().await;
                for (obj, _events) in &ops {
                    let current = guard.get(&obj.id).ok_or_else(|| {
                        DomainError::NotFound(format!("{} not found", obj.id))
                    })?;
                    if obj.version != current.version {
                        return Err(DomainError::VersionConflict {
                            expected: obj.version,
                            actual: current.version,
                        });
                    }
                }
                let mut stored = Vec::new();
                for (obj, _events) in &ops {
                    let mut current = guard.get(&obj.id).cloned().unwrap();
                    current.version += 1;
                    current.state = obj.state.clone();
                    current.updated_by = obj.updated_by.clone();
                    current.updated_at = obj.updated_at;
                    guard.insert(obj.id, current.clone());
                    stored.push(current);
                }
                Ok(stored)
            })
        }
        fn delete(&self, _id: &Uuid, _events: &[Event]) -> BoxFuture<'_, Result<(), DomainError>> {
            Box::pin(async move {
                Err(DomainError::ValidationError("not used".to_string()))
            })
        }
        fn list(
            &self,
            _entity_type: &str,
            _company_id: &str,
            _limit: usize,
        ) -> BoxFuture<'_, Result<Vec<Object>, DomainError>> {
            Box::pin(async move {
                Err(DomainError::ValidationError("not used".to_string()))
            })
        }
        fn count(
            &self,
            _entity_type: &str,
            _company_id: &str,
        ) -> BoxFuture<'_, Result<u64, DomainError>> {
            Box::pin(async move {
                Err(DomainError::ValidationError("not used".to_string()))
            })
        }
        fn get_snapshots(
            &self,
            _object_id: &Uuid,
        ) -> BoxFuture<'_, Result<Vec<ObjectSnapshot>, DomainError>> {
            Box::pin(async move {
                Err(DomainError::ValidationError("not used".to_string()))
            })
        }
        fn restore_snapshot(
            &self,
            _object_id: &Uuid,
            _version: u64,
            _events: &[Event],
        ) -> BoxFuture<'_, Result<Object, DomainError>> {
            Box::pin(async move {
                Err(DomainError::ValidationError("not used".to_string()))
            })
        }
        fn next_document_number(
            &self,
            _entity_type: &str,
            _company_id: &str,
        ) -> BoxFuture<'_, Result<String, DomainError>> {
            Box::pin(async move {
                Err(DomainError::ValidationError("not used".to_string()))
            })
        }
    }

    fn sample_object(id: Uuid, company: &str, version: u64, state: &str) -> Object {
        Object {
            id,
            entity_type: "invoice".to_string(),
            kind: ObjectKind::Document,
            company_id: company.to_string(),
            state: state.to_string(),
            data: json!({"sum": 100}),
            computed: json!({}),
            number: None,
            date: None,
            parent_id: None,
            version,
            created_by: "system".to_string(),
            updated_by: "system".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn op(op_id: Uuid, result: Value) -> PendingOperation {
        PendingOperation {
            op_id,
            op_type: "test.noop".to_string(),
            params: json!({}),
            result: Some(result),
        }
    }

    #[tokio::test]
    async fn begin_add_op_commit_posts_object_in_batch() {
        let repo = MemObjectRepo::new();
        let obj = sample_object(Uuid::new_v4(), "c1", 1, "draft");
        repo.seed(obj.clone()).await;
        let orch = TransactionOrchestrator::new(repo);

        let (handle, count) = orch.begin("bk-1", "c1", None).await.unwrap();
        assert_eq!(count, 0);
        let _ = orch
            .add_op(
                handle,
                "object.post",
                json!({"id": obj.id, "expected_version": 1}),
            )
            .await
            .unwrap();
        orch.commit(handle).await.unwrap();

        let stored = orch.object_repo.get(&obj.id).await.unwrap();
        assert_eq!(stored.version, 2);
        assert_eq!(stored.state, "posted");
    }

    #[tokio::test]
    async fn begin_with_same_business_key_is_idempotent() {
        let repo = MemObjectRepo::new();
        let orch = TransactionOrchestrator::new(repo);

        let (h1, c1) = orch.begin("bk-x", "c1", None).await.unwrap();
        let _ = orch.add_op(h1, "test.noop", json!({"a": 1})).await.unwrap();
        let (h2, c2) = orch.begin("bk-x", "c1", None).await.unwrap();
        assert_eq!(h1, h2);
        assert_eq!(c1, 0);
        assert_eq!(c2, 1, "retry должен вернуть число уже добавленных операций");

        let (h3, c3) = orch.begin("bk-y", "c1", None).await.unwrap();
        assert_ne!(h1, h3);
        assert_eq!(c3, 0);

        let (h4, _) = orch.begin("", "c1", None).await.unwrap();
        let (h5, _) = orch.begin("", "c1", None).await.unwrap();
        assert_ne!(h4, h5);
    }

    #[tokio::test]
    async fn unknown_op_type_rejected_immediately() {
        let repo = MemObjectRepo::new();
        let orch = TransactionOrchestrator::new(repo);
        let (handle, _) = orch.begin("bk-2", "c1", None).await.unwrap();
        let err = orch
            .add_op(handle, "accounting.post", json!({}))
            .await
            .unwrap_err();
        assert!(matches!(err, DomainError::ValidationError(_)));
    }

    #[tokio::test]
    async fn noop_result_echoes_params_for_ref() {
        let repo = MemObjectRepo::new();
        let orch = TransactionOrchestrator::new(repo);
        let (handle, _) = orch.begin("bk-3", "c1", None).await.unwrap();
        let op1 = orch
            .add_op(handle, "test.noop", json!({"target_id": "uuid-123"}))
            .await
            .unwrap();
        let op2 = orch
            .add_op(
                handle,
                "test.noop",
                json!({"$ref": format!("{op1}.params.target_id")}),
            )
            .await
            .unwrap();
        assert_ne!(op1, op2);
        orch.commit(handle).await.unwrap();
    }

    #[tokio::test]
    async fn ref_to_unknown_operation_fails_and_keeps_handle_active() {
        let repo = MemObjectRepo::new();
        let orch = TransactionOrchestrator::new(repo);
        let (handle, _) = orch.begin("bk-5", "c1", None).await.unwrap();
        let ghost = Uuid::new_v4();
        let _ = orch
            .add_op(
                handle,
                "test.noop",
                json!({"$ref": format!("{ghost}.params.x")}),
            )
            .await
            .unwrap();
        let err = orch.commit(handle).await.unwrap_err();
        assert!(matches!(err, DomainError::ValidationError(_)), "{err:?}");

        let count = orch
            .active_transactions
            .read()
            .await
            .get(&handle)
            .map(|t| t.operations.len())
            .unwrap_or(0);
        assert_eq!(count, 1, "handle должен остаться активным после ошибки");
    }

    #[tokio::test]
    async fn ref_resolves_nested_path_and_depth_limit() {
        let repo = MemObjectRepo::new();
        let orch = TransactionOrchestrator::new(repo);

        let id_a: Uuid = "00000000-0000-0000-0000-00000000000a".parse().unwrap();
        let operations = vec![op(id_a, json!({"b": {"c": 42}}))];

        let resolved = orch
            .resolve_refs(json!({"$ref": format!("{id_a}.b.c")}), &operations, 0)
            .unwrap();
        assert_eq!(resolved, json!(42));

        let err = orch
            .resolve_refs(json!({"$ref": format!("{id_a}.b.missing")}), &operations, 0)
            .unwrap_err();
        assert!(matches!(err, DomainError::ValidationError(_)));

        let ghost: Uuid = "00000000-0000-0000-0000-0000000000ff".parse().unwrap();
        let err = orch
            .resolve_refs(json!({"$ref": format!("{ghost}.b")}), &operations, 0)
            .unwrap_err();
        assert!(matches!(err, DomainError::ValidationError(_)));

        let deep = json!({"a":{"a":{"a":{"a":{"a":{"a":{"a":{"a":{"a":{"a":{"a":1}}}}}}}}}}});
        let err = orch.resolve_refs(deep, &operations, 11).unwrap_err();
        assert!(matches!(err, DomainError::ValidationError(_)));

        assert_eq!(
            orch.resolve_refs(json!({"a": [1, {"$ref": format!("{id_a}.b")}]}), &operations, 0)
                .unwrap(),
            json!({"a": [1, {"c": 42}]})
        );
    }

    #[tokio::test]
    async fn post_for_foreign_company_is_rejected() {
        let repo = MemObjectRepo::new();
        let obj = sample_object(Uuid::new_v4(), "other-co", 1, "draft");
        repo.seed(obj.clone()).await;
        let orch = TransactionOrchestrator::new(repo);
        let (handle, _) = orch.begin("bk-6", "c1", None).await.unwrap();
        let _ = orch
            .add_op(handle, "object.post", json!({"id": obj.id}))
            .await
            .unwrap();
        let err = orch.commit(handle).await.unwrap_err();
        assert!(matches!(err, DomainError::ValidationError(_)), "{err:?}");
    }

    #[tokio::test]
    async fn stale_version_conflicts_and_keeps_handle_active() {
        let repo = MemObjectRepo::new();
        let obj = sample_object(Uuid::new_v4(), "c1", 1, "draft");
        repo.seed(obj.clone()).await;
        let orch = TransactionOrchestrator::new(repo.clone());

        let (handle, _) = orch.begin("bk-7", "c1", None).await.unwrap();
        // Внешний писатель доводит объект до v2.
        {
            let mut guard = repo.objects.write().await;
            let mut cur = guard.get(&obj.id).cloned().unwrap();
            cur.version = 2;
            guard.insert(obj.id, cur);
        }

        let _ = orch
            .add_op(
                handle,
                "object.post",
                json!({"id": obj.id, "expected_version": 1}),
            )
            .await
            .unwrap();
        let err = orch.commit(handle).await.unwrap_err();
        assert!(
            matches!(err, DomainError::VersionConflict { .. }),
            "ожидали конфликт версий, получено: {err:?}"
        );

        let still_active = orch
            .active_transactions
            .read()
            .await
            .contains_key(&handle);
        assert!(still_active, "handle должен остаться активным после конфликта");
    }
}