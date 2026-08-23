//! Движок склада: атомарные операции над остатками.
//!
//! Все функции принимают открытую сессию ИСПОЛНИТЕЛЯ (tx_exec) и пишут
//! только через неё. Инварианты:
//! - баланс локации = сумма движений по ней;
//! - партия съедается атомарным условным декрементом (не дважды);
//! - списание идёт строго по FIFO (receipt_date asc);
//! - сторно восстанавливает в ТУ ЖЕ партию; созданные документом партии
//!   удаляются только если нетронуты.

use futures::StreamExt;
use mongodb::bson::{doc, Document};

use crate::core::{CompanyId, PlatformError, PlatformResult};
use crate::db::MongoClient;
use crate::events::ActorSnapshot;

use super::{
    fmt_qty, insufficient, allow_negative, COL_BALANCES, COL_BATCHES, COL_MOVEMENTS,
    MovementKind, NomenclatureType,
};

/// Контекст исполнения движка внутри транзакции.
pub struct EngineCtx<'a> {
    pub db: &'a MongoClient,
    pub session: &'a mut mongodb::ClientSession,
    pub company_id: CompanyId,
    pub actor: ActorSnapshot,
}

/// Ссылка на документ-источник (doc_kind, doc_id).
pub type DocRef = Option<(String, String)>;

/// Партия, съеденная списанием.
#[derive(Debug, Clone, serde::Serialize)]
pub struct EatenPart {
    pub batch_id: String,
    pub qty: f64,
    pub unit_cost: i64,
}

/// Результат списания строки.
#[derive(Debug, Clone, serde::Serialize)]
pub struct IssuedLine {
    pub nomenclature_id: String,
    pub qty: f64,
    pub total_cost: i64,
    pub parts: Vec<EatenPart>,
}

// ── Чтение справочников (внутри снапшота транзакции) ──────

async fn object_data(
    e: &mut EngineCtx<'_>,
    id: &str,
) -> PlatformResult<serde_json::Value> {
    let d = e
        .db
        .collection::<Document>("objects")
        .find_one(doc! { "_id": id })
        .session(&mut *e.session)
        .await
        .map_err(|er| PlatformError::Database(er.to_string()))?
        .ok_or_else(|| PlatformError::NotFound(format!("Объект {id} не найден")))?;
    Ok(d.get("data")
        .cloned()
        .and_then(|b| mongodb::bson::from_bson::<serde_json::Value>(b).ok())
        .unwrap_or_default())
}

// ── Балансы ────────────────────────────────────────────────

async fn bump_balance(
    e: &mut EngineCtx<'_>,
    location_id: &str,
    nomenclature_id: &str,
    delta: f64,
) -> PlatformResult<()> {
    let now = chrono::Utc::now();
    e.db
        .collection::<Document>(COL_BALANCES)
        .update_one(
            doc! { "company_id": e.company_id.0.to_string(), "location_id": location_id, "nomenclature_id": nomenclature_id },
            doc! {
                "$inc": { "quantity": delta },
                "$set": { "updated_at": mongodb::bson::DateTime::from_millis(now.timestamp_millis()) },
                "$setOnInsert": {
                    "company_id": e.company_id.0.to_string(),
                    "location_id": location_id,
                    "nomenclature_id": nomenclature_id,
                },
            },
        )
        .upsert(true)
        .session(&mut *e.session)
        .await
        .map_err(|er| PlatformError::Database(er.to_string()))?;
    Ok(())
}

async fn read_balance(e: &mut EngineCtx<'_>, location_id: &str, nomenclature_id: &str) -> f64 {
    e.db
        .collection::<Document>(COL_BALANCES)
        .find_one(doc! {
            "company_id": e.company_id.0.to_string(),
            "location_id": location_id,
            "nomenclature_id": nomenclature_id,
        })
        .session(&mut *e.session)
        .await
        .ok()
        .flatten()
        .and_then(|d| d.get_f64("quantity").ok().or_else(|| d.get_i32("quantity").map(|v| v as f64).ok()))
        .unwrap_or(0.0)
}

// ── Движения ───────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
async fn write_movement(
    e: &mut EngineCtx<'_>,
    location_id: &str,
    nomenclature_id: &str,
    batch_id: Option<&str>,
    kind: MovementKind,
    quantity: f64,
    unit_cost: i64,
    doc_ref: DocRef,
    responsible_user_id: Option<&str>,
    expected_return_date: Option<&str>,
    is_reversal: bool,
) -> PlatformResult<()> {
    let mut d = doc! {
        "_id": uuid::Uuid::new_v4().to_string(),
        "company_id": e.company_id.0.to_string(),
        "location_id": location_id,
        "nomenclature_id": nomenclature_id,
        "kind": kind.as_str(),
        "quantity": quantity,
        "unit_cost": unit_cost,
        "total_cost": (quantity.abs() as i64) * unit_cost,
        "is_reversal": is_reversal,
        "created_at": mongodb::bson::DateTime::now(),
        "actor": {
            "user_id": e.actor.user_id.0.to_string(),
            "login": e.actor.login.clone(),
            "full_name": e.actor.full_name.clone().unwrap_or_default(),
        },
    };
    if let Some(b) = batch_id {
        d.insert("batch_id", b);
    }
    if let Some((dk, di)) = &doc_ref {
        d.insert("doc_kind", dk);
        d.insert("doc_id", di);
    }
    if let Some(r) = responsible_user_id {
        d.insert("responsible_user_id", r);
    }
    if let Some(d2) = expected_return_date {
        d.insert("expected_return_date", d2);
    }

    e.db
        .collection::<Document>(COL_MOVEMENTS)
        .insert_one(d)
        .session(&mut *e.session)
        .await
        .map_err(|er| PlatformError::Database(er.to_string()))?;
    Ok(())
}

// ── FIFO-списание ──────────────────────────────────────────

/// Съесть qty с локации по FIFO. Возвращает фактические партии и себестоимость.
/// При нехватке: allow_negative=false → ошибка; true → дефицит по средней цене.
#[allow(clippy::too_many_arguments)]
async fn eat_fifo(
    e: &mut EngineCtx<'_>,
    location_id: &str,
    nomenclature_id: &str,
    qty_needed: f64,
    kind: MovementKind,
    doc_ref: &DocRef,
) -> PlatformResult<IssuedLine> {
    let available = read_balance(e, location_id, nomenclature_id).await;
    let allow_neg = allow_negative(e.db, &e.company_id).await;
    if !allow_neg && available + 1e-9 < qty_needed {
        return Err(insufficient(qty_needed, available, nomenclature_id));
    }

    let mut parts: Vec<EatenPart> = Vec::new();
    let mut need = qty_needed.min(available.max(0.0));

    // Читаем живые партии FIFO (снапшот), затем пишем:
    // курсор не должен держать заём сессии во время записей.
    let mut parties: Vec<Document> = Vec::new();
    if need > 0.0 {
        let filter = doc! {
            "company_id": e.company_id.0.to_string(),
            "location_id": location_id,
            "nomenclature_id": nomenclature_id,
            "qty_remaining": { "$gt": 0 },
        };
        let mut cursor = e.db.collection::<Document>(COL_BATCHES)
            .find(filter)
            .sort(doc! { "receipt_date": 1 })
            .session(&mut *e.session)
            .await
            .map_err(|er| PlatformError::Database(er.to_string()))?;
        while let Some(b) = cursor.next(&mut *e.session).await {
            parties.push(b.map_err(|er| PlatformError::Database(er.to_string()))?);
        }
    }

    if need > 0.0 {
        let col = e.db.collection::<Document>(COL_BATCHES);

        for b in parties {
            let bid = b.get_str("_id").unwrap_or("").to_string();
            let remaining = b.get_f64("qty_remaining").unwrap_or(0.0);
            let unit_cost = b.get_i64("unit_cost").unwrap_or(0);
            let take = need.min(remaining);

            let upd = if take >= remaining {
                doc! { "$inc": { "qty_remaining": -take }, "$set": { "status": "exhausted" } }
            } else {
                doc! { "$inc": { "qty_remaining": -take } }
            };
            let res = col
                .update_one(
                    doc! { "_id": &bid, "company_id": e.company_id.0.to_string(), "qty_remaining": { "$gte": take } },
                    upd,
                )
                .session(&mut *e.session)
                .await
                .map_err(|er| PlatformError::Database(er.to_string()))?;
            if res.matched_count == 0 {
                return Err(PlatformError::Internal(format!(
                    "партия {bid}: конкурентное изменение внутри транзакции"
                )));
            }

            write_movement(e, location_id, nomenclature_id, Some(&bid), kind, -take, unit_cost, doc_ref.clone(), None, None, false).await?;
            parts.push(EatenPart { batch_id: bid, qty: take, unit_cost });
            need -= take;
        }
    }

    // Дефицит при разрешённых отрицательных остатках
    let total_from_parts: i64 = parts.iter().map(|p| (p.qty as i64) * p.unit_cost).sum();
    let consumed: f64 = parts.iter().map(|p| p.qty).sum();
    if consumed + 1e-9 < qty_needed {
        let deficit = qty_needed - consumed;
        let avg = if consumed > 0.0 {
            (total_from_parts as f64 / consumed).round() as i64
        } else {
            0
        };
        write_movement(e, location_id, nomenclature_id, None, kind, -deficit, avg, doc_ref.clone(), None, None, false).await?;
        parts.push(EatenPart { batch_id: String::new(), qty: deficit, unit_cost: avg });
    }

    let total_cost: i64 = parts.iter().map(|p| (p.qty.round() as i64) * p.unit_cost).sum();
    bump_balance(e, location_id, nomenclature_id, -qty_needed).await?;

    Ok(IssuedLine {
        nomenclature_id: nomenclature_id.to_string(),
        qty: qty_needed,
        total_cost,
        parts,
    })
}

// ── Расширение номенклатуры (наборы, услуги) ───────────────

/// Развернуть позицию в лист товаров: услуга выпадает, набор раскладывается.
async fn expand_line(
    e: &mut EngineCtx<'_>,
    nomenclature_id: &str,
    qty: f64,
    depth: u8,
    out: &mut Vec<(String, f64)>,
    skipped_services: &mut Vec<String>,
) -> PlatformResult<()> {
    if depth > 5 {
        return Err(PlatformError::Validation(format!(
            "набор {nomenclature_id}: слишком глубокая вложенность"
        )));
    }
    let data = object_data(e, nomenclature_id).await?;
    match NomenclatureType::from_data(&data) {
        NomenclatureType::Service => skipped_services.push(nomenclature_id.to_string()),
        NomenclatureType::Item => out.push((nomenclature_id.to_string(), qty)),
        NomenclatureType::Set => {
            for (comp_id, comp_qty) in super::set_components(&data) {
                Box::pin(expand_line(e, &comp_id, comp_qty * qty, depth + 1, out, skipped_services)).await?;
            }
        }
    }
    Ok(())
}

// ── Публичные операции движка ──────────────────────────────

/// Приход: создать партии, движения, поднять балансы.
pub async fn receipt(
    e: &mut EngineCtx<'_>,
    location_id: &str,
    lines: Vec<super::ReceiptLine>,
    doc_ref: DocRef,
) -> PlatformResult<serde_json::Value> {
    let mut created = Vec::new();
    let mut skipped_services = Vec::new();

    for line in lines {
        let mut items = Vec::new();
        expand_line(e, &line.nomenclature_id, line.qty, 0, &mut items, &mut skipped_services).await?;

        for (nom_id, qty) in items {
            let receipt_date = line.receipt_date.clone().unwrap_or_else(|| {
                chrono::Utc::now().date_naive().to_string()
            });
            let batch_id = uuid::Uuid::new_v4().to_string();
            let batch = doc! {
                "_id": &batch_id,
                "company_id": e.company_id.0.to_string(),
                "location_id": location_id,
                "nomenclature_id": &nom_id,
                "receipt_date": &receipt_date,
                "unit_cost": line.unit_cost,
                "qty_initial": qty,
                "qty_remaining": qty,
                "source_doc_id": doc_ref.as_ref().map(|(_, id)| id.as_str()).unwrap_or(""),
                "status": "active",
                "created_at": mongodb::bson::DateTime::now(),
            };
            e.db.collection::<Document>(COL_BATCHES)
                .insert_one(batch)
                .session(&mut *e.session).await
                .map_err(|er| PlatformError::Database(er.to_string()))?;

            write_movement(e, location_id, &nom_id, Some(&batch_id), MovementKind::Receipt,
                qty, line.unit_cost, doc_ref.clone(), None, None, false).await?;
            bump_balance(e, location_id, &nom_id, qty).await?;

            created.push(serde_json::json!({
                "batch_id": batch_id,
                "nomenclature_id": nom_id,
                "qty": qty,
                "unit_cost": line.unit_cost,
                "receipt_date": receipt_date,
            }));
        }
    }

    Ok(serde_json::json!({ "batches": created, "skipped_services": skipped_services }))
}

/// Списание по FIFO со строки (товары; наборы раскладываются).
pub async fn issue(
    e: &mut EngineCtx<'_>,
    location_id: &str,
    lines: Vec<super::IssueLine>,
    kind: MovementKind,
    doc_ref: DocRef,
) -> PlatformResult<serde_json::Value> {
    let mut issued = Vec::new();
    let mut skipped_services = Vec::new();

    for line in lines {
        let mut items = Vec::new();
        expand_line(e, &line.nomenclature_id, line.qty, 0, &mut items, &mut skipped_services).await?;
        for (nom_id, qty) in items {
            let issued_line = eat_fifo(e, location_id, &nom_id, qty, kind, &doc_ref).await?;
            issued.push(serde_json::to_value(&issued_line).unwrap_or_default());
        }
    }

    Ok(serde_json::json!({ "lines": issued, "skipped_services": skipped_services }))
}

/// Перемещение между любыми локациями (в т.ч. выдача/возврат под отчёт).
/// Цена и дата прихода партии переезжают вместе с товаром.
#[allow(clippy::too_many_arguments)]
pub async fn transfer(
    e: &mut EngineCtx<'_>,
    from_location_id: &str,
    to_location_id: &str,
    lines: Vec<super::IssueLine>,
    handover: bool,
    responsible_user_id: Option<String>,
    expected_return_date: Option<String>,
    link_doc: Option<String>,
    doc_ref: DocRef,
) -> PlatformResult<serde_json::Value> {
    if from_location_id == to_location_id {
        return Err(PlatformError::Validation(
            "Источник и приёмник совпадают".into(),
        ));
    }
    let (kind_out, kind_in) = if handover {
        (MovementKind::HandoverOut, MovementKind::HandoverIn)
    } else {
        (MovementKind::TransferOut, MovementKind::TransferIn)
    };

    let mut moved = Vec::new();
    let mut skipped_services = Vec::new();

    for line in lines {
        let mut items = Vec::new();
        expand_line(e, &line.nomenclature_id, line.qty, 0, &mut items, &mut skipped_services).await?;

        for (nom_id, qty) in items {
            // Съесть на источнике
            let issued = eat_fifo(e, from_location_id, &nom_id, qty, kind_out, &doc_ref).await?;

            // Воссоздать партии на приёмнике с той же ценой и датой
            for part in &issued.parts {
                if part.qty <= 0.0 { continue; }
                let receipt_date = chrono::Utc::now().date_naive().to_string();
                let new_batch_id = uuid::Uuid::new_v4().to_string();
                e.db.collection::<Document>(COL_BATCHES)
                    .insert_one(doc! {
                        "_id": &new_batch_id,
                        "company_id": e.company_id.0.to_string(),
                        "location_id": to_location_id,
                        "nomenclature_id": &nom_id,
                        "receipt_date": &receipt_date,
                        "unit_cost": part.unit_cost,
                        "qty_initial": part.qty,
                        "qty_remaining": part.qty,
                        "source_doc_id": doc_ref.as_ref().map(|(_, id)| id.as_str()).unwrap_or(""),
                        "status": "active",
                        "created_at": mongodb::bson::DateTime::now(),
                    })
                    .session(&mut *e.session).await
                    .map_err(|er| PlatformError::Database(er.to_string()))?;

                write_movement(e, to_location_id, &nom_id, Some(&new_batch_id), kind_in,
                    part.qty, part.unit_cost, doc_ref.clone(),
                    responsible_user_id.as_deref(), expected_return_date.as_deref(), false).await?;
                bump_balance(e, to_location_id, &nom_id, part.qty).await?;

                moved.push(serde_json::json!({
                    "nomenclature_id": nom_id,
                    "qty": part.qty,
                    "unit_cost": part.unit_cost,
                    "batch_id": new_batch_id,
                    "link_doc": link_doc,
                }));
            }
        }
    }

    Ok(serde_json::json!({ "moved": moved, "skipped_services": skipped_services }))
}

/// Инвентаризация: факт против учёта → излишки/недостачи.
pub async fn count(
    e: &mut EngineCtx<'_>,
    location_id: &str,
    facts: Vec<super::IssueLine>,
    doc_ref: DocRef,
) -> PlatformResult<serde_json::Value> {
    let mut results = Vec::new();

    for line in facts {
        let current = read_balance(e, location_id, &line.nomenclature_id).await;
        let diff = line.qty - current;

        if diff > 1e-9 {
            // Излишек: партия по средней стоимости активных (или 0)
            let avg = avg_party_cost(e, location_id, &line.nomenclature_id).await;
            let batch_id = uuid::Uuid::new_v4().to_string();
            e.db.collection::<Document>(COL_BATCHES)
                .insert_one(doc! {
                    "_id": &batch_id,
                    "company_id": e.company_id.0.to_string(),
                    "location_id": location_id,
                    "nomenclature_id": &line.nomenclature_id,
                    "receipt_date": chrono::Utc::now().date_naive().to_string(),
                    "unit_cost": avg,
                    "qty_initial": diff,
                    "qty_remaining": diff,
                    "source_doc_id": doc_ref.as_ref().map(|(_, id)| id.as_str()).unwrap_or(""),
                    "status": "active",
                    "created_at": mongodb::bson::DateTime::now(),
                })
                .session(&mut *e.session).await
                .map_err(|er| PlatformError::Database(er.to_string()))?;
            write_movement(e, location_id, &line.nomenclature_id, Some(&batch_id),
                MovementKind::CountSurplus, diff, avg, doc_ref.clone(), None, None, false).await?;
            bump_balance(e, location_id, &line.nomenclature_id, diff).await?;
            results.push(serde_json::json!({"nomenclature_id": line.nomenclature_id, "surplus": fmt_qty(diff)}));
        } else if diff < -1e-9 {
            eat_fifo(e, location_id, &line.nomenclature_id, -diff, MovementKind::CountShortage, &doc_ref).await?;
            results.push(serde_json::json!({"nomenclature_id": line.nomenclature_id, "shortage": fmt_qty(-diff)}));
        } else {
            results.push(serde_json::json!({"nomenclature_id": line.nomenclature_id, "match": true}));
        }
    }

    Ok(serde_json::json!({ "results": results }))
}

async fn avg_party_cost(e: &mut EngineCtx<'_>, location_id: &str, nomenclature_id: &str) -> i64 {
    let mut cursor = e.db.collection::<Document>(COL_BATCHES)
        .find(doc! {
            "company_id": e.company_id.0.to_string(),
            "location_id": location_id,
            "nomenclature_id": nomenclature_id,
            "qty_remaining": { "$gt": 0 },
        })
        .session(&mut *e.session).await
        .ok();
    let (mut sum, mut cnt) = (0i64, 0i64);
    if let Some(cursor) = cursor.as_mut() {
        while let Some(Ok(b)) = cursor.next(&mut *e.session).await {
            sum += b.get_i64("unit_cost").unwrap_or(0);
            cnt += 1;
        }
    }
    if cnt > 0 { sum / cnt } else { 0 }
}

/// Чтение балансов с фильтрами (можно вызывать внутри транзакции).
pub async fn balances(
    e: &mut EngineCtx<'_>,
    location_id: Option<&str>,
    nomenclature_id: Option<&str>,
) -> PlatformResult<serde_json::Value> {
    let mut filter = doc! { "company_id": e.company_id.0.to_string() };
    if let Some(l) = location_id { filter.insert("location_id", l); }
    if let Some(n) = nomenclature_id { filter.insert("nomenclature_id", n); }

    let mut cursor = e.db.collection::<Document>(COL_BALANCES)
        .find(filter).session(&mut *e.session).await
        .map_err(|er| PlatformError::Database(er.to_string()))?;
    let mut items = Vec::new();
    while let Some(Ok(b)) = cursor.next(&mut *e.session).await {
        items.push(serde_json::json!({
            "location_id": b.get_str("location_id").unwrap_or(""),
            "nomenclature_id": b.get_str("nomenclature_id").unwrap_or(""),
            "quantity": b.get_f64("quantity").unwrap_or(0.0),
        }));
    }
    Ok(serde_json::json!({ "balances": items }))
}

/// Строгое сторно документа: разворот всех его движений.
///
/// Правила:
/// - движения Issue/Shortage/TransferOut/HandoverOut → вернуть количество
///   в ТУ ЖЕ партию (партия оживает, статус active);
/// - движения Receipt/Surplus/TransferIn/HandoverIn → удалить созданную
///   партию; если по ней уже были движения — отказ;
/// - балансы корректируются зеркально.
pub async fn reverse_document(
    e: &mut EngineCtx<'_>,
    doc_id: &str,
) -> PlatformResult<serde_json::Value> {
    let col_m = e.db.collection::<Document>(COL_MOVEMENTS);
    let col_b = e.db.collection::<Document>(COL_BATCHES);

    let mut cursor = col_m
        .find(doc! {
            "company_id": e.company_id.0.to_string(),
            "doc_id": doc_id,
            "is_reversal": { "$ne": true },
        })
        .sort(doc! { "created_at": -1 }) // LIFO
        .session(&mut *e.session)
        .await
        .map_err(|er| PlatformError::Database(er.to_string()))?;

    let mut undone = 0u32;
    while let Some(m) = cursor.next(&mut *e.session).await {
        let m = m.map_err(|er| PlatformError::Database(er.to_string()))?;
        let mid = m.get_str("_id").unwrap_or("").to_string();
        let kind = m.get_str("kind").unwrap_or("").to_string();
        let location = m.get_str("location_id").unwrap_or("").to_string();
        let nom = m.get_str("nomenclature_id").unwrap_or("").to_string();
        let qty_abs = m.get_f64("quantity").unwrap_or(0.0).abs();
        let batch_id = m.get_str("batch_id").unwrap_or("").to_string();

        match kind.as_str() {
            // Расходные: вернуть в ту же партию
            "issue" | "count_shortage" | "transfer_out" | "handover_out" => {
                col_b.update_one(
                    doc! { "_id": &batch_id, "company_id": e.company_id.0.to_string() },
                    doc! {
                        "$inc": { "qty_remaining": qty_abs },
                        "$set": { "status": "active" },
                    },
                )
                .session(&mut *e.session).await
                .map_err(|er| PlatformError::Database(er.to_string()))?;
                bump_balance(e, &location, &nom, qty_abs).await?;
            }
            // Приходные: удалить созданную партию, если нетронута
            "receipt" | "count_surplus" | "transfer_in" | "handover_in" => {
                let party = col_b
                    .find_one(doc! { "_id": &batch_id, "company_id": e.company_id.0.to_string() })
                    .session(&mut *e.session).await
                    .map_err(|er| PlatformError::Database(er.to_string()))?
                    .ok_or_else(|| PlatformError::Validation(format!(
                        "сторно: партия {batch_id} не найдена"
                    )))?;
                let remaining = party.get_f64("qty_remaining").unwrap_or(0.0);
                let initial = party.get_f64("qty_initial").unwrap_or(0.0);
                if (remaining - initial).abs() > 1e-9 {
                    return Err(PlatformError::Validation(format!(
                        "сторно невозможно: по партии уже были движения (осталось {}, было {})",
                        fmt_qty(remaining), fmt_qty(initial)
                    )));
                }
                col_b.delete_one(doc! { "_id": &batch_id, "company_id": e.company_id.0.to_string() })
                    .session(&mut *e.session).await
                    .map_err(|er| PlatformError::Database(er.to_string()))?;
                bump_balance(e, &location, &nom, -qty_abs).await?;
            }
            other => {
                return Err(PlatformError::Validation(format!(
                    "сторно: неизвестный вид движения {other:?}"
                )));
            }
        }

        col_m.update_one(
            doc! { "_id": &mid },
            doc! { "$set": { "reversed": true } },
        )
        .session(&mut *e.session).await
        .map_err(|er| PlatformError::Database(er.to_string()))?;
        undone += 1;
    }

    if undone == 0 {
        return Err(PlatformError::NotFound(format!(
            "движения документа {doc_id} не найдены (или уже отменены)"
        )));
    }

    Ok(serde_json::json!({ "undone_movements": undone, "doc_id": doc_id }))
}
