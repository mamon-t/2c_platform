// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

use std::future::Future;

use crate::audit::{AuditChanges, AuditableAction, service::MongoAuditService, service::AuditService as AuditServiceTrait};
use crate::commands::AppState;
use crate::core::{CompanyId, PlatformError, PlatformResult, UserId};
use crate::db::MongoClient;
use crate::events::ActorSnapshot;
use crate::permission_policy::{PermissionPolicy, PermissionPolicyService};
use crate::user::UserPublic;

// ── CommandOutcome — результат бизнес-логики с метаданными для аудита ──

pub struct CommandOutcome<T> {
    pub result: T,
    pub changes: Option<AuditChanges>,
    pub event_id: Option<String>,
    pub signature_ref: Option<String>,
}

impl<T> CommandOutcome<T> {
    pub fn ok(result: T) -> Self {
        Self { result, changes: None, event_id: None, signature_ref: None }
    }

    pub fn with_changes(result: T, changes: AuditChanges) -> Self {
        Self { result, changes: Some(changes), event_id: None, signature_ref: None }
    }
}

// ── Scope — область действия команды ──

pub enum Scope {
    Company,
    Object(String),
    Metadata,
    Platform,
    None,
}

// ── CommandContext — контекст выполнения команды ──

pub struct CommandContext {
    pub db: MongoClient,
    pub user: UserPublic,
    pub company_id: CompanyId,
    pub role_id: Option<crate::core::RoleId>,
    pub policies: Vec<PermissionPolicy>,
}

impl CommandContext {
    pub fn extract(state: &AppState) -> PlatformResult<Self> {
        let user = state.current_user.as_ref()
            .ok_or_else(|| PlatformError::Auth("Необходима авторизация".into()))?
            .clone();
        let company_id_str = state.current_company_id.as_deref()
            .ok_or_else(|| PlatformError::Auth("Не выбрана компания".into()))?;
        let company_id = CompanyId(uuid::Uuid::parse_str(company_id_str)
            .map_err(|e| PlatformError::Validation(format!("Невалидный company_id: {e}")))?);
        let role_id = state.current_role_id.as_ref()
            .and_then(|s| uuid::Uuid::parse_str(s).ok())
            .map(crate::core::RoleId);
        let policies = state.current_policies.clone().unwrap_or_default();
        let db = state.db.as_ref()
            .ok_or_else(|| PlatformError::Auth("БД не подключена".into()))?
            .clone();

        Ok(Self { db, user, company_id, role_id, policies })
    }

    pub fn actor(&self) -> ActorSnapshot {
        ActorSnapshot {
            user_id: UserId(self.user._id),
            login: self.user.login.clone(),
            full_name: Some(self.user.display_name.clone()).filter(|s| !s.is_empty()),
            position: None,
            company_id: self.company_id.clone(),
        }
    }

    pub fn check_permission(&self, permission: &str) -> PlatformResult<()> {
        if permission.is_empty() {
            return Ok(());
        }
        let parts: Vec<&str> = permission.split('.').collect();
        if parts.len() != 2 {
            return Err(PlatformError::PermissionDenied(
                format!("Невалидный формат действия: {permission}")
            ));
        }
        let subsystem = parts[0];
        let action = parts[1];

        let allowed = PermissionPolicyService::check_access(
            &self.policies, subsystem, None, action,
        );
        if allowed {
            Ok(())
        } else {
            Err(PlatformError::PermissionDenied(
                format!("Доступ запрещён: нет права {permission}")
            ))
        }
    }

    pub async fn execute<F, Fut, T>(
        &self,
        permission: &str,
        scope: Scope,
        audit_action: AuditableAction,
        business_logic: F,
    ) -> PlatformResult<T>
    where
        F: FnOnce(ActorSnapshot) -> Fut + Send,
        Fut: Future<Output = PlatformResult<CommandOutcome<T>>> + Send,
        T: Send,
    {
        // ── PRE: Permission check ──
        self.check_permission(permission)?;

        // ── PRE: Record scope (Object) ──
        if let Scope::Object(ref obj_id) = scope {
            self.check_record_scope(obj_id, permission).await?;
        }

        // ── EXECUTE: Business logic ──
        let outcome = business_logic(self.actor()).await?;

        // ── POST: Audit (warn-and-forget) ──
        let target_id = match &scope {
            Scope::Object(id) => Some(id.clone()),
            _ => None,
        };
        let entry = crate::audit::AuditEntry::new(
            audit_action,
            UserId(self.user._id),
            self.company_id.clone(),
            target_id,
            None,
            outcome.changes.as_ref().map(|_| "".to_string()),
            outcome.changes.clone(),
            outcome.event_id.clone(),
            outcome.signature_ref.clone(),
            None,
            None,
        );
        let audit_svc = MongoAuditService::new();
        if let Err(e) = audit_svc.log(&self.db, entry).await {
            tracing::warn!("Audit write failed: {e}");
        }

        Ok(outcome.result)
    }

    /// Record scope check: если политика имеет record_scope == "own",
    /// разрешаем доступ только к собственным записям.
    async fn check_record_scope(&self, obj_id: &str, permission: &str) -> PlatformResult<()> {
        let parts: Vec<&str> = permission.split('.').collect();
        if parts.len() != 2 { return Ok(()); }
        let subsystem = parts[0];
        let action = parts[1];

        // Ищем подходящую политику
        let matching: Vec<&PermissionPolicy> = self.policies.iter()
            .filter(|p| p.subsystem_code == subsystem)
            .filter(|p| p.actions.iter().any(|a| a == action || a == "*"))
            .collect();

        // Берём policy с наивысшим приоритетом
        let policy = match matching.iter()
            .filter(|p| !p.deny)
            .max_by_key(|p| p.priority)
        {
            Some(p) => *p,
            None => return Ok(()), // нет allow-политики → deny уже был выше
        };

        if policy.record_scope != "own" {
            return Ok(()); // company scope → доступ ко всем записям компании
        }

        // Загружаем объект
        let obj = crate::objects::ObjectService::get(&self.db, uuid::Uuid::parse_str(obj_id)
            .map_err(|e| PlatformError::Validation(format!("Невалидный object_id: {e}")))?).await?;

        // Проверяем: объект принадлежит текущему пользователю
        if obj.created_by == UserId(self.user._id) {
            Ok(())
        } else {
            Err(PlatformError::PermissionDenied(
                format!("Доступ запрещён: запись принадлежит другому пользователю (record_scope = own)")
            ))
        }
    }
}
