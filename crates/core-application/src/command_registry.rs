use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;

type CommandHandler = Arc<
    dyn Fn(Value) -> Pin<Box<dyn Future<Output = Result<Value, String>> + Send>> + Send + Sync,
>;

/// Динамический реестр асинхронных команд, согласно Приложению №1 ТЗ.
///
/// Нагрузка с преобладанием чтения (много `execute`, мало `register`) делает
/// `tokio::sync::RwLock` правильным выбором вместо `std::sync::Mutex`: читатели
/// выполняются конкурентно, не блокируя исполнитель.
#[derive(Default)]
pub struct CommandRegistry {
    handlers: RwLock<HashMap<String, CommandHandler>>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            handlers: RwLock::new(HashMap::new()),
        }
    }

    /// Регистрирует (или заменяет) асинхронный обработчик команды.
    pub async fn register<F, Fut>(&self, name: &str, handler: F)
    where
        F: Fn(Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Value, String>> + Send + 'static,
    {
        let mut map = self.handlers.write().await;
        map.insert(
            name.to_string(),
            Arc::new(move |params| Box::pin(handler(params))),
        );
    }

    /// Удаляет один обработчик команды.
    pub async fn unregister(&self, name: &str) {
        let mut map = self.handlers.write().await;
        map.remove(name);
    }

    /// Удаляет все команды, чьё имя начинается с `prefix`; используется при
    /// удалении или отключении модуля.
    pub async fn remove_by_prefix(&self, prefix: &str) {
        let mut map = self.handlers.write().await;
        map.retain(|name, _| !name.starts_with(prefix));
    }

    /// Выполняет команду и возвращает её результат. Идентификаторы команд уникальны,
    /// поскольку обработчики регистрируются под именами с префиксами, например
    /// `plugin.warehouse.post_document`.
    pub async fn execute(&self, name: &str, params: Value) -> Result<Value, String> {
        let map = self.handlers.read().await;
        match map.get(name) {
            Some(handler) => handler(params).await,
            None => Err(format!("Unknown command: {name}")),
        }
    }

    /// Перечисляет имена всех зарегистрированных команд.
    pub async fn list(&self) -> Vec<String> {
        let map = self.handlers.read().await;
        map.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn register_execute_unregister() {
        let registry = CommandRegistry::new();

        registry
            .register("echo", |params: Value| async move { Ok(params) })
            .await;
        assert_eq!(registry.list().await, vec!["echo".to_string()]);

        let result = registry.execute("echo", json!({"value": 42})).await.unwrap();
        assert_eq!(result, json!({"value": 42}));

        let missing = registry.execute("missing", json!({})).await;
        assert!(missing.is_err());

        registry.unregister("echo").await;
        assert!(registry.list().await.is_empty());
    }

    #[tokio::test]
    async fn remove_by_prefix_drops_only_matching_commands() {
        let registry = CommandRegistry::new();
        registry
            .register(
                "plugin.warehouse.post_document",
                |_: Value| async move { Ok(json!({"ok": true})) },
            )
            .await;
        registry
            .register(
                "core.hello",
                |_: Value| async move { Ok(json!({})) },
            )
            .await;

        registry.remove_by_prefix("plugin.warehouse.").await;
        assert_eq!(registry.list().await, vec!["core.hello".to_string()]);
    }
}