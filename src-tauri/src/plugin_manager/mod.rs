// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

pub mod commands;
pub mod storage;
pub mod workflow;

use extism::*;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Instant;

use crate::modules::required_capability;

// ── Types ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginFunction {
    pub name: String,
    pub label: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub source: String,
    pub functions: Vec<PluginFunction>,
    /// Коды entity_type, проведение которых оркестрирует модуль
    /// (post_object/cancel_object делегируют on_post/on_cancel).
    #[serde(default)]
    pub handled_documents: Vec<String>,
}

/// Единые коды ошибок host-функций (контракт Plugin SDK).
/// Любая host-функция возвращает конверт:
///   успех:  {"ok": true,  "data": ...}
///   ошибка: {"ok": false, "error": {"code": "...", "message": "..."}}
pub mod err {
    pub const NO_DATABASE: &str = "NO_DATABASE";
    pub const NO_COMPANY: &str = "NO_COMPANY";
    pub const INVALID_COMPANY: &str = "INVALID_COMPANY";
    pub const NO_USER: &str = "NO_USER";
    pub const INVALID_USER: &str = "INVALID_USER";
    pub const NO_MODULE_CODE: &str = "NO_MODULE_CODE";
    pub const INVALID_UUID: &str = "INVALID_UUID";
    pub const INVALID_JSON: &str = "INVALID_JSON";
    pub const INVALID_VERSION: &str = "INVALID_VERSION";
    pub const INVALID_ACTION: &str = "INVALID_ACTION";
    pub const NOT_FOUND: &str = "NOT_FOUND";
    pub const DB_ERROR: &str = "DB_ERROR";
    pub const SCRIPT_FAILED: &str = "SCRIPT_FAILED";
}

pub struct WasmPlugin {
    plugin: Plugin,
    pub info: ModuleInfo,
    pub ctx: Arc<RwLock<PluginContext>>,
    pub capabilities: Vec<String>,
}

// ── Mutable context (обновляется при каждом plugin_call) ───

#[derive(Clone, Default)]
pub struct PluginContext {
    pub company_id: Option<String>,
    pub user_id: Option<String>,
    pub user_login: Option<String>,
    pub display_name: Option<String>,
    /// Активная роль (для совместимости)
    pub role_id: Option<String>,
    /// ВСЕ активные роли пользователя в компании (мультипрофиль)
    pub role_ids: Vec<String>,
}

// ── HostData (только db + общий контекст) ──────────────────

#[derive(Clone)]
pub struct HostData {
    pub db: Option<crate::db::MongoClient>,
    pub ctx: Arc<RwLock<PluginContext>>,
    pub module_code: Option<String>,
    pub capabilities: Vec<String>,
}

// ── Response envelope (контракт Plugin SDK) ────────────────

pub(crate) fn error_response(code: &str, message: &str) -> String {
    serde_json::json!({
        "ok": false,
        "error": {
            "code": code,
            "message": message,
        }
    })
    .to_string()
}

pub(crate) fn ok_response(data: serde_json::Value) -> String {
    serde_json::json!({
        "ok": true,
        "data": data,
    })
    .to_string()
}

pub(crate) fn check_capability(hd: &HostData, function_name: &str) -> Result<(), String> {
    if let Some(required) = required_capability(function_name) {
        if !hd.capabilities.iter().any(|c| c == required) {
            let module_code = hd.module_code.as_deref().unwrap_or("unknown");
            return Err(error_response(
                "CAPABILITY_DENIED",
                &format!(
                    "Модуль '{}' не имеет capability '{}' для вызова '{}'",
                    module_code, required, function_name
                ),
            ));
        }
    }
    Ok(())
}

// ── Host Functions ─────────────────────────────────────────

// --- create_object (capability: objects.create) ---

extism::host_fn!(create_object_impl(user_data: HostData; entity_type_id: String, data: String) -> String {
    let hd = user_data.get()?.lock().unwrap().clone();
    if let Err(e) = check_capability(&hd, "create_object") {
        return Ok(e);
    }

    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let db_client = match hd.db {
                Some(client) => client,
                None => return error_response("NO_DATABASE", "База данных не инициализирована"),
            };

            let data_val: serde_json::Value = match serde_json::from_str(&data) {
                Ok(d) => d,
                Err(e) => return error_response("INVALID_JSON", &format!("Невалидный JSON: {}", e)),
            };

            let ctx = hd.ctx.read().unwrap();

            let company_id = match ctx.company_id.as_ref() {
                Some(cid) => match uuid::Uuid::parse_str(cid) {
                    Ok(uid) => crate::core::CompanyId(uid),
                    Err(_) => return error_response("INVALID_COMPANY", "Невалидный UUID компании"),
                },
                None => return error_response("NO_COMPANY", "Компания не выбрана"),
            };

            let user_id = match ctx.user_id.as_ref() {
                Some(uid) => match uuid::Uuid::parse_str(uid) {
                    Ok(uuid) => crate::core::UserId(uuid),
                    Err(_) => return error_response("INVALID_USER", "Невалидный UUID пользователя"),
                },
                None => return error_response("NO_USER", "Пользователь не аутентифицирован"),
            };

            let actor = crate::events::ActorSnapshot {
                user_id: user_id.clone(),
                login: ctx.user_login.clone().unwrap_or_default(),
                full_name: ctx.display_name.clone(),
                position: None,
                company_id: company_id.clone(),
            };

            let input = crate::objects::CreateObjectInput {
                entity_type_id,
                data: data_val,
                parent_id: None,
                date: None,
            };

            match crate::objects::service::ObjectService::create(&db_client, input, company_id, user_id, actor).await {
                Ok(outcome) => ok_response(serde_json::json!({ "id": outcome.result._id.to_string() })),
                Err(e) => error_response("CREATE_FAILED", &e.to_string()),
            }
        })
    });
    Ok(result)
});

// --- list_objects (capability: objects.read) ---

extism::host_fn!(list_objects_impl(user_data: HostData; entity_type_id: String, limit: String) -> String {
    let hd = user_data.get()?.lock().unwrap().clone();
    if let Err(e) = check_capability(&hd, "list_objects") {
        return Ok(e);
    }

    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let db = match hd.db {
                Some(client) => client,
                None => return error_response("NO_DATABASE", "База данных не инициализирована"),
            };

            let ctx = hd.ctx.read().unwrap();
            let company_id = match ctx.company_id.as_ref() {
                Some(cid) => match uuid::Uuid::parse_str(cid) {
                    Ok(uid) => crate::core::CompanyId(uid),
                    Err(_) => return error_response("INVALID_COMPANY", "Невалидный UUID компании"),
                },
                None => return error_response("NO_COMPANY", "Компания не выбрана"),
            };

            let limit: i64 = limit.parse().unwrap_or(100);
            let filters = crate::objects::ObjectFilters {
                entity_type_id: Some(entity_type_id),
                limit: Some(limit),
                ..Default::default()
            };

            match crate::objects::service::ObjectService::list(&db, company_id, filters).await {
                Ok(page) => {
                    let objects: Vec<serde_json::Value> = page.objects.into_iter().map(|o| {
                        let state_str = serde_json::to_string(&o.state).unwrap_or_default()
                            .trim_matches('"').to_string();
                        serde_json::json!({
                            "id": o._id.to_string(),
                            "number": o.number,
                            "date": o.date,
                            "state": state_str,
                            "version": o.version,
                            "data": o.data,
                        })
                    }).collect();
                    ok_response(serde_json::json!({ "objects": objects, "total_count": page.total_count }))
                }
                Err(e) => error_response("LIST_FAILED", &e.to_string()),
            }
        })
    });
    Ok(result)
});

// --- get_object (capability: objects.read) ---

extism::host_fn!(pub get_object_impl(user_data: HostData; id: String) -> String {
    let hd = user_data.get()?.lock().unwrap().clone();
    if let Err(e) = check_capability(&hd, "get_object") {
        return Ok(e);
    }

    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let db = match hd.db {
                Some(client) => client,
                None => return error_response("NO_DATABASE", "База данных не инициализирована"),
            };

            let uuid = match uuid::Uuid::parse_str(&id) {
                Ok(u) => u,
                Err(_) => return error_response("INVALID_UUID", "Невалидный UUID объекта"),
            };

            match crate::objects::service::ObjectService::get(&db, uuid).await {
                Ok(obj) => {
                    let state_str = serde_json::to_string(&obj.state).unwrap_or_default()
                        .trim_matches('"').to_string();
                    ok_response(serde_json::json!({
                        "id": obj._id.to_string(),
                        "number": obj.number,
                        "date": obj.date,
                        "state": state_str,
                        "version": obj.version,
                        "data": obj.data,
                        "entity_type_id": obj.entity_type_id,
                    }))
                }
                Err(e) => error_response("GET_FAILED", &e.to_string()),
            }
        })
    });
    Ok(result)
});

// --- update_object (capability: objects.update) ---

extism::host_fn!(update_object_impl(user_data: HostData; id: String, data: String, version: String) -> String {
    let hd = user_data.get()?.lock().unwrap().clone();
    if let Err(e) = check_capability(&hd, "update_object") {
        return Ok(e);
    }

    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let db = match hd.db {
                Some(client) => client,
                None => return error_response("NO_DATABASE", "База данных не инициализирована"),
            };

            let uuid = match uuid::Uuid::parse_str(&id) {
                Ok(u) => u,
                Err(_) => return error_response("INVALID_UUID", "Невалидный UUID объекта"),
            };

            let data_val: serde_json::Value = match serde_json::from_str(&data) {
                Ok(d) => d,
                Err(e) => return error_response("INVALID_JSON", &format!("Невалидный JSON: {}", e)),
            };

            let ver: i64 = match version.parse() {
                Ok(v) => v,
                Err(_) => return error_response("INVALID_VERSION", "Версия должна быть числом"),
            };

            let ctx = hd.ctx.read().unwrap();
            let company_id = match ctx.company_id.as_ref() {
                Some(cid) => match uuid::Uuid::parse_str(cid) {
                    Ok(uid) => crate::core::CompanyId(uid),
                    Err(_) => return error_response("INVALID_COMPANY", "Невалидный UUID компании"),
                },
                None => return error_response("NO_COMPANY", "Компания не выбрана"),
            };

            let user_id = match ctx.user_id.as_ref() {
                Some(uid) => match uuid::Uuid::parse_str(uid) {
                    Ok(uuid) => crate::core::UserId(uuid),
                    Err(_) => return error_response("INVALID_USER", "Невалидный UUID пользователя"),
                },
                None => return error_response("NO_USER", "Пользователь не аутентифицирован"),
            };

            let actor = crate::events::ActorSnapshot {
                user_id: user_id.clone(),
                login: ctx.user_login.clone().unwrap_or_default(),
                full_name: ctx.display_name.clone(),
                position: None,
                company_id: company_id.clone(),
            };

            let input = crate::objects::UpdateObjectInput {
                data: data_val,
                version: ver,
                reason: Some("Обновление через WASM-модуль".into()),
            };

            match crate::objects::service::ObjectService::update(&db, uuid, input, user_id, actor, company_id).await {
                Ok(outcome) => ok_response(serde_json::json!({ "id": outcome.result._id.to_string(), "version": outcome.result.version })),
                Err(e) => error_response("UPDATE_FAILED", &e.to_string()),
            }
        })
    });
    Ok(result)
});

// --- log_message (capability: logging) ---

extism::host_fn!(pub log_message_impl(user_data: HostData; msg: String) -> String {
    let hd = user_data.get()?.lock().unwrap().clone();
    if let Err(e) = check_capability(&hd, "log_message") {
        return Ok(e);
    }

    let module_code = hd.module_code.unwrap_or_else(|| "unknown".into());
    tracing::info!("[Module:{}] {}", module_code, msg);
    Ok(String::new())
});

// --- get_entity_type (capability: metadata.read) ---

extism::host_fn!(pub get_entity_type_impl(user_data: HostData; id: String) -> String {
    let hd = user_data.get()?.lock().unwrap().clone();
    if let Err(e) = check_capability(&hd, "get_entity_type") {
        return Ok(e);
    }

    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let db = match hd.db {
                Some(client) => client,
                None => return error_response("NO_DATABASE", "База данных не инициализирована"),
            };

            let uuid = match uuid::Uuid::parse_str(&id) {
                Ok(u) => u,
                Err(_) => return error_response("INVALID_UUID", "Невалидный UUID типа сущности"),
            };

            match crate::meta::service::EntityTypeService::get(&db, uuid).await {
                Ok(et) => ok_response(serde_json::json!({
                    "id": et._id.to_string(),
                    "code": et.code,
                    "name": et.name,
                    "kind": serde_json::to_string(&et.kind).unwrap_or_default().trim_matches('"'),
                })),
                Err(e) => error_response("GET_ENTITY_TYPE_FAILED", &e.to_string()),
            }
        })
    });
    Ok(result)
});

// --- list_entity_fields (capability: metadata.read) ---

extism::host_fn!(list_entity_fields_impl(user_data: HostData; entity_type_id: String) -> String {
    let hd = user_data.get()?.lock().unwrap().clone();
    if let Err(e) = check_capability(&hd, "list_entity_fields") {
        return Ok(e);
    }

    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let db = match hd.db {
                Some(client) => client,
                None => return error_response("NO_DATABASE", "База данных не инициализирована"),
            };

            let uuid = match uuid::Uuid::parse_str(&entity_type_id) {
                Ok(u) => u,
                Err(_) => return error_response("INVALID_UUID", "Невалидный UUID типа сущности"),
            };

            match crate::meta::service::EntityFieldService::list_by_type(&db, uuid).await {
                Ok(fields) => {
                    let fields_json: Vec<serde_json::Value> = fields.into_iter().map(|f| {
                        let kind_str = serde_json::to_string(&f.field_kind).unwrap_or_default()
                            .trim_matches('"').to_string();
                        serde_json::json!({
                            "id": f._id.to_string(),
                            "code": f.code,
                            "name": f.name,
                            "field_kind": kind_str,
                            "is_required": f.is_required,
                            "is_readonly": f.is_readonly,
                        })
                    }).collect();
                    ok_response(serde_json::json!({ "fields": fields_json }))
                }
                Err(e) => error_response("LIST_FIELDS_FAILED", &e.to_string()),
            }
        })
    });
    Ok(result)
});

// ── Wasmtime disk cache ──────────────────────────────────────

/// Возвращает путь к TOML-конфигу кэша wasmtime.
/// Создаёт каталог + config.toml при первом вызове (OnceLock).
/// Если `dirs::cache_dir()` недоступен — кэш работает через дефолт wasmtime.
fn wasmtime_cache_toml() -> Option<&'static std::path::Path> {
    static PATH: OnceLock<Option<std::path::PathBuf>> = OnceLock::new();

    PATH.get_or_init(|| {
        let base = dirs::cache_dir().unwrap_or_else(|| std::env::temp_dir()).join("2c-platform").join("wasmtime-cache");
        if let Err(e) = std::fs::create_dir_all(&base) {
            tracing::warn!("[wasmtime-cache] Не удалось создать каталог {base:?}: {e}");
            return None;
        }
        let toml = base.join("config.toml");
        if !toml.exists() {
            let content = format!("[cache]\ndirectory = \"{}\"\n", base.display());
            if let Err(e) = std::fs::write(&toml, &content) {
                tracing::warn!("[wasmtime-cache] Не удалось создать {}: {e}", toml.display());
                return None;
            }
            tracing::info!("[wasmtime-cache] Создан: {}", toml.display());
        }
        Some(toml)
    }).as_deref()
}

// ── WasmPlugin ─────────────────────────────────────────────

impl WasmPlugin {
    /// Загрузить WASM-модуль с ресурсными лимитами и capability checks.
    pub async fn load(wasm_bytes: Vec<u8>, wasm_name: String, host_data: HostData) -> Result<Self, String> {
        let mut manifest = Manifest::new([Wasm::data(wasm_bytes)]);
        manifest.timeout_ms = Some(10_000);
        manifest.memory.max_pages = Some(256);

        let ctx = host_data.ctx.clone();
        let capabilities = host_data.capabilities.clone();
        let mut builder = PluginBuilder::new(&manifest)
            .with_function("create_object", [PTR, PTR], [PTR], UserData::new(host_data.clone()), create_object_impl)
            .with_function("list_objects",  [PTR, PTR], [PTR], UserData::new(host_data.clone()), list_objects_impl)
            .with_function("get_object",    [PTR],      [PTR], UserData::new(host_data.clone()), get_object_impl)
            .with_function("update_object", [PTR, PTR, PTR], [PTR], UserData::new(host_data.clone()), update_object_impl)
            .with_function("transition_object", [PTR, PTR, PTR], [PTR], UserData::new(host_data.clone()), workflow::transition_object_impl)
            .with_function("log_message",   [PTR],      [],    UserData::new(host_data.clone()), log_message_impl)
            .with_function("get_entity_type",    [PTR], [PTR], UserData::new(host_data.clone()), get_entity_type_impl)
            .with_function("list_entity_fields", [PTR], [PTR], UserData::new(host_data.clone()), list_entity_fields_impl)
            .with_function("kv_put",    [PTR, PTR], [PTR], UserData::new(host_data.clone()), storage::kv_put_impl)
            .with_function("kv_get",    [PTR],      [PTR], UserData::new(host_data.clone()), storage::kv_get_impl)
            .with_function("kv_list",   [PTR],      [PTR], UserData::new(host_data.clone()), storage::kv_list_impl)
            .with_function("kv_delete", [PTR],      [PTR], UserData::new(host_data.clone()), storage::kv_delete_impl)
            .with_function("kv_put_if_absent", [PTR, PTR], [PTR], UserData::new(host_data.clone()), storage::kv_put_if_absent_impl)
            .with_function("run_script",  [PTR, PTR], [PTR], UserData::new(host_data.clone()), workflow::run_script_impl)
            .with_function("notify_user", [PTR, PTR, PTR], [PTR], UserData::new(host_data.clone()), workflow::notify_user_impl)
            .with_function("whoami", [],      [PTR], UserData::new(host_data.clone()), workflow::whoami_impl)
            .with_function("now_ms", [],      [PTR], UserData::new(host_data.clone()), workflow::now_ms_impl)
            .with_function("module_settings", [], [PTR], UserData::new(host_data.clone()), workflow::module_settings_impl)
            .with_function("emit_event", [PTR, PTR, PTR], [PTR], UserData::new(host_data.clone()), workflow::emit_event_impl)
            .with_function("signature_required", [PTR, PTR, PTR], [PTR], UserData::new(host_data.clone()), workflow::signature_required_impl)
            .with_function("cms_verify", [PTR, PTR], [PTR], UserData::new(host_data.clone()), workflow::cms_verify_impl)
            .with_function("users_by_role", [PTR], [PTR], UserData::new(host_data.clone()), workflow::users_by_role_impl)
            .with_function("tx_begin",   [PTR],           [PTR], UserData::new(host_data.clone()), workflow::tx_begin_impl)
            .with_function("tx_add_op",  [PTR, PTR, PTR], [PTR], UserData::new(host_data.clone()), workflow::tx_add_op_impl)
            .with_function("tx_commit",  [PTR],           [PTR], UserData::new(host_data.clone()), workflow::tx_commit_impl)
            .with_function("stock_doc_cost", [PTR],       [PTR], UserData::new(host_data.clone()), workflow::stock_doc_cost_impl)
            .with_fuel_limit(10_000_000);

        if let Some(cache_path) = wasmtime_cache_toml() {
            builder = builder.with_cache_config(cache_path);
        }

        let t0 = Instant::now();
        let mut plugin = builder.build()
            .map_err(|e| format!("Ошибка загрузки плагина {}: {}", wasm_name, e))?;
        tracing::info!("[Plugin compile] {} build={}ms", wasm_name, t0.elapsed().as_millis());

        let t1 = Instant::now();
        let info_json = plugin.call::<&[u8], String>("get_info", b"")
            .map_err(|e| format!("get_info() не удался: {}", e))?;
        tracing::info!("[Plugin init] {} get_info={}ms", wasm_name, t1.elapsed().as_millis());

        let wasm_info: WasmModuleInfo = serde_json::from_str(&info_json)
            .map_err(|e| format!("get_info() невалидный JSON: {}", e))?;

        let info = ModuleInfo {
            id: uuid::Uuid::new_v4().to_string(),
            name: wasm_info.name.clone(),
            version: wasm_info.version,
            source: "bytes".into(),
            functions: wasm_info.functions,
            handled_documents: wasm_info.handled_documents,
        };

        tracing::info!(
            "[Plugin loaded] {} v{} — {} functions: {} (fuel=10M, memory=256 pages, timeout=10s, capabilities=[{}])",
            info.name, info.version, info.functions.len(),
            info.functions.iter().map(|f| f.name.as_str()).collect::<Vec<_>>().join(", "),
            capabilities.join(", "),
        );

        Ok(Self { plugin, info, ctx, capabilities })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_context(
        &self,
        company_id: Option<String>,
        user_id: Option<String>,
        user_login: Option<String>,
        display_name: Option<String>,
        role_id: Option<String>,
        role_ids: Vec<String>,
    ) {
        if let Ok(mut ctx) = self.ctx.write() {
            ctx.company_id = company_id;
            ctx.user_id = user_id;
            ctx.user_login = user_login;
            ctx.display_name = display_name;
            ctx.role_id = role_id;
            ctx.role_ids = role_ids;
        }
    }

    pub fn call(&mut self, function: &str, input: &[u8]) -> Result<Vec<u8>, String> {
        self.plugin
            .call::<&[u8], Vec<u8>>(function, input)
            .map_err(|e| format!("Ошибка вызова '{}': {}", function, e))
    }
}

// ── Deserialization helper for get_info() ──────────────────

/// Манифест, возвращаемый модулем из get_info().
/// Единственный источник правды о модуле: код, версия,
/// запрашиваемые capabilities и требуемые RBAC-политики.
#[derive(Deserialize)]
pub struct WasmModuleInfo {
    pub name: String,
    pub version: String,
    pub functions: Vec<PluginFunction>,
    #[serde(default)]
    pub handled_documents: Vec<String>,
    /// Код модуля (если не указан — используется name).
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    /// Требуемая версия API хост-функций (по умолчанию текущая).
    #[serde(default)]
    pub api_version: Option<String>,
    /// Запрашиваемые capabilities (валидируются хостом при установке).
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Требуемые RBAC-политики ("subsystem.action"), создаются при install.
    #[serde(default)]
    pub permissions: Vec<String>,
}

#[cfg(test)]
mod cache_tests {
    use super::*;

    #[test]
    fn wasmtime_cache_config_created_and_valid() {
        let path = wasmtime_cache_toml();
        assert!(path.is_some(), "cache toml path должен быть Some");
        let p = path.unwrap();
        assert!(p.exists(), "{} должен существовать", p.display());
        let content = std::fs::read_to_string(p).expect("чтение config.toml");
        assert!(content.contains("[cache]"), "секция [cache]: {content}");
        assert!(content.contains("directory"), "ключ directory: {content}");
        // Повторный вызов возвращает тот же путь (OnceLock)
        assert_eq!(wasmtime_cache_toml(), path);
    }
}
