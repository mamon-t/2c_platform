//! Хаб серверных уведомлений (ТЗ v3.1, §10): рассылка `ServerPush` по WebSocket.
//!
//! Сессии подписываются на широковещательный канал; `push` публикует
//! `RpcMessage::ServerPush` всем подключённым клиентам (фильтрация по компании
//! и модулю — на стороне клиента в этой подфазе).

use std::sync::Arc;

use tokio::sync::broadcast;
use uuid::Uuid;

use crate::rpc_message::RpcMessage;

/// Ёмкость широковещательного канала уведомлений.
const PUSH_CHANNEL_CAPACITY: usize = 256;

/// Хаб `ServerPush`-уведомлений.
pub struct PushHub {
    tx: broadcast::Sender<RpcMessage>,
}

impl PushHub {
    /// Создаёт хаб с широковещательным каналом.
    pub fn new() -> Arc<Self> {
        let (tx, _) = broadcast::channel(PUSH_CHANNEL_CAPACITY);
        Arc::new(Self { tx })
    }

    /// Возвращает приёмник уведомлений для одной WebSocket-сессии.
    pub fn subscribe(&self) -> broadcast::Receiver<RpcMessage> {
        self.tx.subscribe()
    }

    /// Публикует `ServerPush` всем подписчикам.
    pub fn push(&self, module: &str, event_type: &str, payload: serde_json::Value) {
        let message = RpcMessage::ServerPush {
            id: Uuid::new_v4().to_string(),
            module: module.to_string(),
            event_type: event_type.to_string(),
            payload,
        };
        // Получателей нет — некритично, лог на уровне debug опускаем.
        let _ = self.tx.send(message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn subscriber_receives_pushed_message() {
        let hub = PushHub::new();
        let mut rx = hub.subscribe();
        hub.push("core", "test.pushed", json!({"v": 1}));
        let message = rx.recv().await.unwrap();
        match message {
            RpcMessage::ServerPush { module, event_type, payload, .. } => {
                assert_eq!(module, "core");
                assert_eq!(event_type, "test.pushed");
                assert_eq!(payload, json!({"v": 1}));
            }
            other => panic!("ожидали ServerPush, получено: {other:?}"),
        }
    }

    #[tokio::test]
    async fn push_before_subscribe_is_dropped_for_late_subscriber() {
        let hub = PushHub::new();
        hub.push("core", "early", json!({}));
        let mut rx = hub.subscribe();
        hub.push("core", "late", json!({}));
        let message = rx.recv().await.unwrap();
        match message {
            RpcMessage::ServerPush { event_type, .. } => assert_eq!(event_type, "late"),
            other => panic!("ожидали ServerPush, получено: {other:?}"),
        }
    }
}