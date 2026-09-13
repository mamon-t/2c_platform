use core_domain::event::ActorSnapshot;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

/// Контекст выполнения скрипта Rhai (ТЗ v3.1 §15): доступен в скрипте как
/// объект `ctx` с полями `args`, `user`, `company`, `entity_type`, `action`,
/// `object`, `changes`, `settings`. Каналы `db`, `ledger`, `emit`, `notify`,
/// `log` регистрируются в движке как функции Core API (см. `RhaiScriptEngine`).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ScriptContext {
    /// Произвольные параметры вызова команды `script.execute`.
    pub args: Option<Value>,
    /// Исполнитель команды; `None` для анонимного/системного запуска.
    pub user: Option<ActorSnapshot>,
    pub company_id: Option<Uuid>,
    /// Привязка к типу сущности (для validator/event handler скриптов).
    pub entity_type: Option<String>,
    /// Выполняемое действие (например `document.create`).
    pub action: Option<String>,
    /// Текущий объект (для `formula`/`validator`/`report`).
    pub object: Option<Value>,
    /// Изменения полей (для `validator`/`before_action`/`after_action`).
    pub changes: Option<Value>,
    /// Настройки, доступные скрипту без обращения к БД.
    pub settings: Value,
    /// Флаг тестового запуска: `db_query` и `emit_event` не пишут в БД.
    pub test_run: bool,
}

impl ScriptContext {
    /// Облегчённая инициализация для тестов и команды `script.test_run`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Компания как строка для записи в события и фильтрации запросов.
    pub fn company_key(&self) -> Option<String> {
        self.company_id.map(|uuid| uuid.to_string())
    }
}