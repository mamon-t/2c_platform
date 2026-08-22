use chrono::Utc;
use futures::StreamExt;
use mongodb::bson::{doc, Document};
use tracing::info;

use crate::audit::AuditChanges;
use crate::core::{PlatformError, PlatformResult};
use crate::core::middleware::CommandOutcome;
use crate::db::MongoClient;
use crate::events::{ActorSnapshot, EventService, StreamType};
use super::*;

const COLLECTION: &str = "print_templates";

// ── PrintService ───────────────────────────────────────────

pub struct PrintService;

impl PrintService {
    pub async fn list(
        db: &MongoClient,
        entity_type: &str,
        form_code: Option<&str>,
    ) -> PlatformResult<Vec<PrintTemplate>> {
        let col = db.collection::<Document>(COLLECTION);
        let mut filter = doc! { "entity_type": entity_type, "is_active": true };
        if let Some(fc) = form_code {
            filter.insert("form_code", fc);
        }
        let mut cursor = col.find(filter).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        let mut result = Vec::new();
        while let Some(res) = cursor.next().await {
            let doc = res.map_err(|e| PlatformError::Database(e.to_string()))?;
            if let Ok(t) = deserialize_template(&doc) { result.push(t); }
        }
        result.sort_by(|a, b| b.is_default.cmp(&a.is_default).then(a.name.cmp(&b.name)));
        Ok(result)
    }

    pub async fn get(db: &MongoClient, id: uuid::Uuid) -> PlatformResult<PrintTemplate> {
        let col = db.collection::<Document>(COLLECTION);
        let doc = col.find_one(doc! { "_id": id.to_string() }).await
            .map_err(|e| PlatformError::Database(e.to_string()))?
            .ok_or_else(|| PlatformError::NotFound(format!("Шаблон {id} не найден")))?;
        deserialize_template(&doc).map_err(|e| PlatformError::NotFound(e))
    }

    pub async fn get_default(
        db: &MongoClient,
        entity_type: &str,
        form_code: &str,
    ) -> PlatformResult<PrintTemplate> {
        let col = db.collection::<Document>(COLLECTION);
        let doc = col.find_one(doc! {
            "entity_type": entity_type,
            "form_code": form_code,
            "is_default": true,
            "is_active": true,
        }).await
            .map_err(|e| PlatformError::Database(e.to_string()))?
            .ok_or_else(|| PlatformError::NotFound(format!(
                "Шаблон по умолчанию для {entity_type}/{form_code} не найден"
            )))?;
        deserialize_template(&doc).map_err(|e| PlatformError::NotFound(e))
    }

    pub async fn create(
        db: &MongoClient,
        input: CreatePrintTemplateInput,
        created_by: Option<String>,
        actor: ActorSnapshot,
    ) -> PlatformResult<CommandOutcome<PrintTemplate>> {
        let mut session = db.client().start_session().await
            .map_err(|e| PlatformError::Database(format!("start_session: {}", e)))?;
        session.start_transaction().await
            .map_err(|e| PlatformError::Database(format!("start_transaction: {}", e)))?;

        let now = Utc::now();
        let tmpl = PrintTemplate {
            _id: uuid::Uuid::new_v4(),
            code: input.code,
            name: input.name,
            entity_type: input.entity_type,
            form_code: input.form_code,
            template_body: input.template_body,
            css_styles: input.css_styles.unwrap_or_default(),
            paper_format: input.paper_format.unwrap_or(PaperFormat::A4),
            orientation: input.orientation.unwrap_or(Orientation::Portrait),
            margins: input.margins.unwrap_or_default(),
            is_default: input.is_default.unwrap_or(false),
            is_active: true,
            version: 1,
            valid_from: None,
            valid_to: None,
            company_id: None,
            before_print_script: input.before_print_script,
            created_by,
            created_at: now,
            updated_at: now,
        };
        let doc = serialize_template(&tmpl)?;
        db.collection::<Document>(COLLECTION).insert_one(doc).session(&mut session).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;

        let svc = EventService::new();
        let payload = serde_json::json!({ "code": tmpl.code, "name": tmpl.name });
        let cid = actor.company_id.clone();
        let _ = svc.append_with_session(db, &mut session, StreamType::Object, &tmpl._id.to_string(), "print_template.created", payload, actor, cid, None, None).await;

        session.commit_transaction().await
            .map_err(|e| PlatformError::Database(format!("commit_transaction: {}", e)))?;

        info!("PrintTemplate created: {} ({})", tmpl.code, tmpl._id);
        let changes = AuditChanges::new()
            .field_new("code", &tmpl.code);
        Ok(CommandOutcome { result: tmpl, changes: Some(changes), event_id: None, signature_ref: None })
    }

    pub async fn update(
        db: &MongoClient,
        id: uuid::Uuid,
        input: UpdatePrintTemplateInput,
    ) -> PlatformResult<PrintTemplate> {
        let col = db.collection::<Document>(COLLECTION);
        let mut set = doc! { "updated_at": mongodb::bson::to_bson(&Utc::now()).unwrap() };
        if let Some(ref n) = input.name { set.insert("name", n); }
        if let Some(ref b) = input.template_body { set.insert("template_body", b); }
        if let Some(ref c) = input.css_styles { set.insert("css_styles", c); }
        if let Some(ref p) = input.paper_format {
            set.insert("paper_format", serde_json::to_string(p).unwrap_or_default().trim_matches('"'));
        }
        if let Some(ref o) = input.orientation {
            set.insert("orientation", serde_json::to_string(o).unwrap_or_default().trim_matches('"'));
        }
        if let Some(ref m) = input.margins {
            if let Ok(bson) = mongodb::bson::to_bson(m) { set.insert("margins", bson); }
        }
        if let Some(d) = input.is_default { set.insert("is_default", d); }
        if let Some(a) = input.is_active { set.insert("is_active", a); }
        if let Some(ref s) = input.before_print_script { set.insert("before_print_script", s); }
        col.update_one(doc! { "_id": id.to_string() }, doc! { "$set": set }).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        Self::get(db, id).await
    }

    pub async fn delete(db: &MongoClient, id: uuid::Uuid, actor: ActorSnapshot) -> PlatformResult<CommandOutcome<()>> {
        let old = Self::get(db, id).await?;

        let mut session = db.client().start_session().await
            .map_err(|e| PlatformError::Database(format!("start_session: {}", e)))?;
        session.start_transaction().await
            .map_err(|e| PlatformError::Database(format!("start_transaction: {}", e)))?;

        let col = db.collection::<Document>(COLLECTION);
        col.delete_one(doc! { "_id": id.to_string() }).session(&mut session).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;

        let svc = EventService::new();
        let payload = serde_json::json!({ "code": old.code, "name": old.name });
        let cid = actor.company_id.clone();
        let _ = svc.append_with_session(db, &mut session, StreamType::Object, &id.to_string(), "print_template.deleted", payload, actor, cid, None, None).await;

        session.commit_transaction().await
            .map_err(|e| PlatformError::Database(format!("commit_transaction: {}", e)))?;

        info!("PrintTemplate deleted: {id}");
        let changes = AuditChanges::new()
            .field_new("code", &old.code);
        Ok(CommandOutcome { result: (), changes: Some(changes), event_id: None, signature_ref: None })
    }

    // ── Render ────────────────────────────────────────────

    pub async fn render(
        db: &MongoClient,
        template_id: uuid::Uuid,
        object_id: uuid::Uuid,
    ) -> PlatformResult<String> {
        let template = Self::get(db, template_id).await?;
        let context = Self::assemble_view_model(db, object_id).await?;
        let mut ctx = context;

        if let Some(ref script) = template.before_print_script {
            if !script.trim().is_empty() {
                let computed = run_before_print_hook(script, &ctx)?;
                ctx.computed = computed;
            }
        }

        renderer::render_html(&template, &ctx)
            .map_err(|e| PlatformError::Internal(format!("Ошибка рендеринга: {e}")))
    }

    pub async fn render_default(
        db: &MongoClient,
        entity_type: &str,
        form_code: &str,
        object_id: uuid::Uuid,
    ) -> PlatformResult<String> {
        let template = Self::get_default(db, entity_type, form_code).await?;
        let context = Self::assemble_view_model(db, object_id).await?;
        let mut ctx = context;

        if let Some(ref script) = template.before_print_script {
            if !script.trim().is_empty() {
                let computed = run_before_print_hook(script, &ctx)?;
                ctx.computed = computed;
            }
        }

        renderer::render_html(&template, &ctx)
            .map_err(|e| PlatformError::Internal(format!("Ошибка рендеринга: {e}")))
    }

    // ── View Model Assembly ───────────────────────────────

    pub async fn assemble_view_model(
        db: &MongoClient,
        object_id: uuid::Uuid,
    ) -> PlatformResult<PrintContext> {
        use crate::objects::service::ObjectService;

        let obj = ObjectService::get(db, object_id).await?;

        let et_col = db.collection::<Document>("entity_types");
        let et_doc = et_col.find_one(doc! { "code": &obj.entity_type_id }).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        let entity_type: serde_json::Value = et_doc
            .and_then(|d| mongodb::bson::from_document(d).ok())
            .unwrap_or(serde_json::Value::Null);

        let comp_col = db.collection::<Document>("companies");
        let company: serde_json::Value = comp_col
            .find_one(doc! { "_id": obj.company_id.0.to_string() }).await
            .map_err(|e| PlatformError::Database(e.to_string()))?
            .and_then(|d| mongodb::bson::from_document(d).ok())
            .unwrap_or(serde_json::Value::Null);

        let parent = if let Some(ref pid) = obj.parent_id {
            let parent_id = pid.clone();
            if let Ok(uuid) = uuid::Uuid::parse_str(&parent_id) {
                ObjectService::get(db, uuid).await.ok().map(|p| {
                    let state_str = serde_json::to_string(&p.state).unwrap_or_default()
                        .trim_matches('"').to_string();
                    serde_json::json!({
                        "id": p._id.to_string(),
                        "number": p.number,
                        "date": p.date,
                        "state": state_str,
                        "data": p.data,
                    })
                })
            } else { None }
        } else { None };

        Ok(PrintContext {
            object: serde_json::json!({
                "id": obj._id.to_string(),
                "number": obj.number,
                "date": obj.date,
                "state": format!("{:?}", obj.state).to_lowercase(),
                "version": obj.version,
                "data": obj.data,
            }),
            entity_type,
            company,
            parent,
            computed: serde_json::Value::Object(serde_json::Map::new()),
            print_info: PrintInfo {
                print_date: Utc::now().format("%d.%m.%Y").to_string(),
                page_number: 1,
                total_pages: 1,
                watermark: None,
            },
        })
    }

    pub async fn ensure_seed_templates(db: &MongoClient) -> PlatformResult<()> {
        let col = db.collection::<Document>(COLLECTION);
        let count = col.count_documents(doc! {}).await
            .map_err(|e| PlatformError::Database(e.to_string()))?;
        if count == 0 {
            seed::seed_templates(db).await?;
        }
        Ok(())
    }
}

// ── Rhai beforePrint hook ──────────────────────────────────

fn run_before_print_hook(script: &str, context: &PrintContext) -> PlatformResult<serde_json::Value> {
    let mut engine = rhai::Engine::new();
    let mut scope = rhai::Scope::new();

    scope.push("object", context.object.to_string());
    scope.push("company", context.company.to_string());

    engine.register_fn("format_money", |amount: f64| -> String {
        let abs = amount.abs();
        let whole = abs as i64;
        let frac = ((abs - whole as f64) * 100.0).round() as i64;
        let sign = if amount < 0.0 { "-" } else { "" };
        format!("{}{} \u{20BD}", sign, whole)
    });

    engine.register_fn("format_date", |val: &str| -> String {
        if let Ok(dt) = chrono::NaiveDate::parse_from_str(val, "%Y-%m-%d") {
            dt.format("%d.%m.%Y").to_string()
        } else {
            val.to_string()
        }
    });

    engine.register_fn("format_number", |val: f64| -> String {
        format!("{:.2}", val)
    });

    let result = engine.eval_with_scope::<rhai::Dynamic>(&mut scope, script)
        .map_err(|e| PlatformError::Script(format!("beforePrint: {e}")))?;

    serde_json::to_value(&result.to_string())
        .map_err(|e| PlatformError::Internal(format!("beforePrint serialize: {e}")))
}

// ── Serialize / Deserialize ────────────────────────────────

fn serialize_template(t: &PrintTemplate) -> PlatformResult<Document> {
    let mut doc = Document::new();
    doc.insert("_id", t._id.to_string());
    doc.insert("code", &t.code);
    doc.insert("name", &t.name);
    doc.insert("entity_type", &t.entity_type);
    doc.insert("form_code", &t.form_code);
    doc.insert("template_body", &t.template_body);
    doc.insert("css_styles", &t.css_styles);
    doc.insert("paper_format", serde_json::to_string(&t.paper_format).unwrap_or_default().trim_matches('"'));
    doc.insert("orientation", serde_json::to_string(&t.orientation).unwrap_or_default().trim_matches('"'));
    if let Ok(bson) = mongodb::bson::to_bson(&t.margins) { doc.insert("margins", bson); }
    doc.insert("is_default", t.is_default);
    doc.insert("is_active", t.is_active);
    doc.insert("version", t.version);
    if let Some(ref vf) = t.valid_from { doc.insert("valid_from", mongodb::bson::to_bson(vf).unwrap()); }
    if let Some(ref vt) = t.valid_to { doc.insert("valid_to", mongodb::bson::to_bson(vt).unwrap()); }
    if let Some(ref cid) = t.company_id { doc.insert("company_id", cid); }
    if let Some(ref s) = t.before_print_script { doc.insert("before_print_script", s); }
    if let Some(ref cb) = t.created_by { doc.insert("created_by", cb); }
    doc.insert("created_at", mongodb::bson::to_bson(&t.created_at).unwrap());
    doc.insert("updated_at", mongodb::bson::to_bson(&t.updated_at).unwrap());
    Ok(doc)
}

fn deserialize_template(doc: &Document) -> Result<PrintTemplate, String> {
    let get_str = |key: &str| -> String {
        doc.get_str(key).unwrap_or("").to_string()
    };
    let get_bool = |key: &str| -> bool {
        doc.get_bool(key).unwrap_or(false)
    };
    let get_i32 = |key: &str| -> i32 {
        doc.get_i32(key).unwrap_or(1) as i32
    };

    let id_str = doc.get_str("_id").map_err(|e| e.to_string())?;
    let _id = uuid::Uuid::parse_str(id_str).map_err(|e| e.to_string())?;

    let paper_format = match get_str("paper_format").as_str() {
        "a5" => PaperFormat::A5,
        "letter" => PaperFormat::Letter,
        _ => PaperFormat::A4,
    };
    let orientation = match get_str("orientation").as_str() {
        "landscape" => Orientation::Landscape,
        _ => Orientation::Portrait,
    };

    let margins: PrintMargins = doc.get("margins")
        .and_then(|v| mongodb::bson::from_bson(v.clone()).ok())
        .unwrap_or_default();

    let valid_from = doc.get_datetime("valid_from").ok()
        .and_then(|v| chrono::DateTime::from_timestamp_millis(v.timestamp_millis()));
    let valid_to = doc.get_datetime("valid_to").ok()
        .and_then(|v| chrono::DateTime::from_timestamp_millis(v.timestamp_millis()));
    let created_at = doc.get_datetime("created_at")
        .and_then(|v| Ok(chrono::DateTime::from_timestamp_millis(v.timestamp_millis())))
        .unwrap_or_else(|_| Some(Utc::now()))
        .unwrap_or_else(Utc::now);
    let updated_at = doc.get_datetime("updated_at")
        .and_then(|v| Ok(chrono::DateTime::from_timestamp_millis(v.timestamp_millis())))
        .unwrap_or_else(|_| Some(Utc::now()))
        .unwrap_or_else(Utc::now);

    Ok(PrintTemplate {
        _id,
        code: get_str("code"),
        name: get_str("name"),
        entity_type: get_str("entity_type"),
        form_code: get_str("form_code"),
        template_body: get_str("template_body"),
        css_styles: get_str("css_styles"),
        paper_format,
        orientation,
        margins,
        is_default: get_bool("is_default"),
        is_active: get_bool("is_active"),
        version: get_i32("version"),
        valid_from,
        valid_to,
        company_id: doc.get_str("company_id").ok().map(String::from),
        before_print_script: doc.get_str("before_print_script").ok().map(String::from),
        created_by: doc.get_str("created_by").ok().map(String::from),
        created_at,
        updated_at,
    })
}