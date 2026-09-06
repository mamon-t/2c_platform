//! Общий запуск подключения к SurrealDB.

use core_domain::error::DomainError;
use surrealdb::engine::any::Any;
use surrealdb::Surreal;

/// Открывает единственное подключение к SurrealDB по WebSocket, выполняет
/// аутентификацию пользователя root и выбирает целевое пространство
/// имён и базу данных. Возвращаемый общий клиент клонируется в хранилище
/// событий и во все репозитории, так что одна сессия на процесс обслуживает
/// всё приложение.
pub async fn connect_db(
    host: &str,
    user: &str,
    pass: &str,
    ns: &str,
    db_name: &str,
) -> Result<Surreal<Any>, DomainError> {
    let endpoint = format!("ws://{host}");
    let db = surrealdb::engine::any::connect(&endpoint)
        .await
        .map_err(|e| DomainError::Storage(format!("connect {endpoint}: {e}")))?;
    db.signin(surrealdb::opt::auth::Root {
        username: user.to_string(),
        password: pass.to_string(),
    })
    .await
    .map_err(|e| DomainError::Storage(format!("signin: {e}")))?;
    db.use_ns(ns)
        .use_db(db_name)
        .await
        .map_err(|e| DomainError::Storage(format!("use ns/db {ns}/{db_name}: {e}")))?;
    Ok(db)
}