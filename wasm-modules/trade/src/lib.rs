// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

//! Модуль «Торговля» — оркестратор поступлений, реализаций и возвратов.
//!
//! Не хранит остатки и не считает себестоимость — вызывает операции
//! склада (stock.*) и учёта (accounting.post) через tx_exec.
//! Настройки счетов берутся из module_settings() per company.

use extism_pdk::*;
use serde::{Deserialize, Serialize};

// ── Host functions ─────────────────────────────────────────

#[host_fn]
extern "ExtismHost" {
    fn get_object(id: String) -> String;
    fn get_entity_type(id: String) -> String;
    fn module_settings() -> String;
    fn tx_begin(business_key: String) -> String;
    fn tx_add_op(handle: String, op_name: String, params_json: String) -> String;
    fn tx_commit(handle: String) -> String;
    fn emit_event(stream_id: String, event_type: String, payload_json: String) -> String;
    fn now_ms() -> String;
    fn notify_user(recipient: String, subject: String, body: String) -> String;
    fn log_message(msg: String);
    /// Себестоимость списаний документа из движений склада (проекция).
    fn stock_doc_cost(doc_id: String) -> String;
}

// ── Манифест ───────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub struct PluginFunction {
    pub name: String,
    pub label: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Serialize, Deserialize)]
pub struct ModuleInfo {
    pub name: String,
    pub version: String,
    pub code: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub api_version: Option<String>,
    pub capabilities: Vec<String>,
    pub permissions: Vec<String>,
    pub handles_documents: Vec<String>,
    pub functions: Vec<PluginFunction>,
}

#[plugin_fn]
pub fn get_info() -> FnResult<Json<ModuleInfo>> {
    fn f(name: &str, label: &str, description: &str) -> PluginFunction {
        PluginFunction {
            name: name.into(), label: label.into(), description: description.into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": { "id": {"type": "string"} },
                "required": ["id"],
            }),
        }
    }

    Ok(Json(ModuleInfo {
        name: "trade".into(),
        version: "0.1.0".into(),
        code: Some("trade".into()),
        author: Some("2C Platform".into()),
        description: Some("Торговля: поступления, реализации, возвраты. Атомарно с остатками и проводками.".into()),
        api_version: Some("1.0".into()),
        capabilities: vec![
            "objects.read".into(),
            "metadata.read".into(),
            "transactions".into(),
            "logging".into(),
        ],
        permissions: vec![
            "trade.read".into(),
            "trade.use".into(),
        ],
        handles_documents: vec![
            "PURCHASE".into(),
            "SALES".into(),
            "CUSTOMER_RETURN".into(),
            "SUPPLIER_RETURN".into(),
        ],
        functions: vec![
            f("on_post", "Проведение", "Пачка: склад + учёт + object.post."),
            f("on_cancel", "Отмена", "Пачка: stock.reverse + accounting.reverse + object.cancel."),
        ],
    }))
}

// ── Helpers ────────────────────────────────────────────────

fn unwrap_host(raw: String) -> anyhow::Result<serde_json::Value> {
    let v: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| anyhow::anyhow!("host невалидный JSON: {e}"))?;
    if v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false) {
        Ok(v.get("data").cloned().unwrap_or(serde_json::Value::Null))
    } else {
        let code = v["error"]["code"].as_str().unwrap_or("UNKNOWN");
        let msg = v["error"]["message"].as_str().unwrap_or("");
        Err(anyhow::anyhow!("{code}: {msg}"))
    }
}

/// Настройки модуля: счета + use_accounting.
#[derive(Debug, Clone)]
struct Accounts {
    use_accounting: bool,
    goods: String,
    supplier_settlements: String,
    customer_settlements: String,
    revenue: String,
    cogs: String,
    expenses_44: String,
}

impl Accounts {
    fn from_settings(settings: &serde_json::Value) -> Self {
        let a = |k: &str| -> String {
            // Дефолтные коды типового торгового плана
            let defaults = serde_json::json!({
                "goods": "41", "supplier_settlements": "60",
                "customer_settlements": "62", "revenue": "90.1",
                "cogs": "90.2", "expenses_44": "44"
            });
            settings["accounts"][k].as_str()
                .map(String::from)
                .unwrap_or_else(|| defaults[k].as_str().unwrap_or("").to_string())
        };
        Self {
            use_accounting: settings.get("use_accounting")
                .and_then(|v| v.as_bool()).unwrap_or(true),
            goods: a("goods"),
            supplier_settlements: a("supplier_settlements"),
            customer_settlements: a("customer_settlements"),
            revenue: a("revenue"),
            cogs: a("cogs"),
            expenses_44: a("expenses_44"),
        }
    }
}

fn unwrap_settings(raw: String) -> anyhow::Result<Accounts> {
    let v = unwrap_host(raw)?;
    Ok(Accounts::from_settings(&v))
}

/// Добавить операцию в пачку.
fn add(handle: &str, op: &str, params: serde_json::Value) -> anyhow::Result<String> {
    let raw = unsafe { tx_add_op(handle.to_string(), op.to_string(), params.to_string()) }?;
    let data = unwrap_host(raw)?;
    Ok(data["op_id"].as_str().unwrap_or("?").to_string())
}

/// Событие в Трубу (warn-and-forget).
fn emit(doc_id: &str, event_type: &str, extra: serde_json::Value) {
    let mut p = serde_json::json!({"doc_id": doc_id});
    if let (Some(e), Some(base)) = (extra.as_object(), p.as_object_mut()) {
        for (k, v) in e { base.insert(k.clone(), v.clone()); }
    }
    match unsafe { emit_event(doc_id.to_string(), event_type.to_string(), p.to_string()) } {
        Ok(r) => { if let Err(e) = unwrap_host(r) {
            let _ = unsafe { log_message(format!("[trade] emit {event_type}: {e}")) };
        }}
        Err(e) => { let _ = unsafe { log_message(format!("[trade] emit: {e}")) }; }
    }
}

/// Прочитать документ + код типа сущности.
fn load_doc(id: &str) -> anyhow::Result<(serde_json::Value, String)> {
    let raw = unsafe { get_object(id.to_string()) }?;
    let obj = unwrap_host(raw)?;
    let et_id = obj["entity_type_id"].as_str()
        .ok_or_else(|| anyhow::anyhow!("документ без entity_type_id"))?.to_string();
    let et_raw = unsafe { get_entity_type(et_id.clone()) }?;
    let et = unwrap_host(et_raw)?;
    let code = et["code"].as_str()
        .ok_or_else(|| anyhow::anyhow!("тип без кода"))?.to_string();
    Ok((obj, code))
}

/// Распределить доп. расходы пропорционально суммам строк.
/// Возвращает доп. стоимость на каждую строку (в копейках).
fn allocate_extra_costs(lines: &[LineData], extra_total_kop: i64) -> Vec<i64> {
    if extra_total_kop <= 0 || lines.is_empty() { return vec![0; lines.len()]; }

    let total_amount: f64 = lines.iter().map(|l| l.amount).sum();
    if total_amount.abs() < 0.01 { return vec![0; lines.len()]; }

    let mut alloc = Vec::with_capacity(lines.len());
    let mut distributed: i64 = 0;
    for (i, line) in lines.iter().enumerate() {
        if i == lines.len() - 1 {
            // Последняя строка получает остаток (округление)
            alloc.push(extra_total_kop - distributed);
        } else {
            let share = ((line.amount * 100.0).round() as i64 * extra_total_kop / (total_amount * 100.0).round() as i64) as i64;
            alloc.push(share);
            distributed += share;
        }
    }
    alloc
}

// ── Данные строки документа ────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct LineData {
    #[serde(default)]
    pub nomenclature_id: String,
    #[serde(default = "d_one")]
    pub qty: f64,
    /// Цена продажи/закупки (не себестоимость!)
    #[serde(default)]
    pub price: f64,
    /// Сумма строки (price * qty), если не задана — вычислим
    #[serde(default)]
    pub amount: f64,
    /// Доп. расходы на строку (копейки, только для поступления)
    #[serde(default)]
    pub extra_cost: i64,
    /// Идентификатор строки (line_ref движений склада). Пусто → l{индекс}.
    /// Себестоимость строки в документе НЕ пишется: она читается из движений
    /// по (doc_id, line_ref) через stock_doc_cost.
    #[serde(default)]
    pub line_id: String,
}

fn d_one() -> f64 { 1.0 }

/// line_ref для движений склада: явный line_id или стабильный индекс строки.
/// Документ после проведения неизменен — индекс стабилен между попытками постинга.
fn line_ref_of(l: &LineData, i: usize) -> String {
    if l.line_id.is_empty() { format!("l{i}") } else { l.line_id.clone() }
}

impl LineData {
    fn amount_kop(&self) -> i64 {
        if self.amount > 0.0 { (self.amount * 100.0).round() as i64 }
        else { ((self.price * self.qty) * 100.0).round() as i64 }
    }
}

// ── on_post ────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct PostInput {
    pub id: String,
    #[serde(default)]
    pub expected_version: Option<i64>,
}

#[plugin_fn]
pub fn on_post(Json(input): Json<PostInput>) -> FnResult<Json<serde_json::Value>> {
    let (doc, type_code) = load_doc(&input.id)?;
    let data = doc.get("data").cloned().unwrap_or_default();

    // Парсим строки
    let lines: Vec<LineData> = data["lines"].as_array()
        .map(|a| a.iter().filter_map(|l| serde_json::from_value(l.clone()).ok()).collect())
        .unwrap_or_default();
    if lines.is_empty() && type_code != "COUNT" {
        return Err(anyhow::anyhow!("VALIDATION: документ без строк").into());
    }

    let warehouse = data["warehouse_id"].as_str()
        .ok_or_else(|| anyhow::anyhow!("VALIDATION: требуется warehouse_id"))?
        .to_string();

    // Настройки счетов
    let settings_raw = unsafe { module_settings() }?;
    let accounts = Accounts::from_settings(&unwrap_host(settings_raw)?);

    // Начинаем пачку
    let handle_raw = unsafe { tx_begin(format!("post-{}", input.id)) }?;
    let handle_val = unwrap_host(handle_raw)?;
    let h = handle_val["handle"].as_str()
        .ok_or_else(|| anyhow::anyhow!("tx_begin без handle"))?;

    // Складская операция по типу документа (op_id для $ref проводок)
    let stock_op: Option<String> = match type_code.as_str() {
        "PURCHASE" => post_purchase(h, &warehouse, &lines, &data, &input.id)?,
        "SALES" => post_sales(h, &warehouse, &lines, &data, &accounts, &input.id)?,
        "CUSTOMER_RETURN" => post_customer_return(h, &warehouse, &data, &accounts, &lines, &input.id)?,
        "SUPPLIER_RETURN" => post_supplier_return(h, &warehouse, &lines, &data, &accounts, &input.id)?,
        other => return Err(anyhow::anyhow!(
            "VALIDATION: тип {other} не оркестрируется модулем торговли"
        ).into()),
    };

    // Учётные проводки (если включены)
    if accounts.use_accounting {
        add_accounting_entries(h, &type_code, &data, &input.id, &accounts, stock_op.as_deref())?;
    }

    // Переход документа
    let ver = input.expected_version.unwrap_or_else(|| doc["version"].as_i64().unwrap_or(1));
    add(h, "object.post", serde_json::json!({
        "object_id": input.id,
        "expected_version": ver,
    }))?;

    // Коммит
    let commit_raw = unsafe { tx_commit(h.to_string()) }?;
    let result = unwrap_host(commit_raw)?;

    // Событие
    let event_type = match type_code.as_str() {
        "PURCHASE" => "trade.purchase_posted",
        "SALES" => "trade.sales_posted",
        "CUSTOMER_RETURN" => "trade.customer_returned",
        _ => "trade.supplier_returned",
    };
    emit(&input.id, event_type, serde_json::json!({
        "total": data["total"],
        "entity_type": type_code,
    }));

    // Уведомление инициатору
    if let Some(initiator) = data["created_by"].as_str() {
        let _ = unsafe { notify_user(
            initiator.to_string(),
            format!("{} проведён", type_code),
            format!("Документ {} проведён атомарно.", input.id),
        ) }?;
    }

    let _ = unsafe { log_message(format!(
        "[trade] {} {} проведён атомарно", type_code, input.id
    )) };

    Ok(Json(serde_json::json!({ "posted": true })))
}

// ── Типовые пачки ──────────────────────────────────────────

/// Поступление: склад.receipt → результат для учётной операции.
fn post_purchase(
    h: &str, warehouse: &str, lines: &[LineData], data: &serde_json::Value,
    doc_id: &str,
) -> anyhow::Result<Option<String>> {
    let extra_total = data["extra_costs_total"].as_f64()
        .map(|v| (v * 100.0).round() as i64).unwrap_or(0);
    let extras = allocate_extra_costs(lines, extra_total);

    let receipt_lines: Vec<serde_json::Value> = lines.iter().enumerate().map(|(i, l)| {
        let unit_price_kop = (l.price * 100.0).round() as i64;
        let qty = l.qty as i64;
        let extra_per_unit = if qty > 0 { extras[i] / qty } else { 0 };
        serde_json::json!({
            "nomenclature_id": l.nomenclature_id,
            "qty": l.qty,
            "unit_cost": unit_price_kop + extra_per_unit,
        })
    }).collect();

    add(h, "stock.receipt", serde_json::json!({
        "location_id": warehouse,
        "lines": receipt_lines,
        "doc_kind": "PURCHASE",
        "doc_id": doc_id,
    }))?;

    Ok(None)
}

/// Реализация: FIFO списание → себестоимость из результата.
fn post_sales(
    h: &str, warehouse: &str, lines: &[LineData], _data: &serde_json::Value,
    accounts: &Accounts, doc_id: &str,
) -> anyhow::Result<Option<String>> {
    let issue_lines: Vec<serde_json::Value> = lines.iter().enumerate().map(|(i, l)| {
        serde_json::json!({
            "nomenclature_id": l.nomenclature_id,
            "qty": l.qty,
            "line_ref": line_ref_of(l, i),
        })
    }).collect();

    // COGS проводка берётся из результата через $ref ({op}.total_cost)
    let op_id = add(h, "stock.issue", serde_json::json!({
        "location_id": warehouse,
        "lines": issue_lines,
        "doc_kind": "SALES",
        "doc_id": doc_id,
    }))?;

    let _ = accounts; // используется в add_accounting_entries
    Ok(Some(op_id))
}

/// Возврат от покупателя: приёмка на склад по себестоимости строк ИСХОДНОЙ
/// реализации (проекция движений source_sales_id), средняя по строке:
/// Σcost_строки / Σqty_строки × возвращаемое_количество.
fn post_customer_return(
    h: &str, warehouse: &str, data: &serde_json::Value,
    _accounts: &Accounts, lines: &[LineData], doc_id: &str,
) -> anyhow::Result<Option<String>> {
    // Себестоимость списаний исходной реализации (если указан источник)
    let src: Option<serde_json::Value> = match data["source_sales_id"].as_str() {
        Some(sid) if !sid.is_empty() => {
            let raw = unsafe { stock_doc_cost(sid.to_string()) }?;
            match unwrap_host(raw) {
                Ok(v) => Some(v),
                Err(e) => {
                    let _ = unsafe { log_message(format!(
                        "[trade] stock_doc_cost({sid}) недоступен: {e}; приёмка по нулевой стоимости"
                    )) };
                    None
                }
            }
        }
        _ => None,
    };
    let src_lines = src.as_ref()
        .and_then(|v| v["lines"].as_array()).cloned().unwrap_or_default();

    let receipt_lines: Vec<serde_json::Value> = lines.iter().enumerate().map(|(i, l)| {
        let lref = line_ref_of(l, i);
        // Приоритет: та же строка исходника (line_ref), затем номенклатура
        let src_line = src_lines.iter()
            .find(|s| s["line_ref"].as_str() == Some(lref.as_str()))
            .or_else(|| src_lines.iter()
                .find(|s| s["nomenclature_id"].as_str() == Some(l.nomenclature_id.as_str())));
        let unit_cost = src_line
            .and_then(|s| {
                let cost = s["cost"].as_f64()?;
                let sqty = s["qty"].as_f64()?;
                if sqty <= 0.0 || l.qty <= 0.0 { return None; }
                Some((cost / sqty * l.qty).round() as i64)
            })
            .unwrap_or(0);
        serde_json::json!({
            "nomenclature_id": l.nomenclature_id,
            "qty": l.qty,
            "unit_cost": unit_cost,
        })
    }).collect();

    let op_id = add(h, "stock.receipt", serde_json::json!({
        "location_id": warehouse,
        "lines": receipt_lines,
        "doc_kind": "CUSTOMER_RETURN",
        "doc_id": doc_id,
    }))?;

    Ok(Some(op_id))
}

/// Возврат поставщику: списание с текущего свободного остатка.
fn post_supplier_return(
    h: &str, warehouse: &str, lines: &[LineData], _data: &serde_json::Value,
    _accounts: &Accounts, doc_id: &str,
) -> anyhow::Result<Option<String>> {
    let issue_lines: Vec<serde_json::Value> = lines.iter().enumerate().map(|(i, l)| {
        serde_json::json!({
            "nomenclature_id": l.nomenclature_id,
            "qty": l.qty,
            "line_ref": line_ref_of(l, i),
        })
    }).collect();

    let op_id = add(h, "stock.issue", serde_json::json!({
        "location_id": warehouse,
        "lines": issue_lines,
        "doc_kind": "SUPPLIER_RETURN",
        "doc_id": doc_id,
    }))?;

    Ok(Some(op_id))
}

/// Учётные проводки после складской операции.
/// stock_op — op_id складской операции пачки: себестоимость берётся из ЕЁ
/// результата через $ref (проекция движений), не из полей документа.
fn add_accounting_entries(
    handle: &str, type_code: &str, data: &serde_json::Value, doc_id: &str,
    accounts: &Accounts, stock_op: Option<&str>,
) -> anyhow::Result<()> {
    let _ = data;
    let total_kop = data["total"].as_f64()
        .map(|v| (v * 100.0).round() as i64).unwrap_or(0);

    // Нулевые суммы отфильтрует accounting.post (сервисы без себестоимости).
    let entries: Vec<serde_json::Value> = match type_code {
        "PURCHASE" => vec![
            serde_json::json!({"debit_code": accounts.goods, "credit_code": accounts.supplier_settlements, "amount": total_kop}),
        ],
        "SALES" => {
            let mut v = vec![serde_json::json!({
                "debit_code": accounts.customer_settlements,
                "credit_code": accounts.revenue,
                "amount": total_kop,
            })];
            if let Some(op) = stock_op {
                // Себестоимость продаж = Σ total_cost списаний документа
                v.push(serde_json::json!({
                    "debit_code": accounts.cogs,
                    "credit_code": accounts.goods,
                    "amount": {"$ref": format!("{op}.total_cost")},
                }));
            }
            v
        }
        "CUSTOMER_RETURN" => {
            let ret_kop = total_kop.abs();
            let mut v = vec![
                serde_json::json!({"debit_code": accounts.revenue, "credit_code": accounts.customer_settlements, "amount": ret_kop}),
            ];
            if let Some(op) = stock_op {
                // Товар возвращается на склад по той же цифре, что принята
                // складом (Дт 41 Кт 90.2) — один источник: результат receipt.
                v.push(serde_json::json!({
                    "debit_code": accounts.goods,
                    "credit_code": accounts.cogs,
                    "amount": {"$ref": format!("{op}.total_cost")},
                }));
            }
            v
        }
        "SUPPLIER_RETURN" => {
            let ret_kop = total_kop.abs();
            vec![
                serde_json::json!({"debit_code": accounts.supplier_settlements, "credit_code": accounts.goods, "amount": ret_kop}),
            ]
        }
        _ => vec![],
    };

    if !entries.is_empty() {
        add(handle, "accounting.post", serde_json::json!({
            "doc_kind": type_code,
            "doc_id": doc_id,
            "lines": entries,
        }))?;
    }
    Ok(())
}

// ── on_cancel ──────────────────────────────────────────────

#[plugin_fn]
pub fn on_cancel(Json(input): Json<PostInput>) -> FnResult<Json<serde_json::Value>> {
    let (_doc, _code) = load_doc(&input.id)?;

    let handle_raw = unsafe { tx_begin(format!("cancel-{}", input.id)) }?;
    let handle_val = unwrap_host(handle_raw)?;
    let h = handle_val["handle"].as_str()
        .ok_or_else(|| anyhow::anyhow!("tx_begin без handle"))?;

    add(h, "stock.reverse", serde_json::json!({ "target_doc_id": input.id }))?;
    add(h, "accounting.reverse_by_doc", serde_json::json!({ "target_doc_id": input.id }))?;

    // Читаем актуальную версию из документа
    let cur_ver = _doc["version"].as_i64().unwrap_or(1);
    add(h, "object.cancel", serde_json::json!({
        "object_id": input.id,
        "expected_version": cur_ver,
    }))?;

    let commit_raw = unsafe { tx_commit(h.to_string()) }?;
    let result = unwrap_host(commit_raw)?;

    emit(&input.id, "trade.doc_cancelled", serde_json::json!({}));

    let _ = unsafe { log_message(format!(
        "[trade] документ {} отменён со сторно", input.id
    )) };

    Ok(Json(result))
}
