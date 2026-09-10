//! Идемпотентность RPC-запросов (ТЗ v3.1, §10, защита от дублей).
//!
//! Ключ дедупликации — `(actor_user_id, request_id)`. Пока аутентификации нет,
//! `actor_user_id` отсутствует, и ключ сводится к идентификатору запроса.
//! Успешные ответы кэшируются на 5 минут; протухшие записи вычищаются
//! фоновым сборщиком (по образцу `TransactionOrchestrator`).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde_json::Value;
use tokio::runtime::Handle;
use tokio::sync::RwLock;
use tokio::time::MissedTickBehavior;

/// TTL кэшированного ответа: повторная доставка в этом окне считается дублем.
const IDEMPOTENCY_TTL: Duration = Duration::from_secs(5 * 60);
/// Период обхода сборщика мусора протухших ответов.
const IDEMPOTENCY_GC_INTERVAL: Duration = Duration::from_secs(60);

/// Ключ идемпотентности: исполнитель (id пользователя) и идентификатор запроса.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IdempotencyKey {
    pub actor_user_id: Option<String>,
    pub request_id: String,
}

#[derive(Debug, Clone)]
struct CachedResponse {
    created_at: DateTime<Utc>,
    payload: Value,
}

/// Кэш ответов идемпотентных запросов.
pub struct IdempotencyStore {
    entries: RwLock<HashMap<IdempotencyKey, CachedResponse>>,
}

/// Отчёт об очистке протухших ответов (для логов и тестов).
#[derive(Debug, Clone, Default)]
pub struct IdempotencyGcReport {
    pub pruned: usize,
    pub remaining: usize,
}

impl IdempotencyStore {
    /// Создаёт хранилище и, если вызван внутри tokio-runtime, запускает
    /// фоновую задачу сборщика протухших ответов (60 c, TTL 5 минут).
    /// Вне runtime GC пропускается — его можно отработать вручную через
    /// `prune_expired`.
    pub fn new() -> Arc<Self> {
        let store = Arc::new(Self {
            entries: RwLock::new(HashMap::new()),
        });
        if let Ok(handle) = Handle::try_current() {
            let weak = Arc::downgrade(&store);
            handle.spawn(async move {
                let mut ticker = tokio::time::interval(IDEMPOTENCY_GC_INTERVAL);
                ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
                loop {
                    ticker.tick().await;
                    match weak.upgrade() {
                        Some(store) => {
                            store.prune_expired().await;
                        }
                        None => break,
                    }
                }
            });
        }
        store
    }

    /// Возвращает кэшированный ответ по ключу, если он существует и не протух.
    pub async fn get(&self, key: &IdempotencyKey) -> Option<Value> {
        let mut guard = self.entries.write().await;
        match guard.get(key) {
            Some(entry) if entry.created_at + chrono::Duration::from_std(IDEMPOTENCY_TTL).unwrap_or_default() >= Utc::now() => {
                Some(entry.payload.clone())
            }
            Some(_) => {
                guard.remove(key);
                None
            }
            None => None,
        }
    }

    /// Сохраняет успешный ответ по ключу (перезаписывая предыдущий).
    pub async fn put(&self, key: IdempotencyKey, payload: Value) {
        self.entries.write().await.insert(
            key,
            CachedResponse {
                created_at: Utc::now(),
                payload,
            },
        );
    }

    /// Удаляет ответы, созданные раньше `now - IDEMPOTENCY_TTL`.
    /// Публичен для тестов и явного вызова в дополнение к фоновому GC.
    pub async fn prune_expired(&self) -> IdempotencyGcReport {
        let cutoff = Utc::now() - chrono::Duration::from_std(IDEMPOTENCY_TTL).unwrap_or_default();
        let mut guard = self.entries.write().await;
        let before = guard.len();
        guard.retain(|_, entry| entry.created_at >= cutoff);
        IdempotencyGcReport {
            pruned: before - guard.len(),
            remaining: guard.len(),
        }
    }

    /// Текущее число записей (для тестов и диагностики).
    pub async fn len(&self) -> usize {
        self.entries.read().await.len()
    }

    /// Пусто ли хранилище.
    pub async fn is_empty(&self) -> bool {
        self.len().await == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn get_after_put_returns_payload() {
        let store = IdempotencyStore::new();
        let key = IdempotencyKey {
            actor_user_id: None,
            request_id: "req-1".to_string(),
        };
        assert_eq!(store.get(&key).await, None);
        store
            .put(key.clone(), serde_json::json!({"ok": true}))
            .await;
        assert_eq!(store.get(&key).await, Some(serde_json::json!({"ok": true})));
    }

    #[tokio::test]
    async fn overwrite_replaces_previous() {
        let store = IdempotencyStore::new();
        let key = IdempotencyKey {
            actor_user_id: None,
            request_id: "req-1".to_string(),
        };
        store.put(key.clone(), serde_json::json!({"v": 1})).await;
        store.put(key.clone(), serde_json::json!({"v": 2})).await;
        assert_eq!(store.get(&key).await, Some(serde_json::json!({"v": 2})));
    }

    #[tokio::test]
    async fn prune_expired_removes_stale_and_keeps_fresh() {
        let store = IdempotencyStore::new();
        let key = IdempotencyKey {
            actor_user_id: None,
            request_id: "req-1".to_string(),
        };
        store.put(key.clone(), serde_json::json!({"v": 1})).await;
        let stale = IdempotencyKey {
            actor_user_id: None,
            request_id: "req-2".to_string(),
        };
        store.entries.write().await.insert(
            stale.clone(),
            CachedResponse {
                created_at: Utc::now() - chrono::Duration::from_std(IDEMPOTENCY_TTL).unwrap_or_default()
                    - chrono::Duration::seconds(1),
                payload: serde_json::json!({"v": 2}),
            },
        );
        let report = store.prune_expired().await;
        assert_eq!(report.pruned, 1);
        assert_eq!(report.remaining, 1);
        assert!(store.get(&stale).await.is_none());
        assert!(store.get(&key).await.is_some());
    }
}