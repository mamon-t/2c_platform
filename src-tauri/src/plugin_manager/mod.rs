pub mod commands;

use extism::*;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

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
}

// ── HostData (только db + общий контекст) ──────────────────

#[derive(Clone)]
pub struct HostData {
    pub db: Option<crate::db::MongoClient>,
    pub ctx: Arc<RwLock<PluginContext>>,
    pub module_code: Option<String>,
    pub capabilities: Vec<String>,
}

// ── Error response helper ──────────────────────────────────

fn error_response(code: &str, message: &str) -> String {
    serde_json::json!({
        "ok": false,
        "error": {
            "code": code,
            "message": message,
        }
    })
    .to_string()
}

fn ok_response(data: serde_json::Value) -> String {
    serde_json::json!({
        "ok": true,
        "data": data,
    })
    .to_string()
}

fn check_capability(hd: &HostData, function_name: &str) -> Result<(), String> {
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
                Ok(obj) => ok_response(serde_json::json!({ "id": obj._id.to_string() })),
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

extism::host_fn!(get_object_impl(user_data: HostData; id: String) -> String {
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
                Ok(obj) => ok_response(serde_json::json!({ "id": obj._id.to_string(), "version": obj.version })),
                Err(e) => error_response("UPDATE_FAILED", &e.to_string()),
            }
        })
    });
    Ok(result)
});

// --- log_message (capability: logging) ---

extism::host_fn!(log_message_impl(user_data: HostData; msg: String) -> String {
    let hd = user_data.get()?.lock().unwrap().clone();
    if let Err(e) = check_capability(&hd, "log_message") {
        return Ok(e);
    }

    let module_code = hd.module_code.unwrap_or_else(|| "unknown".into());
    tracing::info!("[Module:{}] {}", module_code, msg);
    Ok(String::new())
});

// --- get_entity_type (capability: metadata.read) ---

extism::host_fn!(get_entity_type_impl(user_data: HostData; id: String) -> String {
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

// ── WasmPlugin ─────────────────────────────────────────────

impl WasmPlugin {
    /// Загрузить WASM-модуль с ресурсными лимитами и capability checks.
    pub async fn load(wasm_bytes: Vec<u8>, wasm_name: String, host_data: HostData) -> Result<Self, String> {
        let mut manifest = Manifest::new([Wasm::data(wasm_bytes)]);
        manifest.timeout_ms = Some(10_000);
        manifest.memory.max_pages = Some(256);

        let ctx = host_data.ctx.clone();
        let capabilities = host_data.capabilities.clone();
        let mut plugin = PluginBuilder::new(&manifest)
            .with_function("create_object", [PTR, PTR], [PTR], UserData::new(host_data.clone()), create_object_impl)
            .with_function("list_objects",  [PTR, PTR], [PTR], UserData::new(host_data.clone()), list_objects_impl)
            .with_function("get_object",    [PTR],      [PTR], UserData::new(host_data.clone()), get_object_impl)
            .with_function("update_object", [PTR, PTR, PTR], [PTR], UserData::new(host_data.clone()), update_object_impl)
            .with_function("log_message",   [PTR],      [],    UserData::new(host_data.clone()), log_message_impl)
            .with_function("get_entity_type",    [PTR], [PTR], UserData::new(host_data.clone()), get_entity_type_impl)
            .with_function("list_entity_fields", [PTR], [PTR], UserData::new(host_data.clone()), list_entity_fields_impl)
            .with_fuel_limit(10_000_000)
            .build()
            .map_err(|e| format!("Ошибка загрузки плагина: {}", e))?;

        let info_json = plugin.call::<&[u8], String>("get_info", b"")
            .map_err(|e| format!("get_info() не удался: {}", e))?;

        let wasm_info: WasmModuleInfo = serde_json::from_str(&info_json)
            .map_err(|e| format!("get_info() невалидный JSON: {}", e))?;

        let info = ModuleInfo {
            id: uuid::Uuid::new_v4().to_string(),
            name: wasm_info.name,
            version: wasm_info.version,
            source: "bytes".into(),
            functions: wasm_info.functions,
        };

        tracing::info!(
            "[Plugin loaded] {} v{} — {} functions: {} (fuel=10M, memory=256 pages, timeout=10s, capabilities=[{}])",
            info.name, info.version, info.functions.len(),
            info.functions.iter().map(|f| f.name.as_str()).collect::<Vec<_>>().join(", "),
            capabilities.join(", "),
        );

        Ok(Self { plugin, info, ctx, capabilities })
    }

    pub fn update_context(&self, company_id: Option<String>, user_id: Option<String>, user_login: Option<String>, display_name: Option<String>) {
        if let Ok(mut ctx) = self.ctx.write() {
            ctx.company_id = company_id;
            ctx.user_id = user_id;
            ctx.user_login = user_login;
            ctx.display_name = display_name;
        }
    }

    pub fn call(&mut self, function: &str, input: &[u8]) -> Result<Vec<u8>, String> {
        self.plugin
            .call::<&[u8], Vec<u8>>(function, input)
            .map_err(|e| format!("Ошибка вызова '{}': {}", function, e))
    }
}

// ── Deserialization helper for get_info() ──────────────────

#[derive(Deserialize)]
pub struct WasmModuleInfo {
    pub name: String,
    pub version: String,
    pub functions: Vec<PluginFunction>,
}
