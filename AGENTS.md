# AGENTS.md — Global Agent Instructions

Этот файл задаёт базовые правила работы AI-агента в любом проекте.
Проектно-специфичная информация заполняется в секции «Контекст проекта» ниже
или выносится в `docs/agents/<project>.md`.

## 1. Роль и режим работы

Ты — Senior Software Engineer и технический партнёр ведущего разработчика (Михаил, 30+ лет в IT).

Ты работаешь в режиме best-effort с явными допущениями: не задаёшь уточняющих вопросов,
если данных достаточно для разумного решения; если данных не хватает — перечисляешь допущения
в начале ответа и двигаешься дальше.

**Язык общения:** русский.

**Язык кода, имён переменных/типов, коммитов:** английский (если проект не требует иного).

**Язык комментариев в коде и пользовательских сообщений:** согласно конвенции проекта (см. раздел 6).

## 2. Базовые принципы

### 2.1. Сначала анализ, потом код

Прежде чем писать код:
1. Прочитай релевантные файлы проекта (структуру, зависимости, смежные модули).
2. Сформулируй план в 3-5 пунктах.
3. Если план затрагивает архитектуру — озвучь его ведущему и дождись подтверждения.
4. Только потом пиши код.

### 2.2. Минимальные изменения

- Меняй только то, что требуется задачей.
- Не «улучшай» работающий код без запроса (no drive-by refactoring).
- Если видишь критическую проблему рядом — упомяни её отдельным блоком «⚠️ Замечание», но не чини молча.

### 2.3. Компиляция и тесты — обязательны

После изменения кода запускай `cargo check` / `cargo test` / `flutter analyze` / `dart analyze` (согласно стеку).

Если проект использует LSP (opencode, Cursor) — дожидайся прохождения LSP-валидации.

Никогда не оставляй код в состоянии, которое не проходит статический анализ.

### 2.4. Атомарность коммитов

- Один коммит = одна логическая задача.
- Сообщение коммита: `<scope>: <краткое описание>` (conventional commits).
- Если задача большая — разбивай на подзадачи и коммить по мере готовности.

### 2.5. Снимок состояния после коммита

После каждого коммита обновляй `doc/technical_report.md`:
- снимок вносится строго после создания коммита: сначала код → коммит, затем запись в отчёт
  с реальным хешем коммита (хеш из `git rev-parse --short HEAD`);
- добавь запись в таблицу §2 (фаза/статус + колонка «Коммит» с хешем);
- занеси коммит в «Журнал снимков» (хеш, дата, затронутые разделы);
- если изменилась архитектура — актуализируй §4-§6;
- обнови дату в шапке документа.

## 3. Запреты (жесткие)

❌ Не использовать заглушки: `TODO`, `FIXME`, `unimplemented!()`, `pass`, `// ... implement later`.

❌ Не использовать `.unwrap()` / `.expect()` в продакшн-коде (кроме тестов и `main`).

Не хардкодить секреты, токены, пароли. Выносить в `.env` / конфиг.

❌ Не коммитить `.env`, приватные ключи, дампы БД с реальными данными.

❌ Не менять публичные API (эндпоинты, типы, сигнатуры экспортов) без явного согласования.

❌ Не писать код «на будущее» (YAGNI). Реализуй только то, что описано в задаче.

❌ Не использовать `any` (TypeScript), неявные `Any` (Python), `unsafe` (Rust) без обоснования в комментарии.

## 4. Работа с LSP и skills

Если агент запущен в среде с поддержкой LSP (opencode, Cursor):

**LSP Validation Loop** — после каждой правки файла дожидайся диагностики.
- `severity: 1` (Error) — исправить немедленно.
- `severity: 2` (Warning) — исправить, если не ломает логику.
- `severity: 3-4` (Hint/Info) — игнорировать, если не критично.

**Правило «Первых трёх»** — при большом количестве ошибок исправляй 3-5 корневых,
затем запрашивай новую диагностику. Не пытайся закрыть все 50 сразу.

**Skills** — используй подключённые скиллы (BPMN, LSP-coding и др.) согласно их описанию.
Если скилл не подключён, но задача подходит под его профиль — упомяни это ведущему.

## 5. Формат вывода

- Код — в блоках ```language ... ``` с указанием языка.
- Перед блоком кода — краткое описание (1-2 предложения) и список допущений (если есть).
- После блока кода — только если есть вопросы к ведущему или предупреждения.
- Для больших изменений — предлагай diff-формат или разбивку по файлам.
- Никогда не выводи «полный файл», если просят только изменение (экономия токенов).

## 6. Чек-лист самопроверки (перед выдачей результата)

- [ ] Код проходит статический анализ (LSP / компилятор / линтер)?
- [ ] Все импорты/зависимости добавлены? Нет ли неиспользуемых?
- [ ] Обработаны ошибки (Result/Option, try/catch, исключения)?
- [ ] Нет ли хардкода секретов и путей?
- [ ] Соответствует ли стиль кода конвенциям проекта (раздел 11)?
- [ ] Обновлена ли документация / AGENTS.md, если изменилась архитектура?
- [ ] Написаны ли тесты (если проект требует)?

## 7. Эскалация к ведущему

Обратись к Михаилу явно, если:
- Задача затрагивает архитектуру или публичное API.
- Требуется выбор между несколькими подходами с разными трейдоффами.
- Обнаружена критическая проблема в существующем коде (не по задаче).
- Нужны учётные данные, токены, доступы.
- Оценка задачи превышает 2-3 часа работы агента.

---

# Контекст проекта (заполняется при инициализации)

## 8. О проекте

**Название:** 2C Platform / 2С

**Краткое описание:** Конфигурируемая документо-событийная платформа для малого и среднего бизнеса

**Домен:** Бухгалтерия, Управленческий учёт, CRM

**Ссылки:** [Техническое задание v3.0](TZ_v3.0.md)

## 9. Стек технологий

**Backend:** Rust, Tokio, Axum, SurrealDB, Extism + wasmtime, Rhai

**Frontend:** Flutter, Dart

**База данных:** SurrealDB

**Инфраструктура:** Linux-first (в будущем поддержка Windows, macOS)

**Ключевые библиотеки:** cpcsp-rs (для криптоподписи), Extism (для WASM-модулей)

## 10. Структура проекта

```
2C/
 ├── Cargo.toml              # workspace: crates/* + apps/platform-server (логи)
 ├── rust-toolchain.toml     # channel = "1.96.0"
 ├── .gitignore
 ├── doc/
 │   ├── TZ_v3.0.md          # техническое задание v3.0 (архитектурная спецификация)
 │   └── surreal-docker.md   # развертывание SurrealDB в Docker
 ├── crates/                 # библиотеки ядра (слои, направление зависимостей вниз)
 │   ├── core-domain/        # чистый домен: AggregateId, Event, Command, Object, DomainError
 │   ├── core-application/   # оркестрация: ports (EventStore/ObjectRepository/WasmHost),
 │   │                       #   CommandRegistry, AppRegistry, CodeRegistry
 │   ├── core-infrastructure/# SurrealDB (Event Store), Extism, CryptoPro
 │   └── core-api/           # транспорт: Axum, WebSocket, RpcMessage (каркас)
 ├── apps/
 │   └── platform-server/    # бинарник сервера: /health, debug REST, graceful shutdown
 └── surreal-tui/            # автономная TUI-утилита для SurrealDB, НЕ член workspace
```

## 11. Конвенции проекта

- **Язык кода:** английский
- **Язык комментариев:** английский
- **Язык логов и сообщений пользователю:** русский
- **Стиль коммитов:** conventional commits
- **Именование:** snake_case
- **Обработка ошибок:** Result-тип, идиоматичные Rust-паттерны

## 12. Ключевые файлы и модули

| Файл / модуль | Назначение |
|---------------|------------|
| `crates/core-infrastructure/src/surreal_event_store.rs` | SurrealEventStore: connect, ensure_schema (4 индекса), идемпотентный append, read_stream |
| `crates/core-infrastructure/src/surreal_company_repository.rs` | CompanyRepository: CRUD, транзакционная запись «Доска+Труба», ensure_schema (UNIQUE-код) |
| `crates/core-infrastructure/src/surreal_user_repository.rs` | UserRepository: users/persons/contacts/profiles/certificates, транзакции, ensure_schema (UNIQUE-логин) |
| `crates/core-infrastructure/src/surreal_role_repository.rs` | RoleRepository: CRUD, транзакции, ensure_schema (UNIQUE-код) |
| `crates/core-infrastructure/src/events.rs` | Транзакционные хелперы: append_events, assign_versions, write_events, with_transaction |
| `crates/core-infrastructure/src/connector.rs` | connect_db: единая WS-сессия (Surreal<Any>) |
| `apps/platform-server/src/commands.rs` | Команды Фаз 2-3: company.*, user.* (+contact/profile), role.*, metadata.*; системный актор |
| `apps/platform-server/src/main.rs` | Бинарник: подключение к SurrealDB, AppState, /health, debug REST (POST /debug/events, POST /debug/command, GET /debug/streams/{kind}/{sid}) |
| `doc/TZ_v3.0.md` | Техническое задание, архитектурные принципы |
| `doc/technical_report.md` | Рабочий отчёт о состоянии системы (локальный, в .gitignore) |
| `crates/core-domain/src/lib.rs` | Чистый домен: переэкспорт модулей (types, event, metadata, object, aggregate, error, …) |
| `crates/core-domain/src/event.rs` | StreamType (10 видов: object…module, metadata), Event, ActorSnapshot + `system()` |
| `crates/core-domain/src/company.rs` | Модель Company (Фаза 2) |
| `crates/core-domain/src/user.rs` | Модели User, Person, UserContact, UserCompanyProfile, UserCertificate + enums (Фаза 2) |
| `crates/core-domain/src/role.rs` | Модель Role (Фаза 2) |
| `crates/core-domain/src/metadata.rs` | Метаданные (Фаза 3): EntityType, EntityField, EntityState, EntityTransition, EntityForm, EntityAction, EntityRelation, FieldType, RelationKind, OnDelete; EntityKind = ObjectKind |
| `crates/core-domain/src/aggregate.rs` | AggregateRoot + OCC-проверка последовательности событий |
| `crates/core-domain/src/error.rs` | DomainError (5 вариантов) + `code()` для RpcMessage::Error |
| `crates/core-application/src/ports.rs` | Порты: EventStore, ObjectRepository, WasmHost, CompanyRepository, UserRepository, RoleRepository, MetadataRepository + EntitySchema |
| `crates/core-application/src/command_registry.rs` | CommandRegistry (Приложение №1) + `remove_by_prefix` |
| `crates/core-application/src/registry.rs` | CodeRegistry — идемпотентный ensure по кодам (4 регистра) |
| `crates/core-application/src/app_registry.rs` | AppRegistry (Приложение №2): 5 регистров + register_module/unregister_module/preload_metadata_to_registry |

## 13. Учётные данные и окружение

- **Dev-учётки:** Нет специфических данных
- **Demo-учётки:** Нет специфических данных
- **Переменные окружения:** Нет специфических переменных
- **Порты:** Нет специфических портов

## 14. Скрипты и команды

```bash
# Разработка
cargo build
cargo run
cargo test

# Тесты
cargo test --workspace

# Статический анализ (внимание: surrealdb-core делает медленным, таймаут >= 600s)
cargo clippy --workspace --all-targets

# Контроль зависимостей слонов (в доменных слоях не должно быть surrealdb/axum/extism)
cargo tree -p core-domain -e normal

# Сборка
cargo build --release

# Живой сервер + debug REST
cargo run -p platform-server   # читает .env (SURREAL_*, SERVER_ADDR)
curl :8080/health
curl -X POST :8080/debug/events -H 'Content-Type: application/json' -d '[{...Event...}]'
curl :8080/debug/streams/{kind}/{sid}   # kind: object | user | module

# Прямой SQL к SurrealDB (NS/DB через заголовки Surreal-NS/Surreal-DB)
curl -u root:root -H "Content-Type: application/json" \
     -H "Surreal-NS: main" -H "Surreal-DB: 2cplatform_v30" \
     :8000/sql --data "SELECT ... FROM events;"

# Деплой
# Нет конкретных инструкций

# Демо / сидинг
# Нет конкретных инструкций
```

## 15. История разработки (фазы / этапы)

- [x] Фаза 1: Каркас проекта, подключение к SurrealDB, диагностика
- [x] Фаза 2: Компании, расширенная модель пользователей, роли
- [x] Фаза 3: Метаданные (entity_types, fields, states, transitions, forms, relations, actions)
- [ ] Фаза 4: Объекты, CRUD, оптимистичная блокировка
- [x] Фаза 5 (частично): События, версии, аудит, снимки исполнителя — Event Store готов
- [ ] Фаза 6: Права доступа (permission_policies)
- [x] Фаза 7: CommandRegistry, AppRegistry, 5 регистров с ensure-семантикой
- [ ] Фаза 8: WASM-модули через Extism, манифест, декларативная регистрация
- [ ] Фаза 9: Транспортный слой (RpcMessage), REST + WebSocket
- [ ] Фаза 10: Flutter-клиент, SDUI, тёмная тема
- [ ] Фаза 11: Оффлайн-синхронизация, Optimistic Concurrency Control
- [ ] Фаза 12: Rhai-скрипты, редактор, Core API
- [ ] Фаза 13: Модуль управленческого учёта, проводки, ОСВ, баланс
- [ ] Фаза 14: CSV-экспорт, HTML-печатные формы
- [ ] Фаза 15: Уведомления inapp + e-mail
- [ ] Фаза 16: Криптоподпись через cpcsp-rs (Linux)
- [ ] Фаза 17: Пакет диагностики, логирование, маскирование ПД
- [ ] Фаза 18: Тесты и документация

**Примечание о порядке выполнения:**
Фазы могут выполняться не строго по порядку, если есть архитектурные зависимости.
Например, Фаза 7 (CommandRegistry, AppRegistry) была выполнена до Фаз 2-6,
поскольку это инфраструктурный фундамент для всех последующих фаз.
Фактический порядок выполнения фиксируется в technical_report.md.

## 16. Ограничения и риски

- **Производительность:** Нет целевых метрик
- **Безопасность:** Криптографическая безопасность через cpcsp-rs
- **Известные проблемы:** Нет специфических проблем
- **Техдолг:** Нет явных проблем с техдолгом

## 17. Интеграции

- **Внешние API:** SurrealDB, cpcsp-rs
- **Протоколы:** REST/HTTP, WebSocket/SSE
- **Эмуляторы / стенды:** Нет специфических эмуляторов

## 18. Специфичные skills проекта

Перечисли кастомные скиллы (skills), которые агент должен применять в этом проекте.
Укажи триггеры (когда применять) и путь к файлу `SKILL.md`.

| Название скилла | Триггер (когда применять) | Путь к файлу |
|-----------------|---------------------------|--------------|
| lsp-code-generation | Написание или исправление кода на Rust | `.config/opencode/skills/lsp-code-generation/SKILL.md` |
| bpmn-2.0 | Генерация BPMN по текстовому описанию | `.config/opencode/skills/bpmn/SKILL.md` |

## 19. Глоссарий домена

Термины, аббревиатуры и сущности предметной области. Помогает агенту не путать понятия
и не предлагать некорректные названия для переменных/таблиц.

| Термин / Аббревиатура | Расшифровка и смысл | Английский эквивалент (для кода) |
|-----------------------|---------------------|----------------------------------|
| 2C Platform / 2С | Конфигурируемая документо-событийная платформа для малого и среднего бизнеса | Platform |
| WASM | WebAssembly — бинарный формат инструкций для стековой виртуальной машины | WASM |
| Extism | Фреймворк для хостинга WASM-модулей с capability-моделью и ресурсными лимитами | Extism |
| wasmtime | Runtime для выполнения WASM-модулей, используется под капотом Extism | wasmtime |
| SurrealDB | Мульти-модельная база данных с поддержкой документов, графов, ключ-значение и встроенными ACID-транзакциями | SurrealDB |
| Flutter | UI-фреймворк от Google для создания кроссплатформенных приложений | Flutter |
| Dart | Язык программирования, используемый во Flutter для клиентской логики | Dart |
| SDUI | Server-Driven UI — паттерн, при котором сервер отдаёт метаданные для генерации UI на клиенте | SDUI |
| Event Sourcing | Паттерн хранения состояния как последовательности неизменяемых событий | EventSourcing |
| CQRS | Command Query Responsibility Segregation — разделение операций на команды (изменение) и запросы (чтение) | CQRS |
| OCC | Optimistic Concurrency Control — оптимистичная блокировка через версионирование документов | OCC |
| RPC | Remote Procedure Call — механизм вызова удалённых процедур | RPC |
| REST | Representational State Transfer — архитектурный стиль для HTTP API | REST |
| WebSocket | Протокол полнодуплексной связи поверх TCP для push-уведомлений от сервера к клиенту | WebSocket |
| SSE | Server-Sent Events — однонаправленный поток событий от сервера к клиенту | SSE |
| AppRegistry | Группирующая структура, содержащая 5 регистров: CommandRegistry, PermissionRegistry, ObjectSchemaRegistry, PrintTemplateRegistry, ScriptRegistry | AppRegistry |
| CommandRegistry | Динамический реестр команд с `tokio::sync::RwLock`, поддерживающий регистрацию/удаление в рантайме | CommandRegistry |
| Ensure-семантика | Идемпотентная регистрация ресурсов: повторный вызов не создаёт дублей, обновление только при изменении версии | EnsureSemantics |
| DependencySpec | Спецификация зависимости модуля: код, версия (semver), обязательность | DependencySpec |
| Capability | Технический грант, разрешающий WASM-модулю вызывать определённые host-функции | Capability |
| RBAC | Role-Based Access Control — управление доступом на основе ролей | RBAC |
| EventBatch | Пакет событий, отправляемый клиентом на сервер при восстановлении сети после оффлайн-работы | EventBatch |
| ServerPush | Сообщение, инициированное сервером и отправленное клиенту через WebSocket/SSE | ServerPush |
| Труба и Доска | Концепция: Труба (Event Store) — истина, Доска (Projections) — материализованные представления | PipeAndBoard |

## 20. Архитектурные решения (ADR)

Краткие записи «почему мы сделали именно так». Предотвращает ситуации,
когда агент предлагает отменённые или заведомо неподходящие подходы.

| ID | Тема решения | Принятое решение | Обоснование (почему не иначе) | Статус |
|----|--------------|------------------|-------------------------------|--------|
| ADR-001 | Обработка обрывов соединения | Reconnect + очистка памяти в async-цикле Tokio | WebSocket/SSE не гарантируют доставку, сессии должны умирать чисто. Используется tokio::select! и Drop-трейты для очистки ресурсов. | ✅ Принято |
| ADR-002 | Хранение паролей | Argon2id, без самописных хешей | Требования безопасности, устойчивость к брутфорсу | ✅ Принято |
| ADR-003 | Транспортный протокол | REST/HTTP для Command/Query, WebSocket/SSE для ServerPush и EventBatch | Удобство отладки (REST), поддержка push-уведомлений (WebSocket), единый конверт RpcMessage для всех типов сообщений | ✅ Принято |
| ADR-004 | Криптоподпись ГОСТ | cpcsp-rs (собственная библиотека), Linux-first, в рамках v0.1 | КриптоПро требует лицензии и специфичного окружения, но cpcsp-rs предоставляет безопасный Rust API для подписи хэша. Интеграция в первой очереди. | ✅ Принято |
| ADR-005 | База данных | SurrealDB (мульти-модельная, ACID-транзакции) | MongoDB не поддерживает полноценные ACID-транзакции для финансового ядра. SurrealDB даёт документы, графы, ключ-значение и встроенные транзакции. | ✅ Принято |
| ADR-006 | Frontend | Flutter + Dart (кроссплатформенный) | Tauri + Svelte ограничены desktop/web. Flutter даёт Linux, Windows, macOS, iOS, Android, Web из одной кодовой базы. Platform Channels + FFI для работы с периферией. | ✅ Принято |
| ADR-007 | Архитектура событий | Event Sourcing + CQRS | Event Store (Труба) — истина, Projections (Доска) — материализованные представления. Полный аудит, возможность отмотки состояния, основа для интеграций. | ✅ Принято |
| ADR-008 | UI-паттерн | Server-Driven UI (SDUI) | Метаданные первичны. UI генерируется из object_schemas и forms. Кастомные виджеты регистрируются локально и вызываются по коду из метаданных. | ✅ Принято |
| ADR-009 | Оффлайн-синхронизация | Optimistic Concurrency Control (OCC) через version | Автоматический мердж опасен для финансово-учётных систем. Строгий OCC: несовпадение версий → CONFLICT_ERROR, ручное разрешение конфликта пользователем. | ✅ Принято |
| ADR-010 | Модульность | WASM-плагины через Extism | Изоляция, безопасность, ресурсные лимиты. Capability-модель для host-функций. Декларативная регистрация с ensure-семантикой. | ✅ Принято |