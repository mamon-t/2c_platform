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
Сейчас всё выполняется от системного исполнителя `ActorSnapshot::system()` — аутентификации ещё нет.

## Workspace и слои

Workspace: `crates/core-domain` → `crates/core-application` → `crates/core-infrastructure`
+ `crates/core-api` (транспорт, пока каркас — 1 строка в `lib.rs`) + бинарник `apps/platform-server`.
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
| `crates/core-domain/src/{event,company,user,role,permission,audit,metadata,object,wasm_manifest,aggregate,error}.rs` | Модели и типы домена, 10 `StreamType`'ов, `DomainError` |
| `crates/core-application/src/ports.rs` | Порты: EventStore, все `*Repository`, WasmHost, EntitySchema |
| `crates/core-application/src/command_registry.rs` | CommandRegistry (префиксные команды) + `CommandExecutionPipeline` (аудит + RBAC перед каждой командой) |
| `crates/core-application/src/permission_manager.rs`, `seed.rs`, `registry.rs`, `app_registry.rs` | Deny-by-default RBAC, сид системных ролей/политик, ensure-регистры |
| `crates/core-infrastructure/src/connector.rs`, `events.rs` | `connect_db` (единая WS-сессия), транзакционные хелперы append/assign_versions/with_transaction |
| `crates/core-infrastructure/src/surreal_{event_store,company,user,role,permission_policy,object,audit,metadata}_repository.rs` | SQL-доступ по коллекциям; у каждого `ensure_schema()` с UNIQUE-индексами; схема создаётся при старте, а не SQL-миграциями |
| `crates/core-infrastructure/src/extism_wasm_host.rs`, `module_kv.rs` | WASM-хост (Extism 1.30), host-функции, KV-хранилище модулей |
| `apps/platform-server/src/main.rs` | Старт: ensure_schema всех репо, регистрация команд по фазам, attach RBAC-pipeline, /health + debug REST |
| `apps/platform-server/src/commands.rs` | Все команды: `company.*`, `user.*` (+contact/profile), `role.*` (+`role.seed`), `metadata.*`, `object.*` (+snapshot), `document.number.*`, `audit.*`, `system.migrate_permissions` |

## Команды

```bash
cargo build
cargo test                 # ср. ниже: интеграционные тесты НЕ требуют живого SurrealDB
cargo test -p core-infrastructure   # только интеграционные приёмочные (mem://)
cargo clippy --workspace --all-targets   # ⚠️ surrealdb-core делает медленным — ставь таймаут >= 600s
cargo +1.96.0 fmt --all    # toolchain уже закреплён, + не нужен

# Сервер (читает .env через dotenvy)
cargo run -p platform-server
curl :8080/health
curl -X POST :8080/debug/command -H 'Content-Type: application/json' \
     -d '{"name":"company.create","params":{"code":"x","name":"X"}}'
curl -X POST :8080/debug/events -H 'Content-Type: application/json' -d '[{...Event...}]'
curl :8080/debug/streams/object/{sid}

# Прямой SQL к SurrealDB (NS/DB — заголовки)
curl -u root:root -H "Surreal-NS: main" -H "Surreal-DB: 2cplatform_v30" :8000/sql \
     --data "SELECT * FROM events LIMIT 5;"
```

## Тесты — как это реально работает

- Интеграционные тесты `core-infrastructure` (`tests/phase5_rbac.rs`, `tests/hello_wasm.rs`)
  гоняются на `mem://`-базе: репозитории подключаются к SurrealDB через feature `kv-mem`
  (активируется только для тестов в `[dev-dependencies]`). Живой сервер не нужен.
- `hello_wasm.rs` требует фикстуру `crates/core-infrastructure/tests/fixtures/hello.wasm`.
  Она компилируется из `examples/hello_plugin` и закоммичена; после изменения модуля —
  пересобрать и заменить (см. ниже).
- Модульные тесты внутри репозиториев (напр., `ObjectRepository`) — тоже `mem://`, без внешних сервисов.

## Сборка WASM-фикстуры (вне workspace)

```bash
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown -p hello_plugin \
    --manifest-path examples/hello_plugin/Cargo.toml
cp examples/hello_plugin/target/wasm32-unknown-unknown/release/hello_plugin.wasm \
   crates/core-infrastructure/tests/fixtures/hello.wasm
```

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

## Статус фаз (что уже работает)

Реализовано: Фазы 1–8 (каркас, компании/пользователи/роли, метаданные, аудит, RBAC,
объекты с OCC, Event Store, CommandRegistry/AppRegistry с ensure-семантикой). Фаза 9 (WASM/Extism):
готова подфаза 8b (манифест v2, ExtismWasmHost, ModuleKv, host-fn 8a–8b
объекты и метаданные, hello_plugin с objects_probe) — коммит `3d9e614`.
Не начинать Фазы 10+ (транспорт, Flutter, оффлайн, Rhai, учёт, экспорт, уведомления, криптоподпись, диагностика, тесты).
Детали фазирования и приёмки — `doc/TZ_v3.1.md`, фактический порядок — `doc/technical_report.md`.

## Решения, которые не предлагать заново

- **RBAC**: deny-by-default, `PermissionManager` + pipeline встроен в каждую команду (ADR-012).
- **БД**: SurrealDB, а не MongoDB (нужны ACID-транзакции) (ADR-005).
- **Синхронизация**: строгий OCC через `version`, конфликт → ручное разрешение (ADR-009).
- **Модульность**: WASM-плагины через Extism с capability-моделью (ADR-010).
- **Криптоподпись**: cpcsp-rs, Linux-first (ADR-004).
- **UI будущего клиента**: Flutter + SDUI из метаданных (ADR-006, ADR-008).
- **JMJ-слой**: Event Store — true source, `audit_log` — операционный, с retention (ADR-011).