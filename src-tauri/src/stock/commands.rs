// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

//! IPC-команды модуля склада.

use tauri::State;
use tokio::sync::Mutex;

use crate::commands::AppState;
use crate::core::middleware::CommandContext;
use crate::db::MongoClient;
use crate::meta::service::{
    EntityFieldService, EntityTypeService, EntityTransitionService,
};
use crate::meta::{CreateEntityFieldInput, CreateEntityTypeInput, CreateEntityTransitionInput};

/// Создать тип сущности, если его ещё нет. Возвращает id.
async fn ensure_type(
    db: &MongoClient,
    code: &str,
    name: &str,
    kind: crate::core::EntityKind,
    description: &str,
) -> Result<String, String> {
    let existing = EntityTypeService::list(db, None)
        .await
        .map_err(|e| e.to_string())?;
    if let Some(t) = existing.iter().find(|t| t.code == code) {
        return Ok(t._id.to_string());
    }
    let t = EntityTypeService::create(
        db,
        None,
        CreateEntityTypeInput {
            code: code.into(),
            name: name.into(),
            kind,
            description: Some(description.into()),
            icon: None,
        },
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(t._id.to_string())
}

async fn ensure_field(
    db: &MongoClient,
    et_id: &str,
    code: &str,
    name: &str,
    field_kind: crate::core::FieldKind,
    required: bool,
    extra: serde_json::Value,
) -> Result<(), String> {
    // Идемпотентность: поле уже есть?
    let fields = EntityFieldService::list_by_type(db, uuid::Uuid::parse_str(et_id).unwrap())
        .await
        .map_err(|e| e.to_string())?;
    if fields.iter().any(|f| f.code == code) {
        return Ok(());
    }

    let mut input = CreateEntityFieldInput {
        entity_type_id: et_id.into(),
        code: code.into(),
        name: name.into(),
        field_kind,
        is_required: Some(required),
        is_readonly: Some(false),
        default_value: None,
        enum_values: None,
        reference_entity: None,
        group_name: None,
    };
    if let Some(v) = extra.get("enum_values").and_then(|v| v.as_array()) {
        input.enum_values = Some(
            v.iter().filter_map(|x| x.as_str().map(String::from)).collect(),
        );
    }
    if let Some(v) = extra.get("reference_entity").and_then(|v| v.as_str()) {
        input.reference_entity = Some(v.into());
    }
    EntityFieldService::create(db, input).await.map_err(|e| e.to_string())?;
    Ok(())
}

async fn ensure_transition(
    db: &MongoClient,
    et_id: &str,
    from: &str,
    to: &str,
    policy: &str,
) -> Result<(), String> {
    let list = EntityTransitionService::list_by_type(db, uuid::Uuid::parse_str(et_id).unwrap())
        .await
        .map_err(|e| e.to_string())?;
    if list.iter().any(|t| t.from_state == from && t.to_state == to) {
        return Ok(());
    }
    EntityTransitionService::create(
        db,
        CreateEntityTransitionInput {
            entity_type_id: et_id.into(),
            code: format!("{from}_to_{to}"),
            name: format!("{from} → {to}"),
            from_state: from.into(),
            to_state: to.into(),
            required_policy: Some(policy.into()),
            require_signature: None,
        },
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Засеять метаданные склада: каталоги номенклатуры/локаций и четыре
/// типа документов с полями и переходом Черновик→Проведён.
#[tauri::command]
pub async fn stock_seed_metadata(state: State<'_, Mutex<AppState>>) -> Result<String, String> {
    let s = state.lock().await;
    let ctx = CommandContext::extract(&s).map_err(|e| e.to_string())?;
    ctx.check_permission("settings.manage").map_err(|e| e.to_string())?;
    let db = s.db.as_ref().ok_or("Не подключено к MongoDB")?.clone();
    drop(s);

    use crate::core::{EntityKind, FieldKind};

    // ── Каталоги ──
    let nom = ensure_type(&db, "NOMENCLATURE", "Номенклатура", EntityKind::Catalog,
        "Общий справочник товаров, услуг и наборов").await?;
    ensure_field(&db, &nom, "code", "Код", FieldKind::String, true, serde_json::json!({})).await?;
    ensure_field(&db, &nom, "type", "Тип", FieldKind::Enum, true,
        serde_json::json!({"enum_values": ["item", "service", "set"]})).await?;
    ensure_field(&db, &nom, "category", "Категория", FieldKind::String, false, serde_json::json!({})).await?;
    ensure_field(&db, &nom, "unit", "Единица измерения", FieldKind::String, false, serde_json::json!({})).await?;
    ensure_field(&db, &nom, "min_qty", "Минимальный остаток", FieldKind::Money, false, serde_json::json!({})).await?;
    ensure_field(&db, &nom, "components", "Компоненты набора", FieldKind::Table, false,
        serde_json::json!({})).await?;

    let loc = ensure_type(&db, "STOCK_LOCATION", "Место учёта", EntityKind::Catalog,
        "Склады, подотчётники, места использования").await?;
    ensure_field(&db, &loc, "type", "Тип", FieldKind::Enum, true,
        serde_json::json!({"enum_values": ["warehouse", "custodian", "usage"]})).await?;
    ensure_field(&db, &loc, "is_active", "Активен", FieldKind::Boolean, false, serde_json::json!({})).await?;

    // ── Документы ──
    let mk_doc = |code: &'static str, name: &'static str, desc: &'static str| {
        ensure_type(&db, code, name, EntityKind::Document, desc)
    };

    let move_t = mk_doc("MOVE", "Перемещение", "Перемещение между местами учёта").await?;
    ensure_field(&db, &move_t, "from_location_id", "Откуда", FieldKind::Reference, true,
        serde_json::json!({"reference_entity": "STOCK_LOCATION"})).await?;
    ensure_field(&db, &move_t, "to_location_id", "Куда", FieldKind::Reference, true,
        serde_json::json!({"reference_entity": "STOCK_LOCATION"})).await?;
    ensure_field(&db, &move_t, "lines", "Строки", FieldKind::Table, true, serde_json::json!({})).await?;
    ensure_transition(&db, &move_t, "draft", "posted", "documents.approve").await?;

    let count_t = mk_doc("COUNT", "Инвентаризация", "Сверка факта и учёта").await?;
    ensure_field(&db, &count_t, "location_id", "Место учёта", FieldKind::Reference, true,
        serde_json::json!({"reference_entity": "STOCK_LOCATION"})).await?;
    ensure_field(&db, &count_t, "lines", "Факты", FieldKind::Table, true, serde_json::json!({})).await?;
    ensure_transition(&db, &count_t, "draft", "posted", "documents.approve").await?;

    let handover_t = mk_doc("HANDOVER", "Выдача под отчёт",
        "Выдача имущества подотчётному лицу").await?;
    ensure_field(&db, &handover_t, "from_location_id", "Со склада", FieldKind::Reference, true,
        serde_json::json!({"reference_entity": "STOCK_LOCATION"})).await?;
    ensure_field(&db, &handover_t, "to_location_id", "Кому (подотчётник)", FieldKind::Reference, true,
        serde_json::json!({"reference_entity": "STOCK_LOCATION"})).await?;
    ensure_field(&db, &handover_t, "responsible_user_id", "Ответственный", FieldKind::User, true,
        serde_json::json!({})).await?;
    ensure_field(&db, &handover_t, "expected_return_date", "Ожидаемый возврат", FieldKind::Date, false,
        serde_json::json!({})).await?;
    ensure_field(&db, &handover_t, "lines", "Имущество", FieldKind::Table, true, serde_json::json!({})).await?;
    ensure_transition(&db, &handover_t, "draft", "posted", "documents.approve").await?;

    let ret_t = mk_doc("HANDOVER_RETURN", "Возврат из подотчёта",
        "Возврат имущества на склад").await?;
    ensure_field(&db, &ret_t, "from_location_id", "От кого", FieldKind::Reference, true,
        serde_json::json!({"reference_entity": "STOCK_LOCATION"})).await?;
    ensure_field(&db, &ret_t, "to_location_id", "На склад", FieldKind::Reference, true,
        serde_json::json!({"reference_entity": "STOCK_LOCATION"})).await?;
    ensure_field(&db, &ret_t, "source_handover_id", "Документ выдачи", FieldKind::String, false,
        serde_json::json!({})).await?;
    ensure_field(&db, &ret_t, "lines", "Имущество", FieldKind::Table, true, serde_json::json!({})).await?;
    ensure_transition(&db, &ret_t, "draft", "posted", "documents.approve").await?;

    Ok(format!(
        "Метаданные склада готовы: NOMENCLATURE={nom}, STOCK_LOCATION={loc}, MOVE={move_t}, COUNT={count_t}, HANDOVER={handover_t}, HANDOVER_RETURN={ret_t}"
    ))
}

// ── Отчёты (stock.read) ────────────────────────────────────

use futures::StreamExt;
use mongodb::bson::doc;

/// Балансы с фильтрами.
#[tauri::command]
pub async fn stock_balances(
    location_id: Option<String>,
    nomenclature_id: Option<String>,
    state: State<'_, Mutex<AppState>>,
) -> Result<serde_json::Value, String> {
    let s = state.lock().await;
    let ctx = CommandContext::extract(&s).map_err(|e| e.to_string())?;
    ctx.check_permission("stock.read").map_err(|e| e.to_string())?;
    let db = s.db.as_ref().ok_or("Не подключено к MongoDB")?;

    let mut filter = doc! { "company_id": ctx.company_id.0.to_string() };
    if let Some(l) = &location_id { filter.insert("location_id", l); }
    if let Some(n) = &nomenclature_id { filter.insert("nomenclature_id", n); }

    let mut cursor = db.collection::<mongodb::bson::Document>(super::COL_BALANCES)
        .find(filter).await.map_err(|e| e.to_string())?;
    let mut items = Vec::new();
    while let Some(Ok(d)) = cursor.next().await {
        items.push(serde_json::json!({
            "location_id": d.get_str("location_id").unwrap_or(""),
            "nomenclature_id": d.get_str("nomenclature_id").unwrap_or(""),
            "quantity": d.get_f64("quantity").unwrap_or(0.0),
        }));
    }
    Ok(serde_json::json!({ "balances": items }))
}

/// «Что у кого на руках»: остатки на локациях-подотчётниках
/// с данными последней выдачи.
#[tauri::command]
pub async fn stock_report_handover(state: State<'_, Mutex<AppState>>) -> Result<serde_json::Value, String> {
    let s = state.lock().await;
    let ctx = CommandContext::extract(&s).map_err(|e| e.to_string())?;
    ctx.check_permission("stock.read").map_err(|e| e.to_string())?;
    let db = s.db.as_ref().ok_or("Не подключено к MongoDB")?.clone();
    drop(s);
    let company = ctx.company_id.0.to_string();

    // Подотчётники-локации
    let mut custodians: Vec<(String, serde_json::Value)> = Vec::new();
    let mut cursor = db.collection::<mongodb::bson::Document>("objects")
        .find(doc! { "company_id": company.clone(), "entity_type_id": "STOCK_LOCATION_LOC" })
        .await.map_err(|e| e.to_string())?;
    while let Some(Ok(d)) = cursor.next().await {
        if d.get_str("entity_type_id") == Ok("STOCK_LOCATION_LOC") { continue; }
    }
    // Локации ищем по данным объектов типа STOCK_LOCATION с type=custodian:
    // entity_type_id — id типа; получим его один раз.
    let et = db.collection::<mongodb::bson::Document>("entity_types")
        .find_one(doc! { "code": "STOCK_LOCATION" }).await.map_err(|e| e.to_string())?;
    let Some(et) = et else { return Ok(serde_json::json!({"items": []})) };
    let et_id = et.get_str("_id").unwrap_or("").to_string();

    let mut cursor = db.collection::<mongodb::bson::Document>("objects")
        .find(doc! { "company_id": company.clone(), "entity_type_id": &et_id })
        .await.map_err(|e| e.to_string())?;
    while let Some(Ok(d)) = cursor.next().await {
        let data = d.get("data").cloned()
            .and_then(|b| mongodb::bson::from_bson::<serde_json::Value>(b).ok())
            .unwrap_or_default();
        if data["type"] == serde_json::json!("custodian") {
            custodians.push((d.get_str("_id").unwrap_or("").to_string(), data));
        }
    }
    drop(cursor);

    let col_mov = db.collection::<mongodb::bson::Document>(super::COL_MOVEMENTS);
    let col_bal = db.collection::<mongodb::bson::Document>(super::COL_BALANCES);

    let mut items = Vec::new();
    for (loc_id, loc_data) in custodians {
        let mut mcursor = col_mov
            .find(doc! {
                "company_id": company.clone(),
                "location_id": &loc_id,
                "kind": "handover_in",
                "is_reversal": { "$ne": true },
            })
            .sort(doc! { "created_at": -1 })
            .await.map_err(|e| e.to_string())?;
        while let Some(Ok(m)) = mcursor.next().await {
            let nom = m.get_str("nomenclature_id").unwrap_or("");
            // Текущий остаток этой позиции у этого подотчётника
            let bal = col_bal.find_one(doc! {
                "company_id": company.clone(),
                "location_id": &loc_id,
                "nomenclature_id": nom,
            }).await.map_err(|e| e.to_string())?;
            let qty = bal.and_then(|d| d.get_f64("quantity").ok()).unwrap_or(0.0);
            if qty <= 1e-9 { continue; } // уже вернули

            items.push(serde_json::json!({
                "location_id": loc_id,
                "custodian_name": loc_data["name"],
                "responsible_user_id": m.get_str("responsible_user_id").unwrap_or(""),
                "expected_return_date": m.get_str("expected_return_date").unwrap_or(""),
                "nomenclature_id": nom,
                "qty_on_hand": qty,
                "issued_at": mongodb::bson::DateTime::now().timestamp_millis(),
                "issued_at_ms": m.get_datetime("created_at").map(|t| t.timestamp_millis()).unwrap_or(0),
            }));
        }
    }

    // Дедупликация по (location, nomenclature): оставляем последнюю выдачу
    Ok(serde_json::json!({ "items": items }))
}

/// Просроченные возвраты из подотчёта.
#[tauri::command]
pub async fn stock_report_overdue(state: State<'_, Mutex<AppState>>) -> Result<serde_json::Value, String> {
    let full = stock_report_handover(state).await?;
    let today = chrono::Utc::now().date_naive().to_string();
    let items = full["items"].as_array().cloned().unwrap_or_default()
        .into_iter()
        .filter(|i| i["expected_return_date"].is_string())
        .filter(|i| {
            let due = i["expected_return_date"].as_str().unwrap_or("9999");
            due < today.as_str()
        })
        .collect::<Vec<_>>();
    Ok(serde_json::json!({ "items": items, "today": today }))
}

// ── Политики подписи (settings.manage) ─────────────────────

#[derive(serde::Deserialize)]
pub struct UpsertSignaturePolicyInput {
    pub module: String,
    pub action: String,
    pub name: String,
    #[serde(default)]
    pub condition: serde_json::Value,
    pub required: bool,
}

#[tauri::command]
pub async fn signature_policies_list(
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<super::signature::SignaturePolicy>, String> {
    let s = state.lock().await;
    let ctx = CommandContext::extract(&s).map_err(|e| e.to_string())?;
    ctx.check_permission("settings.manage").map_err(|e| e.to_string())?;
    super::signature::SignatureService::list(&s.db.as_ref().unwrap(), &ctx.company_id, None)
        .await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn signature_policies_upsert(
    input: UpsertSignaturePolicyInput,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let s = state.lock().await;
    let ctx = CommandContext::extract(&s).map_err(|e| e.to_string())?;
    ctx.check_permission("settings.manage").map_err(|e| e.to_string())?;

    let policy = super::signature::SignaturePolicy {
        id: uuid::Uuid::new_v4().to_string(),
        company_id: ctx.company_id.0.to_string(),
        module: input.module,
        action: input.action,
        name: input.name,
        condition: input.condition,
        required: input.required,
    };
    super::signature::SignatureService::upsert(&s.db.as_ref().unwrap(), &ctx.company_id, policy.clone())
        .await.map_err(|e| e.to_string())?;
    crate::audit_log!(s, s.db.as_ref().unwrap(), crate::audit::AuditableAction::SaveSettings,
        target_id = format!("{}:{}", policy.module, policy.action));
    Ok(())
}

#[tauri::command]
pub async fn signature_policies_delete(
    module: String,
    action: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<u64, String> {
    let s = state.lock().await;
    let ctx = CommandContext::extract(&s).map_err(|e| e.to_string())?;
    ctx.check_permission("settings.manage").map_err(|e| e.to_string())?;
    super::signature::SignatureService::delete(&s.db.as_ref().unwrap(), &ctx.company_id, &module, &action)
        .await.map_err(|e| e.to_string())
}

/// Требуется ли подпись для действия над документом (для фронта до поста).
#[tauri::command]
pub async fn signature_required_for_doc(
    module: String,
    action: String,
    doc_id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<bool, String> {
    let s = state.lock().await;
    let ctx = CommandContext::extract(&s).map_err(|e| e.to_string())?;
    ctx.check_permission("stock.read").map_err(|e| e.to_string())?;
    let db = s.db.as_ref().ok_or("Не подключено к MongoDB")?.clone();
    drop(s);

    let uid = uuid::Uuid::parse_str(&doc_id).map_err(|e| e.to_string())?;
    let obj = crate::objects::service::ObjectService::get(&db, uid).await.map_err(|e| e.to_string())?;
    super::signature::SignatureService::evaluate(&db, &ctx.company_id, &module, &action, &obj.data)
        .await.map_err(|e| e.to_string())
}
