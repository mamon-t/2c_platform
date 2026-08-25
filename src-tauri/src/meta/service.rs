// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

use futures::StreamExt;
use mongodb::bson::{doc, Document};
use tracing::info;

use super::*;
use crate::core::{CompanyId, PlatformError, PlatformResult};
use crate::db::MongoClient;

// ── EntityType ──────────────────────────────────────────────

pub struct EntityTypeService;

impl EntityTypeService {
    pub async fn list(db: &MongoClient, company_id: Option<CompanyId>) -> PlatformResult<Vec<EntityType>> {
        let col = db.collection::<Document>("entity_types");
        // Компания видит свои типы + глобальные (company_id отсутствует/null)
        let filter = match company_id {
            Some(cid) => doc! { "$or": [
                doc! { "company_id": cid.0.to_string() },
                doc! { "company_id": mongodb::bson::Bson::Null },
            ] },
            None => doc! {},
        };
        let mut cursor = col.find(filter).sort(doc! { "code": 1 }).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        let mut result = Vec::new();
        while let Some(res) = cursor.next().await {
            let doc = res.map_err(|e| PlatformError::Database(e.to_string()))?;
            if let Ok(et) = deserialize_entity_type(&doc) { result.push(et); }
        }
        Ok(result)
    }

    pub async fn get(db: &MongoClient, id: uuid::Uuid) -> PlatformResult<EntityType> {
        let col = db.collection::<Document>("entity_types");
        let doc = col.find_one(doc! { "_id": id.to_string() }).await
            .map_err(|e| PlatformError::Database(e.to_string()))?
            .ok_or_else(|| PlatformError::NotFound(format!("EntityType {id} не найден")))?;
        deserialize_entity_type(&doc).map_err(|_| PlatformError::NotFound("Ошибка десериализации".into()))
    }

    pub async fn create(db: &MongoClient, company_id: Option<CompanyId>, input: CreateEntityTypeInput) -> PlatformResult<EntityType> {
        let now = Utc::now();
        let et = EntityType {
            _id: uuid::Uuid::new_v4(),
            company_id,
            code: input.code.clone(),
            name: input.name,
            kind: input.kind,
            description: input.description,
            icon: input.icon,
            is_active: true,
            created_at: now,
            updated_at: now,
        };
        let mut doc = Document::new();
        doc.insert("_id", et._id.to_string());
        if let Some(ref cid) = et.company_id { doc.insert("company_id", cid.0.to_string()); }
        doc.insert("code", &et.code);
        doc.insert("name", &et.name);
        doc.insert("kind", serde_json::to_string(&et.kind).unwrap_or_default().trim_matches('"').to_string());
        if let Some(ref d) = et.description { doc.insert("description", d); }
        if let Some(ref i) = et.icon { doc.insert("icon", i); }
        doc.insert("is_active", et.is_active);
        doc.insert("occurred_at", mongodb::bson::to_bson(&et.created_at).unwrap());
        doc.insert("created_at", mongodb::bson::to_bson(&et.created_at).unwrap());
        doc.insert("updated_at", mongodb::bson::to_bson(&et.updated_at).unwrap());

        db.collection::<Document>("entity_types").insert_one(doc).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        info!("EntityType created: {} ({})", et.code, et._id);
        Ok(et)
    }

    pub async fn update(db: &MongoClient, id: uuid::Uuid, input: UpdateEntityTypeInput) -> PlatformResult<EntityType> {
        let col = db.collection::<Document>("entity_types");
        let mut set = doc! { "updated_at": mongodb::bson::to_bson(&Utc::now()).unwrap() };
        if let Some(ref n) = input.name { set.insert("name", n); }
        if let Some(ref d) = input.description { set.insert("description", d); }
        if let Some(ref i) = input.icon { set.insert("icon", i); }
        if let Some(a) = input.is_active { set.insert("is_active", a); }
        col.update_one(doc! { "_id": id.to_string() }, doc! { "$set": set }).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Self::get(db, id).await
    }

    pub async fn delete(db: &MongoClient, id: uuid::Uuid) -> PlatformResult<()> {
        let col = db.collection::<Document>("entity_types");
        col.delete_one(doc! { "_id": id.to_string() }).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        // Каскадное удаление вложенных сущностей
        let field_col = db.collection::<Document>("entity_fields");
        field_col.delete_many(doc! { "entity_type_id": id.to_string() }).await.ok();
        let state_col = db.collection::<Document>("entity_states");
        state_col.delete_many(doc! { "entity_type_id": id.to_string() }).await.ok();
        let trans_col = db.collection::<Document>("entity_transitions");
        trans_col.delete_many(doc! { "entity_type_id": id.to_string() }).await.ok();
        let form_col = db.collection::<Document>("entity_forms");
        form_col.delete_many(doc! { "entity_type_id": id.to_string() }).await.ok();
        let act_col = db.collection::<Document>("entity_actions");
        act_col.delete_many(doc! { "entity_type_id": id.to_string() }).await.ok();
        info!("EntityType deleted: {id}");
        Ok(())
    }
}

// ── EntityField ─────────────────────────────────────────────

pub struct EntityFieldService;

impl EntityFieldService {
    pub async fn list_by_type(db: &MongoClient, entity_type_id: uuid::Uuid) -> PlatformResult<Vec<EntityField>> {
        let col = db.collection::<Document>("entity_fields");
        let mut cursor = col.find(doc! { "entity_type_id": entity_type_id.to_string() })
            .sort(doc! { "order": 1 }).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        let mut result = Vec::new();
        while let Some(res) = cursor.next().await {
            let doc = res.map_err(|e| PlatformError::Database(e.to_string()))?;
            if let Ok(f) = deserialize_entity_field(&doc) { result.push(f); }
        }
        Ok(result)
    }

    pub async fn get(db: &MongoClient, id: uuid::Uuid) -> PlatformResult<EntityField> {
        let col = db.collection::<Document>("entity_fields");
        let doc = col.find_one(doc! { "_id": id.to_string() }).await
            .map_err(|e| PlatformError::Database(e.to_string()))?
            .ok_or_else(|| PlatformError::NotFound(format!("EntityField {id} не найден")))?;
        deserialize_entity_field(&doc).map_err(|_| PlatformError::NotFound("Ошибка десериализации".into()))
    }

    pub async fn create(db: &MongoClient, input: CreateEntityFieldInput) -> PlatformResult<EntityField> {
        let now = Utc::now();
        let et_id = uuid::Uuid::parse_str(&input.entity_type_id)
            .map_err(|_| PlatformError::Validation("Невалидный entity_type_id".into()))?;
        let max_order = Self::max_order(db, et_id).await.unwrap_or(0);
        let f = EntityField {
            _id: uuid::Uuid::new_v4(),
            entity_type_id: et_id,
            code: input.code,
            name: input.name,
            field_kind: input.field_kind,
            is_required: input.is_required.unwrap_or(false),
            is_readonly: input.is_readonly.unwrap_or(false),
            default_value: input.default_value,
            enum_values: input.enum_values,
            reference_entity: input.reference_entity,
            order: max_order + 1,
            group_name: input.group_name,
            created_at: now,
            updated_at: now,
        };
        let mut doc = Document::new();
        doc.insert("_id", f._id.to_string());
        doc.insert("entity_type_id", f.entity_type_id.to_string());
        doc.insert("code", &f.code);
        doc.insert("name", &f.name);
        doc.insert("field_kind", serde_json::to_string(&f.field_kind).unwrap_or_default().trim_matches('"').to_string());
        doc.insert("is_required", f.is_required);
        doc.insert("is_readonly", f.is_readonly);
        doc.insert("order", f.order);
        if let Some(ref g) = f.group_name { doc.insert("group_name", g); }
        if let Some(ref e) = f.enum_values { doc.insert("enum_values", e.join(",")); }
        if let Some(ref r) = f.reference_entity { doc.insert("reference_entity", r); }
        doc.insert("created_at", mongodb::bson::to_bson(&f.created_at).unwrap());
        doc.insert("updated_at", mongodb::bson::to_bson(&f.updated_at).unwrap());

        db.collection::<Document>("entity_fields").insert_one(doc).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(f)
    }

    pub async fn update(db: &MongoClient, id: uuid::Uuid, input: UpdateEntityFieldInput) -> PlatformResult<EntityField> {
        let col = db.collection::<Document>("entity_fields");
        let mut set = doc! { "updated_at": mongodb::bson::to_bson(&Utc::now()).unwrap() };
        if let Some(ref n) = input.name { set.insert("name", n); }
        if let Some(r) = input.is_required { set.insert("is_required", r); }
        if let Some(r) = input.is_readonly { set.insert("is_readonly", r); }
        if let Some(ref g) = input.group_name { set.insert("group_name", g); }
        if let Some(o) = input.order { set.insert("order", o); }
        col.update_one(doc! { "_id": id.to_string() }, doc! { "$set": set }).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Self::get(db, id).await
    }

    pub async fn delete(db: &MongoClient, id: uuid::Uuid) -> PlatformResult<()> {
        db.collection::<Document>("entity_fields").delete_one(doc! { "_id": id.to_string() }).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(())
    }

    async fn max_order(db: &MongoClient, entity_type_id: uuid::Uuid) -> PlatformResult<i32> {
        let col = db.collection::<Document>("entity_fields");
        let mut cursor = col.find(doc! { "entity_type_id": entity_type_id.to_string() })
            .sort(doc! { "order": -1 }).limit(1).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        if let Some(Ok(doc)) = cursor.next().await {
            Ok(doc.get_i32("order").unwrap_or(0))
        } else {
            Ok(0)
        }
    }
}

// ── EntityState ─────────────────────────────────────────────

pub struct EntityStateService;

impl EntityStateService {
    pub async fn list_by_type(db: &MongoClient, entity_type_id: uuid::Uuid) -> PlatformResult<Vec<EntityState>> {
        let col = db.collection::<Document>("entity_states");
        let mut cursor = col.find(doc! { "entity_type_id": entity_type_id.to_string() })
            .sort(doc! { "order": 1 }).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        let mut result = Vec::new();
        while let Some(res) = cursor.next().await {
            let doc = res.map_err(|e| PlatformError::Database(e.to_string()))?;
            if let Ok(s) = deserialize_entity_state(&doc) { result.push(s); }
        }
        Ok(result)
    }

    pub async fn create(db: &MongoClient, input: CreateEntityStateInput) -> PlatformResult<EntityState> {
        let et_id = uuid::Uuid::parse_str(&input.entity_type_id)
            .map_err(|_| PlatformError::Validation("Невалидный entity_type_id".into()))?;
        let max_order = Self::max_order(db, et_id).await.unwrap_or(0);
        let s = EntityState {
            _id: uuid::Uuid::new_v4(),
            entity_type_id: et_id,
            code: input.code,
            name: input.name,
            is_initial: input.is_initial.unwrap_or(false),
            is_final: input.is_final.unwrap_or(false),
            color: input.color,
            order: max_order + 1,
        };
        let mut doc = Document::new();
        doc.insert("_id", s._id.to_string());
        doc.insert("entity_type_id", s.entity_type_id.to_string());
        doc.insert("code", &s.code);
        doc.insert("name", &s.name);
        doc.insert("is_initial", s.is_initial);
        doc.insert("is_final", s.is_final);
        doc.insert("order", s.order);
        if let Some(ref c) = s.color { doc.insert("color", c); }

        db.collection::<Document>("entity_states").insert_one(doc).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(s)
    }

    pub async fn update(db: &MongoClient, id: uuid::Uuid, input: UpdateEntityStateInput) -> PlatformResult<()> {
        let col = db.collection::<Document>("entity_states");
        let mut set = doc! {};
        if let Some(ref n) = input.name { set.insert("name", n); }
        if let Some(ref c) = input.color { set.insert("color", c); }
        if let Some(f) = input.is_final { set.insert("is_final", f); }
        if !set.is_empty() {
            col.update_one(doc! { "_id": id.to_string() }, doc! { "$set": set }).await
                .map_err(|e| PlatformError::Database(e.to_string()))?;
        }
        Ok(())
    }

    pub async fn delete(db: &MongoClient, id: uuid::Uuid) -> PlatformResult<()> {
        db.collection::<Document>("entity_states").delete_one(doc! { "_id": id.to_string() }).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(())
    }

    async fn max_order(db: &MongoClient, entity_type_id: uuid::Uuid) -> PlatformResult<i32> {
        let col = db.collection::<Document>("entity_states");
        let mut cursor = col.find(doc! { "entity_type_id": entity_type_id.to_string() })
            .sort(doc! { "order": -1 }).limit(1).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        if let Some(Ok(doc)) = cursor.next().await {
            Ok(doc.get_i32("order").unwrap_or(0))
        } else { Ok(0) }
    }
}

// ── EntityTransition ────────────────────────────────────────

pub struct EntityTransitionService;

impl EntityTransitionService {
    pub async fn list_by_type(db: &MongoClient, entity_type_id: uuid::Uuid) -> PlatformResult<Vec<EntityTransition>> {
        let col = db.collection::<Document>("entity_transitions");
        let mut cursor = col.find(doc! { "entity_type_id": entity_type_id.to_string() }).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        let mut result = Vec::new();
        while let Some(res) = cursor.next().await {
            let doc = res.map_err(|e| PlatformError::Database(e.to_string()))?;
            if let Ok(t) = deserialize_entity_transition(&doc) { result.push(t); }
        }
        Ok(result)
    }

    pub async fn create(db: &MongoClient, input: CreateEntityTransitionInput) -> PlatformResult<EntityTransition> {
        let et_id = uuid::Uuid::parse_str(&input.entity_type_id)
            .map_err(|_| PlatformError::Validation("Невалидный entity_type_id".into()))?;
        let t = EntityTransition {
            _id: uuid::Uuid::new_v4(),
            entity_type_id: et_id,
            code: input.code,
            name: input.name,
            from_state: input.from_state,
            to_state: input.to_state,
            required_policy: input.required_policy,
            require_signature: input.require_signature.unwrap_or(false),
        };
        let mut doc = Document::new();
        doc.insert("_id", t._id.to_string());
        doc.insert("entity_type_id", t.entity_type_id.to_string());
        doc.insert("code", &t.code);
        doc.insert("name", &t.name);
        doc.insert("from_state", &t.from_state);
        doc.insert("to_state", &t.to_state);
        if let Some(ref p) = t.required_policy { doc.insert("required_policy", p); }
        doc.insert("require_signature", t.require_signature);

        db.collection::<Document>("entity_transitions").insert_one(doc).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(t)
    }

    pub async fn update(db: &MongoClient, id: uuid::Uuid, input: UpdateEntityTransitionInput) -> PlatformResult<()> {
        let col = db.collection::<Document>("entity_transitions");
        let mut set = doc! {};
        if let Some(ref n) = input.name { set.insert("name", n); }
        if let Some(ref p) = input.required_policy { set.insert("required_policy", p); }
        if let Some(s) = input.require_signature { set.insert("require_signature", s); }
        if !set.is_empty() {
            col.update_one(doc! { "_id": id.to_string() }, doc! { "$set": set }).await
                .map_err(|e| PlatformError::Database(e.to_string()))?;
        }
        Ok(())
    }

    pub async fn delete(db: &MongoClient, id: uuid::Uuid) -> PlatformResult<()> {
        db.collection::<Document>("entity_transitions").delete_one(doc! { "_id": id.to_string() }).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(())
    }
}

// ── EntityForm ──────────────────────────────────────────────

pub struct EntityFormService;

impl EntityFormService {
    pub async fn list_by_type(db: &MongoClient, entity_type_id: uuid::Uuid) -> PlatformResult<Vec<EntityForm>> {
        let col = db.collection::<Document>("entity_forms");
        let mut cursor = col.find(doc! { "entity_type_id": entity_type_id.to_string() }).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        let mut result = Vec::new();
        while let Some(res) = cursor.next().await {
            let doc = res.map_err(|e| PlatformError::Database(e.to_string()))?;
            if let Ok(f) = deserialize_entity_form(&doc) { result.push(f); }
        }
        Ok(result)
    }

    pub async fn create(db: &MongoClient, input: CreateEntityFormInput) -> PlatformResult<EntityForm> {
        let et_id = uuid::Uuid::parse_str(&input.entity_type_id)
            .map_err(|_| PlatformError::Validation("Невалидный entity_type_id".into()))?;
        let now = Utc::now();
        let f = EntityForm {
            _id: uuid::Uuid::new_v4(),
            entity_type_id: et_id,
            code: input.code,
            name: input.name,
            layout: input.layout,
            created_at: now,
            updated_at: now,
        };
        let mut doc = Document::new();
        doc.insert("_id", f._id.to_string());
        doc.insert("entity_type_id", f.entity_type_id.to_string());
        doc.insert("code", &f.code);
        doc.insert("name", &f.name);
        if let Ok(bson) = mongodb::bson::to_bson(&f.layout) { doc.insert("layout", bson); }
        doc.insert("created_at", mongodb::bson::to_bson(&f.created_at).unwrap());
        doc.insert("updated_at", mongodb::bson::to_bson(&f.updated_at).unwrap());

        db.collection::<Document>("entity_forms").insert_one(doc).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(f)
    }

    pub async fn update(db: &MongoClient, id: uuid::Uuid, input: UpdateEntityFormInput) -> PlatformResult<()> {
        let col = db.collection::<Document>("entity_forms");
        let mut set = doc! { "updated_at": mongodb::bson::to_bson(&Utc::now()).unwrap() };
        if let Some(ref n) = input.name { set.insert("name", n); }
        if let Some(ref l) = input.layout { if let Ok(bson) = mongodb::bson::to_bson(l) { set.insert("layout", bson); } }
        col.update_one(doc! { "_id": id.to_string() }, doc! { "$set": set }).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(())
    }

    pub async fn delete(db: &MongoClient, id: uuid::Uuid) -> PlatformResult<()> {
        db.collection::<Document>("entity_forms").delete_one(doc! { "_id": id.to_string() }).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(())
    }
}

// ── EntityAction ────────────────────────────────────────────

pub struct EntityActionService;

impl EntityActionService {
    pub async fn list_by_type(db: &MongoClient, entity_type_id: uuid::Uuid) -> PlatformResult<Vec<EntityAction>> {
        let col = db.collection::<Document>("entity_actions");
        let mut cursor = col.find(doc! { "entity_type_id": entity_type_id.to_string() }).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        let mut result = Vec::new();
        while let Some(res) = cursor.next().await {
            let doc = res.map_err(|e| PlatformError::Database(e.to_string()))?;
            if let Ok(a) = deserialize_entity_action(&doc) { result.push(a); }
        }
        Ok(result)
    }

    pub async fn get(db: &MongoClient, id: uuid::Uuid) -> PlatformResult<EntityAction> {
        let col = db.collection::<Document>("entity_actions");
        let doc = col.find_one(doc! { "_id": id.to_string() }).await
            .map_err(|e| PlatformError::Database(e.to_string()))?
            .ok_or_else(|| PlatformError::NotFound(format!("EntityAction {id} не найдена")))?;
        deserialize_entity_action(&doc).map_err(|_| PlatformError::NotFound("Ошибка десериализации".into()))
    }

    pub async fn create(db: &MongoClient, input: CreateEntityActionInput) -> PlatformResult<EntityAction> {
        let et_id = uuid::Uuid::parse_str(&input.entity_type_id)
            .map_err(|_| PlatformError::Validation("Невалидный entity_type_id".into()))?;
        let handler_kind = input.handler_kind.as_deref()
            .and_then(|s| serde_json::from_str(&format!("\"{s}\"")).ok())
            .unwrap_or(ActionHandlerKind::Custom);
        let a = EntityAction {
            _id: uuid::Uuid::new_v4(),
            entity_type_id: et_id,
            code: input.code,
            name: input.name,
            description: input.description,
            action_type: input.action_type,
            handler_kind,
            target_state: input.target_state,
            handler_ref: input.handler_ref,
            required_policy: input.required_policy,
            is_dangerous: input.is_dangerous.unwrap_or(false),
            created_at: Utc::now(),
        };
        let mut doc = Document::new();
        doc.insert("_id", a._id.to_string());
        doc.insert("entity_type_id", a.entity_type_id.to_string());
        doc.insert("code", &a.code);
        doc.insert("name", &a.name);
        if let Some(ref d) = a.description { doc.insert("description", d); }
        if let Some(ref t) = a.action_type { doc.insert("action_type", t); }
        doc.insert("handler_kind", serde_json::to_string(&a.handler_kind).unwrap_or_default().trim_matches('"').to_string());
        if let Some(ref s) = a.target_state { doc.insert("target_state", s); }
        if let Some(ref r) = a.handler_ref { doc.insert("handler_ref", r); }
        if let Some(ref p) = a.required_policy { doc.insert("required_policy", p); }
        doc.insert("is_dangerous", a.is_dangerous);
        doc.insert("created_at", mongodb::bson::to_bson(&a.created_at).unwrap());

        db.collection::<Document>("entity_actions").insert_one(doc).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(a)
    }

    pub async fn update(db: &MongoClient, id: uuid::Uuid, input: UpdateEntityActionInput) -> PlatformResult<()> {
        let col = db.collection::<Document>("entity_actions");
        let mut set = doc! {};
        if let Some(ref n) = input.name { set.insert("name", n); }
        if let Some(ref d) = input.description { set.insert("description", d); }
        if let Some(ref t) = input.action_type { set.insert("action_type", t); }
        if let Some(ref hk) = input.handler_kind { set.insert("handler_kind", hk); }
        if let Some(ref s) = input.target_state { set.insert("target_state", s); }
        if let Some(ref r) = input.handler_ref { set.insert("handler_ref", r); }
        if let Some(ref p) = input.required_policy { set.insert("required_policy", p); }
        if let Some(d) = input.is_dangerous { set.insert("is_dangerous", d); }
        if !set.is_empty() {
            col.update_one(doc! { "_id": id.to_string() }, doc! { "$set": set }).await
                .map_err(|e| PlatformError::Database(e.to_string()))?;
        }
        Ok(())
    }

    pub async fn delete(db: &MongoClient, id: uuid::Uuid) -> PlatformResult<()> {
        db.collection::<Document>("entity_actions").delete_one(doc! { "_id": id.to_string() }).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Ok(())
    }
}

// ── Deserialize helpers ─────────────────────────────────────

fn deserialize_entity_type(doc: &Document) -> Result<EntityType, ()> {
    let _id = uuid::Uuid::parse_str(doc.get_str("_id").unwrap_or("")).map_err(|_| ())?;
    let company_id = doc.get_str("company_id").ok().map(|s| {
        uuid::Uuid::parse_str(s).map(CompanyId).unwrap_or(CompanyId(_id))
    });
    let kind_str = doc.get_str("kind").unwrap_or("document");
    let kind: EntityKind = serde_json::from_str(&format!("\"{kind_str}\"")).unwrap_or(EntityKind::Document);
    Ok(EntityType {
        _id,
        company_id,
        code: doc.get_str("code").unwrap_or("").to_string(),
        name: doc.get_str("name").unwrap_or("").to_string(),
        kind,
        description: doc.get_str("description").ok().map(String::from),
        icon: doc.get_str("icon").ok().map(String::from),
        is_active: doc.get_bool("is_active").unwrap_or(true),
        created_at: doc.get_datetime("created_at").ok()
            .and_then(|v| chrono::DateTime::from_timestamp_millis(v.timestamp_millis()))
            .unwrap_or_else(Utc::now),
        updated_at: doc.get_datetime("updated_at").ok()
            .and_then(|v| chrono::DateTime::from_timestamp_millis(v.timestamp_millis()))
            .unwrap_or_else(Utc::now),
    })
}

fn deserialize_entity_field(doc: &Document) -> Result<EntityField, ()> {
    let _id = uuid::Uuid::parse_str(doc.get_str("_id").unwrap_or("")).map_err(|_| ())?;
    let entity_type_id = uuid::Uuid::parse_str(doc.get_str("entity_type_id").unwrap_or("")).map_err(|_| ())?;
    let kind_str = doc.get_str("field_kind").unwrap_or("string");
    let field_kind: FieldKind = serde_json::from_str(&format!("\"{kind_str}\"")).unwrap_or(FieldKind::String);
    let enum_values = doc.get_str("enum_values").ok().map(|s| s.split(',').map(String::from).collect());
    Ok(EntityField {
        _id,
        entity_type_id,
        code: doc.get_str("code").unwrap_or("").to_string(),
        name: doc.get_str("name").unwrap_or("").to_string(),
        field_kind,
        is_required: doc.get_bool("is_required").unwrap_or(false),
        is_readonly: doc.get_bool("is_readonly").unwrap_or(false),
        default_value: None,
        enum_values,
        reference_entity: doc.get_str("reference_entity").ok().map(String::from),
        order: doc.get_i32("order").unwrap_or(0),
        group_name: doc.get_str("group_name").ok().map(String::from),
        created_at: doc.get_datetime("created_at").ok()
            .and_then(|v| chrono::DateTime::from_timestamp_millis(v.timestamp_millis()))
            .unwrap_or_else(Utc::now),
        updated_at: doc.get_datetime("updated_at").ok()
            .and_then(|v| chrono::DateTime::from_timestamp_millis(v.timestamp_millis()))
            .unwrap_or_else(Utc::now),
    })
}

fn deserialize_entity_state(doc: &Document) -> Result<EntityState, ()> {
    let _id = uuid::Uuid::parse_str(doc.get_str("_id").unwrap_or("")).map_err(|_| ())?;
    let entity_type_id = uuid::Uuid::parse_str(doc.get_str("entity_type_id").unwrap_or("")).map_err(|_| ())?;
    Ok(EntityState {
        _id,
        entity_type_id,
        code: doc.get_str("code").unwrap_or("").to_string(),
        name: doc.get_str("name").unwrap_or("").to_string(),
        is_initial: doc.get_bool("is_initial").unwrap_or(false),
        is_final: doc.get_bool("is_final").unwrap_or(false),
        color: doc.get_str("color").ok().map(String::from),
        order: doc.get_i32("order").unwrap_or(0),
    })
}

fn deserialize_entity_transition(doc: &Document) -> Result<EntityTransition, ()> {
    let _id = uuid::Uuid::parse_str(doc.get_str("_id").unwrap_or("")).map_err(|_| ())?;
    let entity_type_id = uuid::Uuid::parse_str(doc.get_str("entity_type_id").unwrap_or("")).map_err(|_| ())?;
    Ok(EntityTransition {
        _id,
        entity_type_id,
        code: doc.get_str("code").unwrap_or("").to_string(),
        name: doc.get_str("name").unwrap_or("").to_string(),
        from_state: doc.get_str("from_state").unwrap_or("").to_string(),
        to_state: doc.get_str("to_state").unwrap_or("").to_string(),
        required_policy: doc.get_str("required_policy").ok().map(String::from),
        require_signature: doc.get_bool("require_signature").unwrap_or(false),
    })
}

fn deserialize_entity_form(doc: &Document) -> Result<EntityForm, ()> {
    let _id = uuid::Uuid::parse_str(doc.get_str("_id").unwrap_or("")).map_err(|_| ())?;
    let entity_type_id = uuid::Uuid::parse_str(doc.get_str("entity_type_id").unwrap_or("")).map_err(|_| ())?;
    let layout = doc.get("layout").and_then(|v| {
        if let Some(d) = v.as_document() { mongodb::bson::from_document(d.clone()).ok() }
        else { Some(serde_json::Value::Null) }
    }).unwrap_or(serde_json::Value::Null);
    Ok(EntityForm {
        _id,
        entity_type_id,
        code: doc.get_str("code").unwrap_or("").to_string(),
        name: doc.get_str("name").unwrap_or("").to_string(),
        layout,
        created_at: doc.get_datetime("created_at").ok()
            .and_then(|v| chrono::DateTime::from_timestamp_millis(v.timestamp_millis()))
            .unwrap_or_else(Utc::now),
        updated_at: doc.get_datetime("updated_at").ok()
            .and_then(|v| chrono::DateTime::from_timestamp_millis(v.timestamp_millis()))
            .unwrap_or_else(Utc::now),
    })
}

fn deserialize_entity_action(doc: &Document) -> Result<EntityAction, ()> {
    let _id = uuid::Uuid::parse_str(doc.get_str("_id").unwrap_or("")).map_err(|_| ())?;
    let entity_type_id = uuid::Uuid::parse_str(doc.get_str("entity_type_id").unwrap_or("")).map_err(|_| ())?;
    let handler_kind_str = doc.get_str("handler_kind").unwrap_or("custom");
    let handler_kind: ActionHandlerKind = serde_json::from_str(&format!("\"{handler_kind_str}\"")).unwrap_or(ActionHandlerKind::Custom);
    Ok(EntityAction {
        _id,
        entity_type_id,
        code: doc.get_str("code").unwrap_or("").to_string(),
        name: doc.get_str("name").unwrap_or("").to_string(),
        description: doc.get_str("description").ok().map(String::from),
        action_type: doc.get_str("action_type").ok().map(String::from),
        handler_kind,
        target_state: doc.get_str("target_state").ok().map(String::from),
        handler_ref: doc.get_str("handler_ref").ok().map(String::from),
        required_policy: doc.get_str("required_policy").ok().map(String::from),
        is_dangerous: doc.get_bool("is_dangerous").unwrap_or(false),
        created_at: doc.get_datetime("created_at").ok()
            .and_then(|v| chrono::DateTime::from_timestamp_millis(v.timestamp_millis()))
            .unwrap_or_else(Utc::now),
    })
}

use chrono::Utc;
