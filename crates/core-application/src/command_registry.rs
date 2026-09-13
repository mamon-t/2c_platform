use crate::permission_manager::PermissionManager;
use crate::ports::AuditRepository;
use core_domain::audit::{AuditEntry, AuditResult, AuditTarget};
use core_domain::error::DomainError;
use core_domain::event::ActorSnapshot;
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Метаданные команды, используемые `CommandExecutionPipeline`.
#[derive(Debug, Clone)]
pub struct CommandMetadata {
    /// Право, требуемое для выполнения команды (код политики доступа).
    /// `None` — команда исполняется без проверки прав.
    pub required_permission: Option<String>,
}

impl CommandMetadata {
    /// Команда без требований к правам.
    pub fn unrestricted() -> Self {
        Self {
            required_permission: None,
        }
    }

    /// Команда, требующая указанного права.
    pub fn requires(permission: &str) -> Self {
        Self {
            required_permission: Some(permission.to_string()),
        }
    }
}

/// Контекст выполнения команды: исполнитель и параметры проверки прав.
#[derive(Debug, Clone, Default)]
pub struct CommandExecutionCtx {
    pub actor: Option<ActorSnapshot>,
    pub module_code: Option<String>,
    pub entity_type: Option<String>,
}

type CommandHandler = Arc<
    dyn Fn(Value, CommandExecutionCtx) -> Pin<Box<dyn Future<Output = Result<Value, DomainError>> + Send>>
        + Send
        + Sync,
>;

/// Динамический реестр асинхронных команд, согласно Приложению №1 ТЗ.
///
/// Нагрузка с преобладанием чтения (много `execute`, мало `register`) делает
/// `tokio::sync::RwLock` правильным выбором вместо `std::sync::Mutex`: читатели
/// выполняются конкурентно, не блокируя исполнитель.
///
/// Реестр может быть обёрнут в `CommandExecutionPipeline` через [`Self::attach_pipeline`]:
/// тогда каждая команда автоматически проверяется на требуемое право и
/// протоколируется в `AuditRepository`.
#[derive(Default)]
pub struct CommandRegistry {
    handlers: RwLock<HashMap<String, CommandHandler>>,
    metadata: RwLock<HashMap<String, CommandMetadata>>,
    pipeline: RwLock<Option<PipelineState>>,
}

#[derive(Clone)]
struct PipelineState {
    audit: Arc<dyn AuditRepository>,
    permissions: Arc<PermissionManager>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            handlers: RwLock::new(HashMap::new()),
            metadata: RwLock::new(HashMap::new()),
            pipeline: RwLock::new(None),
        }
    }

    /// Подключает CommandExecutionPipeline: проверку прав и аудит команд.
    /// Вызов идемпотентен — повторная установка обновляет зависимости.
    pub async fn attach_pipeline(
        &self,
        audit: Arc<dyn AuditRepository>,
        permissions: Arc<PermissionManager>,
    ) {
        *self.pipeline.write().await = Some(PipelineState { audit, permissions });
    }

    /// Регистрирует (или заменяет) обработчик команды вместе с её метаданными.
    pub async fn register_with_metadata<F, Fut>(
        &self,
        name: &str,
        metadata: CommandMetadata,
        handler: F,
    ) where
        F: Fn(Value, CommandExecutionCtx) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Value, DomainError>> + Send + 'static,
    {
        let mut map = self.handlers.write().await;
        map.insert(
            name.to_string(),
            Arc::new(move |params, ctx| Box::pin(handler(params, ctx))),
        );
        self.metadata.write().await.insert(name.to_string(), metadata);
    }

    /// Регистрирует обработчик команды без требований к правам (обёртка над
    /// [`Self::register_with_metadata`] с `CommandMetadata::unrestricted`).
    pub async fn register<F, Fut>(&self, name: &str, handler: F)
    where
        F: Fn(Value, CommandExecutionCtx) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Value, DomainError>> + Send + 'static,
    {
        self.register_with_metadata(name, CommandMetadata::unrestricted(), handler)
            .await;
    }

    /// Удаляет один обработчик команды вместе с ее метаданными.
    pub async fn unregister(&self, name: &str) {
        let mut map = self.handlers.write().await;
        map.remove(name);
        self.metadata.write().await.remove(name);
    }

    /// Удаляет все команды, чьё имя начинается с `prefix`; используется при
    /// удалении или отключении модуля.
    pub async fn remove_by_prefix(&self, prefix: &str) {
        let mut map = self.handlers.write().await;
        map.retain(|name, _| !name.starts_with(prefix));
        self.metadata.write().await.retain(|name, _| !name.starts_with(prefix));
    }

    /// Выполняет команду от имени системного исполнителя с пустым контекстом.
    pub async fn execute(&self, name: &str, params: Value) -> Result<Value, DomainError> {
        self.execute_ctx(name, params, CommandExecutionCtx::default()).await
    }

    /// Выполняет команду через `CommandExecutionPipeline`.
    ///
    /// Если pipeline подключён:
    /// 1. Логируется аудит `command.executed` со стадией `started`.
    /// 2. Если у команды задан `required_permission` — проверяется право
    ///    (`PermissionManager::check_access`).
    /// 3. При отказе логируется аудит `permission.denied`, бизнес-хендлер
    ///    **не вызывается**, возвращается `DomainError::PermissionDenied`.
    /// 4. Иначе выполняется бизнес-хендлер и логируется аудит `command.executed`
    ///    со стадией `finished` (или `failed` при ошибке).
    ///
    /// Системный исполнитель (`ActorSnapshot::system()`) обходит проверку прав —
    /// она применяется только к командам с реальным `user_id`.
    pub async fn execute_ctx(
        &self,
        name: &str,
        params: Value,
        ctx: CommandExecutionCtx,
    ) -> Result<Value, DomainError> {
        let actor = ctx.actor.clone().unwrap_or_else(ActorSnapshot::system);
        let handler = {
            let map = self.handlers.read().await;
            map.get(name).cloned()
        }
        .ok_or_else(|| DomainError::NotFound(format!("команда '{name}' не найдена")))?;
        let metadata = self.metadata.read().await.get(name).cloned();

        let pipeline = self.pipeline.read().await.clone();
        let Some(pipeline) = pipeline else {
            return handler(params, ctx).await;
        };

        let company_id = ctx
            .actor
            .as_ref()
            .and_then(|a| a.company_id)
            .or(actor.company_id);

        let audit = pipeline.audit.clone();
        let audit_plan = |stage: &str, result: AuditResult| {
            let audit = audit.clone();
            let plan = AuditEntry {
                id: Uuid::new_v4(),
                action: "command.executed".to_string(),
                actor: actor.clone(),
                target: Some(AuditTarget {
                    entity_type: ctx.entity_type.clone(),
                    entity_id: None,
                    entity_code: Some(name.to_string()),
                    company_id,
                }),
                result,
                details: Some(serde_json::json!({
                    "command": name,
                    "stage": stage,
                    "required_permission": metadata.as_ref().and_then(|m| m.required_permission.as_deref()),
                })),
                ip_address: None,
                user_agent: None,
                company_id,
                timestamp: chrono::Utc::now(),
            };
            (plan.clone(), async move { audit.log(plan).await })
        };

        let (_, start_fut) = audit_plan("started", AuditResult::Success);
        if let Err(e) = start_fut.await {
            return Err(DomainError::Storage(format!("audit start: {e}")));
        }

        let permitted = match &metadata.as_ref().and_then(|m| m.required_permission.as_deref()) {
            Some(permission) => match ctx.actor.as_ref().and_then(|a| a.user_id) {
                Some(user_id) => match company_id {
                    Some(cid) => pipeline
                        .permissions
                        .check(
                            &user_id,
                            &cid,
                            ctx.module_code.as_deref(),
                            ctx.entity_type.as_deref(),
                            permission,
                        )
                        .await
                        .map_err(|e| DomainError::Storage(format!("permission check: {e}")))?,
None => false,
                    },
                // Отсутствие user_id разрешено только системному исполнителю
                // (внутрипроцессный bootstrap) либо вызову без явного актора.
                // Аноним (actor без токена) прав не получает.
                None => ctx
                    .actor
                    .as_ref()
                    .map(|a| a.is_system())
                    .unwrap_or(true),
            },
            None => true,
        };

        if !permitted {
            let denied = AuditEntry {
                id: Uuid::new_v4(),
                action: "permission.denied".to_string(),
                actor: actor.clone(),
                target: Some(AuditTarget {
                    entity_type: ctx.entity_type.clone(),
                    entity_id: None,
                    entity_code: Some(name.to_string()),
                    company_id,
                }),
                result: AuditResult::Failure {
                    reason: "permission denied".to_string(),
                },
                details: Some(serde_json::json!({
                    "command": name,
                    "stage": "started",
                    "permission": metadata.as_ref().and_then(|m| m.required_permission.as_deref()),
                })),
                ip_address: None,
                user_agent: None,
                company_id,
                timestamp: chrono::Utc::now(),
            };
            audit.clone().log(denied).await.map_err(|e| DomainError::Storage(format!("audit denied: {e}")))?;
            return Err(DomainError::PermissionDenied(format!(
                "недостаточно прав на команду {name}"
            )));
        }

        let result = handler(params, ctx.clone()).await;
        match result {
            Ok(value) => {
                let (_, end_fut) = audit_plan("finished", AuditResult::Success);
                end_fut.await.map_err(|e| DomainError::Storage(format!("audit finish: {e}")))?;
                Ok(value)
            }
            Err(err) => {
                let (_, fail_fut) = audit_plan(
                    "failed",
                    AuditResult::Failure {
                        reason: err.to_string(),
                    },
                );
                fail_fut.await.map_err(|e| DomainError::Storage(format!("audit failed: {e}")))?;
                Err(err)
            }
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
            .register("echo", |params: Value, _ctx: CommandExecutionCtx| async move {
                Ok(params)
            })
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
                |_: Value, _ctx: CommandExecutionCtx| async move { Ok(json!({"ok": true})) },
            )
            .await;
        registry
            .register(
                "core.hello",
                |_: Value, _ctx: CommandExecutionCtx| async move { Ok(json!({})) },
            )
            .await;

        registry.remove_by_prefix("plugin.warehouse.").await;
        assert_eq!(registry.list().await, vec!["core.hello".to_string()]);
    }
}