use core_domain::error::DomainError;
use std::sync::Arc;

use crate::command_registry::CommandRegistry;
use crate::registry::CodeRegistry;

/// Groups the five registries of a module and encapsulates ensure-semantics,
/// per Appendix 2 of the spec. The four code-based registries (permissions,
/// object schemas, print templates, scripts) mark module presence idempotently;
/// the manifest blocks that populate them arrive with the WASM layer.
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

    /// Idempotently marks a module as present in the four code-based registries.
    /// Command handlers are registered separately through `commands`.
    pub async fn register_module(&self, module_code: &str) -> Result<(), DomainError> {
        self.permissions.ensure(module_code).await;
        self.object_schemas.ensure(module_code).await;
        self.print_templates.ensure(module_code).await;
        self.scripts.ensure(module_code).await;
        Ok(())
    }

    /// Removes a module from all registries, including its prefixed commands.
    pub async fn unregister_module(&self, module_code: &str) -> Result<(), DomainError> {
        let prefix = format!("plugin.{module_code}.");
        self.commands.remove_by_prefix(&prefix).await;
        self.permissions.remove(module_code).await;
        self.object_schemas.remove(module_code).await;
        self.print_templates.remove(module_code).await;
        self.scripts.remove(module_code).await;
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
}