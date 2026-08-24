//! Модуль «Торговля» — оркестратор поступлений, реализаций и возвратов.
//!
//! WASM-плагин собирает пачки tx_exec из операций склада и учёта.
//! Этот нативный модуль поставляет: seed метаданных (7 типов сущностей),
//! частичные индексы objects, «цена на дату».

pub mod commands;
pub mod indexes;
pub mod seed;

/// Коды типов сущностей метамодели торговли.
pub const ET_COUNTERPARTY: &str = "COUNTERPARTY";
pub const ET_PRICE_TYPE: &str = "PRICE_TYPE";
pub const ET_PRICE: &str = "PRICE";
pub const ET_PURCHASE: &str = "PURCHASE";
pub const ET_SALES: &str = "SALES";
pub const ET_CUSTOMER_RETURN: &str = "CUSTOMER_RETURN";
pub const ET_SUPPLIER_RETURN: &str = "SUPPLIER_RETURN";
