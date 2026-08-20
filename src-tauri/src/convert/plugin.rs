use extism::*;
use super::{ImportRequest, ImportResult, ExportRequest, ExportResult, ModuleInfo};
use std::sync::{Arc, Mutex};

pub struct ConvertPlugin {
    plugin: Plugin,
    pub info: ModuleInfo,
}

#[derive(Clone)]
struct HostData {
    db_uri: String,
    db_name: String,
    company_id: Option<String>,
    user_id: Option<String>,
    user_login: Option<String>,
    display_name: Option<String>,
}

extism::host_fn!(create_object_impl(user_data: HostData; entity_type_id: String, data: String) -> String {
    let data_arc = user_data.get()?;
    let hd = data_arc.lock().unwrap().clone();
    let rt = tokio::runtime::Handle::current();

    let result = rt.block_on(async {
        let db = match crate::db::MongoClient::connect(&hd.db_uri, &hd.db_name).await {
            Ok(db) => db,
            Err(e) => return serde_json::json!({"ok": false, "error": format!("DB connect: {}", e)}).to_string(),
        };

        let data: serde_json::Value = match serde_json::from_str(&data) {
            Ok(d) => d,
            Err(e) => return serde_json::json!({"ok": false, "error": format!("Invalid JSON: {}", e)}).to_string(),
        };

        let company_id = match hd.company_id.as_ref() {
            Some(cid) => {
                let uid = uuid::Uuid::parse_str(cid).unwrap_or_default();
                crate::core::CompanyId(uid)
            }
            None => return serde_json::json!({"ok": false, "error": "No company selected"}).to_string(),
        };

        let user_id = match hd.user_id.as_ref() {
            Some(uid) => {
                let uuid = uuid::Uuid::parse_str(uid).unwrap_or_default();
                crate::core::UserId(uuid)
            }
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
            data,
            parent_id: None,
            date: None,
        };

        match crate::objects::service::ObjectService::create(&db, input, company_id, user_id, actor).await {
            Ok(obj) => serde_json::json!({"ok": true, "id": obj._id.to_string()}).to_string(),
            Err(e) => serde_json::json!({"ok": false, "error": e.to_string()}).to_string(),
        }
    });
    Ok(result)
});

impl ConvertPlugin {
    pub fn load(wasm_bytes: Vec<u8>, wasm_name: String, app_state: Arc<tokio::sync::Mutex<crate::commands::AppState>>) -> Result<Self, String> {
        let hd = {
            let state = tokio::runtime::Handle::current().block_on(async { app_state.lock().await });
            HostData {
                db_uri: state.config.mongodb_uri.clone().unwrap_or_default(),
                db_name: state.config.mongodb_database.clone().unwrap_or_default(),
                company_id: state.current_company_id.clone(),
                user_id: state.current_user.as_ref().map(|u| u._id.to_string()),
                user_login: state.current_user.as_ref().map(|u| u.login.clone()),
                display_name: state.current_user.as_ref().map(|u| u.display_name.clone()),
            }
        };

        let manifest = Manifest::new([Wasm::data(wasm_bytes)]);

        let plugin = PluginBuilder::new(&manifest)
            .with_function("create_object", [PTR, PTR], [PTR], UserData::new(hd), create_object_impl)
            .build()
            .map_err(|e| format!("Failed to load plugin: {}", e))?;

        let info = ModuleInfo {
            id: uuid::Uuid::new_v4().to_string(),
            name: wasm_name,
            version: "0.1.0".into(),
            path: "bytes".into(),
            formats: vec!["csv".into(), "json".into(), "yaml".into(), "xml".into()],
        };

        Ok(Self { plugin, info })
    }

    pub fn import_data(&mut self, req: &ImportRequest) -> Result<ImportResult, String> {
        let input = serde_json::to_vec(req).map_err(|e| format!("Serialize error: {}", e))?;
        let output = self.plugin.call::<&[u8], Vec<u8>>("import_data", &input)
            .map_err(|e| format!("Plugin call error: {}", e))?;
        serde_json::from_slice(&output)
            .map_err(|e| format!("Deserialize error: {}", e))
    }

    pub fn export_data(&mut self, req: &ExportRequest) -> Result<ExportResult, String> {
        let input = serde_json::to_vec(req).map_err(|e| format!("Serialize error: {}", e))?;
        let output = self.plugin.call::<&[u8], Vec<u8>>("export_data", &input)
            .map_err(|e| format!("Plugin call error: {}", e))?;
        serde_json::from_slice(&output)
            .map_err(|e| format!("Deserialize error: {}", e))
    }

    pub fn get_info(&mut self) -> Result<serde_json::Value, String> {
        let output = self.plugin.call::<&[u8], Vec<u8>>("get_info", b"")
            .map_err(|e| format!("Plugin call error: {}", e))?;
        serde_json::from_slice(&output)
            .map_err(|e| format!("Deserialize error: {}", e))
    }
}
