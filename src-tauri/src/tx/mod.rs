// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

//! Механизм транзакционных операций (tx_exec).
//!
//! Исполнитель атомарно выполняет пачку декларативных операций
//! в одной транзакции MongoDB. На нём ляжет оркестрация предметных
//! модулей: склад, торговля, учёт, подотчёт.
//!
//! Принципы:
//! - внутри транзакции — только данные; побочки (уведомления) — снаружи;
//! - идемпотентность через журнал, записываемый ВНУТРИ той же транзакции;
//! - обработчики получают открытую сессию исполнителя — никаких вложенных tx;
//! - права двух уровней: право на пачку + право каждого обработчика.

pub mod executor;
pub mod handlers;
pub mod journal;
pub mod registry;
pub mod indexes;
pub mod session;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::core::{CompanyId, PlatformError};
use crate::events::ActorSnapshot;
use crate::permission_policy::{PermissionPolicy, PermissionPolicyService};

// ── Операция ───────────────────────────────────────────────

/// Одна операция пачки.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxOperation {
    /// Уникален в пределах пачки; ключ результата в op_results.
    pub op_id: String,
    /// Имя из реестра: object.transition, test.noop, stock.issue…
    pub op: String,
    /// Параметры; могут содержать {"$ref": "op_id.path"} на предыдущие результаты.
    #[serde(default)]
    pub params: serde_json::Value,
}

// ── Контекст ───────────────────────────────────────────────

/// Контекст вызова: снапшот компании/актора/прав на момент запуска.
/// Обработчики проверяют свои права по этому снапшоту (defense in depth).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxContext {
    pub company_id: CompanyId,
    pub actor: ActorSnapshot,
    pub policies: Vec<PermissionPolicy>,
}

impl TxContext {
    /// Deny-by-default проверка права из снапшота политик.
    /// Пустое право = проверка не требуется (как в middleware).
    pub fn check_permission(&self, permission: &str) -> Result<(), PlatformError> {
        if permission.is_empty() {
            return Ok(());
        }
        let parts: Vec<&str> = permission.split('.').collect();
        if parts.len() != 2 {
            return Err(PlatformError::PermissionDenied(format!(
                "Невалидный формат права: {permission}"
            )));
        }
        if PermissionPolicyService::check_access(&self.policies, parts[0], None, parts[1]) {
            Ok(())
        } else {
            Err(PlatformError::PermissionDenied(format!(
                "Доступ запрещён: нет права {permission}"
            )))
        }
    }
}

// ── Пачка ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionPackage {
    /// Ключ идемпотентности. Генерится вызывающим ДО первой отправки
    /// и переиспользуется при ретрае. Уникален в рамках компании.
    pub idempotency_key: String,
    /// Право на шаблон пачки (цельное бизнес-действие), опционально.
    #[serde(default)]
    pub required_permission: Option<String>,
    /// Порядок выполнения строго равен порядку массива.
    pub operations: Vec<TxOperation>,
    pub context: TxContext,
    pub created_at: DateTime<Utc>,
    /// После этого момента пачка не выполняется (защита от протухших ретраев).
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
}

impl TransactionPackage {
    /// Структурная валидация — строго ДО открытия транзакции.
    pub fn validate(&self) -> Result<(), String> {
        if self.idempotency_key.trim().is_empty() {
            return Err("idempotency_key не может быть пустым".into());
        }
        if self.operations.is_empty() {
            return Err("пачка не может быть пустой".into());
        }
        let mut seen = std::collections::HashSet::new();
        for op in &self.operations {
            if op.op_id.trim().is_empty() {
                return Err(format!("операция {:?}: пустой op_id", op.op));
            }
            if op.op.trim().is_empty() {
                return Err(format!("op_id {:?}: пустое имя операции", op.op_id));
            }
            if !seen.insert(&op.op_id) {
                return Err(format!("дублирующийся op_id: {:?}", op.op_id));
            }
        }
        if let Some(exp) = self.expires_at {
            if exp <= self.created_at {
                return Err("expires_at должен быть позже created_at".into());
            }
        }
        Ok(())
    }

    /// Протухла ли пачка.
    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        self.expires_at.map(|e| e <= now).unwrap_or(false)
    }
}

// ── Результаты ─────────────────────────────────────────────

/// Успешный результат исполнения: результаты по op_id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxResult {
    pub op_results: HashMap<String, serde_json::Value>,
}

/// Ошибка исполнения с указанием упавшей операции.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxError {
    pub message: String,
    pub failed_op: Option<String>,
}

impl TxError {
    pub fn new(message: impl Into<String>) -> Self {
        Self { message: message.into(), failed_op: None }
    }

    pub fn with_failed_op(mut self, op_id: &str) -> Self {
        self.failed_op = Some(op_id.to_string());
        self
    }
}

impl std::fmt::Display for TxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.failed_op {
            Some(op) => write!(f, "{} (операция: {op})", self.message),
            None => write!(f, "{}", self.message),
        }
    }
}

impl From<PlatformError> for TxError {
    fn from(e: PlatformError) -> Self {
        TxError::new(e.to_string())
    }
}

// ── $ref-связывание ────────────────────────────────────────

/// Глубоко заменить {"$ref": "op_id.path.to"} в params результатами
/// предыдущих операций. Ссылка вперёд/на несуществующий op_id = ошибка.
pub fn resolve_refs(
    params: &serde_json::Value,
    results: &HashMap<String, serde_json::Value>,
) -> Result<serde_json::Value, String> {
    resolve_node(params, results)
}

fn resolve_node(
    node: &serde_json::Value,
    results: &HashMap<String, serde_json::Value>,
) -> Result<serde_json::Value, String> {
    use serde_json::Value;

    // Точка связывания?
    if let Value::Object(map) = node {
        if map.len() == 1 {
            if let Some(Value::String(reference)) = map.get("$ref") {
                return lookup_ref(reference, results);
            }
        }
        let mut out = serde_json::Map::new();
        for (k, v) in map {
            out.insert(k.clone(), resolve_node(v, results)?);
        }
        return Ok(Value::Object(out));
    }

    if let Value::Array(arr) = node {
        let mut out = Vec::with_capacity(arr.len());
        for v in arr {
            out.push(resolve_node(v, results)?);
        }
        return Ok(Value::Array(out));
    }

    Ok(node.clone())
}

fn lookup_ref(
    reference: &str,
    results: &HashMap<String, serde_json::Value>,
) -> Result<serde_json::Value, String> {
    use serde_json::Value;

    let mut segments = reference.split('.');
    let Some(op_id) = segments.next() else {
        return Err(format!("пустая ссылка $ref: {reference:?}"));
    };
    let Some(mut current) = results.get(op_id) else {
        return Err(format!(
            "$ref {reference:?}: операция {op_id:?} ещё не выполнена или не существует"
        ));
    };

    for seg in segments {
        match current {
            Value::Object(map) => {
                current = map.get(seg).ok_or_else(|| {
                    format!("$ref {reference:?}: нет поля {seg:?} в результате {op_id:?}")
                })?;
            }
            Value::Array(arr) => {
                let idx: usize = seg.parse().map_err(|_| {
                    format!("$ref {reference:?}: индекс {seg:?} не число")
                })?;
                current = arr.get(idx).ok_or_else(|| {
                    format!("$ref {reference:?}: индекс {idx} вне диапазона результата {op_id:?}")
                })?;
            }
            other => {
                return Err(format!(
                    "$ref {reference:?}: сегмент {seg:?} неприменим к значению {other}"
                ))
            }
        }
    }
    Ok(current.clone())
}

// ── Тесты (чистые, без БД) ─────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn pkg(ops: Vec<TxOperation>) -> TransactionPackage {
        let now = Utc::now();
        TransactionPackage {
            idempotency_key: "key-1".into(),
            required_permission: None,
            operations: ops,
            context: TxContext {
                company_id: CompanyId(uuid::Uuid::nil()),
                actor: ActorSnapshot {
                    user_id: crate::core::UserId(uuid::Uuid::nil()),
                    login: "t".into(),
                    full_name: None,
                    position: None,
                    company_id: CompanyId(uuid::Uuid::nil()),
                },
                policies: vec![],
            },
            created_at: now,
            expires_at: None,
        }
    }

    // ── validate ──

    #[test]
    fn valid_package_passes() {
        let p = pkg(vec![TxOperation {
            op_id: "a".into(),
            op: "test.noop".into(),
            params: json!({}),
        }]);
        assert!(p.validate().is_ok());
    }

    #[test]
    fn empty_key_rejected() {
        let mut p = pkg(vec![]);
        p.idempotency_key = "  ".into();
        assert!(p.validate().unwrap_err().contains("idempotency_key"));
    }

    #[test]
    fn empty_operations_rejected() {
        assert!(pkg(vec![]).validate().unwrap_err().contains("пустой"));
    }

    #[test]
    fn duplicate_op_id_rejected() {
        let mk = |id: &str| TxOperation { op_id: id.into(), op: "x".into(), params: json!({}) };
        let err = pkg(vec![mk("a"), mk("a")]).validate().unwrap_err();
        assert!(err.contains("дублирующийся"), "{err}");
    }

    #[test]
    fn empty_op_name_rejected() {
        let ops = vec![TxOperation { op_id: "a".into(), op: " ".into(), params: json!({}) }];
        assert!(pkg(ops).validate().unwrap_err().contains("пустое имя"));
    }

    #[test]
    fn expired_window_rejected() {
        let mut p = pkg(vec![TxOperation { op_id: "a".into(), op: "x".into(), params: json!({}) }]);
        p.expires_at = Some(p.created_at);
        assert!(p.validate().unwrap_err().contains("expires_at"));

        p.expires_at = Some(Utc::now() + chrono::Duration::minutes(5));
        assert!(p.validate().is_ok());
    }

    #[test]
    fn is_expired_works() {
        let mut p = pkg(vec![]);
        p.expires_at = Some(Utc::now() - chrono::Duration::seconds(1));
        assert!(p.is_expired(Utc::now()));
        p.expires_at = Some(Utc::now() + chrono::Duration::hours(1));
        assert!(!p.is_expired(Utc::now()));
        p.expires_at = None;
        assert!(!p.is_expired(Utc::now()));
    }

    // ── resolve_refs ──

    fn results() -> HashMap<String, serde_json::Value> {
        HashMap::from([
            ("op1".to_string(), json!({"id": "obj-9", "nested": {"deep": [10, 20]}})),
            ("op2".to_string(), json!("строка")),
        ])
    }

    #[test]
    fn ref_resolves_field() {
        let out = resolve_refs(&json!({"object_id": {"$ref": "op1.id"}}), &results()).unwrap();
        assert_eq!(out, json!({"object_id": "obj-9"}));
    }

    #[test]
    fn ref_resolves_nested_path_and_array_index() {
        let out = resolve_refs(&json!({"$ref": "op1.nested.deep.1"}), &results()).unwrap();
        assert_eq!(out, json!(20));
    }

    #[test]
    fn refs_inside_arrays_and_nested_objects() {
        let params = json!({
            "items": [
                {"target": {"$ref": "op1.id"}},
                {"keep": 1}
            ],
            "meta": {"src": {"$ref": "op2"}}
        });
        let out = resolve_refs(&params, &results()).unwrap();
        assert_eq!(out["items"][0]["target"], json!("obj-9"));
        assert_eq!(out["items"][1]["keep"], json!(1));
        assert_eq!(out["meta"]["src"], json!("строка"));
    }

    #[test]
    fn object_with_extra_keys_is_not_a_ref() {
        let params = json!({"$ref": "op1.id", "note": "не ссылка — два ключа"});
        let out = resolve_refs(&params, &results()).unwrap();
        assert_eq!(out["$ref"], json!("op1.id"), "литерал должен сохраниться");
        assert_eq!(out["note"], json!("не ссылка — два ключа"));
    }

    #[test]
    fn forward_reference_is_error() {
        let empty = HashMap::new();
        let err = resolve_refs(&json!({"$ref": "future.id"}), &empty).unwrap_err();
        assert!(err.contains("ещё не выполнена"), "{err}");
    }

    #[test]
    fn missing_path_segment_is_error() {
        let err = resolve_refs(&json!({"$ref": "op1.nested.absent"}), &results()).unwrap_err();
        assert!(err.contains("нет поля"), "{err}");
    }

    #[test]
    fn bad_array_index_is_error() {
        assert!(resolve_refs(&json!({"$ref": "op1.nested.deep.9"}), &results()).is_err());
        assert!(resolve_refs(&json!({"$ref": "op2.0"}), &results()).is_err());
    }
}
