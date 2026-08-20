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
- Сброс БД для RBAC: `db.getSiblingDB("2c_platform").dropDatabase()` (SSH в Docker-хост)
- После сброса: first-boot пересоздаст всё с permission_policy_ids в ролях
