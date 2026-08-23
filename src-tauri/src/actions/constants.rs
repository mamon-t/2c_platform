// ── Константы действий (subsystem.action) ──────────────────────
// Формат: "subsystem.action" — совпадает с кодами PermissionPolicy
// Неизвестное действие = нет разрешающей политики = deny (deny-by-default)

// ── Платформа ──
pub const PLATFORM_ACCESS: &str = "platform.access";

// ── Компании ──
pub const COMPANIES_READ: &str = "companies.read";
pub const COMPANIES_CREATE: &str = "companies.create";
pub const COMPANIES_UPDATE: &str = "companies.update";
pub const COMPANIES_DELETE: &str = "companies.delete";

// ── Пользователи ──
pub const USERS_READ: &str = "users.read";
pub const USERS_CREATE: &str = "users.create";
pub const USERS_UPDATE: &str = "users.update";
pub const USERS_DELETE: &str = "users.delete";

// ── Роли ──
pub const ROLES_READ: &str = "roles.read";
pub const ROLES_CREATE: &str = "roles.create";
pub const ROLES_UPDATE: &str = "roles.update";
pub const ROLES_DELETE: &str = "roles.delete";

// ── Контакты ──
pub const CONTACTS_READ: &str = "contacts.read";
pub const CONTACTS_CREATE: &str = "contacts.create";
pub const CONTACTS_UPDATE: &str = "contacts.update";
pub const CONTACTS_DELETE: &str = "contacts.delete";
pub const CONTACTS_MANAGE: &str = "contacts.manage";

// ── Документы ──
pub const DOCUMENTS_READ: &str = "documents.read";
pub const DOCUMENTS_CREATE: &str = "documents.create";
pub const DOCUMENTS_UPDATE: &str = "documents.update";
pub const DOCUMENTS_DELETE: &str = "documents.delete";
pub const DOCUMENTS_APPROVE: &str = "documents.approve";
pub const DOCUMENTS_CANCEL: &str = "documents.cancel";

// ── Справочники ──
pub const CATALOGS_READ: &str = "catalogs.read";
pub const CATALOGS_CREATE: &str = "catalogs.create";
pub const CATALOGS_UPDATE: &str = "catalogs.update";
pub const CATALOGS_DELETE: &str = "catalogs.delete";

// ── Метаданные (entity_types, fields, states, transitions, actions, forms) ──
pub const METADATA_READ: &str = "metadata.read";
pub const METADATA_CREATE: &str = "metadata.create";
pub const METADATA_UPDATE: &str = "metadata.update";
pub const METADATA_DELETE: &str = "metadata.delete";

// ── Отчёты ──
pub const REPORTS_READ: &str = "reports.read";
pub const REPORTS_CREATE: &str = "reports.create";
pub const REPORTS_EXPORT: &str = "reports.export";

// ── Скрипты ──
pub const SCRIPTS_READ: &str = "scripts.read";
pub const SCRIPTS_CREATE: &str = "scripts.create";
pub const SCRIPTS_EXECUTE: &str = "scripts.execute";

// ── Аудит ──
pub const AUDIT_READ: &str = "audit.read";

// ── Настройки ──
pub const SETTINGS_READ: &str = "settings.read";
pub const SETTINGS_MANAGE: &str = "settings.manage";

// ── Печатные формы ──
pub const PRINT_READ: &str = "print.read";
pub const PRINT_CREATE: &str = "print.create";
pub const PRINT_UPDATE: &str = "print.update";
pub const PRINT_DELETE: &str = "print.delete";

// ── Плагины / WASM ──
pub const PLUGINS_READ: &str = "plugins.read";
pub const PLUGINS_MANAGE: &str = "plugins.manage";
pub const PLUGINS_EXECUTE: &str = "plugins.execute";

// ── Нумерация ──
pub const NUMBERING_READ: &str = "numbering.read";
pub const NUMBERING_MANAGE: &str = "numbering.manage";

// ── Модули (прикладные) ──
pub const MODULES_READ: &str = "modules.read";
pub const MODULES_MANAGE: &str = "modules.manage";

// ── Матрица: команда → (действие, scope, AuditableAction) ──────
// scope: "C" = Company, "O" = Object, "M" = Metadata, "P" = Platform, "N" = None (public)

pub struct CommandMapping {
    pub command: &'static str,
    pub permission: &'static str,
    pub scope: ScopeTag,
    pub audit: &'static str,
}

pub enum ScopeTag {
    Company,
    Object,
    Metadata,
    Platform,
    None,
}

pub const COMMAND_MAP: &[CommandMapping] = &[
    // ── Системные (без auth) ──
    CommandMapping { command: "get_diagnostics", permission: PLATFORM_ACCESS, scope: ScopeTag::Platform, audit: "" },
    CommandMapping { command: "connect_db", permission: "", scope: ScopeTag::None, audit: "" },
    CommandMapping { command: "authenticate", permission: "", scope: ScopeTag::None, audit: "login" },

    // ── Rhai ──
    CommandMapping { command: "validate_rhai_script", permission: SCRIPTS_READ, scope: ScopeTag::Company, audit: "" },
    CommandMapping { command: "execute_rhai_script", permission: SCRIPTS_EXECUTE, scope: ScopeTag::Company, audit: "execute_script" },

    // ── Компании ──
    CommandMapping { command: "list_companies", permission: COMPANIES_READ, scope: ScopeTag::Company, audit: "" },
    CommandMapping { command: "get_company", permission: COMPANIES_READ, scope: ScopeTag::Company, audit: "" },
    CommandMapping { command: "create_company", permission: COMPANIES_CREATE, scope: ScopeTag::Company, audit: "create_company" },
    CommandMapping { command: "update_company", permission: COMPANIES_UPDATE, scope: ScopeTag::Company, audit: "update_company" },
    CommandMapping { command: "delete_company", permission: COMPANIES_DELETE, scope: ScopeTag::Company, audit: "delete_company" },

    // ── Пользователи ──
    CommandMapping { command: "list_users", permission: USERS_READ, scope: ScopeTag::Company, audit: "" },
    CommandMapping { command: "get_user", permission: USERS_READ, scope: ScopeTag::Company, audit: "" },
    CommandMapping { command: "create_user", permission: USERS_CREATE, scope: ScopeTag::Company, audit: "create_user" },
    CommandMapping { command: "update_user", permission: USERS_UPDATE, scope: ScopeTag::Company, audit: "update_user" },
    CommandMapping { command: "delete_user", permission: USERS_DELETE, scope: ScopeTag::Company, audit: "delete_user" },
    CommandMapping { command: "get_me", permission: PLATFORM_ACCESS, scope: ScopeTag::Platform, audit: "" },
    CommandMapping { command: "switch_company", permission: PLATFORM_ACCESS, scope: ScopeTag::Platform, audit: "switch_company" },

    // ── Персона / Профили ──
    CommandMapping { command: "get_person", permission: USERS_READ, scope: ScopeTag::Company, audit: "" },
    CommandMapping { command: "update_person", permission: USERS_UPDATE, scope: ScopeTag::Company, audit: "update_person" },
    CommandMapping { command: "list_user_profiles", permission: USERS_READ, scope: ScopeTag::Company, audit: "" },
    CommandMapping { command: "add_user_profile", permission: USERS_CREATE, scope: ScopeTag::Company, audit: "add_user_profile" },
    CommandMapping { command: "update_user_profile", permission: USERS_UPDATE, scope: ScopeTag::Company, audit: "update_user_profile" },
    CommandMapping { command: "remove_user_profile", permission: USERS_DELETE, scope: ScopeTag::Company, audit: "remove_user_profile" },

    // ── Контакты ──
    CommandMapping { command: "list_user_contacts", permission: CONTACTS_READ, scope: ScopeTag::Company, audit: "" },
    CommandMapping { command: "create_contact", permission: CONTACTS_CREATE, scope: ScopeTag::Company, audit: "create_contact" },
    CommandMapping { command: "update_contact", permission: CONTACTS_UPDATE, scope: ScopeTag::Company, audit: "update_contact" },
    CommandMapping { command: "delete_contact", permission: CONTACTS_DELETE, scope: ScopeTag::Company, audit: "delete_contact" },
    CommandMapping { command: "get_contact_types", permission: CONTACTS_READ, scope: ScopeTag::Company, audit: "" },
    CommandMapping { command: "save_contact_types", permission: CONTACTS_MANAGE, scope: ScopeTag::Company, audit: "save_settings" },

    // ── Сертификаты ──
    CommandMapping { command: "list_user_certificates", permission: USERS_READ, scope: ScopeTag::Company, audit: "" },
    CommandMapping { command: "deactivate_certificate", permission: USERS_UPDATE, scope: ScopeTag::Company, audit: "deactivate_certificate" },

    // ── Роли ──
    CommandMapping { command: "list_roles", permission: ROLES_READ, scope: ScopeTag::Company, audit: "" },
    CommandMapping { command: "create_role", permission: ROLES_CREATE, scope: ScopeTag::Company, audit: "create_role" },
    CommandMapping { command: "update_role", permission: ROLES_UPDATE, scope: ScopeTag::Company, audit: "update_role" },
    CommandMapping { command: "delete_role", permission: ROLES_DELETE, scope: ScopeTag::Company, audit: "delete_role" },

    // ── Политики доступа ──
    CommandMapping { command: "list_permission_policies", permission: ROLES_READ, scope: ScopeTag::Company, audit: "" },
    CommandMapping { command: "create_permission_policy", permission: ROLES_CREATE, scope: ScopeTag::Company, audit: "create_permission_policy" },
    CommandMapping { command: "delete_permission_policy", permission: ROLES_DELETE, scope: ScopeTag::Company, audit: "delete_permission_policy" },
    CommandMapping { command: "get_my_permissions", permission: PLATFORM_ACCESS, scope: ScopeTag::Platform, audit: "" },

    // ── Настройки ──
    CommandMapping { command: "get_app_config", permission: SETTINGS_READ, scope: ScopeTag::Platform, audit: "" },
    CommandMapping { command: "save_app_config", permission: SETTINGS_MANAGE, scope: ScopeTag::Platform, audit: "save_settings" },

    // ── Аудит ──
    CommandMapping { command: "list_audit_logs", permission: AUDIT_READ, scope: ScopeTag::Company, audit: "" },
    CommandMapping { command: "get_audit_entry", permission: AUDIT_READ, scope: ScopeTag::Company, audit: "" },

    // ── События ──
    CommandMapping { command: "list_events", permission: DOCUMENTS_READ, scope: ScopeTag::Company, audit: "" },
    CommandMapping { command: "get_event", permission: DOCUMENTS_READ, scope: ScopeTag::Company, audit: "" },
    CommandMapping { command: "list_stream_events", permission: DOCUMENTS_READ, scope: ScopeTag::Company, audit: "" },

    // ── Entity Types ──
    CommandMapping { command: "list_entity_types", permission: METADATA_READ, scope: ScopeTag::Metadata, audit: "" },
    CommandMapping { command: "get_entity_type", permission: METADATA_READ, scope: ScopeTag::Metadata, audit: "" },
    CommandMapping { command: "create_entity_type", permission: METADATA_CREATE, scope: ScopeTag::Metadata, audit: "" },
    CommandMapping { command: "update_entity_type", permission: METADATA_UPDATE, scope: ScopeTag::Metadata, audit: "" },
    CommandMapping { command: "delete_entity_type", permission: METADATA_DELETE, scope: ScopeTag::Metadata, audit: "" },

    // ── Entity Fields ──
    CommandMapping { command: "list_entity_fields", permission: METADATA_READ, scope: ScopeTag::Metadata, audit: "" },
    CommandMapping { command: "create_entity_field", permission: METADATA_CREATE, scope: ScopeTag::Metadata, audit: "" },
    CommandMapping { command: "update_entity_field", permission: METADATA_UPDATE, scope: ScopeTag::Metadata, audit: "" },
    CommandMapping { command: "delete_entity_field", permission: METADATA_DELETE, scope: ScopeTag::Metadata, audit: "" },

    // ── Entity States ──
    CommandMapping { command: "list_entity_states", permission: METADATA_READ, scope: ScopeTag::Metadata, audit: "" },
    CommandMapping { command: "create_entity_state", permission: METADATA_CREATE, scope: ScopeTag::Metadata, audit: "" },
    CommandMapping { command: "update_entity_state", permission: METADATA_UPDATE, scope: ScopeTag::Metadata, audit: "" },
    CommandMapping { command: "delete_entity_state", permission: METADATA_DELETE, scope: ScopeTag::Metadata, audit: "" },

    // ── Entity Transitions ──
    CommandMapping { command: "list_entity_transitions", permission: METADATA_READ, scope: ScopeTag::Metadata, audit: "" },
    CommandMapping { command: "create_entity_transition", permission: METADATA_CREATE, scope: ScopeTag::Metadata, audit: "" },
    CommandMapping { command: "update_entity_transition", permission: METADATA_UPDATE, scope: ScopeTag::Metadata, audit: "" },
    CommandMapping { command: "delete_entity_transition", permission: METADATA_DELETE, scope: ScopeTag::Metadata, audit: "" },

    // ── Entity Forms ──
    CommandMapping { command: "list_entity_forms", permission: METADATA_READ, scope: ScopeTag::Metadata, audit: "" },
    CommandMapping { command: "create_entity_form", permission: METADATA_CREATE, scope: ScopeTag::Metadata, audit: "" },
    CommandMapping { command: "update_entity_form", permission: METADATA_UPDATE, scope: ScopeTag::Metadata, audit: "" },
    CommandMapping { command: "delete_entity_form", permission: METADATA_DELETE, scope: ScopeTag::Metadata, audit: "" },

    // ── Entity Actions ──
    CommandMapping { command: "list_entity_actions", permission: METADATA_READ, scope: ScopeTag::Metadata, audit: "" },
    CommandMapping { command: "create_entity_action", permission: METADATA_CREATE, scope: ScopeTag::Metadata, audit: "" },
    CommandMapping { command: "update_entity_action", permission: METADATA_UPDATE, scope: ScopeTag::Metadata, audit: "" },
    CommandMapping { command: "delete_entity_action", permission: METADATA_DELETE, scope: ScopeTag::Metadata, audit: "" },

    // ── Entity Action execution ──
    CommandMapping { command: "validate_entity_transition", permission: DOCUMENTS_READ, scope: ScopeTag::Object, audit: "" },
    CommandMapping { command: "execute_entity_action", permission: DOCUMENTS_APPROVE, scope: ScopeTag::Object, audit: "execute_entity_action" },

    // ── Криптографическая подпись ──
    CommandMapping { command: "list_crypto_certificates", permission: DOCUMENTS_READ, scope: ScopeTag::Company, audit: "" },
    CommandMapping { command: "sign_document", permission: DOCUMENTS_APPROVE, scope: ScopeTag::Company, audit: "sign_document" },
    CommandMapping { command: "verify_document_signature", permission: DOCUMENTS_READ, scope: ScopeTag::Company, audit: "" },

    // ── Объекты ──
    CommandMapping { command: "list_objects", permission: DOCUMENTS_READ, scope: ScopeTag::Company, audit: "" },
    CommandMapping { command: "get_object", permission: DOCUMENTS_READ, scope: ScopeTag::Company, audit: "" },
    CommandMapping { command: "create_object", permission: DOCUMENTS_CREATE, scope: ScopeTag::Company, audit: "create_document" },
    CommandMapping { command: "update_object", permission: DOCUMENTS_UPDATE, scope: ScopeTag::Object, audit: "update_document" },
    CommandMapping { command: "post_object", permission: DOCUMENTS_APPROVE, scope: ScopeTag::Object, audit: "post_document" },
    CommandMapping { command: "cancel_object", permission: DOCUMENTS_CANCEL, scope: ScopeTag::Object, audit: "cancel_document" },
    CommandMapping { command: "restore_object_version", permission: DOCUMENTS_UPDATE, scope: ScopeTag::Object, audit: "restore_document" },
    CommandMapping { command: "list_object_versions", permission: DOCUMENTS_READ, scope: ScopeTag::Company, audit: "" },

    // ── WASM плагины ──
    CommandMapping { command: "wasm_load", permission: PLUGINS_EXECUTE, scope: ScopeTag::Platform, audit: "" },
    CommandMapping { command: "wasm_unload", permission: PLUGINS_MANAGE, scope: ScopeTag::Platform, audit: "" },
    CommandMapping { command: "wasm_list", permission: PLUGINS_READ, scope: ScopeTag::Platform, audit: "" },
    CommandMapping { command: "plugin_call", permission: PLUGINS_EXECUTE, scope: ScopeTag::Platform, audit: "" },

    // ── Печатные формы ──
    CommandMapping { command: "print_list_templates", permission: PRINT_READ, scope: ScopeTag::Company, audit: "" },
    CommandMapping { command: "print_get_template", permission: PRINT_READ, scope: ScopeTag::Company, audit: "" },
    CommandMapping { command: "print_create_template", permission: PRINT_CREATE, scope: ScopeTag::Company, audit: "create_print_template" },
    CommandMapping { command: "print_update_template", permission: PRINT_UPDATE, scope: ScopeTag::Company, audit: "update_print_template" },
    CommandMapping { command: "print_delete_template", permission: PRINT_DELETE, scope: ScopeTag::Company, audit: "delete_print_template" },
    CommandMapping { command: "print_render", permission: PRINT_READ, scope: ScopeTag::Company, audit: "" },

    // ── Нумерация ──
    CommandMapping { command: "numbering_list", permission: NUMBERING_READ, scope: ScopeTag::Company, audit: "" },
    CommandMapping { command: "numbering_get", permission: NUMBERING_READ, scope: ScopeTag::Company, audit: "" },
    CommandMapping { command: "numbering_update_format", permission: NUMBERING_MANAGE, scope: ScopeTag::Company, audit: "save_settings" },
    CommandMapping { command: "numbering_reset", permission: NUMBERING_MANAGE, scope: ScopeTag::Company, audit: "save_settings" },

    // ── Модули (прикладные) ──
    CommandMapping { command: "modules_list", permission: MODULES_READ, scope: ScopeTag::Platform, audit: "" },
    CommandMapping { command: "modules_get", permission: MODULES_READ, scope: ScopeTag::Platform, audit: "" },
    CommandMapping { command: "modules_install", permission: MODULES_MANAGE, scope: ScopeTag::Platform, audit: "" },
    CommandMapping { command: "modules_uninstall", permission: MODULES_MANAGE, scope: ScopeTag::Platform, audit: "" },
    CommandMapping { command: "modules_enable", permission: MODULES_MANAGE, scope: ScopeTag::Platform, audit: "" },
    CommandMapping { command: "modules_disable", permission: MODULES_MANAGE, scope: ScopeTag::Platform, audit: "" },
    CommandMapping { command: "modules_update_settings", permission: MODULES_MANAGE, scope: ScopeTag::Platform, audit: "" },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_map_no_duplicates() {
        let mut names: Vec<&str> = COMMAND_MAP.iter().map(|m| m.command).collect();
        let before = names.len();
        names.dedup();
        assert_eq!(before, names.len(), "COMMAND_MAP has duplicate command names");
    }

    #[test]
    fn command_map_count_matches() {
        // After migration all 100 commands should be mapped
        assert!(COMMAND_MAP.len() >= 100, "COMMAND_MAP has {} entries, expected >= 100", COMMAND_MAP.len());
    }

    #[test]
    fn audit_actions_are_valid() {
        for m in COMMAND_MAP {
            if !m.audit.is_empty() {
                let result: Result<crate::audit::AuditableAction, _> = m.audit.parse();
                assert!(result.is_ok(), "Invalid audit action '{}' for command '{}'", m.audit, m.command);
            }
        }
    }

    #[test]
    fn read_commands_have_no_audit() {
        for m in COMMAND_MAP {
            if m.permission.ends_with(".read") {
                assert!(m.audit.is_empty(), "Read-only command '{}' should not have audit action '{}'", m.command, m.audit);
            }
        }
    }
}
