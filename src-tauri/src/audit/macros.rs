use super::{AuditChanges, AuditEntry, AuditableAction, service::{MongoAuditService, AuditService}};
use crate::commands::AppState;
use crate::core::{CompanyId, UserId};
use crate::db::MongoClient;

pub async fn fire_audit(
    state: &AppState,
    db: &MongoClient,
    action: AuditableAction,
    target_id: Option<String>,
    entity_type: Option<String>,
    object_id: Option<String>,
    changes: Option<AuditChanges>,
    event_id: Option<String>,
) {
    if let Some(ref user) = state.current_user {
        let cid_str = state.current_company_id.as_deref().unwrap_or("");
        let cid = uuid::Uuid::parse_str(cid_str).unwrap_or_default();

        let entry = AuditEntry::new(
            action,
            UserId(user._id),
            CompanyId(cid),
            target_id,
            entity_type,
            object_id,
            changes,
            event_id,
            None,
            None,
            None,
        );
        let svc = MongoAuditService::new();
        if let Err(e) = svc.log(db, entry).await {
            tracing::error!("Audit save failed: {e}");
        }
    }
}

/// Convenience macro that wraps `fire_audit`.
///
/// Usage:
/// ```rust,ignore
/// audit_log!(state, db, AuditableAction::CreateUser, target_id = id);
/// audit_log!(state, db, AuditableAction::UpdateUser, target_id = id,
///     changes => { "status" => "active" => "disabled" });
/// ```
#[macro_export]
macro_rules! audit_log {
    (
        $state:expr, $db:expr, $action:expr
        $(, target_id = $tid:expr )?
        $(, entity_type = $et:expr )?
        $(, object_id = $oid:expr )?
        $(, event_id = $eid:expr )?
        $(, changes => { $($field:expr => $old:expr => $new:expr),* $(,)? } )?
    ) => {{
        let mut __c = $crate::audit::AuditChanges::new();
        $(
            __c = __c.field($field, $old, $new);
        )*
        let __changes: Option<$crate::audit::AuditChanges> = if __c.is_empty() { None } else { Some(__c) };

        $crate::audit::macros::fire_audit(
            &$state, &$db, $action,
            $crate::__opt_string!($( $tid )?),
            $crate::__opt_string!($( $et )?),
            $crate::__opt_string!($( $oid )?),
            __changes,
            $crate::__opt_string!($( $eid )?),
        ).await;
    }};
}

/// Internal helper — converts optional expression to `Option<String>`.
#[macro_export]
macro_rules! __opt_string {
    ($val:expr) => { Some($val.to_string()) };
    () => { None };
}
