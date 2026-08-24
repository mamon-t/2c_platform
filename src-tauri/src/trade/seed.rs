//! Seed метаданных торговли: 3 каталога + 4 типа документов.
//! Двухпроходный: сначала типы, затем поля с UUID-ссылками.

use crate::core::{EntityKind, FieldKind};
use crate::db::MongoClient;
use crate::meta::service::{
    EntityFieldService, EntityTypeService, EntityTransitionService,
};
use crate::meta::{CreateEntityFieldInput, CreateEntityTypeInput, CreateEntityTransitionInput};

pub const ET_COUNTERPARTY: &str = "COUNTERPARTY";
pub const ET_PRICE_TYPE: &str = "PRICE_TYPE";
pub const ET_PRICE: &str = "PRICE";
pub const ET_PURCHASE: &str = "PURCHASE";
pub const ET_SALES: &str = "SALES";
pub const ET_CUSTOMER_RETURN: &str = "CUSTOMER_RETURN";
pub const ET_SUPPLIER_RETURN: &str = "SUPPLIER_RETURN";

type TypeIds = std::collections::HashMap<String, String>;

#[allow(clippy::too_many_arguments)]
async fn field(
    db: &MongoClient,
    et_id: &str,
    code: &str,
    name: &str,
    kind: FieldKind,
    required: bool,
    enum_values: Option<&[&str]>,
    reference_type_code: Option<&str>,
    ids: &TypeIds,
) -> Result<(), String> {
    let fields = EntityFieldService::list_by_type(db, uuid::Uuid::parse_str(et_id).unwrap())
        .await.map_err(|e| e.to_string())?;
    if fields.iter().any(|f| f.code == code) { return Ok(()); }

    let reference_uuid = reference_type_code.and_then(|c| ids.get(c).cloned());
    EntityFieldService::create(db, CreateEntityFieldInput {
        entity_type_id: et_id.into(),
        code: code.into(), name: name.into(), field_kind: kind,
        is_required: Some(required), is_readonly: Some(false),
        default_value: None,
        enum_values: enum_values.map(|ev| ev.iter().map(|s| s.to_string()).collect()),
        reference_entity: reference_uuid,
        group_name: None,
    }).await.map(|_| ()).map_err(|e| e.to_string())
}

async fn doc_fields(
    db: &MongoClient, et_id: &str, warehouse_label: &str, ids: &TypeIds,
) -> Result<(), String> {
    field(db, et_id, "warehouse_id", warehouse_label, FieldKind::Reference, true, None, Some("STOCK_LOCATION"), ids).await?;
    field(db, et_id, "lines", "Строки", FieldKind::Table, true, None, None, ids).await?;
    field(db, et_id, "total", "Итог", FieldKind::Money, false, None, None, ids).await?;
    field(db, et_id, "comment", "Комментарий", FieldKind::Text, false, None, None, ids).await
}

async fn transition(db: &MongoClient, et_id: &str) -> Result<(), String> {
    let list = EntityTransitionService::list_by_type(db, uuid::Uuid::parse_str(et_id).unwrap())
        .await.map_err(|e| e.to_string())?;
    if list.iter().any(|t| t.from_state == "draft" && t.to_state == "posted") { return Ok(()); }
    EntityTransitionService::create(db, CreateEntityTransitionInput {
        entity_type_id: et_id.into(),
        code: "draft_to_posted".into(),
        name: "Черновик → Проведён".into(),
        from_state: "draft".into(),
        to_state: "posted".into(),
        required_policy: Some("documents.approve".into()),
        require_signature: None,
    }).await.map(|_| ()).map_err(|e| e.to_string())
}

pub async fn seed(db: &MongoClient) -> Result<String, String> {
    // Проход 1: создать все типы, собрать map код→UUID
    let defs: &[(&str, &str, EntityKind, &str)] = &[
        (ET_COUNTERPARTY, "Контрагенты", EntityKind::Catalog, "Поставщики и покупатели"),
        (ET_PRICE_TYPE, "Типы цен", EntityKind::Catalog, "Розничные, оптовые, закупочные"),
        (ET_PRICE, "Цены", EntityKind::Catalog, "История цен по номенклатуре"),
        ("NOMENCLATURE", "Номенклатура", EntityKind::Catalog, "Товары и услуги"),
        ("STOCK_LOCATION", "Места учёта", EntityKind::Catalog, "Склады, подотчётники"),
        (ET_PURCHASE, "Поступление", EntityKind::Document, "Приход товаров от поставщика"),
        (ET_SALES, "Реализация", EntityKind::Document, "Продажа товаров покупателю"),
        (ET_CUSTOMER_RETURN, "Возврат от покупателя", EntityKind::Document, "Покупатель вернул товар"),
        (ET_SUPPLIER_RETURN, "Возврат поставщику", EntityKind::Document, "Возврат поставщику (брак)"),
    ];
    let existing = EntityTypeService::list(db, None).await.map_err(|e| e.to_string())?;
    let mut ids: TypeIds = existing.into_iter().map(|t| (t.code, t._id.to_string())).collect();
    for (code, name, kind, desc) in defs {
        if !ids.contains_key(*code) {
            let t = EntityTypeService::create(db, None, CreateEntityTypeInput {
                code: (*code).into(), name: (*name).into(), kind: kind.clone(),
                description: Some((*desc).into()), icon: None,
            }).await.map_err(|e| e.to_string())?;
            ids.insert(code.to_string(), t._id.to_string());
        }
    }

    let cp = &ids[ET_COUNTERPARTY];
    let pt = &ids[ET_PRICE_TYPE];
    let pr = &ids[ET_PRICE];
    let pur = &ids[ET_PURCHASE];
    let sal = &ids[ET_SALES];
    let cre = &ids[ET_CUSTOMER_RETURN];
    let sre = &ids[ET_SUPPLIER_RETURN];

    // ── Каталоги ──
    field(db, cp, "name", "Краткое название", FieldKind::String, true, None, None, &ids).await?;
    field(db, cp, "legal_name", "Юр. название", FieldKind::Text, false, None, None, &ids).await?;
    field(db, cp, "counterparty_type", "Тип", FieldKind::Enum, true, Some(&["supplier", "customer", "both"]), None, &ids).await?;
    field(db, cp, "inn", "ИНН", FieldKind::String, false, None, None, &ids).await?;
    field(db, cp, "contacts", "Контакты", FieldKind::Table, false, None, None, &ids).await?;
    field(db, cp, "bank_accounts", "Банковские счета", FieldKind::Table, false, None, None, &ids).await?;
    field(db, cp, "manager_id", "Менеджер", FieldKind::User, false, None, None, &ids).await?;
    field(db, cp, "is_active", "Активен", FieldKind::Boolean, true, None, None, &ids).await?;

    field(db, pt, "code", "Код", FieldKind::String, true, None, None, &ids).await?;
    field(db, pt, "name", "Название", FieldKind::String, true, None, None, &ids).await?;
    field(db, pt, "purpose", "Назначение", FieldKind::Enum, true, Some(&["purchase", "retail", "wholesale", "custom"]), None, &ids).await?;
    field(db, pt, "order", "Порядок", FieldKind::Integer, false, None, None, &ids).await?;
    field(db, pt, "is_active", "Активен", FieldKind::Boolean, false, None, None, &ids).await?;

    field(db, pr, "price_type_id", "Тип цены", FieldKind::Reference, true, None, Some(ET_PRICE_TYPE), &ids).await?;
    field(db, pr, "nomenclature_id", "Номенклатура", FieldKind::Reference, true, None, Some("NOMENCLATURE"), &ids).await?;
    field(db, pr, "value", "Цена", FieldKind::Money, true, None, None, &ids).await?;
    field(db, pr, "valid_from", "Действует с", FieldKind::Date, true, None, None, &ids).await?;
    field(db, pr, "valid_to", "Действует до", FieldKind::Date, false, None, None, &ids).await?;
    field(db, pr, "is_active", "Активна", FieldKind::Boolean, false, None, None, &ids).await?;

    // ── Документы ──
    for (et_id, wh_label) in [(pur, "Склад приёмки"), (sal, "Склад списания"), (cre, "Склад приёмки возврата"), (sre, "Склад списания")] {
        doc_fields(db, et_id, wh_label, &ids).await?;
    }

    field(db, pur, "supplier_id", "Поставщик", FieldKind::Reference, true, None, Some(ET_COUNTERPARTY), &ids).await?;
    field(db, pur, "incoming_doc_number", "№ вх. документа", FieldKind::String, false, None, None, &ids).await?;

    field(db, sal, "customer_id", "Покупатель", FieldKind::Reference, true, None, Some(ET_COUNTERPARTY), &ids).await?;
    field(db, sal, "payment_method", "Оплата", FieldKind::Enum, false, Some(&["cash", "card", "mixed"]), None, &ids).await?;

    field(db, cre, "customer_id", "Покупатель", FieldKind::Reference, true, None, Some(ET_COUNTERPARTY), &ids).await?;
    field(db, cre, "source_sales_id", "Исходная реализация", FieldKind::Reference, true, None, Some(ET_SALES), &ids).await?;

    field(db, sre, "supplier_id", "Поставщик", FieldKind::Reference, true, None, Some(ET_COUNTERPARTY), &ids).await?;

    Ok(format!("seeded {} types, fields+transitions ok", ids.len()))
}
