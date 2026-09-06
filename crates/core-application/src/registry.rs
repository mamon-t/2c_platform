use std::collections::HashSet;
use tokio::sync::RwLock;

/// Идемпотентное хранилище кодов, общее для реестров прав доступа, схем объектов,
/// печатных форм и скриптов. Ensure-семантика на уровне модуля: код
/// вставляется не более одного раза.
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

    /// Регистрирует код, если он отсутствует. Возвращает `true` при новой вставке.
    pub async fn ensure(&self, code: &str) -> bool {
        let mut set = self.codes.write().await;
        set.insert(code.to_string())
    }

    /// Удаляет код. Возвращает `true`, если он присутствовал.
    pub async fn remove(&self, code: &str) -> bool {
        let mut set = self.codes.write().await;
        set.remove(code)
    }

    pub async fn contains(&self, code: &str) -> bool {
        let set = self.codes.read().await;
        set.contains(code)
    }

    /// Возвращает коды в отсортированном порядке для стабильной итерации и отображения.
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