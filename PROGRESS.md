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

### Что сделано
- MongoDB Replica Set rs0 на Docker (192.168.31.31:27017)
- База `2c_platform` с 24 коллекциями и индексами
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
- Проверки: cargo check ✅ (0 errors), svelte-check 0 errors ✅, vite build ✅

### Технические детали
- `tokio::sync::Mutex` для AppState (async Tauri commands)
- `futures::StreamExt` для Cursor итерации (mongodb 3.x)
- UUID хранятся как строки в MongoDB (Bson совместимость)
- First-boot: при первом `authenticate("admin", "admin")` создаётся:
  - Компания "Основная компания" (код MAIN)
  - Роль "Суперадминистратор" (код SUPERADMIN)
  - Пользователь admin/admin

### Следующий этап
- Этап 3: Метаданные (EntityType, EntityField, EntityState, EntityTransition, EntityAction)
