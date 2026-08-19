use crate::core::{PlatformError, PlatformResult};
use mongodb::{
    Client, Collection, Database,
    bson::doc,
    options::ClientOptions,
};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticsInfo {
    pub connected: bool,
    pub host: String,
    pub version: Option<String>,
    pub replica_set: Option<String>,
    pub ok: bool,
}

#[derive(Clone)]
pub struct MongoClient {
    client: Client,
    db_name: String,
}

impl MongoClient {
    pub async fn connect(uri: &str, db_name: &str) -> PlatformResult<Self> {
        let mut opts = ClientOptions::parse(uri)
            .await
            .map_err(|e| PlatformError::Database(format!("Невалидный URI: {e}")))?;
        opts.server_api = Some(
            mongodb::options::ServerApi::builder()
                .version(mongodb::options::ServerApiVersion::V1)
                .build(),
        );

        let client = Client::with_options(opts)
            .map_err(|e| PlatformError::Database(format!("Ошибка подключения: {e}")))?;

        info!("Подключение к MongoDB: {}", db_name);

        Ok(Self {
            client,
            db_name: db_name.to_string(),
        })
    }

    pub fn database(&self) -> Database {
        self.client.database(&self.db_name)
    }

    pub fn collection<T: Send + Sync>(&self, name: &str) -> Collection<T> {
        self.database().collection(name)
    }

    pub async fn diagnostics(&self) -> DiagnosticsInfo {
        let mut info = DiagnosticsInfo {
            connected: false,
            host: String::new(),
            version: None,
            replica_set: None,
            ok: false,
        };

        match self.client.list_database_names().await {
            Ok(_) => {
                info.connected = true;
                info.ok = true;
                if let Ok(db) = self.database().run_command(doc! { "buildInfo": 1 }).await {
                    info.version = db.get_str("version").ok().map(String::from);
                    info.host = db.get_str("host").ok().unwrap_or("неизвестно").to_string();
                }
                if let Ok(rs) = self.database().run_command(doc! { "replSetGetStatus": 1 }).await {
                    info.replica_set = rs.get_str("set").ok().map(String::from);
                }
            }
            Err(e) => {
                error!("Ошибка диагностики MongoDB: {}", e);
            }
        }

        info
    }
}
