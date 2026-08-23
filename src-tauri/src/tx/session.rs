//! Сессия-строитель пачек для WASM-плагинов.
//!
//! Через границу песочницы нельзя передать &mut TransactionPackage,
//! поэтому плагин собирает пачку тремя вызовами:
//!   tx_begin(business_key) -> handle
//!   tx_add_op(handle, op, params_json) -> op_id  (op_id раздаёт ядро)
//!   tx_commit(handle) -> материализованный TransactionPackage → исполнитель
//!
//! Mongo-транзакция открывается только на tx_commit и живёт ровно
//! на время выполнения. Состояние сессии короткоживущее: протухшие
//! сессии вычищаются при каждом begin.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::core::CompanyId;
use crate::events::ActorSnapshot;

use super::{TransactionPackage, TxOperation};

/// Сессия считается брошенной после этого срока.
const SESSION_TTL: Duration = Duration::from_secs(600);

/// Накопленная пачка до коммита.
pub struct PendingTx {
    pub business_key: String,
    pub company_id: CompanyId,
    pub actor: ActorSnapshot,
    /// role_id строкой — политики загружаются на коммите.
    pub role_id: Option<String>,
    pub operations: Vec<TxOperation>,
    created_at: Instant,
}

static SESSIONS: OnceLock<Mutex<HashMap<String, PendingTx>>> = OnceLock::new();

fn sessions() -> &'static Mutex<HashMap<String, PendingTx>> {
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Начать сборку пачки. Возвращает handle.
/// Заодно вычищает брошенные сессии.
pub fn begin(
    business_key: impl Into<String>,
    company_id: CompanyId,
    actor: ActorSnapshot,
    role_id: Option<String>,
) -> String {
    let handle = format!("tx-{}", uuid::Uuid::new_v4());

    let mut map = sessions().lock().unwrap();
    let now = Instant::now();
    map.retain(|_, pending| now.duration_since(pending.created_at) < SESSION_TTL);

    map.insert(
        handle.clone(),
        PendingTx {
            business_key: business_key.into(),
            company_id,
            actor,
            role_id,
            operations: Vec::new(),
            created_at: now,
        },
    );
    handle
}

/// Добавить операцию в конец пачки. op_id раздаёт ядро (op_1, op_2, …),
/// вызывающий сразу получает его для $ref-связывания.
pub fn add_op(
    handle: &str,
    op_name: &str,
    params_json: &str,
) -> Result<String, String> {
    let mut map = sessions().lock().unwrap();
    let pending = map
        .get_mut(handle)
        .ok_or_else(|| format!("TX_SESSION_NOT_FOUND: сессия {handle:?} не найдена или уже закрыта"))?;

    if op_name.trim().is_empty() {
        return Err("VALIDATION: пустое имя операции".into());
    }
    let params: Value = if params_json.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str(params_json)
            .map_err(|e| format!("INVALID_JSON: параметры операции: {e}"))?
    };

    let op_id = format!("op_{}", pending.operations.len() + 1);
    pending.operations.push(TxOperation {
        op_id: op_id.clone(),
        op: op_name.to_string(),
        params,
    });
    Ok(op_id)
}

/// Забрать сессию и собрать TransactionPackage (сессия закрывается).
pub fn take_and_build(
    handle: &str,
    policies: Vec<crate::permission_policy::PermissionPolicy>,
) -> Result<TransactionPackage, String> {
    let mut map = sessions().lock().unwrap();
    let pending = map
        .remove(handle)
        .ok_or_else(|| format!("TX_SESSION_NOT_FOUND: сессия {handle:?} не найдена или уже закрыта"))?;

    Ok(TransactionPackage {
        idempotency_key: pending.business_key,
        required_permission: None,
        operations: pending.operations,
        context: super::TxContext {
            company_id: pending.company_id,
            actor: pending.actor,
            policies,
        },
        created_at: chrono::Utc::now(),
        expires_at: None,
    })
}

// ── Тесты (без БД и WASM) ──────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::UserId;
    use serde_json::json;

    fn actor() -> ActorSnapshot {
        ActorSnapshot {
            user_id: UserId(uuid::Uuid::nil()),
            login: "plugin".into(),
            full_name: None,
            position: None,
            company_id: CompanyId(uuid::Uuid::nil()),
        }
    }

    #[test]
    fn begin_add_build_preserves_order_and_ids() {
        let h = begin("biz-key", CompanyId(uuid::Uuid::nil()), actor(), None);

        let id1 = add_op(&h, "test.noop", r#"{"step":1}"#).unwrap();
        let id2 = add_op(&h, "object.post", r#"{"object_id":"x","expected_version":1}"#).unwrap();
        assert_eq!(id1, "op_1");
        assert_eq!(id2, "op_2");

        // Повторное использование закрытой сессии — ошибка
        let pkg = take_and_build(&h, vec![]).unwrap();
        assert_eq!(pkg.idempotency_key, "biz-key");
        assert_eq!(pkg.operations.len(), 2);
        assert_eq!(pkg.operations[0].op_id, "op_1");
        assert_eq!(pkg.operations[1].params["object_id"], json!("x"));
        assert!(pkg.validate().is_ok());

        assert!(add_op(&h, "test.noop", "").is_err());
        assert!(take_and_build(&h, vec![]).is_err());
    }

    #[test]
    fn unknown_handle_rejected() {
        assert!(add_op("nope", "test.noop", "{}").is_err());
        assert!(take_and_build("nope", vec![]).is_err());
    }

    #[test]
    fn invalid_params_rejected_without_consuming_session() {
        let h = begin("k2", CompanyId(uuid::Uuid::nil()), actor(), None);
        let err = add_op(&h, "test.noop", "{broken json").unwrap_err();
        assert!(err.contains("INVALID_JSON"), "{err}");
        // Сессия жива — повторный корректный вызов работает
        assert_eq!(add_op(&h, "test.noop", "").unwrap(), "op_1");
    }

    #[test]
    fn empty_op_name_rejected() {
        let h = begin("k3", CompanyId(uuid::Uuid::nil()), actor(), None);
        let err = add_op(&h, "  ", "{}").unwrap_err();
        assert!(err.contains("пустое имя"), "{err}");
    }
}
