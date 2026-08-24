mod auth;
mod actions;
mod audit;
mod commands;
mod company;
pub mod core;
mod crypto;
pub mod db;
mod devices;
pub mod events;
pub mod ledger;
mod meta;
mod modules;
mod notify;
mod objects;
mod numbering;
pub mod permission_policy;
mod person;
pub mod plugin_manager;
mod print;
mod rhai;
mod role;
mod settings;
mod signing;
pub mod stock;
pub mod tx;
mod user;
mod user_certificate;
mod user_contact;
mod user_profile;

use commands::AppState;
use tokio::sync::Mutex;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    dotenvy::dotenv().ok();
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::default().build())
        .plugin(tauri_plugin_shell::init())
        .manage(Mutex::new(AppState::new()))
        .invoke_handler(tauri::generate_handler![
            commands::get_diagnostics,
            commands::validate_rhai_script,
            commands::execute_rhai_script,
            commands::connect_db,
            commands::list_companies,
            commands::get_company,
            commands::create_company,
            commands::update_company,
            commands::delete_company,
            commands::list_users,
            commands::get_user,
            commands::create_user,
            commands::update_user,
            commands::delete_user,
            commands::authenticate,
            commands::create_role,
            commands::list_roles,
            commands::delete_role,
            commands::get_me,
            commands::get_app_config,
            commands::save_app_config,
            commands::get_person,
            commands::update_person,
            commands::list_user_contacts,
            commands::create_contact,
            commands::update_contact,
            commands::delete_contact,
            commands::list_user_profiles,
            commands::add_user_profile,
            commands::update_user_profile,
            commands::remove_user_profile,
            commands::list_user_certificates,
            commands::deactivate_certificate,
            commands::switch_company,
            commands::get_contact_types,
            commands::save_contact_types,
            commands::list_audit_logs,
            commands::get_audit_entry,
            commands::list_permission_policies,
            commands::create_permission_policy,
            commands::delete_permission_policy,
            commands::update_role,
            commands::get_my_permissions,
            commands::list_events,
            commands::get_event,
            commands::list_stream_events,
            commands::list_entity_types,
            commands::get_entity_type,
            commands::create_entity_type,
            commands::update_entity_type,
            commands::delete_entity_type,
            commands::list_entity_fields,
            commands::create_entity_field,
            commands::update_entity_field,
            commands::delete_entity_field,
            commands::list_entity_states,
            commands::create_entity_state,
            commands::update_entity_state,
            commands::delete_entity_state,
            commands::list_entity_transitions,
            commands::create_entity_transition,
            commands::update_entity_transition,
            commands::delete_entity_transition,
            commands::list_entity_forms,
            commands::create_entity_form,
            commands::update_entity_form,
            commands::delete_entity_form,
            commands::list_entity_actions,
            commands::create_entity_action,
            commands::update_entity_action,
            commands::delete_entity_action,
            commands::validate_entity_transition,
            commands::execute_entity_action,
            commands::list_objects,
            commands::get_object,
            commands::create_object,
            commands::update_object,
            commands::post_object,
            commands::cancel_object,
            commands::restore_object_version,
            commands::list_object_versions,
            plugin_manager::commands::wasm_load,
            plugin_manager::commands::wasm_unload,
            plugin_manager::commands::wasm_list,
            plugin_manager::commands::plugin_call,
            print::commands::print_list_templates,
            print::commands::print_get_template,
            print::commands::print_create_template,
            print::commands::print_update_template,
            print::commands::print_delete_template,
            print::commands::print_render,
            numbering::commands::numbering_list,
            numbering::commands::numbering_get,
            numbering::commands::numbering_update_format,
            numbering::commands::numbering_reset,
            modules::commands::modules_list,
            modules::commands::modules_get,
            modules::commands::modules_install,
            modules::commands::modules_uninstall,
            modules::commands::modules_enable,
            modules::commands::modules_disable,
            modules::commands::modules_update_settings,
            signing::commands::list_crypto_certificates,
            signing::commands::sign_document,
            signing::commands::verify_document_signature,
            signing::commands::create_test_certificate,
            stock::commands::stock_seed_metadata,
            stock::commands::stock_balances,
            stock::commands::stock_report_handover,
            stock::commands::stock_report_overdue,
            stock::commands::signature_policies_list,
            stock::commands::signature_policies_upsert,
            stock::commands::signature_policies_delete,
            stock::commands::signature_required_for_doc,
            ledger::commands::ledger_accounts_list,
            ledger::commands::ledger_account_create,
            ledger::commands::ledger_account_update,
            ledger::commands::ledger_periods_list,
            ledger::commands::ledger_period_set_state,
            commands::notifications_list,
            commands::notifications_mark_read,
            devices::commands::devices_list,
            devices::commands::devices_get,
            devices::commands::devices_save,
            devices::commands::devices_delete,
            devices::commands::devices_connect,
            devices::commands::devices_disconnect,
            devices::commands::devices_test,
            devices::commands::devices_list_ports,
            devices::commands::devices_wedge_scan,
        ])
        .setup(|app| {
            tracing::info!("2C Platform запускается...");
            let window = app.get_webview_window("main").unwrap();
            window.set_title("2C Platform v0.1")?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("ошибка при запуске 2C Platform");
}
