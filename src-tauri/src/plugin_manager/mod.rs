pub mod commands;

use extism::*;
use mongodb::Database; // Добавляем тип БД
use serde::{Deserialize, Serialize};
//use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub source: String,
    pub functions: Vec<String>,
}

pub struct WasmPlugin {
    plugin: Plugin,
    pub info: ModuleInfo,
}

// Обновляем HostData: вместо строк храним живую ссылку на БД (клонирование дешевое, это Arc под капотом)
#[derive(Clone)]
struct HostData {
    db:  Option<crate::db::MongoClient>,
    company_id: Option<String>,
    user_id: Option<String>,
    user_login: Option<String>,
    display_name: Option<String>,
}

extism::host_fn!(create_object_impl(user_data: HostData; entity_type_id: String, data: String) -> String {
    // В твоей версии Extism get()? возвращает Arc<Mutex<HostData>>, поэтому делаем lock и clone
    let hd = user_data.get()?.lock().unwrap().clone();

    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            // Достаем MongoClient из Option
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

impl WasmPlugin {
    // Теперь принимаем готовый HostData, а не Arc<Mutex<AppState>>
    pub fn load(wasm_bytes: Vec<u8>, wasm_name: String, host_data: HostData) -> Result<Self, String> {
        let manifest = Manifest::new([Wasm::data(wasm_bytes)]);
        let plugin = PluginBuilder::new(&manifest)
            .with_function("create_object", [PTR, PTR], [PTR], UserData::new(host_data), create_object_impl)
            .build()
            .map_err(|e| format!("Failed to load plugin: {}", e))?;
            
        let info = ModuleInfo {
            id: uuid::Uuid::new_v4().to_string(),
            name: wasm_name,
            version: "0.1.0".into(),
            source: "bytes".into(),
            functions: vec![],
        };
        
        Ok(Self { plugin, info })
    }

    pub fn call(&mut self, function: &str, input: &[u8]) -> Result<Vec<u8>, String> {
        self.plugin
            .call::<&[u8], Vec<u8>>(function, input)
            .map_err(|e| format!("Plugin call '{}' error: {}", function, e))
    }
}
