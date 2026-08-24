//! Движок склада — нативный слой инвариантов.
//!
//! Разделение ответственности:
//! - ДВИЖОК (этот модуль): атомарные операции над остатками — приход,
//!   списание по FIFO, перемещение, выдача/возврат под отчёт,
//!   инвентаризация, сторно. Гарантирует: баланс = сумме движений,
//!   партия не съедается дважды, сторно возвращает в ту же партию.
//! - ОРКЕСТРАЦИЯ (обработчики документов, плагины): собирают пачки
//!   из движковых операций через tx_exec. Движок не знает, в какую
//!   пачку его операцию положат.
//!
//! Справочники (номенклатура, места учёта) и документы живут в
//! универсальной коллекции objects через метамодель. Конвенции полей
//! data см. `nomenclature::` и `location::` ниже.

pub mod commands;
pub mod engine;
pub mod handlers;
pub mod indexes;

use serde::{Deserialize, Serialize};

// ── Коллекции ──────────────────────────────────────────────

pub const COL_MOVEMENTS: &str = "stock_movements";
pub const COL_BATCHES: &str = "stock_batches";
pub const COL_BALANCES: &str = "stock_balances";

/// Коды типов сущностей метамодели.
pub const ET_NOMENCLATURE: &str = "NOMENCLATURE";
pub const ET_STOCK_LOCATION: &str = "STOCK_LOCATION";

// ── Конвенции полей data номенклатуры ──────────────────────

/// Тип позиции номенклатуры (data.type).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NomenclatureType {
    /// Товар — приходуется и списывается.
    Item,
    /// Услуга — в остатках не участвует (движок пропускает).
    Service,
    /// Набор — не приходуется; при списании раскладывается на компоненты.
    Set,
}

impl NomenclatureType {
    pub fn from_data(data: &serde_json::Value) -> Self {
        match data.get("type").and_then(|v| v.as_str()) {
            Some("service") => Self::Service,
            Some("set") => Self::Set,
            _ => Self::Item,
        }
    }
}

/// Компонент набора: {nomenclature_id, qty}.
pub fn set_components(data: &serde_json::Value) -> Vec<(String, f64)> {
    data.get("components")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|c| {
                    Some((
                        c.get("nomenclature_id")?.as_str()?.to_string(),
                        c.get("qty")?.as_f64().unwrap_or(1.0),
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

// ── Строки операций ────────────────────────────────────────

/// Строка прихода.
#[derive(Debug, Clone, Deserialize)]
pub struct ReceiptLine {
    pub nomenclature_id: String,
    pub qty: f64,
    /// Цена за единицу, минорные единицы (копейки).
    pub unit_cost: i64,
    /// Дата прихода — ключ FIFO. Пусто → сейчас.
    #[serde(default)]
    pub receipt_date: Option<String>,
}

/// Строка расхода/перемещения/инвентаризации.
#[derive(Debug, Clone, Deserialize)]
pub struct IssueLine {
    pub nomenclature_id: String,
    pub qty: f64,
}

// ── Вид движения ───────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MovementKind {
    Receipt,
    Issue,
    TransferIn,
    TransferOut,
    HandoverOut,
    HandoverIn,
    CountSurplus,
    CountShortage,
}

impl MovementKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Receipt => "receipt",
            Self::Issue => "issue",
            Self::TransferIn => "transfer_in",
            Self::TransferOut => "transfer_out",
            Self::HandoverOut => "handover_out",
            Self::HandoverIn => "handover_in",
            Self::CountSurplus => "count_surplus",
            Self::CountShortage => "count_shortage",
        }
    }
}

// ── Настройки компании (единая точка чтения) ───────────────

pub const SETTINGS_COLLECTION: &str = "app_settings";

/// Разрешены ли отрицательные остатки (настройка компании).
/// По умолчанию запрещены.
pub async fn allow_negative(
    db: &crate::db::MongoClient,
    company_id: &crate::core::CompanyId,
) -> bool {
    let d = db
        .collection::<mongodb::bson::Document>(SETTINGS_COLLECTION)
        .find_one(mongodb::bson::doc! {
            "company_id": company_id.0.to_string(),
            "key": "stock",
        })
        .await;
    d.ok()
        .flatten()
        .and_then(|d| d.get_document("value").ok().cloned())
        .and_then(|v| v.get_bool("allow_negative").ok())
        .unwrap_or(false)
}

// ── Ошибки домена ──────────────────────────────────────────

pub fn insufficient(needed: f64, available: f64, nomenclature_id: &str) -> crate::core::PlatformError {
    crate::core::PlatformError::Validation(format!(
        "Недостаточно {}: нужно {}, есть {}",
        nomenclature_id, fmt_qty(needed), fmt_qty(available)
    ))
}

/// Форматирование количества без хвостовых нулей.
pub fn fmt_qty(q: f64) -> String {
    let s = format!("{q:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}
