# 2C Platform — Журнал прогресса

## Этап 1: Каркас проекта ✅
**Дата:** 19.08.2026
**Коммит:** 6952d27

### Что сделано
- Tauri v2 + Svelte 5 + Vite + TypeScript
- Tailwind CSS v4 + Skeleton UI
- Rust ядро: 11 модулей (core, db, meta, events, auth, audit, ledger, rhai, crypto, notify, commands)
- Transport-адаптеры: Tauri IPC, HTTP, Mock
- Frontend: sidebar layout, темы (light/dark/system), пакет диагностики
- Типы данных по ТЗ: CompanyId, UserId, EntityKind, FieldKind, ObjectState
- Авторизация: JWT + Argon2
- MongoDB: подключение по URI, diagnostics (version, replica set)
- Rhai: sandbox execute/validate
- Проверки: cargo check ✅, svelte-check 0 errors ✅, vite build ✅

## Этап 2: MongoDB + Компании, Пользователи, Роли ✅
**Дата:** 19.08.2026
**Коммит:** 81c250e, 8bb8ed4

### Что сделано
- MongoDB Replica Set rs0 на Docker (192.168.31.31:27017)
- База `2c_platform` с коллекциями и индексами
- Rust модули CRUD: CompanyService, UserService, RoleService
- Tauri IPC команды: list/get/create/update/delete для компаний, пользователей, ролей
- Авторизация: authenticate + JWT token + first-boot auto-creation (admin/admin)
- Password hashing: Argon2id через Rust argon2 crate
- Frontend:
  - Страница подключения к MongoDB (URI + database name)
  - Страница логина
  - Страница компаний (таблица + create/edit/delete)
  - Страница пользователей (таблица + create/delete)
  - Страница ролей (таблица + create/delete)
  - Навигация: компании, пользователи, роли в sidebar
- Проверки: cargo check ✅, svelte-check 0 errors ✅, vite build ✅

### Технические детали
- `tokio::sync::Mutex` для AppState (async Tauri commands)
- `futures::StreamExt` для Cursor итерации (mongodb 3.x)
- UUID хранятся как строки в MongoDB (Bson совместимость)
- First-boot: при первом `authenticate("admin", "admin")` создаётся:
  - Компания "Основная компания" (код MAIN)
  - Роль "Суперадминистратор" (код SUPERADMIN)
  - Пользователь admin/admin

## Этап 3: Расширение модели пользователя + Аудит + RBAC ✅
**Дата:** 20.08.2026

### Что сделано

#### 3.1 Модель пользователя по add1
- 5 коллекций: companies, users, roles, persons, user_contacts, user_profiles, user_certificates
- PersonService — ФИО, отображаемое имя, аватар, birthdate
- UserContactService — каналы связи (email/phone/messenger), валидация, нормализация телефонов (+7), привязка к назначению
- UserProfileService — рабочие профили (компания+роль+должность+отдел)
- UserCertificateService — сертификаты ЭЦП
- SettingsService — контактные типы (backoffice.app_settings)

#### 3.2 Аудит (TZ-compliant)
- **Структура AuditEntry** (14 полей по TZ): timestamp, user_id, user_login, company_id, action, target_type, target_id, target_code, target_name, changes, ip, source, session_id, metadata
- **AuditableAction enum** — 35 вариантов: авторизация, компании, пользователи, роли, контакты, профили, сертификаты, настройки, + будущие (документы, каталоги, события)
- **AuditChanges** — нормализованный формат old→new с FieldChange (field, old, new)
- **AuditService trait** + MongoAuditService — mockable, cursor-based pagination (AuditFilters)
- **audit_log! macro** — компактный вызов с company_id из контекста
- **fire_audit() async helper** — для runtime-computed changes
- **MongoDB индексы** — 4 составных индекса
- **Frontend** — AuditPage с registry-based UI (FA icons + русские названия), пагинация
- **Исправления аудита changes**: все 9 update/create/delete команд теперь логируют diff old→new

#### 3.3 RBAC
- PermissionPolicy модель + PermissionPolicyService CRUD
- RoleService расширен: permission_policy_ids в role
- 37 политик по умолчанию (подсистемы: companies, users, roles, contacts, profiles, certificates, audit, settings)
- Сидинг политик при first-boot
- Frontend: auth.ts (permissions, hasPermission), navigation.ts (requiredPermission), страница "Мой доступ"
- Навигация и CRUD команды фильтруются по правам

### Изменения файлов (основные)
- `src-tauri/src/audit/` — 7 новых файлов (mod, actions, changes, filters, service, macros, indexes)
- `src-tauri/src/permission_policy/` — mod.rs
- `src-tauri/src/person/` — mod.rs
- `src-tauri/src/user_contact/` — mod.rs
- `src-tauri/src/user_profile/` — mod.rs
- `src-tauri/src/user_certificate/` — mod.rs
- `src-tauri/src/settings/` — mod.rs
- `src-tauri/src/commands/mod.rs` — все CRUD + audit + RBAC команды
- `src/lib/services/api.ts` — все типы + AUDIT_ACTION_META + API методы
- `src/lib/components/AuditPage.svelte` — аудит с пагинацией
- `src/lib/components/SettingsPage.svelte` — контактные типы
- `src/lib/stores/auth.ts` — permissions, hasPermission
- `src/lib/stores/navigation.ts` — requiredPermission, filtered nav
- `src/App.svelte` — 4 вкладки (пользователи, роли, аудит, настройки)

### Проверки
- cargo check: 0 ошибок ✅
- svelte-check: 0 ошибок ✅
- Async-trait добавлен в Cargo.toml для AuditService trait

### Требуется
- ~~Сброс БД для RBAC~~ ✅ Сброшена 20.08.2026

## Этап 4: Event Store (ядро «Труба и Доски») ✅
**Дата:** 20.08.2026
**Коммит:** 6aa0818

### Что сделано
- **Event модель** — полная структура Event (append-only):
  - stream_type (Object/User/Module), stream_id, event_type, version (auto-increment в потоке)
  - payload (JSON), metadata (ActorSnapshot: user_id, login, full_name, position, company_id)
  - company_id, correlation_id, causation_id, signature_ref, occurred_at
- **ActorSnapshot** — снимок исполнителя для читаемой истории
- **EventService** —append-only запись с атомарной инкрементацией version:
  - `append()` — запись события с version++ (MAX(stream)+1)
  - `list_stream()` — чтение всего потока (version history объекта)
  - `list()` — список с фильтрами (stream_type, event_type, date_from/to, correlation_id) + пагинация
  - `get()` — чтение по ID
  - `last_version()` — последний version в потоке (для оптимистичной блокировки)
- **MongoDB индексы** — 4 составных: stream, event_type+time, company+time, correlation_id
- **IPC команды** — list_events, get_event, list_stream_events
- **Index creation** — индексы audit_log + events создаются при connect_db
- **Frontend** — EventsPage.svelte (таблица с фильтрами, раскрытие payload, пагинация)
- **Navigation** — вкладка «События» в sidebar

### Проверки
- cargo check: 0 ошибок ✅
- svelte-check: 0 ошибок ✅

## Этап 5: Метаданные (описание сущностей) ✅
**Дата:** 20.08.2026
**Коммит:** 2c88c4b

### Что сделано
- **6 коллекций метаданных**:
  - `entity_types` — типы сущностей (Document/Catalog/Register/Task/Contract/Project/Setting/Custom)
  - `entity_fields` — поля описания (17 типов: string, text, integer, money, date, enum, reference, table, formula...)
  - `entity_states` — состояния (Draft, Active, Posted, Cancelled, Archived, Deleted...)
  - `entity_transitions` — переходы между состояниями (с привязкой к политике прав)
  - `entity_forms` — макеты форм (JSON layout)
  - `entity_actions` — действия над сущностями (с флагом is_dangerous)
- **6 CRUD сервисов** с deserialize helpers для MongoDB Documents:
  - EntityTypeService: list, get, create, update, delete (каскадное удаление вложенных)
  - EntityFieldService: list_by_type, get, create, update, delete, reorder (auto-order)
  - EntityStateService: list_by_type, create, update, delete
  - EntityTransitionService: list_by_type, create, update, delete
  - EntityFormService: list_by_type, create, update, delete
  - EntityActionService: list_by_type, create, update, delete
- **24 IPC команды** (CRUD для каждого типа)
- **6 MongoDB индексов** (unique code per company, ordered lists)
- **Frontend** — MetadataPage.svelte:
  - Боковая панель с деревом типов сущностей
  - Создание/удаление типов с выбором kind
  - Вкладки: Поля / Состояния / Переходы
  - Создание полей с выбором FieldKind
  - Создание состояний с начальным/конечным флагами
  - Визуальный переход (from → to)
- **Navigation** — вкладка «Метаданные» (settings/manage)

### Проверки
- cargo check: 0 ошибок ✅
- svelte-check: 0 ошибок ✅

## Этап 6: Objects («Доска» — универсальная коллекция) ✅
**Дата:** 20.08.2026
**Коммит:** f33f152

### Что сделано
- **Object модель** — универсальная коллекция для всех сущностей:
  - entity_type_id, kind, company_id, state, data (JSON), number, version
- **ObjectSnapshot** — снимки версий с причиной изменения
- **ObjectService** — CRUD + бизнес-операции:
  - create (version=1, Draft, snapshot), get, list (фильтры, пагинация)
  - update (оптимистичная блокировка version), post (Draft→Posted, номер 000001)
  - cancel (Posted→Cancelled), restore_version (из snapshot)
- **Интеграция с EventStore** — каждая мутация → событие + ActorSnapshot
- **MongoDB индексы** — 6 индексов
- **IPC команды** — 8 команд
- **Frontend** — ObjectsPage.svelte: дерево типов, таблица, JSON-данные, действия, история версий

### Проверки
- cargo check: 0 ошибок ✅
- svelte-check: 0 ошибок ✅

## Этап 7: Динамический UI (генерация форм из метаданных) ✅
**Дата:** 20.08.2026
**Коммит:** 8261b20

### Что сделано
- **ObjectEditor.svelte** — динамическая форма из entity_fields:
  - Виджеты по FieldKind: string, text, integer, money, date, datetime, boolean, enum, reference
  - Группировка полей по group_name
  - Блокировка полей (is_readonly) и формы (state != draft)
  - JSON-редактор для продвинутых пользователей
  - Кнопки переходов из entity_transitions (текущее состояние → доступные)
  - История версий с восстановлением
- **ObjectsPage** интегрирован с ObjectEditor:
  - Клик по объекту → динамическая форма
  - «Создать» → выбор типа → создание → открытие формы
  - Автообновление таблицы после сохранения

### Проверки
- cargo check: 0 ошибок ✅
- svelte-check: 0 ошибок ✅

## Этап 8: Конвертация данных (WASM-модуль на Extism) ✅
**Дата:** 20.08.2026
**Коммит:** c1f3d73 + 56d8c31 (refactor)

### Что сделано
- **WASM Plugin** (wasm-modules/convert/) — Extism PDK на Rust:
  - CSV парсинг (header → field codes, маппинг колонок)
  - JSON парсинг (массив объектов или single object)
  - YAML парсинг (sequence/mapping → JSON)
  - XML парсинг (quick-xml: `<object><field>...`)
  - Обратный экспорт во все 4 формата
  - Host function: `create_object` для создания объектов в ядре
  - Компиляция: `cargo build --target wasm32-unknown-unknown --release` (620KB)
- **Host** (src-tauri/src/convert/):
  - `mod.rs` — типы (ImportRequest, ImportResult, ExportRequest, ExportResult, ModuleInfo)
  - `plugin.rs` — ConvertPlugin: загрузка .wasm, host functions через extism::host_fn!, import/export через Extism API
  - `commands.rs` — IPC: load_wasm_module, unload_wasm_module, list_wasm_modules, import_objects_via_wasm, export_objects_via_wasm
- **AppState** расширен полем `wasm_modules: Option<HashMap<String, ConvertPlugin>>`
- **Frontend** — ConvertPage.svelte:
  - Левая панель: список WASM-модулей, загрузка .wasm файла
  - Вкладки Импорт/Экспорт: выбор модуля, формата, типа объекта
  - Drag-and-drop для файлов импорта
  - Автоматическое скачивание при экспорте
  - Журнал операций
- **Навигация**: «Конвертация» (settings/manage permission)

### Зависимости
- Host: `extism = "1.4"` (wasmtime под капотом)
- Plugin: `extism-pdk = "1"`, `csv = "1"`, `serde_yaml = "0.9"`, `quick-xml = "0.36"`

### Проверки
- cargo check: 0 ошибок ✅
- svelte-check: 0 ошибок ✅
- WASM plugin: компилируется (620KB) ✅

## Этап 9: Печать + Рефакторинг плагинов + Нумерация ✅
**Дата:** 20.08.2026
**Коммиты:** b7e60cd (F6), ba15ae7 (F7), ca4c15d (F8)

### F6: Print Forms Engine v0.1
- **PrintService** — шаблонизатор для печатных форм (Handlebars-подобный синтаксис)
- Подстановка полей объекта: `{{data.field_name}}`, `{{number}}`, `{{date}}`
- Подстановка метаданных: `{{entity_type.code}}`, `{{state}}`
- Табличные данные: `{{#each rows}}...{{/each}}`
- Условия: `{{#if data.sum}}...{{/if}}`
- IPC команды: render_print_form, list_print_forms
- Frontend: PrintPage.svelte — форма редактирования шаблона + превью рендера в реальном времени

### F7: Архитектура плагинов (рефакторинг)
- **PluginManager** выделен из convert в отдельный модуль (`src-tauri/src/plugin_manager/`)
- WASM runtime (Extism) обобщён для любых плагинов, не только конвертации
- Host function `plugin_call` — плагины могут вызывать функции ядра (create_object, get_object, list_objects)
- Плагины самодескрибирующиеся: JSON-описание `plugin_info()` при загрузке
- Старые команды convert переименованы в `wasm_load`, `wasm_unload`, `wasm_list`, `plugin_call`

### F8: Нумерация документов
- **NumberingService** — атомарный генератор номеров через MongoDB `findOneAndUpdate` с `upsert`
- Формат номера: `{company_prefix}-{entity_code}-{seq:6}` (например `MAIN-DOCA-000001`)
- Формат настраивается per entity_type per company
- Счётчик хранится в коллекции `number_sequences`
- Транзакционная версия: `next_number_with_session()` для использования внутри ObjectService
- IPC команды: get_numbering_rules, update_numbering_rule, reset_numbering_counter, preview_next_number
- Frontend: NumberingPage.svelte — таблица правил нумерации, сброс счётчика, превью следующего номера

## Этап 10: MongoDB Transactions + Валидация данных ✅
**Дата:** 20.08.2026
**Коммит:** 2526322

### Что сделано
- **Транзакции MongoDB** — все 5 мутаций объектов (create, update, post, cancel, restore_version) обёрнуты в `session.start_transaction()` → операции → `session.commit_transaction()`:
  - Запись события (EventStore), создание/обновление снапшота, инкремент номера — атомарно
  - При ошибке — `session.abort_transaction()` (rollback)
  - Приватные `*_inner()` методы для каждого типа мутации (избегание async closure проблем)
- **Валидация данных** (`objects/validation.rs`):
  - Проверка `is_required` — обязательные поля
  - Проверка `is_readonly` — запрет изменения защищённых полей
  - Проверка типов: 18 типов полей (string, text, integer, money, float, percent, date, datetime, boolean, email, phone, url, reference, enum, table, json, file, formula)
  - Валидация `enum_values` — допустимые значения
  - `validate_field_value()` — валидация одного поля по EntityField
  - `validate_data()` — валидация всех полей объекта
- **Транзакционная нумерация** — `next_number_with_session()` использует сессию для атомарного инкремента внутри транзакции
- **Публичный `MongoClient::client()`** — прокси для `start_session()` из модуля db
- **Составные индексы** — добавлен индекс `entity_type_id + company_id + updated_at` для高效的 list_by_type+company

### Проверки
- cargo check: 0 ошибок ✅
- svelte-check: 0 ошибок ✅

## Этап 11: Полный RBAC для Object Operations ✅
**Дата:** 20.08.2026
**Коммит:** 791894c

### Что сделано
- **PermissionPolicyService::check_access()** — deny-by-default проверка прав:
  - Приоритет: deny-политики имеют приоритет над allow
  - Wildcard entity_type: политика с `entity_type: None` применяется ко всем типам
  - Tracing логирование каждого решения (allow/deny + причина)
- **AppState.current_policies** — кэш политик, загружается при `authenticate` и `switch_company`
  - `RoleService::get_policies(db, &role)` разрешает role.permission_policy_ids → Vec<PermissionPolicy>
- **RBAC в 8 object commands:**
  - `list_objects` / `get_object` → `documents.read`
  - `create_object` → `documents.create`
  - `update_object` → `documents.update`
  - `post_object` → `documents.approve`
  - `cancel_object` → `documents.cancel`
  - `restore_object_version` → `documents.update`
  - `list_object_versions` → `documents.read`
- **Владение компанией** — `get_object` и `list_object_versions` проверяют `company_id` объекта (нельзя читать объекты другой компании)
- **Frontend RBAC:**
  - Кнопка «Создать» скрыта без `documents.create`
  - Кнопка «Сохранить» скрыта без `documents.update`
  - Кнопка «Восстановить» скрыта без `documents.update`
  - Кнопки переходов фильтруются по `documents.approve` / `documents.cancel`

### Проверки
- cargo check: 0 ошибок ✅
- svelte-check: 0 ошибок ✅

## Этап 12: Аудит объектов + Логирование индексов + Events tracing ✅
**Дата:** 21.08.2026
**Коммиты:** b0fb1b3, f6f589b

### Что сделано

#### Аудит Object Operations
- **RestoreDocument** — новый AuditableAction (label: «Восстановление документа», icon: `fa-clock-rotate-left`)
- `audit_log!` добавлен во все 5 mutable object commands:
  - `create_object` → `CreateDocument`
  - `update_object` → `UpdateDocument`
  - `post_object` → `PostDocument`
  - `cancel_object` → `CancelDocument`
  - `restore_version` → `RestoreDocument`

#### Index Error Handling
- Все `let _ = collection.create_index(...)` заменены на `if let Err(e) = ... { warn!(...) }`
- 4 файла indexes (objects, events, audit, meta) — теперь каждая ошибка создания индекса логируется с описанием коллекции и имени индекса

#### Events Debug Tracing
- `EventService::append` — добавлен `debug!` с event_id, actor_login, payload_keys
- `EventService::append_with_session` — добавлен `info!` + `debug!` (раньше не логировался вообще)

### Проверки
- cargo check: 0 ошибок ✅
- svelte-check: 0 ошибок ✅

## Этап 13: RBAC Print/Scripts/Plugins/Numbering + Security Fixes ✅
**Дата:** 21.08.2026
**Коммит:** 09d3544

### RBAC: 16 новых команд с проверкой доступа

#### Новые seed-политики (12 штук)
- `print.read`, `print.create`, `print.update`, `print.delete` — шаблоны печати
- `plugins.read`, `plugins.manage`, `plugins.execute` — WASM-плагины
- `numbering.read`, `numbering.manage` — нумерация документов
- (`scripts.read/create/execute` уже существовали)

#### Защищённые команды (16 новых)
- **Print** (6): `print_list_templates` → `print.read`, `print_get_template` → `print.read`, `print_render` → `print.read`, `print_create_template` → `print.create`, `print_update_template` → `print.update`, `print_delete_template` → `print.delete`
- **Plugins** (4): `wasm_list` → `plugins.read`, `wasm_load` → `plugins.manage`, `wasm_unload` → `plugins.manage`, `plugin_call` → `plugins.execute`
- **Numbering** (4): `numbering_list` → `numbering.read`, `numbering_get` → `numbering.read`, `numbering_update_format` → `numbering.manage`, `numbering_reset` → `numbering.manage`
- **Scripts** (2): `validate_rhai_script` → `scripts.read`, `execute_rhai_script` → `scripts.execute`

#### Итого по RBAC
- 24/93 команд защищены (было 8)
- Назначение ролей: SUPERADMIN — всё, ADMIN — print + numbering (read+write), VIEWER — print.read, plugins.read, numbering.read

#### Frontend
- `navigation.ts`: `print` → `print.read`, `convert` → `plugins.read`, `numbering` → `numbering.read`

### Security Fixes: WASM-плагины

#### Проблема 1: Замороженный контекст (FIXED)
- **Было:** `HostData` с company_id/user_id клонировался при `wasm_load`, плагин использовал устаревшие данные после `switch_company`
- **Стало:** `Arc<RwLock<PluginContext>>` обновляется свежими данными из AppState перед каждым `plugin_call`
- **Impact:** устранена кросс-компанийная утечка данных через плагины

#### Проблема 2: Lock contention (FIXED)
- **Было:** `plugin_call` удерживал глобальный `AppState` mutex на всё время выполнения WASM — все команды платформы встали
- **Стало:** плагин извлекается из HashMap → mutex отпускается → `spawn_blocking` + `std::sync::Mutex` для плагина → плагин возвращается обратно
- **Impact:** плагин не блокирует остальные команды

#### Проблема 3: Нет лимитов (FIXED)
- **Было:** `plugin.call()` без timeout, fuel, memory limits — бесконечный цикл = OOM или hang
- **Стало:** `timeout_ms=10000`, `fuel_limit=10_000_000`, `memory=256 pages` (16MB), `tokio::time::timeout(30s)` на уровне команды

#### Проблема 4: block_in_place (-addressed)
- `block_in_place` используется в host functions для async DB из sync WASM — работает на multi-thread runtime (Tauri default)
- Теперь вызывается из `spawn_blocking` (не из async context) — безопасно

### Security Fixes: Rhai Sandbox

#### Проблема: Мёртвый код (FIXED)
- **Было:** `Sandbox { timeout, max_ops }` поля не применялись к `Engine` — `Engine::new()` без ограничений
- **Стало:** `engine.set_max_operations(self.max_ops)` применяется в `execute()` и `validate()`
- Rhai не имеет встроенного `set_timeout` — лимит операций является основным механизмом защиты

### Проверки
- cargo check: 0 ошибок ✅
- svelte-check: 0 ошибок ✅

## Этап 14: Страницы Скрипты и Отчёты + ObjectEditor-фикс ✅
**Дата:** 21.08.2026
**Коммиты:** f76d4f1 (ObjectEditor), 1b14973 (Scripts+Reports)

### ObjectEditor — полировка
- `$derived(() => {...})` → `$derived.by(() => {...})` (рекомендация Svelte 5)
- Save: кнопка активна для состояний draft + active (было только draft)
- Required валидация: обязательные поля проверяются перед сохранением
- Confirmation: диалог подтверждения для post/cancel/restore
- Все 17 типов полей работают: array, table, json (textarea), user, company (text input), formula/computed (read-only display), reference (select dropdown с lookup)

### ScriptsPage.svelte
- Rhai-редактор с подсчётом строк
- Кнопка «Валидировать» → validate_rhai_script
- Кнопка «Выполнить» → execute_rhai_script
- Панель контекста (JSON) + результат + лог
- RBAC: execute permission
- Справочник по API контекста

### ReportsPage.svelte
- Сводные карточки: всего объектов, типов, черновиков, проведено
- Разбивка по состояниям (bar chart)
- Разбивка по типам объектов (bar chart)
- Таблица последних 10 объектов
- Auto-refresh

### Проверки
- cargo check: 0 ошибок ✅
- svelte-check: 0 ошибок ✅

---

## Этап 15: Прикладные модули (WASM) — Инфраструктура

**Коммит: в прогрессе**

### Задача
Построить инфраструктуру для прикладных WASM-модулей с:
- Capability-based безопасностью
- Per-company lifecycle (install → enable/disable → uninstall)
- MongoDB persistence (модули + привязки к компаниям)
- Полным API хост-функций для работы с объектами, метаданными, логированием
- UI для управления модулями

### Архитектура API

#### Capability System
- Модуль декларирует capabilities в манифесте (через `get_info()`)
- Хост проверяет capability при каждом вызове host-функции
- `required_capability()` — маппинг: имя функции → требуемая capability

#### Допустимые capabilities
```
objects.create, objects.read, objects.update, objects.delete,
metadata.read, events.emit, numbering.next, logging, notifications
```

#### API Versioning
- `CURRENT_API_VERSION = "1.0"` — проверяется при установке
- Модуль указывает `api_version` в манифесте; если версия не совместима — установка блокируется

#### Host Functions (7 шт., расширяемые)
| Функция | Capability | Описание |
|---|---|---|
| `create_object` | objects.create | Создание объекта |
| `list_objects` | objects.read | Список объектов (с пагинацией) |
| `get_object` | objects.read | Получение объекта по UUID |
| `update_object` | objects.update | Обновление данных объекта |
| `log_message` | logging | Логирование сообщения |
| `get_entity_type` | metadata.read | Получение типа сущности |
| `list_entity_fields` | metadata.read | Список полей типа сущности |

> Планируются: `emit_event`, `next_number`, `notify_user`

#### Structured Error Responses
```json
{ "ok": false, "error": { "code": "CAPABILITY_DENIED", "message": "..." } }
```

### Реализовано

#### Backend (Rust)

**`src-tauri/src/modules/mod.rs`** — Типы и константы
- `InstalledModule` — BSON документ в коллекции `modules`
- `CompanyModule` — привязка модуля к компании (включение/отключение + настройки)
- `ModuleManifest` — манифест из `get_info()`
- `ModuleStatus` — `installed | enabled | disabled`
- `VALID_CAPABILITIES` — белый список capabilities
- `required_capability()` — маппинг функций на capabilities
- `CURRENT_API_VERSION = "1.0"`
- Error helpers: `module_not_found`, `already_installed`, `capability_denied`, `api_version_mismatch`, `invalid_manifest`

**`src-tauri/src/modules/service.rs`** — CRUD + lifecycle
- `ModuleService::install` — валидация WASM → сохранение в MongoDB
- `ModuleService::uninstall` — удаление модуля + company привязок
- `ModuleService::enable/disable` — per-company lifecycle
- `ModuleService::list` — все модули с merged статусом из company_modules
- `ModuleService::get/get_by_code` — чтение модуля
- `ModuleService::update_settings` — per-company настройки

**`src-tauri/src/modules/indexes.rs`** — MongoDB индексы
- `modules.code` (уникальный), `modules.api_version`
- `company_modules.company_id + module_id` (уникальный), `company_modules.company_id`

**`src-tauri/src/modules/commands.rs`** — 7 IPC команд
- `modules_list`, `modules_get`, `modules_install`, `modules_uninstall`
- `modules_enable`, `modules_disable`, `modules_update_settings`

**`src-tauri/src/plugin_manager/mod.rs`** — Переписан
- `HostData` с `module_code` + `capabilities`
- 7 host-функций с capability checks + structured error responses
- `WasmPlugin::load()` async
- Fuel: 10M, Memory: 256 pages, Timeout: 10s

**`src-tauri/src/plugin_manager/commands.rs`** — Переписан
- `wasm_load` принимает `capabilities: Vec<String>`
- Async загрузка `WasmPlugin::load`

**`src-tauri/src/lib.rs`** — `mod modules` + 7 команд в generate_handler
**`src-tauri/src/commands/mod.rs`** — `modules::indexes::ensure_indexes` при подключении

#### Frontend (TypeScript/Svelte)

**`src/lib/services/api.ts`** — `InstalledModule`, `ModuleStatus`, 7 API-методов

**`src/lib/components/ModulesPage.svelte`** — UI управления модулями
- Карточки модулей с expanded details (capabilities, functions)
- Upload WASM-файла для установки
- Enable/Disable/Uninstall
- RBAC: plugins.read / plugins.manage

**`src/lib/stores/navigation.ts`** — `convert` → `modules` (Прикладные модули)
**`src/App.svelte`** — Routes: `ModulesPage` вместо `ConvertPage`

### Проверки
- cargo check: 0 ошибок ✅
- svelte-check: 0 ошибок ✅
