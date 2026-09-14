# AGENTS.md — 2C Platform

Глобальные правила агента (роль, LSP-валидация, запреты на заглушки/`unwrap`, формат вывода)
заданы в `~/.config/opencode/AGENTS.md` и загружаются автоматически. Ниже — только то,
что репозиторий не отвечает сам: слои, команды, тесты, live-отладка, конвенции.

## Проект и архитектура

Конфигурируемая документо-событийная платформа (бухгалтерия, управленческий учёт, CRM).
Спецификация: `doc/TZ_v3.1.md` (`doc/TZ_v3.0.md` — архив).

Сквозная модель **Труба + Доска**: каждая изменяющая команда атомарно пишет и событие
в Event Store (Труба — истина), и материализованную запись (Доска — проекции), и снимок
в `audit_log` (двухуровневое журналирование: бизнес-события ≠ операционный аудит).
С Фазы 10b есть JWT-аутентификация (`user.login`/`user.logout`,
`Authorization: Bearer` в `/rpc` и `?token=` в `/ws`); большинство команд
исполняются от системного исполнителя `ActorSnapshot::system()`.

## Workspace и слои

Workspace: `crates/core-domain` → `crates/core-application` → `crates/core-infrastructure`
+ `crates/core-api` (транспорт RpcMessage: `/rpc`, `/ws`, PushHub) + бинарник `apps/platform-server`.
Toolchain закреплён в `rust-toolchain.toml` (channel 1.96.0, компоненты rustfmt + clippy).

- Зависимости направлены вниз. `core-domain` — чистый домен, в нём НЕ должно быть
  `surrealdb`/`axum`/`extism`/`tokio`. Проверка: `cargo tree -p core-domain -e normal`.
  `core-infrastructure` — единственный слой с SurrealDB и Extism.
- `examples/hello_plugin` — **вне workspace** (exclude в корневом Cargo.toml); собирается
  отдельно под `wasm32-unknown-unknown` (см. «Сборка WASM-фикстуры»).
- Никаих CI-workflow'ов и конфигов rustfmt/clippy в репозитории нет.

## Ключевые файлы

| Файл | Зачем нужен |
|---|---|
| `crates/core-domain/src/{event,company,user,role,permission,audit,metadata,object,wasm_manifest,module,aggregate,error,types,password}.rs` | Модели и типы домена, 10 `StreamType`'ов, `DomainError`, `Version` (u64), Argon2id `hash_password`/`verify_password` |
| `crates/core-application/src/ports.rs` | Порты: EventStore, все `*Repository` (+`ModuleRepository`), WasmHost, EntitySchema |
| `crates/core-application/src/command_registry.rs` | CommandRegistry (префиксные команды) + `CommandExecutionPipeline` (аудит + RBAC перед каждой командой) |
| `crates/core-application/src/permission_manager.rs`, `seed.rs`, `registry.rs`, `app_registry.rs` | Deny-by-default RBAC, сид системных ролей/политик (4 роли/6 политик, `SeedSummary`, дополнение существующих ролей), ensure-регистры |
| `crates/core-application/src/bootstrap.rs` | `system.bootstrap`: `bootstrap_platform` — однократная инициализация платформы (компания → суперадмин Argon2id → сид → привязка роли + primary-профиль → аудит); доступна только системному актору (`requires("system.bootstrap")`) |
| `crates/core-application/src/auth.rs` | `AuthService` (10b): `login`/`logout`, аудит `user.login`/`user.login_failed`/`user.logout`, блокировка ≥5 попыток на 15 мин, `TokenManager`-порт |
| `crates/core-application/src/module_manager.rs` | `ModuleManager` (9b): install/uninstall/enable/disable + декларативная регистрация манифеста (политики, схемы, команды `plugin.*`) |
| `crates/core-application/src/transaction_orchestrator.rs` | `TransactionOrchestrator` (9e): begin/add_op/commit транзакций модулей, `$ref`-связывание, идемпотентность по business_key, GC (TTL 5 мин) |
| `crates/core-infrastructure/src/connector.rs`, `events.rs` | `connect_db` (единая WS-сессия), транзакционные хелперы append/assign_versions/with_transaction |
| `crates/core-infrastructure/src/surreal_{event_store,company,user,role,permission_policy,object,audit,metadata,module}_repository.rs` | SQL-доступ по коллекциям; у каждого `ensure_schema()` с UNIQUE-индексами; схема создаётся при старте, а не SQL-миграциями |
| `crates/core-infrastructure/src/extism_wasm_host.rs`, `module_kv.rs` | WASM-хост (Extism 1.30), host-функции, KV-хранилище модулей |
| `crates/core-api/src/{rpc_message,routes,token,ws,push_hub,error_mapping,idempotency}.rs` | (10a/10b/10c): конверт `RpcMessage`, `POST /rpc`, `JwtTokenManager`, `GET /ws`, `PushHub`, маппинг ошибок, идемпотентность |
| `apps/platform-server/src/main.rs` | Старт: ensure_schema всех репо, регистрация команд по фазам, attach RBAC-pipeline, /health + debug REST, merge RpcMessage-роутера (`/rpc`, `/ws`) |
| `apps/platform-server/src/commands.rs` | Все команды: `company.*`, `user.*` (+contact/profile), `role.*` (+`role.seed`), `metadata.*`, `object.*` (+snapshot), `document.number.*`, `audit.*`, `module.*`, `system.migrate_permissions` |

## Команды

```bash
cargo build
cargo test                 # ср. ниже: интеграционные тесты НЕ требуют живого SurrealDB
cargo test -p core-infrastructure   # только интеграционные приёмочные (mem://)
cargo clippy --workspace --all-targets   # ⚠️ surrealdb-core делает медленным — ставь таймаут >= 600s
❌ cargo fmt --all — НЕ использовать, ломает форматирование. Допускаются только ручные правки.

# Сервер (читает .env через dotenvy)
cargo run -p platform-server
curl :8080/health
curl -X POST :8080/debug/command -H 'Content-Type: application/json' \
     -d '{"name":"company.create","params":{"code":"x","name":"X"}}'
curl -X POST :8080/debug/events -H 'Content-Type: application/json' -d '[{...Event...}]'
curl :8080/debug/streams/object/{sid}
curl -X POST :8080/rpc -H 'Content-Type: application/json' \
     -d '{"type":"command","id":"1","module":"core","action":"company.create","payload":{"code":"x","name":"X"}}' \
     -H 'Authorization: Bearer <JWT>'

# Прямой SQL к SurrealDB (NS/DB — заголовки)
curl -u root:root -H "Surreal-NS: main" -H "Surreal-DB: 2cplatform_v30" :8000/sql \
     --data "SELECT * FROM events LIMIT 5;"
```

## Тесты — как это реально работает

- Интеграционные тесты `core-infrastructure` (`tests/phase5_rbac.rs`, `tests/hello_wasm.rs`,
  `tests/phase9b_modules.rs`, `tests/phase9d_preload.rs`) гоняются на `mem://`-базе: репозитории
  подключаются к SurrealDB через feature `kv-mem` (активируется только для тестов в
  `[dev-dependencies]`). Живой сервер не нужен.
- Интеграционные тесты `core-api` (`tests/phase10a_rpc.rs`, `tests/phase10b_auth_rpc.rs`,
  `tests/phase10c_ws.rs`) гоняются без живого сервера: 10a/10b через `Router::oneshot`,
  10c — через live `axum::serve` на ephemeral-порту + `tokio-tungstenite::connect_async`.
- Юнит-тесты внутри крейтов: репозитории (`mem://`), `IdempotencyStore`, `JwtTokenManager`,
  `PushHub`, `password.rs` (Argon2id round-trip).
- `hello_wasm.rs` требует фикстуру `crates/core-infrastructure/tests/fixtures/hello.wasm`.
  Она компилируется из `examples/hello_plugin` и закоммичена; после изменения модуля —
  пересобрать и заменить (см. ниже).
- Модульные тесты внутри репозиториев (напр., `ObjectRepository`) — тоже `mem://`, без внешних сервисов.

## Сборка WASM-фикстуры (вне workspace)

```bash
rustup target add wasm32-unknown-unknown
cd examples/hello_plugin && cargo build --release --target wasm32-unknown-unknown
cd ../..
cp examples/hello_plugin/target/wasm32-unknown-unknown/release/hello_plugin.wasm \
   crates/core-infrastructure/tests/fixtures/hello.wasm
```

Сборку запускать **из каталога `examples/hello_plugin`** (без `--manifest-path`):
только так cargo прочитает локальный `.cargo/config` с обёрткой линкера
`lld-wrapper.sh`, которая отбрасывает `-fuse-ld=*` из глобального
`~/.cargo/config` (этот флаг rust-lld на wasm32 не принимает).

## Окружение

`.env` (gitignored, образец — в репо отсутствует; обязателен для запуска сервера):
`SURREAL_HOST=host:8000`, `SURREAL_USER/PASS/NS/DB`, `SERVER_ADDR=0.0.0.0:8080`.
SurrealDB поднимается в Docker (см. `doc/surreal-docker.md`), порт 8000.
`doc/technical_report.md` — локальный рабочий отчёт, также gitignored.

## Конвенции проекта

- Код, идентификаторы, типы — английские; **комментарии в коде и логи/сообщения — русские**.
- Коммиты: conventional commits `<scope>: <описание>`, scope часто `feat(phaseN)`/`docs(agents)`/`chore`.
- Именование snake_case, ошибки — через `Result`, без `unwrap`/`expect` в проде.
- **Снимок после каждого коммита** (обязательно): сначала код → коммит, затем запись
  в `doc/technical_report.md` — таблица фаз §2 с хешем из `git rev-parse --short HEAD`
  и «Журнал снимков» (хеш, дата, затронутые разделы).
- **Факты live-тестирования** (обязательно): неожиданности и ограничения, всплывшие при
  ручной проверке против живой SurrealDB (отказ прав по `company_id`, кривые сценарии,
  нюансы роутинга и т.п.), фиксировать в §7 «Команды и верификация» `doc/technical_report.md`
  сразу по факту — это источник истины для отчёта наравне со снимками коммитов.
- **Денежные суммы** (discipline, Фаза 14+): только целые минимальные единицы валюты
  (копейки) — `Money(i128)` в `core-domain`, плагины используют `i64`-копейки; `f64`/`f32`
  для денег запрещены, дробные суммы отклоняются валидацией `FieldType::Money`.

## Статус фаз

Реализовано: Фазы 1–10 (10a + 10b + 10c), 13 и 14, доработка перед 11б
(SDUI-навигация: `module.navigation`/`platform.modules`, `RoleRepository::update`,
`ManifestNavItem.entity_type`) и `system.bootstrap`. Фазы 1–8:
`Фаза 1` — каркас (слои ядра, Axum 0.8, `/health`, dotenvy, tracing, graceful shutdown);
`Фаза 2` — компании/пользователи/роли (12 команд, «Доска+Труба»);
`Фаза 3` — метаданные (entity_types, fields, states, transitions, forms, relations, actions);
`Фаза 4` — аудит (`audit.log`/`audit.query`, 5 индексов);
`Фаза 5` — RBAC (`PermissionManager` + `CommandExecutionPipeline`, системные роли/политики,
`role.seed`/`system.migrate_permissions`);
`Фаза 6` — объекты, CRUD + OCC по `version`, снимки, атомарная нумерация;
`Фаза 7` — Event Store (Труба): `SurrealEventStore`, индексы, идемпотентный append;
`Фаза 8` — `CommandRegistry`, `AppRegistry`, 5 ensure-регистров.
Фаза 9 (WASM/Extism): host-fn `emit_event`, `users_by_role`, `module_kv`,
`ObjectRepository::count`, менеджер модулей (install/uninstall/enable/disable,
декларативная регистрация политик/схем/команд `plugin.*`), транзакционная
оркестрация `tx_begin`/`tx_add_op`/`tx_commit` (capability `transactions`,
`$ref`-связывание, атомарная пачка `update_batch`), `preload_all` при старте,
reinstall, интеграционные тесты (`phase5_rbac`, `hello_wasm`, `phase9b_modules`,
`phase9d_preload`).
Фаза 10: конверт `RpcMessage` + `POST /rpc` (`core-api`), типизированные ошибки
команд (`CommandRegistry` → `DomainError`), идемпотентность Command, JWT-аутентификация
(`user.login`/`user.logout`, `AuthService`, `JwtTokenManager`, Argon2id-пароли, фикс
RBAC для анонимов), тесты `phase10a_rpc`/`phase10b_auth_rpc`.
Фаза 10c: `PushHub` (broadcast ServerPush), `GET /ws` — WebSocket-транспорт `RpcMessage`
(актор из `?token=`, общий `process()` для REST/WS), `POST /debug/push`,
тесты `phase10c_ws`.
Фаза 13 (скрипты Rhai, ТЗ §15): `RhaiScriptEngine` (песочница `Engine::new_raw()`,
fuel 10M, лимиты коллекций/вложенности, выполнение на OS-потоке с таймаутом,
кэш AST), Core API `log_info/log_error/emit_event/emit_transaction/db_query`
через `await_core_api` (паттерн «spawn на владеющем рантайме + std-канал»),
`ScriptRepository` + модель `Script`, `ScriptContext` (поля `args`/`user`/
`company_id`/`entity_type`/`action`/`object`/`changes`/`settings`/`test_run`),
манифестные скрипты в `ModuleManifest`, host-fn `run_script` — теперь реальная
(была заглушка 9c); политика `platform.scripts` (seed, namespaced actions
`script.manage`/`script.execute`/`script.read`, priority 90, без привязки к
ролям); `CommandHandler` = `Fn(Value, CommandExecutionCtx)` (актор проброшен
в команды); `script_runner::execute_script` (валидации: bind к entity_type,
`is_active`, object-пара); команды `script.create/update/delete` (manage),
`script.list/get/validate` (read), `script.execute` (execute); фикс
блокировки воркера в WASM-пути: std `recv_timeout` → `tokio::sync::mpsc` +
`tokio::time::timeout`. Тесты 210 (`phase13_scripts` +5).
Фаза 14 (модуль управленческого учёта, ТЗ §14): платформенная поддержка —
манифестные команды с `function` (WASM-экспорт подчёркиванием, registry-имя
`plugin.{code}.{command_code}` с точками, контракт `{company_id, input}` →
`{output}`), `ManifestField.options` (enum-опции в декларативных схемах),
host-fn `get_entity_type_by_code` (cap `metadata.read`), op `object.create`
в `TransactionOrchestrator` (атомарный post+insert одной пачкой через
`update_batch` с insert-маркером `version == 0`), конструктор
`TransactionOrchestrator::new(objects, metadata)`; модуль `plugin.accounting`
(`examples/accounting_plugin`, вне workspace): 13 команд (account.*, period.*,
entry.*, doc.post, balance.trial/sheet), 3 схемы (account/accounting_period/
ledger_entry, поля enum-с options, lines[] как Table), 3 permissions
accounting.manage/read/post, capability `transactions`; проводки только
`debit == credit`, сторно — обратные записи, учётные периоды с open/close;
фикстура `accounting.wasm`, интеграционные тесты `phase14_accounting` (12,
полный цикл doc→post→entries→trial balance), всего тестов 223; live-цикл
/rpc + Bearer JWT (account.create → period.open → entry.post → balance.trial).
Доработка перед 11б (SDUI-навигация): системная политика `platform.modules`
(`module.read`, ByCompany, priority 40, привязка staff/guest); `seed_system_roles_and_policies`
→ `SeedSummary{roles_created, policies_added}` + дополнение существующих ролей;
`RoleRepository::update` (+ `role.updated`); `ManifestNavItem.entity_type`;
`ModuleManager::get_navigation` (срез {code, display_name, version, navigation[]},
фильтр по доступным командам актора); команда `module.navigation` (`module.read`);
`system.migrate_permissions` → сид ВСЕХ компаний `{companies_total, seeded, policies_added}`;
тесты `phase11_navigation` (8).
**system.bootstrap** (однократная инициализация платформы): `core-application/bootstrap.rs`
`bootstrap_platform` — компания → суперадмин Argon2id → сид → привязка роли +
primary-профиль → аудит `system.bootstrap` (деталей {company_code, admin_login,
roles_created, policies_seeded}); идемпотентность: код-коллизия → «Компания {code}
уже существует», непустая БД → «Платформа уже инициализирована»; команда
доступна только системному актору (`requires("system.bootstrap")`, `POST /debug/command`);
warn при старте если компаний нет; тесты `phase_bootstrap` (6).
Всего тестов 251.
**Не начинать Фазы 11–12, 15+** (Flutter, оффлайн, учёт, экспорт, уведомления,
криптоподпись, диагностика, тесты; SSE остаётся факультативным дополнением к 10c).
Детали фазирования — `doc/TZ_v3.1.md`, фактический порядок — `doc/technical_report.md`.

## Решения, которые не предлагать заново

- **RBAC**: deny-by-default, `PermissionManager` + pipeline встроен в каждую команду (ADR-012).
- **БД**: SurrealDB, а не MongoDB (нужны ACID-транзакции) (ADR-005).
- **Синхронизация**: строгий OCC через `version`, конфликт → ручное разрешение (ADR-009).
- **Модульность**: WASM-плагины через Extism с capability-моделью (ADR-010).
- **Криптоподпись**: cpcsp-rs, Linux-first (ADR-004).
- **UI будущего клиента**: Flutter + SDUI из метаданных (ADR-006, ADR-008).
- **JMJ-слой**: Event Store — true source, `audit_log` — операционный, с retention (ADR-011).

## Специфичные skills проекта
| Название скилла | Триггер | Путь к файлу |
|---|---|---|
| `lsp-code-generation` | Написание или исправление кода на Rust | `.opencode/skills/lsp-code-generation/SKILL.md` |

## Глоссарий домена
| Термин | Расшифровка | Английский эквивалент |
|---|---|---|
| **Труба** | Event Store (источник истины) | `EventStore` |
| **Доска** | Материализованные проекции | `Projections` |
| **Модуль** | WASM-плагин (Extism) | `Module` / `Plugin` |
