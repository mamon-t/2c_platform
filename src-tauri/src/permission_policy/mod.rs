use chrono::{DateTime, Utc};
use futures::StreamExt;
use mongodb::bson::{doc, Document};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::audit::AuditChanges;
use crate::core::{CompanyId, Id, PlatformError, PlatformResult};
use crate::core::middleware::CommandOutcome;
use crate::db::MongoClient;
use crate::events::{ActorSnapshot, EventService, StreamType};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionPolicy {
    pub _id: Id,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub scope_type: String,
    pub subsystem_code: String,
    pub entity_type: Option<String>,
    pub actions: Vec<String>,
    pub record_scope: String,
    pub deny: bool,
    pub priority: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePermissionPolicyInput {
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub scope_type: String,
    pub subsystem_code: String,
    pub entity_type: Option<String>,
    pub actions: Vec<String>,
    pub record_scope: String,
    pub deny: Option<bool>,
    pub priority: Option<i32>,
}

pub struct PermissionPolicyService;

impl PermissionPolicyService {
    pub fn new() -> Self { Self }

    pub async fn list(db: &MongoClient) -> PlatformResult<Vec<PermissionPolicy>> {
        let col = db.collection::<Document>("permission_policies");
        let mut cursor = col.find(doc! {}).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        let mut result = Vec::new();
        while let Some(doc) = cursor.next().await {
            let doc = doc.map_err(|e| PlatformError::Database(e.to_string()))?;
            let entry: PermissionPolicy = mongodb::bson::from_document(doc)
                .map_err(|e| PlatformError::Database(e.to_string()))?;
            result.push(entry);
        }
        Ok(result)
    }

    pub async fn get(db: &MongoClient, id: Id) -> PlatformResult<PermissionPolicy> {
        let col = db.collection::<Document>("permission_policies");
        let doc = col.find_one(doc! { "_id": id.to_string() }).await
            .map_err(|e| PlatformError::Database(e.to_string()))?
            .ok_or_else(|| PlatformError::NotFound(format!("PermissionPolicy {id} не найдена")))?;
        mongodb::bson::from_document(doc).map_err(|e| PlatformError::Database(e.to_string()))
    }

    pub async fn get_by_ids(db: &MongoClient, ids: &[Id]) -> PlatformResult<Vec<PermissionPolicy>> {
        if ids.is_empty() { return Ok(Vec::new()); }
        let id_strs: Vec<String> = ids.iter().map(|id| id.to_string()).collect();
        let col = db.collection::<Document>("permission_policies");
        let mut cursor = col.find(doc! { "_id": { "$in": &id_strs } }).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        let mut result = Vec::new();
        while let Some(doc) = cursor.next().await {
            let doc = doc.map_err(|e| PlatformError::Database(e.to_string()))?;
            if let Ok(entry) = mongodb::bson::from_document::<PermissionPolicy>(doc) {
                result.push(entry);
            }
        }
        Ok(result)
    }

    pub async fn get_by_codes(db: &MongoClient, codes: &[String]) -> PlatformResult<Vec<PermissionPolicy>> {
        if codes.is_empty() { return Ok(Vec::new()); }
        let col = db.collection::<Document>("permission_policies");
        let mut cursor = col.find(doc! { "code": { "$in": codes } }).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        let mut result = Vec::new();
        while let Some(doc) = cursor.next().await {
            let doc = doc.map_err(|e| PlatformError::Database(e.to_string()))?;
            if let Ok(entry) = mongodb::bson::from_document::<PermissionPolicy>(doc) {
                result.push(entry);
            }
        }
        Ok(result)
    }

    pub async fn create(db: &MongoClient, input: CreatePermissionPolicyInput, actor: ActorSnapshot) -> PlatformResult<CommandOutcome<PermissionPolicy>> {
        let mut session = db.client().start_session().await
            .map_err(|e| PlatformError::Database(format!("start_session: {}", e)))?;
        session.start_transaction().await
            .map_err(|e| PlatformError::Database(format!("start_transaction: {}", e)))?;

        let now = Utc::now();
        let policy = PermissionPolicy {
            _id: Uuid::new_v4(),
            code: input.code,
            name: input.name,
            description: input.description,
            scope_type: input.scope_type,
            subsystem_code: input.subsystem_code,
            entity_type: input.entity_type,
            actions: input.actions,
            record_scope: input.record_scope,
            deny: input.deny.unwrap_or(false),
            priority: input.priority.unwrap_or(0),
            created_at: now,
            updated_at: now,
        };
        let mut doc = mongodb::bson::to_document(&policy).unwrap_or_default();
        doc.insert("_id", policy._id.to_string());
        let col = db.collection::<Document>("permission_policies");
        col.insert_one(doc).session(&mut session).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;

        let svc = EventService::new();
        let payload = serde_json::json!({ "code": policy.code, "name": policy.name });
        let cid = actor.company_id.clone();
        let _ = svc.append_with_session(db, &mut session, StreamType::Object, &policy._id.to_string(), "permission_policy.created", payload, actor, cid, None, None).await;

        session.commit_transaction().await
            .map_err(|e| PlatformError::Database(format!("commit_transaction: {}", e)))?;

        let changes = AuditChanges::new()
            .field_new("code", &policy.code);
        Ok(CommandOutcome { result: policy, changes: Some(changes), event_id: None, signature_ref: None })
    }

    pub async fn delete(db: &MongoClient, id: Id, actor: ActorSnapshot) -> PlatformResult<CommandOutcome<()>> {
        let old = Self::get(db, id).await?;

        let mut session = db.client().start_session().await
            .map_err(|e| PlatformError::Database(format!("start_session: {}", e)))?;
        session.start_transaction().await
            .map_err(|e| PlatformError::Database(format!("start_transaction: {}", e)))?;

        let col = db.collection::<Document>("permission_policies");
        let result = col.delete_one(doc! { "_id": id.to_string() }).session(&mut session).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        if result.deleted_count == 0 {
            return Err(PlatformError::NotFound(format!("PermissionPolicy {id} не найдена")));
        }

        let svc = EventService::new();
        let payload = serde_json::json!({ "code": old.code });
        let cid = actor.company_id.clone();
        let _ = svc.append_with_session(db, &mut session, StreamType::Object, &id.to_string(), "permission_policy.deleted", payload, actor, cid, None, None).await;

        session.commit_transaction().await
            .map_err(|e| PlatformError::Database(format!("commit_transaction: {}", e)))?;

        let changes = AuditChanges::new()
            .field_new("code", &old.code);
        Ok(CommandOutcome { result: (), changes: Some(changes), event_id: None, signature_ref: None })
    }

    pub fn default_policies() -> Vec<(String, String, String, Vec<String>, String)> {
        vec![
            ("platform.access".into(), "Доступ к платформе".into(), "platform".into(), vec!["access".into()], "company".into()),
            ("companies.read".into(), "Просмотр компаний".into(), "companies".into(), vec!["read".into()], "company".into()),
            ("companies.create".into(), "Создание компаний".into(), "companies".into(), vec!["create".into()], "company".into()),
            ("companies.update".into(), "Изменение компаний".into(), "companies".into(), vec!["update".into()], "company".into()),
            ("companies.delete".into(), "Удаление компаний".into(), "companies".into(), vec!["delete".into()], "company".into()),
            ("users.read".into(), "Просмотр пользователей".into(), "users".into(), vec!["read".into()], "company".into()),
            ("users.create".into(), "Создание пользователей".into(), "users".into(), vec!["create".into()], "company".into()),
            ("users.update".into(), "Изменение пользователей".into(), "users".into(), vec!["update".into()], "company".into()),
            ("users.delete".into(), "Удаление пользователей".into(), "users".into(), vec!["delete".into()], "company".into()),
            ("roles.read".into(), "Просмотр ролей".into(), "roles".into(), vec!["read".into()], "company".into()),
            ("roles.create".into(), "Создание ролей".into(), "roles".into(), vec!["create".into()], "company".into()),
            ("roles.update".into(), "Изменение ролей".into(), "roles".into(), vec!["update".into()], "company".into()),
            ("roles.delete".into(), "Удаление ролей".into(), "roles".into(), vec!["delete".into()], "company".into()),
            ("contacts.read".into(), "Просмотр контактов".into(), "contacts".into(), vec!["read".into()], "company".into()),
            ("contacts.create".into(), "Создание контактов".into(), "contacts".into(), vec!["create".into()], "company".into()),
            ("contacts.update".into(), "Изменение контактов".into(), "contacts".into(), vec!["update".into()], "company".into()),
             ("contacts.delete".into(), "Удаление контактов".into(), "contacts".into(), vec!["delete".into()], "company".into()),
             ("contacts.manage".into(), "Управление типами контактов".into(), "contacts".into(), vec!["manage".into()], "company".into()),
            ("documents.read".into(), "Просмотр документов".into(), "documents".into(), vec!["read".into()], "company".into()),
            ("documents.create".into(), "Создание документов".into(), "documents".into(), vec!["create".into()], "company".into()),
            ("documents.update".into(), "Изменение документов".into(), "documents".into(), vec!["update".into()], "company".into()),
            ("documents.delete".into(), "Удаление документов".into(), "documents".into(), vec!["delete".into()], "company".into()),
            ("documents.approve".into(), "Проведение документов".into(), "documents".into(), vec!["approve".into()], "company".into()),
             ("documents.cancel".into(), "Отмена документов".into(), "documents".into(), vec!["cancel".into()], "company".into()),
             ("metadata.read".into(), "Просмотр метаданных".into(), "metadata".into(), vec!["read".into()], "company".into()),
             ("metadata.create".into(), "Создание метаданных".into(), "metadata".into(), vec!["create".into()], "company".into()),
             ("metadata.update".into(), "Изменение метаданных".into(), "metadata".into(), vec!["update".into()], "company".into()),
             ("metadata.delete".into(), "Удаление метаданных".into(), "metadata".into(), vec!["delete".into()], "company".into()),
            ("catalogs.read".into(), "Просмотр справочников".into(), "catalogs".into(), vec!["read".into()], "company".into()),
            ("catalogs.create".into(), "Создание справочников".into(), "catalogs".into(), vec!["create".into()], "company".into()),
            ("catalogs.update".into(), "Изменение справочников".into(), "catalogs".into(), vec!["update".into()], "company".into()),
            ("catalogs.delete".into(), "Удаление справочников".into(), "catalogs".into(), vec!["delete".into()], "company".into()),
            ("reports.read".into(), "Просмотр отчётов".into(), "reports".into(), vec!["read".into()], "company".into()),
            ("reports.create".into(), "Создание отчётов".into(), "reports".into(), vec!["create".into()], "company".into()),
            ("reports.export".into(), "Экспорт отчётов".into(), "reports".into(), vec!["export".into()], "company".into()),
            ("scripts.read".into(), "Просмотр скриптов".into(), "scripts".into(), vec!["read".into()], "company".into()),
            ("scripts.create".into(), "Создание скриптов".into(), "scripts".into(), vec!["create".into()], "company".into()),
            ("scripts.execute".into(), "Выполнение скриптов".into(), "scripts".into(), vec!["execute".into()], "company".into()),
            ("audit.read".into(), "Просмотр журнала".into(), "audit".into(), vec!["read".into()], "company".into()),
            ("settings.read".into(), "Просмотр настроек".into(), "settings".into(), vec!["read".into()], "company".into()),
            ("settings.manage".into(), "Управление настройками".into(), "settings".into(), vec!["manage".into()], "company".into()),
            ("print.read".into(), "Просмотр шаблонов печати".into(), "print".into(), vec!["read".into()], "company".into()),
            ("print.create".into(), "Создание шаблонов печати".into(), "print".into(), vec!["create".into()], "company".into()),
            ("print.update".into(), "Изменение шаблонов печати".into(), "print".into(), vec!["update".into()], "company".into()),
            ("print.delete".into(), "Удаление шаблонов печати".into(), "print".into(), vec!["delete".into()], "company".into()),
            ("plugins.read".into(), "Просмотр плагинов".into(), "plugins".into(), vec!["read".into()], "company".into()),
            ("plugins.manage".into(), "Управление плагинами".into(), "plugins".into(), vec!["manage".into()], "company".into()),
            ("plugins.execute".into(), "Выполнение функций плагинов".into(), "plugins".into(), vec!["execute".into()], "company".into()),
            ("numbering.read".into(), "Просмотр правил нумерации".into(), "numbering".into(), vec!["read".into()], "company".into()),
             ("numbering.manage".into(), "Управление нумерацией".into(), "numbering".into(), vec!["manage".into()], "company".into()),
             ("modules.read".into(), "Просмотр модулей".into(), "modules".into(), vec!["read".into()], "company".into()),
             ("modules.manage".into(), "Управление модулями".into(), "modules".into(), vec!["manage".into()], "company".into()),
            ("devices.read".into(), "Просмотр оборудования".into(), "devices".into(), vec!["read".into()], "company".into()),
            ("devices.manage".into(), "Настройка оборудования".into(), "devices".into(), vec!["manage".into()], "company".into()),
            ("devices.use".into(), "Использование оборудования".into(), "devices".into(), vec!["use".into()], "company".into()),
            ("stock.read".into(), "Просмотр склада".into(), "stock".into(), vec!["read".into()], "company".into()),
            ("stock.use".into(), "Складские операции".into(), "stock".into(), vec!["use".into()], "company".into()),
            ("stock.manage".into(), "Управление складом".into(), "stock".into(), vec!["manage".into()], "company".into()),
            ("accounting.read".into(), "Просмотр учёта".into(), "accounting".into(), vec!["read".into()], "company".into()),
            ("accounting.post".into(), "Проводки учёта".into(), "accounting".into(), vec!["post".into()], "company".into()),
            ("accounting.manage".into(), "Управление учётом".into(), "accounting".into(), vec!["manage".into()], "company".into()),
        ]
    }

    pub async fn ensure_seed_policies(db: &MongoClient) -> PlatformResult<()> {
        let existing = Self::list(db).await?;
        if !existing.is_empty() { return Ok(()); }
        let defaults = Self::default_policies();
        let actor = ActorSnapshot {
            user_id: crate::core::UserId(Uuid::nil()),
            login: "system".to_string(),
            full_name: Some("Система".to_string()),
            position: None,
            company_id: CompanyId(Uuid::nil()),
        };
        for (code, name, subsystem, actions, record_scope) in defaults {
            let _ = Self::create(db, CreatePermissionPolicyInput {
                code, name, description: None,
                scope_type: "subsystem".into(),
                subsystem_code: subsystem,
                entity_type: None, actions, record_scope,
                deny: Some(false), priority: Some(0),
            }, actor.clone()).await;
        }
        Ok(())
    }

    /// Проверить доступ: subsystem + entity_type? + action.
    ///
    /// Логика (deny-by-default):
    /// 1. Фильтруем по subsystem_code (точное совпадение)
    /// 2. Фильтруем по entity_type (если Some — точное совпадение; None-политика = wildcard)
    /// 3. Фильтруем по actions (содержит action или "*")
    /// 4. Среди совпавших: deny=true с наивысшим priority → ДОСТУП ЗАПРЕЩЁН
    /// 5. Среди совпавших: deny=false с наивысшим priority → ДОСТУП РАЗРЕШЁН
    /// 6. Если ничего не совпало → ДОСТУП ЗАПРЕЩЁН
    pub fn check_access(
        policies: &[PermissionPolicy],
        subsystem: &str,
        entity_type: Option<&str>,
        action: &str,
    ) -> bool {
        let matching: Vec<&PermissionPolicy> = policies.iter()
            .filter(|p| p.subsystem_code == subsystem)
            .filter(|p| {
                match entity_type {
                    Some(et) => p.entity_type.as_deref() == Some(et) || p.entity_type.is_none(),
                    None => true,
                }
            })
            .filter(|p| p.actions.iter().any(|a| a == action || a == "*"))
            .collect();

        // Сначала проверяем deny-политики (приоритет: чем выше priority, тем весомее)
        if let Some(deny) = matching.iter()
            .filter(|p| p.deny)
            .max_by_key(|p| p.priority)
        {
            // Есть deny-политика → доступ запрещён
            tracing::debug!(
                "check_access DENIED: subsystem={}, entity_type={:?}, action={}, policy={}",
                subsystem, entity_type, action, deny.code
            );
            return false;
        }

        // Проверяем allow-политики
        if let Some(allow) = matching.iter()
            .filter(|p| !p.deny)
            .max_by_key(|p| p.priority)
        {
            tracing::debug!(
                "check_access ALLOWED: subsystem={}, entity_type={:?}, action={}, policy={}",
                subsystem, entity_type, action, allow.code
            );
            return true;
        }

        // Нет совпавших политик → deny-by-default
        tracing::debug!(
            "check_access DENIED (no match): subsystem={}, entity_type={:?}, action={}",
            subsystem, entity_type, action
        );
        false
    }
}
