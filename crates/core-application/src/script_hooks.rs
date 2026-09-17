//! Авто-хук связанных скриптов в конвейер `object.create`/`object.update`
//! (Фаза 15.4 по ТЗ v3.1 §15).
//!
//! Механика «конвейера»: команда, создающая или обновляющая объект, перед
//! записью выполняет активные скрипты, привязанные к типу сущности объекта
//! (`script.entity_type == Some(entity_type)`):
//!
//! - `formula` — вычисляет `computed`-поля; результат (JSON-объект) сливается
//!   в `object.computed` перед валидацией схемы;
//! - `validator` — возвращает `false`, если объект неприемлем: команда
//!   завершается `DomainError::ValidationError`;
//! - `before_action` — побочные действия до записи; ошибка прерывает команду;
//! - `after_action` — побочные действия после записи (объект уже сохранён);
//!   ошибка прерывает команду, но запись остаётся в БД.
//!
//! Привязка по компании: `ScriptRepository::list(Some(company_id))` возвращает
//! скрипты компании и глобальные; скрипты других компаний не попадают в выборку.
//! Для объектов с пустой компанией (системные/глобальные) применяются только
//! глобальные скрипты.
//!
//! Контекст скрипта: `ctx.user` — актор команды (`ActorSnapshot`), `ctx.action` —
//! `object.create`/`object.update`, `ctx.object` — сериализованный объект,
//! `ctx.changes` — изменения (для update — переданные `data`),
//! `ctx.entity_type`/`ctx.company` — привязка объекта.

use crate::ports::{ScriptEngine, ScriptRepository};
use core_domain::error::DomainError;
use core_domain::event::ActorSnapshot;
use core_domain::script::{Script, ScriptType};
use serde_json::{json, Value};
use uuid::Uuid;

/// Выполняет предзаписные хуки объекта (formula → validator → before_action).
///
/// Возвращает `computed`-поля, вычисленные скриптами `formula` (JSON-объект,
/// пустой `{}`, если формул нет). Выполнение идёт последовательно:
/// формульные скрипты первыми, затем валидаторы (видят уже вычисленные
/// `computed`-поля), затем `before_action`.
///
/// Валидатор считается проваленным, если его результат — JSON `false` или
/// выполнение завершилось ошибкой. Ошибки `before_action` прерывают команду.
///
/// # Errors
///
/// Возвращает `DomainError::ValidationError` при отклонении валидатором,
/// любую ошибку скрипта движка и `DomainError::Storage` при сбое репозитория.
#[allow(clippy::too_many_arguments)]
pub async fn object_pre_hooks(
    scripts: &dyn ScriptRepository,
    engine: &dyn ScriptEngine,
    entity_type: &str,
    company_id: &str,
    action: &str,
    actor: &ActorSnapshot,
    mut object: Value,
    changes: Option<Value>,
) -> Result<Value, DomainError> {
    let bound = load_active_scripts(scripts, entity_type, company_id).await?;

    // 1. Формулы: наполняют computed-поля.
    let mut computed = Value::Object(
        object
            .get("computed")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default(),
    );
    for script in formula_scripts(&bound) {
        let ctx = build_context(entity_type, company_id, action, actor, &object, changes.clone());
        let value = engine.execute(script.source.clone(), &ctx).await?;
        if let Some(map) = value.as_object() {
            for (key, val) in map {
                computed.as_object_mut().unwrap().insert(key.clone(), val.clone());
            }
        }
    }
    if !computed.as_object().unwrap().is_empty() {
        object
            .as_object_mut()
            .unwrap()
            .insert("computed".to_string(), computed.clone());
    }

    // 2. Валидаторы: отклоняют неприемлемый объект.
    for script in validator_scripts(&bound) {
        let ctx = build_context(entity_type, company_id, action, actor, &object, changes.clone());
        let result = engine.execute(script.source.clone(), &ctx).await?;
        if result == Value::Bool(false) {
            return Err(DomainError::ValidationError(format!(
                "валидатор '{}' отклонил объект типа '{}'",
                script.code, entity_type
            )));
        }
    }

    // 3. before_action: побочные действия до записи.
    for script in before_scripts(&bound) {
        let ctx = build_context(entity_type, company_id, action, actor, &object, changes.clone());
        engine.execute(script.source.clone(), &ctx).await?;
    }

    Ok(computed)
}

/// Выполняет послезаписные хуки объекта (`after_action`).
///
/// Вызывается после успешной записи объекта. Ошибка скрипта прерывает
/// команду, однако сам объект уже сохранён в БД.
///
/// # Errors
///
/// Возвращает ошибку скрипта движка и `DomainError::Storage` при сбое
/// репозитория.
#[allow(clippy::too_many_arguments)]
pub async fn object_after_hooks(
    scripts: &dyn ScriptRepository,
    engine: &dyn ScriptEngine,
    entity_type: &str,
    company_id: &str,
    action: &str,
    actor: &ActorSnapshot,
    object: Value,
    changes: Option<Value>,
) -> Result<(), DomainError> {
    let bound = load_active_scripts(scripts, entity_type, company_id).await?;
    for script in after_scripts(&bound) {
        let ctx = build_context(entity_type, company_id, action, actor, &object, changes.clone());
        engine.execute(script.source.clone(), &ctx).await?;
    }
    Ok(())
}

/// Загружает активные скрипты, привязанные к типу сущности и доступные
/// компании (`Some(company_id)` — компания + глобальные; пустая строка —
/// только глобальные).
async fn load_active_scripts(
    scripts: &dyn ScriptRepository,
    entity_type: &str,
    company_id: &str,
) -> Result<Vec<Script>, DomainError> {
    let company_uuid = parse_company_id(company_id)?;
    let all = scripts.list(company_uuid.as_ref()).await?;
    Ok(all
        .into_iter()
        .filter(|s| s.is_active && s.entity_type.as_deref() == Some(entity_type))
        .collect())
}

/// Разбирает компанию как строку идентификатора; пустая строка — `None`
/// (глобальный объект).
fn parse_company_id(company_id: &str) -> Result<Option<Uuid>, DomainError> {
    if company_id.is_empty() {
        return Ok(None);
    }
    Uuid::parse_str(company_id)
        .map(Some)
        .map_err(|e| DomainError::ValidationError(format!("некорректная компания объекта: {e}")))
}

/// Собирает контекст `ScriptContext` для хука.
fn build_context(
    entity_type: &str,
    company_id: &str,
    action: &str,
    actor: &ActorSnapshot,
    object: &Value,
    changes: Option<Value>,
) -> crate::script_context::ScriptContext {
    crate::script_context::ScriptContext {
        args: None,
        user: Some(actor.clone()),
        company_id: parse_company_id(company_id).ok().flatten(),
        entity_type: Some(entity_type.to_string()),
        action: Some(action.to_string()),
        object: Some(object.clone()),
        changes,
        settings: json!({}),
        test_run: false,
    }
}

fn formula_scripts(bound: &[Script]) -> Vec<&Script> {
    bound
        .iter()
        .filter(|s| s.script_type == ScriptType::Formula)
        .collect()
}

fn validator_scripts(bound: &[Script]) -> Vec<&Script> {
    bound
        .iter()
        .filter(|s| s.script_type == ScriptType::Validator)
        .collect()
}

fn before_scripts(bound: &[Script]) -> Vec<&Script> {
    bound
        .iter()
        .filter(|s| s.script_type == ScriptType::BeforeAction)
        .collect()
}

fn after_scripts(bound: &[Script]) -> Vec<&Script> {
    bound
        .iter()
        .filter(|s| s.script_type == ScriptType::AfterAction)
        .collect()
}