//! Детерминированная проверка прав доступа на основе политик (Приложение №7 ТЗ v3.1).

use std::sync::Arc;

use crate::ports::{PermissionPolicyRepository, RoleRepository};
use core_domain::error::DomainError;
use core_domain::permission::{PermissionPolicy, PermissionScopeType};
use uuid::Uuid;

/// Сервис проверки прав доступа. Инкапсулирует алгоритм
/// фильтрация → сортировка → deny overrides allow → deny-by-default.
pub struct PermissionManager {
    roles: Arc<dyn RoleRepository>,
    policies: Arc<dyn PermissionPolicyRepository>,
}

impl PermissionManager {
    pub fn new(
        roles: Arc<dyn RoleRepository>,
        policies: Arc<dyn PermissionPolicyRepository>,
    ) -> Self {
        Self { roles, policies }
    }

    /// Проверяет действие против набора политик по алгоритму из ТЗ v2.2.
    ///
    /// 1. Фильтрация по `scope_type`, `entity_type` и `actions` (совпадение действия
    ///    или `"*"`).
    /// 2. Сортировка совпавших политик по `priority` (убывание).
    /// 3. Любая политика с `deny: true` → запрет (deny overrides allow).
    /// 4. Иначе первая совпавшая политика → разрешение.
    /// 5. Без совпадений → запрет (deny-by-default).
    pub fn check_access(
        policies: &[PermissionPolicy],
        module_code: Option<&str>,
        entity_type: Option<&str>,
        action: &str,
    ) -> bool {
        let mut matching: Vec<&PermissionPolicy> = policies
            .iter()
            .filter(|p| {
                match (&p.scope_type, module_code) {
                    (PermissionScopeType::Platform, _) => true,
                    (PermissionScopeType::Module(m), Some(mc)) => m == mc,
                    (PermissionScopeType::Metadata, _) => true,
                    _ => false,
                }
            })
            .filter(|p| match entity_type {
                Some(et) => p.entity_type.as_deref() == Some(et) || p.entity_type.is_none(),
                None => true,
            })
            .filter(|p| p.actions.iter().any(|a| a == action || a == "*"))
            .collect();
        matching.sort_by_key(|p| std::cmp::Reverse(p.priority));

        // Reшtte первая (по приоритету) совпавшая политика: deny побеждает
        // любые разрешения, deny-by-default при отсутствии совпадений.
        match matching.first() {
            Some(policy) => !policy.deny,
            None => false,
        }
    }

    /// Асинхронная проверка права пользователя в компании.
    ///
    /// Загружает коды политик ролей пользователя, достаёт сами политики по кодам
    /// из репозитория и передаёт их в [`Self::check_access`].
    pub async fn check(
        &self,
        user_id: &Uuid,
        company_id: &Uuid,
        module_code: Option<&str>,
        entity_type: Option<&str>,
        action: &str,
    ) -> Result<bool, DomainError> {
        let codes = self
            .roles
            .get_policies_for_user(user_id, company_id)
            .await?;
        let policies = self.policies.get_by_codes(&codes).await?;
        Ok(Self::check_access(&policies, module_code, entity_type, action))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use core_domain::permission::RecordAccessLevel;

    fn policy(
        code: &str,
        scope: PermissionScopeType,
        entity_type: Option<&str>,
        actions: Vec<&str>,
        deny: bool,
        priority: i32,
    ) -> PermissionPolicy {
        PermissionPolicy {
            id: Uuid::new_v4(),
            code: code.to_string(),
            name: code.to_string(),
            description: None,
            scope_type: scope,
            entity_type: entity_type.map(str::to_string),
            actions: actions.into_iter().map(str::to_string).collect(),
            record_access: RecordAccessLevel::All,
            deny,
            priority,
            module_code: None,
            is_system: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn platform(actions: Vec<&str>) -> PermissionPolicy {
        policy("p", PermissionScopeType::Platform, None, actions, false, 0)
    }

    #[test]
    fn deny_by_default_when_no_match() {
        let policies: Vec<PermissionPolicy> = vec![];
        assert!(!PermissionManager::check_access(&policies, None, None, "create"));
    }

    #[test]
    fn explicit_allow_matches_action_or_wildcard() {
        assert!(PermissionManager::check_access(
            &[platform(vec!["create"])],
            None,
            None,
            "create"
        ));
        assert!(PermissionManager::check_access(
            &[platform(vec!["*"])],
            None,
            None,
            "update"
        ));
        assert!(!PermissionManager::check_access(
            &[platform(vec!["read"])],
            None,
            None,
            "delete"
        ));
    }

    #[test]
    fn entity_type_matches_explicit_or_none() {
        let entity = policy(
            "et",
            PermissionScopeType::Platform,
            Some("invoice"),
            vec!["create"],
            false,
            0,
        );
        let entity_any = policy(
            "any",
            PermissionScopeType::Platform,
            None,
            vec!["create"],
            false,
            0,
        );
        assert!(PermissionManager::check_access(
            std::slice::from_ref(&entity),
            None,
            Some("invoice"),
            "create"
        ));
        assert!(!PermissionManager::check_access(
            std::slice::from_ref(&entity),
            None,
            Some("order"),
            "create"
        ));
        assert!(PermissionManager::check_access(
            std::slice::from_ref(&entity_any),
            None,
            Some("order"),
            "create"
        ));
    }

    #[test]
    fn module_scope_requires_exact_module() {
        let mod_policy = policy(
            "mod",
            PermissionScopeType::Module("invoice".to_string()),
            None,
            vec!["create"],
            false,
            0,
        );
        assert!(PermissionManager::check_access(
            std::slice::from_ref(&mod_policy),
            Some("invoice"),
            None,
            "create"
        ));
        assert!(!PermissionManager::check_access(&[mod_policy], Some("stock"), None, "create"));
    }

    #[test]
    fn deny_overrides_allow_at_higher_priority() {
        let allow_lo = policy(
            "a",
            PermissionScopeType::Platform,
            None,
            vec!["create"],
            false,
            0,
        );
        let deny_hi = policy(
            "d",
            PermissionScopeType::Platform,
            None,
            vec!["create"],
            true,
            10,
        );
        assert!(!PermissionManager::check_access(&[deny_hi, allow_lo], None, None, "create"));
    }

    #[test]
    fn priority_10_beats_priority_0() {
        let allow_lo = policy(
            "a0",
            PermissionScopeType::Platform,
            None,
            vec!["create"],
            false,
            0,
        );
        let deny_hi = policy(
            "d10",
            PermissionScopeType::Platform,
            None,
            vec!["create"],
            true,
            10,
        );
        assert!(!PermissionManager::check_access(&[deny_hi.clone(), allow_lo], None, None, "create"));
        let allow_hi = policy(
            "a10",
            PermissionScopeType::Platform,
            None,
            vec!["create"],
            false,
            10,
        );
        let deny_lo = policy(
            "d0",
            PermissionScopeType::Platform,
            None,
            vec!["create"],
            true,
            0,
        );
        assert!(PermissionManager::check_access(&[deny_lo, allow_hi], None, None, "create"));
    }
}