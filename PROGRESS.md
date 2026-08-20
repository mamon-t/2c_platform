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
**Коммит:** (текущий)

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

### Следующий шаг
- F2: Метаданные (entity_types, entity_fields, entity_states, entity_transitions, entity_forms, entity_actions)

## Этап 5: Метаданные (описание сущностей) ✅
**Дата:** 20.08.2026

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
