pub mod commands;

use extism::*;
use serde::{Deserialize, Serialize};

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
}

// ── HostData ───────────────────────────────────────────────

#[derive(Clone)]
struct HostData {
    db: Option<crate::db::MongoClient>,
    company_id: Option<String>,
    user_id: Option<String>,
    user_login: Option<String>,
    display_name: Option<String>,
}

// ── Host functions ─────────────────────────────────────────

extism::host_fn!(create_object_impl(user_data: HostData; entity_type_id: String, data: String) -> String {
    let hd = user_data.get()?.lock().unwrap().clone();
    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let db_client = match hd.db {
                Some(client) => client,
                None => return serde_json::json!({"ok": false, "error": "Database not initialized"}).to_string(),
            };

            let data_val: serde_json::Value = match serde_json::from_str(&data) {
                Ok(d) => d,
                Err(e) => return serde_json::json!({"ok": false, "error": format!("Invalid JSON: {}", e)}).to_string(),
            };

            let company_id = match hd.company_id.as_ref() {
                Some(cid) => match uuid::Uuid::parse_str(cid) {
                    Ok(uid) => crate::core::CompanyId(uid),
                    Err(_) => return serde_json::json!({"ok": false, "error": "Invalid company UUID"}).to_string(),
                },
                None => return serde_json::json!({"ok": false, "error": "No company selected"}).to_string(),
            };

            let user_id = match hd.user_id.as_ref() {
                Some(uid) => match uuid::Uuid::parse_str(uid) {
                    Ok(uuid) => crate::core::UserId(uuid),
                    Err(_) => return serde_json::json!({"ok": false, "error": "Invalid user UUID"}).to_string(),
                },
                None => return serde_json::json!({"ok": false, "error": "No user authenticated"}).to_string(),
            };

            let actor = crate::events::ActorSnapshot {
                user_id: user_id.clone(),
                login: hd.user_login.clone().unwrap_or_default(),
                full_name: hd.display_name.clone(),
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
                Ok(obj) => serde_json::json!({"ok": true, "id": obj._id.to_string()}).to_string(),
                Err(e) => serde_json::json!({"ok": false, "error": e.to_string()}).to_string(),
            }
        })
    });
    Ok(result)
});

extism::host_fn!(list_objects_impl(user_data: HostData; entity_type_id: String, limit: String) -> String {
    let hd = user_data.get()?.lock().unwrap().clone();
    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let db = match hd.db {
                Some(client) => client,
                None => return r#"[]"#.to_string(),
            };

            let company_id = match hd.company_id.as_ref() {
                Some(cid) => match uuid::Uuid::parse_str(cid) {
                    Ok(uid) => crate::core::CompanyId(uid),
                    Err(_) => return r#"[]"#.to_string(),
                },
                None => return r#"[]"#.to_string(),
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
                    serde_json::to_string(&objects).unwrap_or_else(|_| r#"[]"#.to_string())
                }
                Err(e) => {
                    tracing::error!("[WASM] list_objects error: {}", e);
                    r#"[]"#.to_string()
                }
            }
        })
    });
    Ok(result)
});

extism::host_fn!(log_message_impl(user_data: HostData; msg: String) -> String {
    let _ = user_data.get();
    tracing::info!("[WASM plugin] {}", msg);
    Ok(String::new())
});

// ── WasmPlugin ─────────────────────────────────────────────

impl WasmPlugin {
    pub fn load(wasm_bytes: Vec<u8>, wasm_name: String, host_data: HostData) -> Result<Self, String> {
        let manifest = Manifest::new([Wasm::data(wasm_bytes)]);
        let mut plugin = PluginBuilder::new(&manifest)
            .with_function("create_object", [PTR, PTR], [PTR], UserData::new(host_data.clone()), create_object_impl)
            .with_function("list_objects",  [PTR, PTR], [PTR], UserData::new(host_data.clone()), list_objects_impl)
            .with_function("log_message",   [PTR],      [],    UserData::new(host_data.clone()), log_message_impl)
            .build()
            .map_err(|e| format!("Failed to load plugin: {}", e))?;

        // Call get_info() at load time to discover functions
        let info_json = plugin.call::<&[u8], String>("get_info", b"")
            .map_err(|e| format!("get_info() failed: {}", e))?;

        let wasm_info: WasmModuleInfo = serde_json::from_str(&info_json)
            .map_err(|e| format!("get_info() parse error: {}", e))?;

        let info = ModuleInfo {
            id: uuid::Uuid::new_v4().to_string(),
            name: wasm_info.name,
            version: wasm_info.version,
            source: "bytes".into(),
            functions: wasm_info.functions,
        };

        tracing::info!(
            "[Plugin loaded] {} v{} — {} functions: {}",
            info.name, info.version, info.functions.len(),
            info.functions.iter().map(|f| f.name.as_str()).collect::<Vec<_>>().join(", ")
        );

        Ok(Self { plugin, info })
    }

    pub fn call(&mut self, function: &str, input: &[u8]) -> Result<Vec<u8>, String> {
        self.plugin
            .call::<&[u8], Vec<u8>>(function, input)
            .map_err(|e| format!("Plugin call '{}' error: {}", function, e))
    }
}

// ── Deserialization helper for get_info() ──────────────────

#[derive(Deserialize)]
struct WasmModuleInfo {
    name: String,
    version: String,
    functions: Vec<PluginFunction>,
}
