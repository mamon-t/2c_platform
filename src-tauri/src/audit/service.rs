// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

use async_trait::async_trait;
use futures::StreamExt;
use mongodb::bson::{doc, Document};
use std::collections::HashMap;

use super::changes::{AuditChanges, FieldChange};
use super::filters::{AuditFilters, AuditPage};
use super::{AuditEntry, AuditEntryView};
use crate::core::{CompanyId, Id, PlatformResult, UserId};
use crate::db::MongoClient;

#[async_trait]
pub trait AuditService: Send + Sync {
    async fn log(&self, db: &MongoClient, entry: AuditEntry) -> PlatformResult<()>;
    async fn list(&self, db: &MongoClient, company_id: CompanyId, filters: AuditFilters) -> PlatformResult<AuditPage>;
    async fn get_entry(&self, db: &MongoClient, id: Id) -> PlatformResult<Option<AuditEntryView>>;
    async fn count(&self, db: &MongoClient, company_id: CompanyId, filters: &AuditFilters) -> PlatformResult<i64>;
}

pub struct MongoAuditService;

impl MongoAuditService {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl AuditService for MongoAuditService {
    async fn log(&self, db: &MongoClient, entry: AuditEntry) -> PlatformResult<()> {
        let col = db.collection::<Document>("audit_log");
        let mut doc = Document::new();
        doc.insert("_id", entry._id.to_string());
        doc.insert("user_id", entry.user_id.0.to_string());
        doc.insert("company_id", entry.company_id.0.to_string());
        doc.insert("action", &entry.action);
        doc.insert("target_type", &entry.target_type);
        if let Some(ref tid) = entry.target_id {
            doc.insert("target_id", tid.clone());
        }
        if let Some(ref et) = entry.entity_type {
            doc.insert("entity_type", et.clone());
        }
        if let Some(ref oid) = entry.object_id {
            doc.insert("object_id", oid.clone());
        }
        if let Some(ref changes) = entry.changes {
            let bson = mongodb::bson::to_bson(changes)
                .map_err(|e| crate::core::PlatformError::Database(e.to_string()))?;
            doc.insert("changes", bson);
        }
        if let Some(ref eid) = entry.event_id {
            doc.insert("event_id", eid.clone());
        }
        if let Some(ref sr) = entry.signature_ref {
            doc.insert("signature_ref", sr.clone());
        }
        if let Some(ref ip) = entry.ip_address {
            doc.insert("ip_address", ip.clone());
        }
        if let Some(ref ua) = entry.user_agent {
            doc.insert("user_agent", ua.clone());
        }
        doc.insert("occurred_at", mongodb::bson::to_bson(&entry.occurred_at).unwrap());
        col.insert_one(doc).await.map_err(|e| crate::core::PlatformError::Database(e.to_string()))?;
        Ok(())
    }

    async fn list(&self, db: &MongoClient, company_id: CompanyId, filters: AuditFilters) -> PlatformResult<AuditPage> {
        let col = db.collection::<Document>("audit_log");
        let filter = build_bson_filter(&company_id, &filters);
        let limit = filters.effective_limit();

        let total_count = col
            .count_documents(filter.clone())
            .await
            .map_err(|e| crate::core::PlatformError::Database(e.to_string()))? as i64;

        let mut cursor = col
            .find(filter)
            .sort(doc! { "occurred_at": -1 })
            .limit(limit + 1)
            .await
            .map_err(|e| crate::core::PlatformError::Database(e.to_string()))?;

        let mut rows = Vec::new();
        while let Some(doc) = cursor.next().await {
            let doc = doc.map_err(|e| crate::core::PlatformError::Database(e.to_string()))?;
            rows.push(doc);
        }

        let has_more = rows.len() as i64 > limit;
        if has_more { rows.pop(); }

        let mut entries: Vec<AuditEntryView> = rows
            .into_iter()
            .filter_map(deserialize_entry_view)
            .collect();

        enrich_with_logins(db, &mut entries).await;

        let next_cursor = entries.last().map(|e| {
            e.entry.occurred_at.to_rfc3339()
        });
        let prev_cursor = entries.first().map(|e| {
            e.entry.occurred_at.to_rfc3339()
        });

        Ok(AuditPage {
            entries,
            total_count,
            has_more,
            next_cursor,
            prev_cursor,
        })
    }

    async fn get_entry(&self, db: &MongoClient, id: Id) -> PlatformResult<Option<AuditEntryView>> {
        let col = db.collection::<Document>("audit_log");
        let doc = col
            .find_one(doc! { "_id": id.to_string() })
            .await
            .map_err(|e| crate::core::PlatformError::Database(e.to_string()))?;
        Ok(doc.and_then(deserialize_entry_view))
    }

    async fn count(&self, db: &MongoClient, company_id: CompanyId, filters: &AuditFilters) -> PlatformResult<i64> {
        let col = db.collection::<Document>("audit_log");
        let filter = build_bson_filter(&company_id, filters);
        col.count_documents(filter)
            .await
            .map(|c| c as i64)
            .map_err(|e| crate::core::PlatformError::Database(e.to_string()))
    }
}

fn build_bson_filter(company_id: &CompanyId, filters: &AuditFilters) -> Document {
    let mut f = doc! { "company_id": company_id.0.to_string() };

    if !filters.actions.is_empty() {
        f.insert("action", doc! { "$in": &filters.actions });
    }
    if let Some(ref tt) = filters.target_type {
        f.insert("target_type", tt.clone());
    }
    if let Some(ref tid) = filters.target_id {
        f.insert("target_id", tid.clone());
    }
    if let Some(ref et) = filters.entity_type {
        f.insert("entity_type", et.clone());
    }
    if let Some(ref uid) = filters.user_id {
        f.insert("user_id", uid.clone());
    }
    if let Some(ref from) = filters.date_from {
        f.insert("occurred_at", doc! { "$gte": mongodb::bson::to_bson(from).unwrap() });
    }
    if let Some(ref to) = filters.date_to {
        let existing = f.get_document("occurred_at").cloned().unwrap_or_default();
        let mut combined = existing;
        combined.insert("$lte", mongodb::bson::to_bson(to).unwrap());
        f.insert("occurred_at", combined);
    }
    if let Some(ref before) = filters.before {
        let existing = f.get_document("occurred_at").cloned().unwrap_or_default();
        let mut combined = existing;
        combined.insert("$lt", mongodb::bson::to_bson(before).unwrap());
        f.insert("occurred_at", combined);
    }
    if let Some(ref after) = filters.after {
        let existing = f.get_document("occurred_at").cloned().unwrap_or_default();
        let mut combined = existing;
        combined.insert("$gt", mongodb::bson::to_bson(after).unwrap());
        f.insert("occurred_at", combined);
    }

    f
}

fn deserialize_entry_view(doc: Document) -> Option<AuditEntryView> {
    let entry = deserialize_entry_inner(&doc).ok()?;
    Some(AuditEntryView { entry, user_login: None, target_login: None })
}

fn deserialize_entry_inner(doc: &Document) -> Result<AuditEntry, ()> {
    let _id = Id::parse_str(doc.get_str("_id").unwrap_or("")).map_err(|_| ())?;
    let user_id = UserId(Id::parse_str(doc.get_str("user_id").unwrap_or("")).map_err(|_| ())?);
    let company_id = CompanyId(Id::parse_str(doc.get_str("company_id").unwrap_or("")).map_err(|_| ())?);
    let action = doc.get_str("action").unwrap_or("").to_string();
    let target_type = doc.get_str("target_type").unwrap_or("").to_string();
    let target_id = doc.get_str("target_id").ok().map(String::from);
    let entity_type = doc.get_str("entity_type").ok().map(String::from);
    let object_id = doc.get_str("object_id").ok().map(String::from);
    let event_id = doc.get_str("event_id").ok().map(String::from);
    let signature_ref = doc.get_str("signature_ref").ok().map(String::from);
    let ip_address = doc.get_str("ip_address").ok().map(String::from);
    let user_agent = doc.get_str("user_agent").ok().map(String::from);
    let occurred_at = doc.get_datetime("occurred_at")
        .ok()
        .and_then(|v| chrono::DateTime::from_timestamp_millis(v.timestamp_millis()))
        .unwrap_or_else(chrono::Utc::now);

    let changes = doc.get("changes").and_then(|v| {
        let obj = v.as_document()?;
        let mut fields = HashMap::new();
        for (k, v) in obj {
            if let Some(change_doc) = v.as_document() {
                let old = change_doc.get_str("old").ok().map(String::from);
                let new = change_doc.get_str("new").ok().map(String::from);
                fields.insert(k.clone(), FieldChange { old, new });
            }
        }
        Some(AuditChanges { fields })
    });

    Ok(AuditEntry {
        _id, user_id, company_id, action, target_type, target_id,
        entity_type, object_id, changes, event_id, signature_ref,
        ip_address, user_agent, occurred_at,
    })
}

async fn enrich_with_logins(db: &MongoClient, entries: &mut Vec<AuditEntryView>) {
    let col = db.collection::<Document>("users");
    let mut ids_to_resolve = std::collections::HashSet::new();
    for e in entries.iter() {
        ids_to_resolve.insert(e.entry.user_id.0.to_string());
        if let Some(ref tid) = e.entry.target_id {
            ids_to_resolve.insert(tid.clone());
        }
    }
    if ids_to_resolve.is_empty() { return; }
    let id_strs: Vec<&str> = ids_to_resolve.iter().map(|s| s.as_str()).collect();
    let mut cursor = match col.find(doc! { "_id": { "$in": &id_strs } }).await {
        Ok(c) => c,
        Err(_) => return,
    };
    let mut login_map: HashMap<String, String> = HashMap::new();
    while let Some(doc) = cursor.next().await {
        if let Ok(doc) = doc {
            if let (Ok(id), Ok(login)) = (doc.get_str("_id"), doc.get_str("login")) {
                login_map.insert(id.to_string(), login.to_string());
            }
        }
    }
    for e in entries.iter_mut() {
        let uid = e.entry.user_id.0.to_string();
        e.user_login = login_map.get(&uid).cloned();
        if let Some(ref tid) = e.entry.target_id {
            e.target_login = login_map.get(tid).cloned();
        }
    }
}
