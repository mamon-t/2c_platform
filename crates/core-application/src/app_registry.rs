use core_domain::error::DomainError;
use std::sync::Arc;

use crate::command_registry::CommandRegistry;
use crate::ports::EntitySchema;
use crate::registry::CodeRegistry;

/// Группирует пять реестров модуля и инкапсулирует ensure-семантику,
/// согласно Приложению №2 ТЗ. Четыре реестра на основе кодов (прав доступа,
/// схем объектов, печатных форм, скриптов) идемпотентно отмечают присутствие модуля;
/// блоки манифеста, заполняющие их, приходят вместе с WASM-слоем.
pub struct AppRegistry {
    pub commands: Arc<CommandRegistry>,
    pub permissions: Arc<CodeRegistry>,
    pub object_schemas: Arc<CodeRegistry>,
    pub print_templates: Arc<CodeRegistry>,
    pub scripts: Arc<CodeRegistry>,
}

impl AppRegistry {
    pub fn new() -> Self {
        Self {
            commands: Arc::new(CommandRegistry::new()),
            permissions: Arc::new(CodeRegistry::new()),
            object_schemas: Arc::new(CodeRegistry::new()),
            print_templates: Arc::new(CodeRegistry::new()),
            scripts: Arc::new(CodeRegistry::new()),
        }
    }

    /// Идемпотентно отмечает модуль как присутствующий в четырёх реестрах на основе кодов.
    /// Обработчики команд регистрируются отдельно через `commands`.
    pub async fn register_module(&self, module_code: &str) -> Result<(), DomainError> {
        self.permissions.ensure(module_code).await;
        self.object_schemas.ensure(module_code).await;
        self.print_templates.ensure(module_code).await;
        self.scripts.ensure(module_code).await;
        Ok(())
    }

    /// Удаляет модуль из всех реестров, включая его команды с префиксами.
    pub async fn unregister_module(&self, module_code: &str) -> Result<(), DomainError> {
        let prefix = format!("plugin.{module_code}.");
        self.commands.remove_by_prefix(&prefix).await;
        self.permissions.remove(module_code).await;
        self.object_schemas.remove(module_code).await;
        self.print_templates.remove(module_code).await;
        self.scripts.remove(module_code).await;
        Ok(())
    }

    /// Объявляет типы сущностей, поставляемые модулем, в реестре схем объектов,
    /// следуя ensure-семантике. API для прелоадера модулей (Фаза 8):
    /// блоки манифеста применяются на стороне хоста, а здесь отмечаются как присутствующие
    /// все `entity_type.code`, чтобы последующие обращения не затрагивали
    /// базу данных. Метод идемпотентен, как и `register_module`.
    pub async fn preload_metadata_to_registry(&self, schemas: &[EntitySchema]) -> Result<(), DomainError> {
        for schema in schemas {
            self.object_schemas.ensure(&schema.entity_type.code).await;
        }
        Ok(())
    }
}

impl Default for AppRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    #[tokio::test]
    async fn register_module_is_idempotent_and_full() {
        let registry = AppRegistry::new();

        registry.register_module("stock").await.unwrap();
        registry.register_module("stock").await.unwrap();

        assert!(registry.permissions.contains("stock").await);
        assert!(registry.object_schemas.contains("stock").await);
        assert!(registry.print_templates.contains("stock").await);
        assert!(registry.scripts.contains("stock").await);
        assert_eq!(registry.permissions.list().await, vec!["stock".to_string()]);
    }

    #[tokio::test]
    async fn unregister_module_cleans_everything() {
        let registry = AppRegistry::new();
        registry
            .commands
            .register(
                "plugin.stock.post_document",
                |_: Value| async move { Ok(json!({})) },
            )
            .await;
        registry.register_module("stock").await.unwrap();

        registry.unregister_module("stock").await.unwrap();

        assert!(registry.commands.list().await.is_empty());
        assert!(!registry.permissions.contains("stock").await);
        assert!(!registry.object_schemas.contains("stock").await);
        assert!(!registry.print_templates.contains("stock").await);
        assert!(!registry.scripts.contains("stock").await);
    }

    #[tokio::test]
    async fn preload_metadata_is_idempotent() {
        use crate::ports::EntitySchema;
        use core_domain::metadata::{EntityKind, EntityType};

        let registry = AppRegistry::new();
        let schema = EntitySchema {
            entity_type: EntityType {
                id: uuid::Uuid::new_v4(),
                code: "invoice".to_string(),
                name: "Счёт".to_string(),
                kind: EntityKind::Document,
                company_id: String::new(),
                metadata_version: 1,
                is_system: false,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            },
            fields: vec![],
            states: vec![],
            transitions: vec![],
            forms: vec![],
            actions: vec![],
            relations: vec![],
        };

        registry
            .preload_metadata_to_registry(std::slice::from_ref(&schema))
            .await
            .unwrap();
        registry
            .preload_metadata_to_registry(std::slice::from_ref(&schema))
            .await
            .unwrap();

        assert!(registry.object_schemas.contains("invoice").await);
        assert_eq!(
            registry.object_schemas.list().await,
            vec!["invoice".to_string()]
        );
    }
}