use crate::ports::{ScriptEngine, ScriptRepository};
use core_domain::error::DomainError;
use core_domain::event::ActorSnapshot;
use serde_json::{json, Value};

/// Выполняет скрипт по команде `script.execute` (ТЗ v3.1 §15)
/// с привязкой контекста к исполнителю из JWT.
///
/// Логика валидации:
/// 1. Скрипт разрешается по обязательному параметру `code` в рамках компании
///    актора; отсутствие записи — `DomainError::NotFound`.
/// 2. Отключённый скрипт (`is_active == false`) — `DomainError::ValidationError`.
/// 3. Если запись привязана к `entity_type`, а объект не передан —
///    `DomainError::ValidationError`.
/// 4. Если объект передан, а запись не привязана к типу —
///    `DomainError::ValidationError`.
///
/// Контекст `ScriptContext`: `args`/`entity_type`/`object` из параметров
/// (тип из записи, если параметр не задан), `action = "execute"`, актор и
/// компания берутся из переданного исполнителя.
///
/// # Errors
///
/// Возвращает `DomainError::NotFound` для неизвестного кода,
/// `DomainError::ValidationError` при нарушении правил привязки и ошибках
/// компиляции/выполнения движка, `DomainError::Storage` при сбое БД/лимитах.
pub async fn execute_script(
    scripts: &dyn ScriptRepository,
    engine: &dyn ScriptEngine,
    params: &Value,
    actor: &ActorSnapshot,
) -> Result<Value, DomainError> {
    let code = params
        .get("code")
        .and_then(|v| v.as_str())
        .ok_or_else(|| DomainError::ValidationError("отсутствует обязательный параметр 'code'".to_string()))?
        .to_string();

    let company_id = actor.company_id;
    let record = scripts
        .get_by_code(&code, company_id.as_ref())
        .await?
        .ok_or_else(|| DomainError::NotFound(format!("скрипт с кодом '{code}' не найден")))?;

    if !record.is_active {
        return Err(DomainError::ValidationError(format!("скрипт '{code}' отключён")));
    }

    let object = params.get("object").cloned();
    if let Some(et) = &record.entity_type {
        if object.is_none() {
            return Err(DomainError::ValidationError(format!(
                "скрипт привязан к типу {et}, требуется передать object"
            )));
        }
    } else if object.is_some() {
        return Err(DomainError::ValidationError(
            "скрипт не привязан к типу сущности, object передавать нельзя".to_string(),
        ));
    }

    let entity_type = params
        .get("entity_type")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| record.entity_type.clone());

    let context = crate::script_context::ScriptContext {
        args: params.get("args").cloned(),
        user: Some(actor.clone()),
        company_id,
        entity_type,
        action: Some("execute".to_string()),
        object,
        changes: None,
        settings: json!({}),
        test_run: false,
    };

    engine.execute(record.source.clone(), &context).await
}