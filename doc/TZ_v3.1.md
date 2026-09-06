# 2C Platform / 2С Техническое задание v3.1

**Статус:** Рабочая архитектурная спецификация v3.1 (Living Document)  
**Дата:** 06.09.2026  
**Владелец:** Миха Алексеев  
**Основание:** Исходное ТЗ v2.2, обсуждение архитектуры, решения по модулям, событиям, транспорту, клиенту, аудиту и правам доступа.

**Changelog v3.1:**
- Добавлен раздел 8.5 «Аудит действий (audit_log)» — отдельная подсистема операционного аудита.
- Полностью переработан раздел 13 «Права доступа»: добавлены модели `PermissionPolicy` (с `deny`, `priority`, `actions`), `Role`, `PermissionScopeType`, `RecordAccessLevel`.
- Добавлен `PermissionManager` и `CommandExecutionPipeline` (middleware) для автоматической проверки прав и аудита вызовов команд.
- Добавлены системные роли: `admin`, `staff`, `guest`, `archived`.
- **Изменён порядок фаз в разделе 18:** Фаза 4 — Аудит, Фаза 5 — Права доступа, Фаза 6 — Объекты. Обоснование: инфраструктура безопасности и аудита должна быть готова ДО реализации бизнес-логики, чтобы все объекты сразу создавались с проверкой прав и записью в аудит.
- Обновлён глоссарий и приложения (добавлены модели аудита и прав).

---

## СПИСОК ТЕРМИНОВ И АББРЕВИАТУР

| Термин | Определение |
|--------|-------------|
| **2C Platform / 2С** | Конфигурируемая документо-событийная платформа для малого и среднего бизнеса. |
| **WASM** | WebAssembly — бинарный формат инструкций для стековой виртуальной машины, используется для изолированного выполнения модулей. |
| **Extism** | Фреймворк для хостинга WASM-модулей с capability-моделью и ресурсными лимитами. |
| **wasmtime** | Runtime для выполнения WASM-модулей, используется под капотом Extism. |
| **SurrealDB** | Мульти-модельная база данных с поддержкой документов, графов, ключ-значение и встроенными ACID-транзакциями. |
| **Flutter** | UI-фреймворк от Google для создания кроссплатформенных приложений (Linux, Windows, macOS, iOS, Android, Web). |
| **Dart** | Язык программирования, используемый во Flutter для клиентской логики. |
| **SDUI** | Server-Driven UI — паттерн, при котором сервер отдаёт метаданные для генерации UI на клиенте. |
| **Event Sourcing** | Паттерн хранения состояния как последовательности неизменяемых событий. |
| **CQRS** | Command Query Responsibility Segregation — разделение операций на команды (изменение) и запросы (чтение). |
| **OCC** | Optimistic Concurrency Control — оптимистичная блокировка через версионирование документов. |
| **RPC** | Remote Procedure Call — механизм вызова удалённых процедур. |
| **REST** | Representational State Transfer — архитектурный стиль для HTTP API. |
| **WebSocket** | Протокол полнодуплексной связи поверх TCP для push-уведомлений от сервера к клиенту. |
| **SSE** | Server-Sent Events — однонаправленный поток событий от сервера к клиенту. |
| **AppRegistry** | Группирующая структура, содержащая 5 регистров: CommandRegistry, PermissionRegistry, ObjectSchemaRegistry, PrintTemplateRegistry, ScriptRegistry. |
| **CommandRegistry** | Динамический реестр команд с `tokio::sync::RwLock`, поддерживающий регистрацию/удаление в рантайме. |
| **Ensure-семантика** | Идемпотентная регистрация ресурсов: повторный вызов не создаёт дублей, обновление только при изменении версии. |
| **DependencySpec** | Спецификация зависимости модуля: код, версия (semver), обязательность. |
| **Capability** | Технический грант, разрешающий WASM-модулю вызывать определённые host-функции. |
| **RBAC** | Role-Based Access Control — управление доступом на основе ролей. |
| **EventBatch** | Пакет событий, отправляемый клиентом на сервер при восстановлении сети после оффлайн-работы. |
| **ServerPush** | Сообщение, инициированное сервером и отправленное клиенту через WebSocket/SSE. |
| **Труба и Доска** | Концепция: Труба (Event Store) — истина, Доска (Projections) — материализованные представления. |
| **AuditEntry** | Запись операционного аудита: действие, актор, цель, результат, детали, timestamp. |
| **AuditRepository** | Порт ядра для записи и чтения записей аудита. |
| **PermissionManager** | Сервис в `core-application` для проверки прав доступа на основе политик и ролей. |
| **CommandExecutionPipeline** | Middleware-обёртка вокруг `CommandRegistry::execute`, обеспечивающая автоматическую проверку прав и аудит вызова. |

---

## 1. НАЗНАЧЕНИЕ СИСТЕМЫ

Разрабатывается конфигурируемая документо-событийная платформа для малого и среднего бизнеса.  
**Рабочее название:** 2C Platform / 2С.

**Цели системы:**
1. Гибкая настройка под заказчика без перекомпиляции ядра.
2. Документы, справочники, события, отчёты и права через метамодель.
3. Предметные модули в виде изолированных WASM-плагинов.
4. Современный технологический стек (Rust, SurrealDB, Flutter).
5. Кроссплатформенный клиент (desktop-first, затем мобильные устройства).
6. Полный аудит, версионирование, криптоподпись и строгий RBAC.
7. Оффлайн-работа с последующей синхронизацией.
8. Автоматическая генерация UI из метаданных (Server-Driven UI).

**Базовый модуль первой очереди** — управленческий учёт. Это не полноценная бухгалтерия РСБУ, а управленческий учёт на базе плана счетов и операций.

---

## 2. ОЧЕРЕДИ РАЗРАБОТКИ

### v0.1 — Desktop-first
**Состав:**
- Flutter desktop (Linux, Windows).
- Встроенное Rust-ядро, подключение к SurrealDB по URI.
- Компании, расширенная модель пользователей, метамодель, объекты, документы, события, версии, аудит.
- Rhai-скрипты, управленческий учёт, CSV-экспорт, печатные формы через HTML и системный диалог.
- Тёмная тема, уведомления inapp + e-mail, криптоподпись через cpcsp-rs (Linux-first).
- WASM-модули через Extism, CommandRegistry, AppRegistry.
- Оффлайн-синхронизация с Optimistic Concurrency Control.
- Server-Driven UI (SDUI) для генерации форм и каталогов.
- **Операционный аудит действий (audit_log) и строгий RBAC с CommandExecutionPipeline.**

### v0.2 — Многопользовательский сервер
**Состав:**
- Отдельный Rust-сервис на Axum.
- Централизованная бизнес-логика, многопользовательская работа.
- WebSocket/SSE для ServerPush.
- Подготовка к веб-клиенту (Flutter Web).

### v0.3 — Торговля и расширения
**Состав:**
- Модуль торговли, номенклатура, склады, партии, FIFO/average, составные товары.
- Уведомления через внешние каналы (Telegram, SMS).
- Мобильные клиенты (Flutter Android/iOS) с поддержкой периферии (ТСД, весы, ККМ).

---

## 3. ТЕХНОЛОГИЧЕСКИЙ СТЕК

### Backend
1. **Rust, Tokio** — асинхронный runtime.
2. **Axum** — HTTP-фреймворк для серверного режима.
3. **SurrealDB** — мульти-модельная база данных с ACID-транзакциями.
4. **Extism + wasmtime** — runtime для WASM-модулей.
5. **serde / serde_json** — сериализация.
6. **tracing** — логирование.
7. **cpcsp-rs** — собственная библиотека для работы с КриптоПро CSP.

### Frontend
1. **Flutter** — кроссплатформенный UI-фреймворк.
2. **Dart** — язык клиентской логики.
3. **Riverpod / BLoC** — управление состоянием.
4. **Isar / Hive** — локальное хранилище для оффлайн-режима.
5. **Platform Channels + FFI** — работа с периферией (ТСД, весы, ККМ).

### Скрипты
1. **Rhai** — песочница, таймауты, лимиты, capability API.

### Транспорт
1. **REST/HTTP** — базовый обмен (Command/Query).
2. **WebSocket/SSE** — ServerPush и тяжёлые EventBatch.

---

## 4. ГЛАВНЫЕ АРХИТЕКТУРНЫЕ ПРИНЦИПЫ

1. **Ядро нейтрально к предметной области.** Бухгалтерия, торговля, CRM — это WASM-модули.
2. **Документ — частный случай объекта.**
3. **Метаданные первичны.** UI генерируется из метаданных (SDUI).
4. **Концепция «Труба и Доска» (Pipe and Board):**
   - **Труба (Event Store)** — это истина. Поток неизменяемых фактов (событий).
5. **Команды и События:**
   - **Команда (Command)** — это намерение. Она может быть отклонена. В базу не пишется.
   - **Событие (Event)** — это свершившийся факт. Пишется в базу навсегда.
6. **Модульность через WASM:**
   - Все предметные модули — это WASM-плагины, исполняемые через Extism.
   - Модули регистрируют команды, метаданные, права декларативно.
   - Хост (ядро) обеспечивает изоляцию, безопасность, ресурсные лимиты.
7. **Физическое удаление бизнес-объектов и пользователей с историей запрещено.**
8. **Гибкость управляемая, не анархичная.**
9. **Двухуровневое журналирование:** Event Store хранит бизнес-события (для восстановления состояния), `audit_log` хранит операционные действия (для безопасности и compliance).
10. **Безопасность по умолчанию (Deny-by-default):** Любое действие, не разрешённое явно через `PermissionPolicy`, запрещено. Проверка прав осуществляется централизованно через `CommandExecutionPipeline`.
11. **Инфраструктура безопасности первична:** Аудит и права доступа реализуются ДО бизнес-логики (объектов), чтобы все объекты сразу создавались с проверкой прав и записью в аудит.

---

## 5. МОДЕЛЬ ДАННЫХ ЯДРА

### Основные коллекции (SurrealDB):
1. `companies`
2. `users`
3. `persons`
4. `user_contacts`
5. `user_company_profiles`
6. `user_certificates`
7. `roles`
8. `permission_policies`
9. `modules`
10. `company_modules`
11. `entity_types`
12. `entity_fields`
13. `entity_relations`
14. `entity_states`
15. `entity_transitions`
16. `entity_forms`
17. `entity_actions`
18. `objects`
19. `events`
20. `object_snapshots`
21. **`audit_log`** — операционный аудит действий
22. `scripts`
23. `settings`
24. `migrations`
25. `notification_templates`
26. `notification_outbox`
27. `crypto_providers`
28. `object_signatures`
29. `signing_policies`
30. `diagnostics_log`

### Коллекции модуля управленческого учёта:
1. `accounts`
2. `ledger_entries`
3. `ledger_balances`
4. `accounting_periods`

---

## 6. РАСШИРЕННАЯ МОДЕЛЬ ПОЛЬЗОВАТЕЛЯ

Для учётной системы пользователь — это не просто логин. Модель разделена на 5 сущностей:
1. **users (Учётная запись).** Хранит логин, password_hash, статус (`invited`, `active`, `disabled`, `locked`, `archived`), `role_ids`, параметры безопасности.
2. **persons (Персона).** Хранит `last_name`, `first_name`, `middle_name`, `display_name`. Отображается в интерфейсе, аудите и печатных формах.
3. **user_contacts (Контактные каналы).** Хранит несколько e-mail, телефонов, telegram.
4. **user_company_profiles (Рабочие профили).** Хранит привязку к компаниям.
5. **user_certificates (Сертификаты).** Хранит связь с сертификатами КриптоПро.

В событиях и аудите сохраняется снимок исполнителя (`actor_login`, `actor_full_name`, `actor_position`, `actor_company_id`), чтобы история оставалась читаемой даже при смене фамилии или увольнении.

---

## 7. МЕТАДАННЫЕ И ОБЪЕКТЫ

**Тип сущности** описывается в `entity_types`. Виды: `document`, `catalog`, `register`, `task`, `contract`, `project`, `setting`, `custom`.

**Поля** описываются в `entity_fields`. Типы: `string`, `text`, `integer`, `money`, `date`, `datetime`, `boolean`, `enum`, `reference`, `array`, `table`, `json`, `file`, `user`, `company`, `formula`, `computed`.

**Состояния и переходы** описываются в `entity_states` и `entity_transitions`.

**Универсальная коллекция `objects`** хранит все сущности. Поля: `_id`, `entity_type`, `kind`, `company_id`, `state`, `data`, `computed`, `number`, `date`, `parent_id`, `version`, `created_by`, `updated_by`, `created_at`, `updated_at`.

Документ — это объект с `kind = document`. Высоконагруженные модули могут использовать собственные коллекции (dedicated storage).

**Нумерация документов** уникальна в пределах типа и компании. Номер присваивается атомарно при проведении.

---

## 8. КОМАНДЫ И СОБЫТИЯ (EVENT SOURCING + CQRS)

### Event Sourcing
События хранятся в коллекции `events`. Это append-only журнал.

**Поля события:**
1. `_id` (UUID)
2. `stream_type` (тип потока — см. ниже)
3. `stream_id` (ID объекта)
4. `event_type` (тип события: `object.created`, `document.posted`, `company.updated`, и т.д.)
5. `version` (порядковый номер в потоке)
6. `payload` (данные события)
7. `metadata` (`actor_user_id`, `actor_login`, `actor_full_name`, `actor_position`, `actor_company_id`, `ip_address`)
8. `company_id`
9. `correlation_id` (сквозной ID бизнес-операции)
10. `causation_id` (ID события-причины)
11. `occurred_at` (UTC)

**Типы потоков (StreamType):**
Каждая сущность с собственной историей изменений имеет свой тип потока:
| StreamType | Назначение | Примеры событий |
|------------|------------|-----------------|
| `Object` | Универсальные объекты из коллекции `objects` | `object.created`, `object.updated`, `document.posted` |
| `User` | Учётные записи | `user.created`, `user.status_changed` |
| `Person` | Персоны (ФИО) | `person.updated` |
| `UserContact` | Контактные каналы пользователя | `user_contact.added`, `user_contact.removed` |
| `UserProfile` | Рабочие профили (привязка к компании) | `user_profile.created`, `user_profile.deactivated` |
| `UserCert` | Сертификаты КриптоПро | `user_cert.added`, `user_cert.revoked` |
| `Company` | Компании | `company.created`, `company.updated` |
| `Role` | Роли (RBAC) | `role.created`, `role.permissions_changed` |
| `Module` | WASM-модули | `module.installed`, `module.enabled` |
| `Metadata` | Метаданные сущностей | `metadata.entity_type.created`, `metadata.entity_type.updated` |

**Индексы для events:**
1. `{ stream_type, stream_id, version }` — для чтения истории конкретного объекта
2. `{ event_type, occurred_at }` — для поиска событий по типу
3. `{ company_id, occurred_at }` — для аудита по компании
4. `{ correlation_id }` — для сквозной трассировки бизнес-операций

### CQRS
**Проекции (Projections)** слушают события и обновляют материализованные данные (`objects`, `ledger_balances`). В v0.1 проекции применяются синхронно внутри транзакции SurrealDB.

### Снимок исполнителя (Actor Snapshot)
```rust
pub struct ActorSnapshot {
    pub user_id: Option<Uuid>,      // None для системного актора
    pub login: String,
    pub full_name: String,
    pub position: Option<String>,
    pub company_id: Option<Uuid>,
}
```
**Системный актор:** Для операций, выполняемых системой, используется `ActorSnapshot::system()` с `login: "system"`, `full_name: "Система"`.

### Оптимистичная блокировка (OCC)
Каждый объект имеет поле `version` (u64). При обновлении клиент передаёт `expected_version`. Если текущая версия в БД не совпадает с ожидаемой, операция отклоняется с ошибкой `CONFLICT_ERROR`.

**Реализация:** `AggregateRoot::apply` проверяет `expected = version + pending.len() + 1` при каждом событии; при расхождении возвращает `VersionConflict`.

### 8.5. Аудит действий (audit_log)
**Event Store** и **audit_log** — это две разные подсистемы:
- **Event Store:** Бизнес-события (для восстановления состояния, CQRS). Append-only, никогда не удаляются.
- **Audit Log:** Операционные действия (для безопасности, compliance, отладки). Могут архивироваться/удаляться по retention policy.

**Структура AuditEntry:**
```rust
pub struct AuditEntry {
    pub id: Uuid,
    pub action: String,              // "user.login", "module.install", "command.executed"
    pub actor: ActorSnapshot,        // Кто выполнил действие
    pub target: Option<AuditTarget>, // На что направлено действие
    pub result: AuditResult,         // Успех/неуспех
    pub details: Option<serde_json::Value>, // Дополнительные данные
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub timestamp: DateTime<Utc>,
}

pub struct AuditTarget {
    pub entity_type: Option<String>, // "user", "module", "role"
    pub entity_id: Option<Uuid>,
    pub entity_code: Option<String>, // Для модулей, ролей
    pub company_id: Option<Uuid>,
}

pub enum AuditResult {
    Success,
    Failure { reason: String },
}
```

**Что логируется в audit_log:**
1. Аутентификация: `user.login`, `user.login_failed`, `user.logout`.
2. Управление модулями: `module.install`, `module.uninstall`, `module.enable`.
3. Управление правами: `permission.granted`, `role.created`.
4. Системные операции: `system.backup`, `system.migration`.
5. Декларативная регистрация: `seed_metadata`, `seed_metadata_skipped`.
6. Вызовы команд: `command.executed`, `permission.denied` (через `CommandExecutionPipeline`).

---

## 9. МОДУЛЬНАЯ СИСТЕМА (WASM + EXTISM)

### Жизненный цикл модуля
- **Установка:** Валидация WASM + `get_info()`. Манифест = единственный источник правды. Декларативная регистрация: хост сам применяет `object_schemas` / `permissions` / `navigation` — код плагина НЕ запускается.
- **Включение компании:** `preload_company_modules` из БД, загрузка и повторная декларативная регистрация (идемпотентно).
- **Выгрузка:** `uninstall`/`disable` → удаление из памяти, регистров и меню.

### Манифест `get_info()`
Полный дескриптор v2 включает: `code`, `version`, `display_name`, `api_version`, `capabilities`, `commands`, `permissions`, `object_schemas`, `print_templates`, `scripts`, `metadata_version`, `handles_documents`, `navigation`, `demo`, `settings_schema`, `dependencies`.

### Декларативная регистрация (ensure-семантика)
При `install`/`enable`/`preload` хост применяет манифест сам. `permissions` → политики `permission_policies` по кодам (`scope` из манифеста). Каждое применение метаданных пишется в аудит (`seed_metadata`, актор «система/модуль»).

### Трёхслойная архитектура
1. **Ядро:** Базовые сущности, метаданные, безопасность (`permission_policy`, signing), инфраструктура (`tx`, `events`, `audit`, `plugin_manager`).
2. **Базовые модули:** `core.nomenclature`, `core.counterparty`, `core.chart_of_accounts`.
3. **Прикладные модули:** `plugin.stock`, `plugin.trade`, `plugin.accounting`, `plugin.requests`.

### Ресурсные лимиты
Топливо: 10 000 000 инструкций. Память: 256 страниц (~16 МБ). Таймаут: 10 с внутри плагина, 30 с на весь `plugin_call`. Доступ к ФС/сети: нет.

---

## 10. ТРАНСПОРТНЫЙ СЛОЙ (RPCMESSAGE)

Все сообщения между клиентом и сервером оборачиваются в единый конверт `RpcMessage`:
```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RpcMessage {
    Command { id: String, module: String, action: String, payload: Value },
    Query { id: String, module: String, action: String, payload: Value },
    EventBatch { id: String, module: String, events: Vec<Value> },
    Response { id: String, payload: Value },
    Error { id: String, code: String, message: String, details: Option<Value> },
    ServerPush { id: String, module: String, event_type: String, payload: Value },
}
```
**Транспорт:** REST/HTTP для Command/Query/EventBatch. WebSocket/SSE для ServerPush и тяжёлых EventBatch.

---

## 11. КЛИЕНТСКАЯ ЧАСТЬ (FLUTTER + SDUI)

**Server-Driven UI (SDUI):** Клиент использует паттерн SDUI. Универсальный движок рендерит каталоги и формы на лету на основе `object_schemas` и `forms` из манифеста модуля.
- **Реестр виджетов:** Превращает JSON-описание в нативный виджет.
- **Кастомизация:** Если универсального виджета недостаточно, регистрируется кастомный виджет под кодом (например, `production.gantt_chart`).

**Оффлайн-режим:** Локальная БД (Isar/Hive). При изменении данных клиент записывает событие локально. При наличии сети — отправляет на сервер (`EventBatch`).

---

## 12. ОФФЛАЙН-СИНХРОНИЗАЦИЯ И РАЗРЕШЕНИЕ КОНФЛИКТОВ

**Optimistic Concurrency Control (OCC):**
1. Клиент передаёт `expected_version`.
2. Сервер сверяет версию с текущей в БД.
3. При несовпадении сервер немедленно прерывает транзакцию и возвращает `RpcMessage::Error` с кодом `CONFLICT_ERROR` и деталями (`actual_version`, `modified_by`, `modified_at`).
4. **Автоматического мерджа нет.** Пользователь должен вручную разрешить конфликт.

---

## 13. ПРАВА ДОСТУПА (RBAC)

Модель: `User` -> `Role` -> `PermissionPolicy`. Группы не используются. Наследование ролей не используется (вместо него — «создать на основании» / клонирование).

### 13.1. Модели данных
```rust
pub enum PermissionScopeType {
    Platform,    // Глобальные права
    Module(String), // Права модуля (например, "invoice")
    Metadata,    // Права на метаданные
    None,
}

pub enum RecordAccessLevel {
    Owned,       // Только созданные пользователем
    ByRole,      // Назначенные на роль/пользователя
    ByCompany,   // Все в рамках компании
    All,         // Полный доступ
}

pub struct PermissionPolicy {
    pub id: Uuid,
    pub code: String,                    // "invoice.create"
    pub name: String,
    pub description: Option<String>,
    pub scope_type: PermissionScopeType,
    pub entity_type: Option<String>,     // Для детализации
    pub actions: Vec<String>,            // ["create", "read", "update", "*"]
    pub record_access: RecordAccessLevel,
    pub deny: bool,                      // Явное запрещение (override)
    pub priority: i32,                   // Приоритет при конфликтах
    pub module_code: Option<String>,     // Модуль-источник
    pub is_system: bool,                 // Системная политика
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct Role {
    pub id: Uuid,
    pub company_id: Uuid,
    pub code: String,                    // "admin", "staff", "guest", "archived"
    pub name: String,
    pub description: Option<String>,
    pub permission_policy_codes: Vec<String>, // Ссылки на коды политик
    pub is_system: bool,                 // true для системных ролей
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

### 13.2. Алгоритм проверки прав (PermissionManager)
Сервис `PermissionManager` в `core-application` реализует детерминированную проверку:
1. Получить роли пользователя в компании.
2. Собрать все `permission_policy_codes` из этих ролей.
3. Загрузить политики из `AppRegistry.permission_registry`.
4. Фильтрация:
   - `scope_type` покрывает запрошенный модуль/платформу.
   - `entity_type` совпадает или является wildcard (`None`).
   - `actions` содержит запрошенное действие или `"*"`.
5. Сортировка совпавших политик по `priority` (убывание).
6. **Deny overrides allow:** Если среди совпавших есть политика с `deny: true` и наивысшим приоритетом → доступ запрещён.
7. Если есть политика с `deny: false` и наивысшим приоритетом → доступ разрешён.
8. **Deny-by-default:** Если ничего не совпало → доступ запрещён.

### 13.3. CommandExecutionPipeline (Middleware)
Чтобы команда в `CommandRegistry` не могла «забыть» проверить права, проверка встроена в конвейер выполнения:
1. **Audit Start:** Записать в `audit_log` действие `command.executed` (статус: `started`).
2. **Permission Check:** `PermissionManager` проверяет `required_permission` из метаданных команды. Если отказ → записать в `audit_log` `permission.denied` и вернуть `PERMISSION_ERROR`.
3. **Execute:** Выполнение бизнес-логики команды.
4. **Audit End:** Обновить запись в `audit_log` (статус: `success` или `failed`).

### 13.4. Системные роли
При инициализации компании автоматически создаются системные роли (`is_system: true`):
- `admin`: Полный доступ ко всем действиям (`All`).
- `staff`: Базовые права на чтение и создание своих документов (`Owned`/`ByCompany`).
- `guest`: Только чтение (`Owned`/`ByCompany`).
- `archived`: Роль для уволенных. Даёт право `read` на объекты, созданные этим пользователем, но запрещает создание новых или изменение чужих.

---

## 14. МОДУЛЬ УПРАВЛЕНЧЕСКОГО УЧЁТА

Первый предметный модуль. Не является полноценной бухгалтерией РСБУ.
**Сущности:** `accounts`, `ledger_entries`, `ledger_balances`, `accounting_periods`.
**План счетов:** редактируемый, иерархический, привязан к компании. Типы: `Asset`, `Liability`, `Equity`, `Revenue`, `Expense`, `OffBalance`.
**Проводки** создаются только через API модуля. Перед сохранением проверяется: существование счетов, активность счетов, равенство дебета и кредита, права, дата, период.
**Отмена проведения** не удаляет проводки. Используется сторно или обратные записи.

---

## 15. СКРИПТОВЫЙ ДВИЖОК RHAI

**Типы скриптов:** `formula`, `validator`, `before_action`, `after_action`, `report`, `event_handler`.
**Ограничения:** таймаут, лимит операций, запрет файловой системы, запрет сети по умолчанию, доступ только через capability API.
Ядро предоставляет набор быстрых, скомпилированных Rust-функций (Core API) для Rhai.

---

## 16. КРИПТОПОДПИСЬ

**Основной провайдер первой очереди:** КриптоПро CSP на Linux через библиотеку `cpcsp-rs`.
Платформа не является СКЗИ, не хранит приватные ключи. Подписывается канонический снимок версии объекта.
При включённой политике подписываются: `create non-draft`, `update non-draft`, `post`, `cancel`, `restore_version`. Для черновиков подпись не обязательна.

---

## 17. УВЕДОМЛЕНИЯ, ЭКСПОРТ, ПЕЧАТЬ

- **Уведомления:** Первая очередь: inapp, email (SMTP).
- **Экспорт:** Основной формат: CSV (UTF-8, разделитель `;` по умолчанию).
- **Печатные формы:** Формируются как HTML-страницы со встроенными стилями. Печатная форма всегда светлая.

---

## 18. ЭТАПЫ РАЗРАБОТКИ v0.1

**Обоснование порядка:** Инфраструктура безопасности (аудит и права) реализуется ДО бизнес-логики (объектов), чтобы все объекты сразу создавались с проверкой прав и записью в аудит.

1. Каркас проекта, подключение к SurrealDB, диагностика. ✅ Done
2. Компании, расширенная модель пользователей, роли. ✅ Done
3. Метаданные (`entity_types`, `fields`, `states`). ✅ Done
4. **Аудит действий (`audit_log`) и `AuditRepository`.** ← НОВОЕ в v3.1 (перенесено с позиции 5.5)
5. **Права доступа (`permission_policies`), `PermissionManager`, `CommandExecutionPipeline`, системные роли.** ← НОВОЕ в v3.1 (перенесено с позиции 6)
6. Объекты, CRUD, оптимистичная блокировка. ← Перенесено с позиции 4
7. События, версии, снимки исполнителя. ✅ Done (частично)
8. CommandRegistry, AppRegistry, 5 регистров с ensure-семантикой. ✅ Done
9. WASM-модули через Extism, манифест, декларативная регистрация.
10. Транспортный слой (`RpcMessage`), REST + WebSocket.
11. Flutter-клиент, SDUI, тёмная тема.
12. Оффлайн-синхронизация, Optimistic Concurrency Control.
13. Rhai-скрипты, редактор, Core API.
14. Модуль управленческого учёта, проводки, ОСВ, баланс.
15. CSV-экспорт, HTML-печатные формы.
16. Уведомления inapp + e-mail.
17. Криптоподпись через `cpcsp-rs` (Linux).
18. Пакет диагностики, логирование, маскирование ПД.
19. Тесты и документация.

---

## 19. КРИТЕРИИ ГОТОВНОСТИ v0.1

Система готова, если:
1. Можно подключиться к SurrealDB по URI.
2. Можно создать компанию и пользователя с ФИО, контактами и рабочим профилем.
3. Можно настроить новый тип документа без программирования.
4. Можно создавать и редактировать документы через UI (SDUI).
5. Видна история версий, можно восстановить версию.
6. Каждое действие попадает в аудит со снимком исполнителя.
6.1. **Операционный аудит (`audit_log`) записывает действия пользователей, установку модулей, изменение прав.**
6.2. **Любая команда без явного права доступа блокируется `CommandExecutionPipeline` с записью `permission.denied` в аудит.**
7. Работают Rhai-скрипты в песочнице.
8. Работает управленческий учёт, проведение создаёт события и проводки.
9. ОСВ, журнал проводок, карточка счёта и баланс работают.
10. CSV-экспорт работает с выбором разделителя и кодировки.
11. Печатные формы открываются и печатаются (всегда светлые).
12. Тёмная тема работает в интерфейсе.
13. Уведомления inapp и e-mail работают.
14. Криптоподпись на Linux работает через `cpcsp-rs`.
15. WASM-модули устанавливаются, регистрируют команды, вызываются через `RpcMessage`.
16. Оффлайн-синхронизация работает, конфликты возвращают `CONFLICT_ERROR`.
17. Пакет диагностики формируется, контакты в нём маскируются.
18. Логи и документация на русском языке.

---

## 20. ОТЛОЖЕНО

Откладываются:
1. Визуальный конструктор форм.
2. Marketplace модулей.
3. Группы пользователей.
4. Пакетное проведение.
5. LIFO.
6. Полноценная мультивалютность.
7. Полевые права.
8. CRL/OCSP.
9. Telegram / Max / SMS.
10. Мобильные клиенты (v0.3).
11. Нативные динамические библиотеки (dylib).
12. Retention policy для `audit_log` (архивация/удаление старых записей).

---

## 21. ИТОГ

2C Platform — это не «ещё одна самописная бухгалтерия», а конфигурируемая платформа, где:
1. Ядро нейтрально и работает по принципу «Труба и Доска».
2. Документы являются частным случаем объектов.
3. Метаданные позволяют настраивать сущности и генерировать UI (SDUI).
4. События дают историю и основу интеграций (Event Sourcing + CQRS).
5. **Двухуровневое журналирование:** Event Store для бизнес-событий, `audit_log` для операционного аудита.
6. **Строгий RBAC:** `PermissionManager` с приоритезацией, `deny`-override и `CommandExecutionPipeline` гарантируют, что ни одна команда не выполнится без проверки прав.
7. **Инфраструктура безопасности первична:** Аудит и права реализуются ДО бизнес-логики, чтобы все объекты сразу создавались с проверкой прав и записью в аудит.
8. Модульность через WASM-плагины с изоляцией и capability-моделью.
9. Криптоподпись реализована через нативную библиотеку `cpcsp-rs`.
10. Frontend кроссплатформенный и современный (Flutter + Dart).
11. Оффлайн-работа с Optimistic Concurrency Control.

---

# ПРИЛОЖЕНИЯ

---

## Приложение №1. CommandRegistry
*(См. предыдущие версии ТЗ: динамический реестр с `tokio::sync::RwLock`, поддерживающий `register`/`unregister`/`execute`/`list`/`remove_by_prefix`)*

## Приложение №2. AppRegistry
*(См. предыдущие версии ТЗ: группирующая структура из 5 регистров с ensure-семантикой)*

## Приложение №3. DependencySpec
*(См. предыдущие версии ТЗ: спецификация зависимостей модулей с semver-проверкой)*

## Приложение №4. Host-функции (26) и Capabilities
*(См. предыдущие версии ТЗ: полный список host-fn и правил минимизации)*

## Приложение №5. Конверт host-функций и Оркестрация документов
*(См. предыдущие версии ТЗ: `unwrap_host`, коды ошибок, `handles_documents`, `$ref`-связывание)*

---

## Приложение №6. Модель аудита действий (audit_log)

**Назначение:** Операционный аудит действий пользователей и системы. Отличается от Event Store тем, что хранит операционные действия (для безопасности и compliance), а не бизнес-события.

**Структура:**
```rust
pub struct AuditEntry {
    pub id: Uuid,
    pub action: String,              // "user.login", "module.install", "command.executed"
    pub actor: ActorSnapshot,        // Кто выполнил действие
    pub target: Option<AuditTarget>, // На что направлено действие
    pub result: AuditResult,         // Успех/неуспех
    pub details: Option<serde_json::Value>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub timestamp: DateTime<Utc>,
}

pub struct AuditTarget {
    pub entity_type: Option<String>,
    pub entity_id: Option<Uuid>,
    pub entity_code: Option<String>,
    pub company_id: Option<Uuid>,
}

pub enum AuditResult {
    Success,
    Failure { reason: String },
}

pub struct AuditFilter {
    pub action: Option<String>,
    pub actor_user_id: Option<Uuid>,
    pub target_entity_type: Option<String>,
    pub target_entity_id: Option<Uuid>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub limit: usize,
}

pub trait AuditRepository: Send + Sync {
    async fn log(&self, entry: AuditEntry) -> Result<(), DomainError>;
    async fn query(&self, filter: AuditFilter) -> Result<Vec<AuditEntry>, DomainError>;
}
```

**Индексы для `audit_log`:**
1. `{ action, timestamp }`
2. `{ actor.user_id, timestamp }`
3. `{ target.entity_type, target.entity_id, timestamp }`
4. `{ company_id, timestamp }`
5. `{ result, timestamp }`

---

## Приложение №7. Модель прав доступа (Permissions)

**Назначение:** Строгий, детерминированный контроль доступа на основе ролей и политик с поддержкой явных запретов и приоритетов.

**Структура:**
```rust
pub enum PermissionScopeType {
    Platform,
    Module(String),
    Metadata,
    None,
}

pub enum RecordAccessLevel {
    Owned,
    ByRole,
    ByCompany,
    All,
}

pub struct PermissionPolicy {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub scope_type: PermissionScopeType,
    pub entity_type: Option<String>,
    pub actions: Vec<String>,            // ["create", "read", "update", "*"]
    pub record_access: RecordAccessLevel,
    pub deny: bool,                      // Явное запрещение
    pub priority: i32,                   // Приоритет при конфликтах
    pub module_code: Option<String>,
    pub is_system: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct Role {
    pub id: Uuid,
    pub company_id: Uuid,
    pub code: String,                    // "admin", "staff", "guest", "archived"
    pub name: String,
    pub description: Option<String>,
    pub permission_policy_codes: Vec<String>,
    pub is_system: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

**Алгоритм `PermissionManager::check_access`:**
1. Фильтрация политик по `scope_type`, `entity_type` и `actions` (содержит действие или `"*"`).
2. Сортировка совпавших политик по `priority` (убывание).
3. Если среди совпавших есть политика с `deny: true` → вернуть `false` (Deny overrides allow).
4. Если среди совпавших есть политика с `deny: false` → вернуть `true`.
5. Иначе → вернуть `false` (Deny-by-default).

**CommandExecutionPipeline:**
Обёртка вокруг `CommandRegistry::execute`, которая:
1. Логирует `command.executed` (started) в `audit_log`.
2. Вызывает `PermissionManager::check_access` для `required_permission` команды.
3. При отказе логирует `permission.denied` и возвращает `DomainError::PermissionDenied`.
4. При успехе выполняет команду и логирует `command.executed` (success/failed).

---

**Конец документа.**