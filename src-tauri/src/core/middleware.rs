use std::future::Future;

use crate::audit::{AuditChanges, AuditableAction, macros::fire_audit, service::MongoAuditService, service::AuditService as AuditServiceTrait};
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
            self.check_record_scope(obj_id).await?;
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

    async fn check_record_scope(&self, obj_id: &str) -> PlatformResult<()> {
        let _ = obj_id;
        // TODO: record scope check (выполнится в коммите 5)
        // 1. Загрузить объект по id
        // 2. Найти политику для subsystem "documents" с action "read"
        // 3. Если record_scope == "own" && obj.created_by != self.user.id → deny
        Ok(())
    }
}
