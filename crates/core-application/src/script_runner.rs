use crate::ports::{MetadataRepository, ScriptEngine, ScriptRepository};
use core_domain::error::DomainError;
use core_domain::event::ActorSnapshot;
use serde_json::{json, Value};
use std::time::Instant;
use uuid::Uuid;

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

/// Выполняет скрипт в тестовом режиме по команде `script.test` (ТЗ v3.1 §15).
///
/// В отличие от [`execute_script`]:
/// 1. Скрипт выполняется с `ScriptContext::test_run = true`, действие
///    `action = "test"` — побочные эффекты Core API в скрипте должны быть
///    отключены движком;
/// 2. Не применяются гейты `is_active` (отладка отключённого скрипта
///    разрешена) и правила привязки к `entity_type`/`object` (тестовый запуск
///    с минимальным контекстом не требует данных);
/// 3. Время выполнения замеряется и возвращается как `execution_time_ms`.
///
/// Ошибка выполнения скрипта возвращается как ошибка DomainError
/// (как правило `DomainError::ScriptFailure` с координатами) — вызывающая
/// сторона конвертирует её в ошибку RPC.
///
/// # Errors
///
/// Возвращает `DomainError::NotFound` для неизвестного кода,
/// `DomainError::ScriptFailure`/`DomainError::ValidationError` при ошибках
/// компиляции/выполнения движка, `DomainError::Storage` при сбое БД/лимитах.
pub async fn test_script(
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

    let context = crate::script_context::ScriptContext {
        args: params.get("args").cloned(),
        user: Some(actor.clone()),
        company_id,
        entity_type: record.entity_type.clone(),
        action: Some("test".to_string()),
        object: None,
        changes: None,
        settings: json!({}),
        test_run: true,
    };

    let started = Instant::now();
    let result = engine.execute(record.source.clone(), &context).await;
    let execution_time_ms = started.elapsed().as_millis() as u64;
    let result = result?;

    Ok(json!({
        "result": result,
        "execution_time_ms": execution_time_ms,
    }))
}

/// Проверяет скрипт по команде `script.validate` (ТЗ v3.1 §15).
///
/// Источник проверяемого кода берётся либо из явного параметра `source`,
/// либо из записи по `code` (валидация с привязкой не требует `is_active` —
/// отладка отключённого скрипта разрешена). Если скрипт привязан к типу
/// сущности (`entity_type`), дополнительно проверяется существование типа
/// в метаданных компании актора (или в глобальных системных типах при пустой
/// компании).
///
/// Ответ: `{ valid: bool, errors: [{ line, column, message }] }`. Ошибки без
/// координат (например, отсутствующий привязанный тип) получают `null`.
///
/// # Errors
///
/// Возвращает `DomainError` только при недоступности репозиториев (Storage)
/// или отсутствии записи по `code` (NotFound); ошибки самого скрипта
/// накапливаются в поле `errors` ответа.
pub async fn validate_script(
    scripts: &dyn ScriptRepository,
    engine: &dyn ScriptEngine,
    metadata: &dyn MetadataRepository,
    params: &Value,
    actor: &ActorSnapshot,
) -> Result<Value, DomainError> {
    let entity_type = params
        .get("entity_type")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let (source, bound_entity_type) = match params.get("code").and_then(|v| v.as_str()) {
        Some(code) => {
            let company_id = if let Some(raw) = params
                .get("company_id")
                .and_then(|v| v.as_str())
            {
                Some(Uuid::parse_str(raw).map_err(|e| {
                    DomainError::ValidationError(format!("некорректный 'company_id': {e}"))
                })?)
            } else {
                actor.company_id
            };
            let record = scripts
.get_by_code(code, company_id.as_ref())
                .await?
                .ok_or_else(|| {
                    DomainError::NotFound(format!("скрипт с кодом '{code}' не найден"))
                })?;
            (record.source.clone(), entity_type.or(record.entity_type.clone()))
        }
        None => {
            let source = params
                .get("source")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    DomainError::ValidationError(
                        "script.validate: отсутствует параметр 'source'".to_string(),
                    )
                })?
                .to_string();
            if source.is_empty() {
                return Err(DomainError::ValidationError(
                    "script.validate: source не может быть пустым".to_string(),
                ));
            }
            (source, entity_type)
        }
    };

    let mut errors: Vec<Value> = Vec::new();

    if let Some(bound) = &bound_entity_type {
        let company_id = actor
            .company_id
            .map(|u| u.to_string())
            .unwrap_or_default();
        if let Err(DomainError::NotFound(_)) = metadata
            .get_entity_type_by_code(&company_id, bound)
            .await
        {
            errors.push(json!({
                "line": Value::Null,
                "column": Value::Null,
                "message": format!("привязанный тип сущности '{bound}' не найден"),
            }));
        }
    }

    match engine.validate(&source) {
        Ok(()) => {}
        Err(err) => errors.push(json!({
            "line": err.line,
            "column": err.column,
            "message": err.message,
        })),
    }

    Ok(json!({
        "valid": errors.is_empty(),
        "errors": errors,
    }))
}