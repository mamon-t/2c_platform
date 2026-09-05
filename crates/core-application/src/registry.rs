use std::collections::HashSet;
use tokio::sync::RwLock;

/// Idempotent code-based storage shared by the permission, object schema,
/// print template and script registries. Module-level ensure-semantics: a code
/// is inserted at most once.
#[derive(Default)]
pub struct CodeRegistry {
    codes: RwLock<HashSet<String>>,
}

impl CodeRegistry {
    pub fn new() -> Self {
        Self {
            codes: RwLock::new(HashSet::new()),
        }
    }

    /// Registers a code if absent. Returns `true` when newly inserted.
    pub async fn ensure(&self, code: &str) -> bool {
        let mut set = self.codes.write().await;
        set.insert(code.to_string())
    }

    /// Removes a code. Returns `true` when it was present.
    pub async fn remove(&self, code: &str) -> bool {
        let mut set = self.codes.write().await;
        set.remove(code)
    }

    pub async fn contains(&self, code: &str) -> bool {
        let set = self.codes.read().await;
        set.contains(code)
    }

    /// Returns codes in sorted order for stable iteration and display.
    pub async fn list(&self) -> Vec<String> {
        let set = self.codes.read().await;
        let mut items: Vec<String> = set.iter().cloned().collect();
        items.sort();
        items
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn ensure_is_idempotent() {
        let registry = CodeRegistry::new();
        assert!(registry.ensure("doc.read").await);
        assert!(!registry.ensure("doc.read").await);
        assert!(registry.contains("doc.read").await);
        assert_eq!(registry.list().await, vec!["doc.read".to_string()]);
    }

    #[tokio::test]
    async fn remove_deletes_code() {
        let registry = CodeRegistry::new();
        registry.ensure("doc.read").await;

        assert!(registry.remove("doc.read").await);
        assert!(!registry.contains("doc.read").await);
        assert!(!registry.remove("doc.read").await);
    }
}