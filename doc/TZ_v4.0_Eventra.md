# Eventra FluxCore — Техническое задание v4.0

**Статус:** Рабочая архитектурная спецификация v4.0 (Living Document)  
**Дата:** 16.09.2026  
**Владелец:** Миха Алексеев  
**Основание:** Эволюция 2C Platform → Eventra FluxCore, переход к Event-Driven Metadata Platform

---

## Changelog v4.0

- **Переименование:** 2C Platform → Eventra FluxCore
- **Новая идеология:** Event-Driven Metadata Platform (универсальная событийная платформа)
- **Добавлен Rules Router (Layer 0.5)** — декларативная маршрутизация событий на базе YAML-правил
- **Добавлен UniversalEvent** — единый контракт для всех источников (пользователи, устройства, API, cron)
- **Добавлена сущность Workplace** — контекст рабочего места для идентификации источников
- **Добавлены сущности Device / Integration** — мета-модель внешних устройств и интеграций
- **Расширен аудит:** декларативный аудит событий от устройств через Rules Router
- **Добавлен EventBus** — внутренний транспорт на Tokio channels (in-process)
- **Добавлена наблюдаемость:** structured logging, метрики, трассировка (trace_id)
- **Разделение по kind:** команды (намерения) идут в CommandProcessor, события (факты) — напрямую в EventStore
- **Декларативное управление правилами:** правила Rules Router хранятся в БД, управляются через админский UI, поддерживают экспорт/импорт
- **Декларативное управление Device/Integration/Workplace:** сущности хранятся в БД, управляются через SDUI, поддерживают экспорт/импорт
- **Декларативное управление Permission:** политики прав хранятся в БД, управляются через UI, поддерживают экспорт/импорт

---

## СПИСОК ТЕРМИНОВ И АББРЕВИАТУР

| Термин | Определение |
|---|---|
| **Eventra FluxCore** | Событийно-ориентированная платформа на базе метаданных (Event-Driven Metadata Platform). "FluxCore" — ядро, работающее с потоком событий. |
| **UniversalEvent** | Единый контракт события для всех источников (пользователи, устройства, API, cron). Содержит kind, source, context, priority, qos, payload. |
| **Rules Router** | Декларативный движок маршрутизации событий (Layer 0.5). Работает с YAML-правилами, абстрагирован от транспорта. Правила хранятся в БД, управляются через UI. |
| **Workplace** | Рабочее место — логическая или физическая точка, объединяющая контекст действия (пользователь + устройства). Управляется через SDUI, поддерживает экспорт/импорт. |
| **Device** | Внешнее устройство (IoT/Edge/периферия), зарегистрированное в метаданных и аутентифицируемое через API-ключ/сертификат. Управляется через SDUI, поддерживает экспорт/импорт. |
| **Integration** | Внешняя система (API/webhook), зарегистрированная в метаданных и аутентифицируемая через HMAC/OAuth. Управляется через SDUI, поддерживает экспорт/импорт. |
| **EventBus** | Внутренний транспорт на базе Tokio channels (mpsc/broadcast) для передачи UniversalEvent между компонентами ядра. |
| **kind** | Поле UniversalEvent: `command` (намерение, идёт в CommandProcessor) или `event` (факт, идёт в EventStore напрямую). |
| **WASM** | WebAssembly — бинарный формат инструкций для стековой виртуальной машины, используется для изолированного выполнения модулей. |
| **Extism** | Фреймворк для хостинга WASM-модулей с capability-моделью и ресурсными лимитами. |
| **SurrealDB** | Мульти-модельная база данных с поддержкой документов, графов, ключ-значение и встроенными ACID-транзакциями. |
| **Flutter** | UI-фреймворк от Google для создания кроссплатформенных приложений (Linux, Windows, macOS, iOS, Android, Web). |
| **SDUI** | Server-Driven UI — паттерн, при котором сервер отдаёт метаданные для генерации UI на клиенте. |
| **Event Sourcing** | Паттерн хранения состояния как последовательности неизменяемых событий. |
| **CQRS** | Command Query Responsibility Segregation — разделение операций на команды (изменение) и запросы (чтение). |
| **OCC** | Optimistic Concurrency Control — оптимистичная блокировка через версионирование документов. |
| **RPC** | Remote Procedure Call — механизм вызова удалённых процедур. |
| **WebSocket** | Протокол полнодуплексной связи поверх TCP для push-уведомлений от сервера к клиенту. |
| **RBAC** | Role-Based Access Control — управление доступом на основе ролей. |
| **Труба и Доска** | Концепция: Труба (Event Store) — истина, Доска (Projections) — материализованные представления. |
| **DLQ** | Dead Letter Queue — очередь для событий, не прошедших валидацию или обработку, для последующего анализа. |
| **Routing Rule** | Правило маршрутизации событий в YAML-формате, хранящееся в БД и управляемое через админский UI. |

---

## 1. НАЗНАЧЕНИЕ СИСТЕМЫ

**Eventra FluxCore** — это событийно-ориентированная платформа на базе метаданных (Event-Driven Metadata Platform), способная управлять бизнес-процессами, пользовательскими интерфейсами и внешними устройствами (IoT/Edge/Периферия) в едином контуре.

### 1.1. Ключевые принципы

1. **Всё есть событие (UniversalEvent).** Пользователь нажал кнопку, датчик прислал данные, cron запустил задачу — всё это события с единым контрактом.
2. **Метаданные — единственный источник истины.** UI, маршрутизация, бизнес-логика — всё управляется метаданными.
3. **Слабосвязанная архитектура (Loosely Coupled).** Модули не знают друг о друге, они реагируют на события.
4. **Декларативная маршрутизация (Rules Router).** Бизнес-правила, валидация, приоритеты — через YAML, без перекомпиляции. Правила хранятся в БД и управляются через UI.
5. **Полный контекст действия (Workplace + Source Identity).** Каждое событие имеет источник, рабочее место, метод ввода.
6. **Декларативное управление конфигурацией.** Правила маршрутизации, устройства, интеграции, рабочие места, права — всё хранится в БД и управляется через админский UI с поддержкой экспорта/импорта.

### 1.2. Цели системы

- Гибкая настройка под заказчика без перекомпиляции ядра.
- Документы, справочники, события, отчёты и права через метамодель.
- Предметные модули в виде изолированных WASM-плагинов.
- Современный технологический стек (Rust, SurrealDB, Flutter).
- Кроссплатформенный клиент (desktop-first, затем мобильные устройства).
- Полный аудит, версионирование, криптоподпись и строгий RBAC.
- Оффлайн-работа с последующей синхронизацией.
- Автоматическая генерация UI из метаданных (Server-Driven UI).
- **Универсальная обработка событий от любых источников** (пользователи, устройства, внешние API).
- **Декларативная маршрутизация и фильтрация событий** (Rules Router с управлением через UI).
- **Полный контекст действия** (Workplace + Source Identity).
- **Декларативное управление Device/Integration/Workplace/Permission** через админский UI с экспортом/импортом.

### 1.3. Базовый модуль первой очереди

Управленческий учёт. Это не полноценная бухгалтерия РСБУ, а управленческий учёт на базе плана счетов и операций.

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
- Операционный аудит действий (audit_log) и строгий RBAC с CommandExecutionPipeline.
- **Rules Router (Layer 0.5)** — декларативная маршрутизация событий с управлением через UI.
- **UniversalEvent** — единый контракт для всех источников.
- **Workplace** — контекст рабочего места с управлением через UI и экспортом/импортом.
- **Device / Integration** — мета-модель внешних устройств и интеграций с управлением через UI.
- **EventBus** — внутренний транспорт на Tokio channels.
- **Наблюдаемость** — structured logging, метрики, трассировка.
- **Декларативное управление Permission** — экспорт/импорт политик прав.

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

### v0.4 — IoT и внешние интеграции

**Состав:**
- Расширенная поддержка устройств (MQTT, HTTP webhook, UDP).
- Интеграции с внешними системами (REST API, GraphQL).
- Расширенные сценарии Rules Router (Circuit Breaker, Canary Release).

---

## 3. ТЕХНОЛОГИЧЕСКИЙ СТЕК

### Backend

- **Rust, Tokio** — асинхронный runtime.
- **Axum** — HTTP-фреймворк для серверного режима.
- **SurrealDB** — мульти-модельная база данных с ACID-транзакциями.
- **Extism + wasmtime** — runtime для WASM-модулей.
- **serde / serde_json** — сериализация.
- **serde_yaml** — парсинг YAML-правил.
- **tracing** — логирование (structured logging).
- **metrics** — метрики (Prometheus-совместимые).
- **opentelemetry** — трассировка (Jaeger/Zipkin).
- **cpcsp-rs** — собственная библиотека для работы с КриптоПро CSP.

### Frontend

- **Flutter** — кроссплатформенный UI-фреймворк.
- **Dart** — язык клиентской логики.
- **Riverpod / BLoC** — управление состоянием.
- **Isar / Hive** — локальное хранилище для оффлайн-режима.
- **Platform Channels + FFI** — работа с периферией (ТСД, весы, ККМ).

### Скрипты

- **Rhai** — песочница, таймауты, лимиты, capability API.

### Транспорт

- **REST/HTTP** — базовый обмен (Command/Query).
- **WebSocket/SSE** — ServerPush и тяжёлые EventBatch.
- **EventBus (внутренний)** — Tokio channels (mpsc/broadcast).

---

## 4. ГЛАВНЫЕ АРХИТЕКТУРНЫЕ ПРИНЦИПЫ

### 4.1. Ядро нейтрально к предметной области

Бухгалтерия, торговля, CRM — это WASM-модули. Ядро не знает о бизнес-логике.

### 4.2. Документ — частный случай объекта

Все сущности (документы, справочники, устройства, рабочие места) — это объекты с метаданными.

### 4.3. Метаданные первичны

UI генерируется из метаданных (SDUI). Маршрутизация событий управляется метаданными (Rules Router).

### 4.4. Концепция «Труба и Доска» (Pipe and Board)

- **Труба (Event Store)** — это истина. Поток неизменяемых фактов (событий).
- **Доска (Projections)** — это текущее состояние. Материализованные представления (коллекция objects, остатки, ОСВ), которые строятся на основе событий.

### 4.5. Команды и События

- **Команда (Command)** — это намерение. Она может быть отклонена. В базу не пишется.
- **Событие (Event)** — это свершившийся факт. Пишется в базу навсегда.

### 4.6. Модульность через WASM

- Все предметные модули — это WASM-плагины, исполняемые через Extism.
- Модули регистрируют команды, метаданные, права декларативно.
- Хост (ядро) обеспечивает изоляцию, безопасность, ресурсные лимиты.

### 4.7. Физическое удаление запрещено

Физическое удаление бизнес-объектов и пользователей с историей запрещено.

### 4.8. Гибкость управляемая, не анархичная

Двухуровневое журналирование:
- **Event Store** хранит бизнес-события (для восстановления состояния).
- **audit_log** хранит операционные действия (для безопасности и compliance).

### 4.9. Безопасность по умолчанию (Deny-by-default)

Любое действие, не разрешённое явно через `PermissionPolicy`, запрещено. Проверка прав осуществляется централизованно через `CommandExecutionPipeline`.

### 4.10. Инфраструктура безопасности первична

Аудит и права доступа реализуются ДО бизнес-логики (объектов), чтобы все объекты сразу создавались с проверкой прав и записью в аудит.

### 4.11. Универсальная модель событий (UniversalEvent)

Все источники (пользователи, устройства, API, cron) генерируют события с единым контрактом `UniversalEvent`.

### 4.12. Декларативная маршрутизация (Rules Router)

События проходят через Rules Router перед обработкой. Правила описываются в YAML, хранятся в БД, применяются на лету. Управление правилами — через админский UI с экспортом/импортом.

### 4.13. Полный контекст действия

Каждое событие имеет:
- **source** — кто инициировал (пользователь, устройство, система).
- **context** — где и как (workplace, input_method).
- **priority** — критичность (critical, normal, low).
- **qos** — гарантии доставки (exactly-once, at-least-once, at-most-once).

### 4.14. Наблюдаемость (Observability)

Три столпа:
- **Логи** — structured logging с trace_id.
- **Метрики** — Prometheus-совместимые.
- **Трассировка** — OpenTelemetry + Jaeger/Zipkin.

### 4.15. Декларативное управление конфигурацией

Все конфигурационные сущности (правила маршрутизации, устройства, интеграции, рабочие места, права) хранятся в БД и управляются через админский UI. Поддерживается экспорт/импорт в JSON-файл для версионирования и переноса между окружениями.

---

## 5. МОДЕЛЬ ДАННЫХ ЯДРА

### 5.1. Основные коллекции (SurrealDB)

- `companies`
- `users`
- `persons`
- `user_contacts`
- `user_company_profiles`
- `user_certificates`
- `roles`
- `permission_policies`
- `modules`
- `company_modules`
- `entity_types`
- `entity_fields`
- `entity_relations`
- `entity_states`
- `entity_transitions`
- `entity_forms`
- `entity_actions`
- `objects`
- `events`
- `object_snapshots`
- `audit_log` — операционный аудит действий
- `scripts`
- `settings`
- `migrations`
- `notification_templates`
- `notification_outbox`
- `crypto_providers`
- `object_signatures`
- `signing_policies`
- `diagnostics_log`
- **`workplaces`** — рабочие места (НОВОЕ в v4.0)
- **`devices`** — внешние устройства (НОВОЕ в v4.0)
- **`integrations`** — внешние системы (НОВОЕ в v4.0)
- **`dead_letters`** — DLQ для необработанных событий (НОВОЕ в v4.0)
- **`routing_rules`** — правила маршрутизации Rules Router (НОВОЕ в v4.0)

### 5.2. Коллекции модуля управленческого учёта

- `accounts`
- `ledger_entries`
- `ledger_balances`
- `accounting_periods`

---

## 6. РАСШИРЕННАЯ МОДЕЛЬ ПОЛЬЗОВАТЕЛЯ

Для учётной системы пользователь — это не просто логин. Модель разделена на 5 сущностей:

- **users** (Учётная запись). Хранит логин, password_hash, статус (`invited`, `active`, `disabled`, `locked`, `archived`), `role_ids`, параметры безопасности.
- **persons** (Персона). Хранит `last_name`, `first_name`, `middle_name`, `display_name`. Отображается в интерфейсе, аудите и печатных формах.
- **user_contacts** (Контактные каналы). Хранит несколько e-mail, телефонов, telegram.
- **user_company_profiles** (Рабочие профили). Хранит привязку к компаниям.
- **user_certificates** (Сертификаты). Хранит связь с сертификатами КриптоПро.

В событиях и аудите сохраняется снимок исполнителя (`actor_login`, `actor_full_name`, `actor_position`, `actor_company_id`), чтобы история оставалась читаемой даже при смене фамилии или увольнении.

---

## 7. МЕТАДАННЫЕ И ОБЪЕКТЫ

Тип сущности описывается в `entity_types`. Виды: `document`, `catalog`, `register`, `task`, `contract`, `project`, `setting`, `custom`.

Поля описываются в `entity_fields`. Типы: `string`, `text`, `integer`, `money`, `date`, `datetime`, `boolean`, `enum`, `reference`, `array`, `table`, `json`, `file`, `user`, `company`, `formula`, `computed`.

Состояния и переходы описываются в `entity_states` и `entity_transitions`.

Универсальная коллекция `objects` хранит все сущности. Поля: `_id`, `entity_type`, `kind`, `company_id`, `state`, `data`, `computed`, `number`, `date`, `parent_id`, `version`, `created_by`, `updated_by`, `created_at`, `updated_at`.

Документ — это объект с `kind = document`. Высоконагруженные модули могут использовать собственные коллекции (dedicated storage).

Нумерация документов уникальна в пределах типа и компании. Номер присваивается атомарно при проведении.

---

## 8. УНИВЕРСАЛЬНАЯ МОДЕЛЬ СОБЫТИЙ (UniversalEvent)

**НОВОЕ в v4.0**

### 8.1. Контракт UniversalEvent

Все источники (пользователи, устройства, API, cron) генерируют события с единым контрактом:

```json
{
  "kind": "command | event",
  "source": {
    "type": "user | device | workplace | system | external_api",
    "id": "uuid-источника",
    "identity": "jwt | api-key | session-token | null",
    "company_id": "uuid-компании"
  },
  "context": {
    "workplace_id": "uuid-рабочего-места",
    "user_id": "uuid-пользователя (если известен)",
    "session_id": "uuid-сессии",
    "input_method": "ui_form | hardware_button | api | autonomous_sensor"
  },
  "event_type": "DocumentCreated | WeightMeasured | PanicButtonPressed",
  "priority": "critical | normal | low",
  "qos": "exactly-once | at-least-once | at-most-once",
  "timestamp": "2026-09-16T03:00:00Z",
  "payload": { "weight_kg": 15.5, "unit": "kg" },
  "metadata": {
    "trace_id": "uuid",
    "correlation_id": "uuid",
    "causation_id": "uuid"
  }
}
```

### 8.2. Разделение по kind

- **`command` (намерение)** — инициируется пользователем или UI. Требует проверки прав, валидации и может быть отклонено. Идёт в `CommandProcessor`.
- **`event` (факт)** — инициируется устройством, внешней системой или cron. Не может быть отклонено (оно уже произошло). Идёт напрямую в `EventStore`.

### 8.3. Идентификация источников (Source Identity)

Каждый источник обязан быть зарегистрирован и аутентифицирован:

| type | id | identity | Где регистрируется |
|---|---|---|---|
| **user** | `user_id` (UUID) | JWT token | `users` таблица |
| **device** | `device_id` (UUID) | API-ключ / сертификат | `devices` таблица (метаданные) |
| **external_api** | `integration_id` (UUID) | HMAC-ключ / OAuth token | `integrations` таблица (метаданные) |
| **system** | `"cron"` / `"bootstrap"` | Нет (доверенный внутренний) | Не регистрируется |

### 8.4. Контекст рабочего места (Workplace)

**Workplace** — это логическая или физическая точка, объединяющая контекст действия. Она привязана к `Company` и имеет список подключенных устройств.

**Решение проблемы «тупых» устройств:** Если USB-кнопка или дешевый датчик не умеют аутентифицироваться, событие обогащается **клиентом рабочего места** или **шлюзом**. Клиент знает свой `workplace_id` и текущую сессию, и подставляет их в `context`.

**Результат:** В `audit_log` всегда видно не "stupid human", а "Действие на Рабочем месте 'Пост охраны №3' в 14:00".

---

## 9. RULES ROUTER (Layer 0.5)

**НОВОЕ в v4.0**

### 9.1. Назначение

Декларативный движок маршрутизации событий на базе YAML-правил. Работает с UniversalEvent, абстрагирован от транспорта.

### 9.2. Функции

1. **Маршрутизация по kind:** Направляет `command` в `CommandProcessor`, а `event` — напрямую в `EventStore`.
2. **Приоритизация и QoS:** Разделяет потоки на `critical` (обход очередей), `normal`, `low`.
3. **Rate Limiting:** Агрегация мусорной телеметрии (например, 1000 событий/сек от датчика → 1 агрегированное событие).
4. **Защита и Валидация:** Отсев невалидных пакетов до того, как они нагрузят WASM/Rhai.
5. **Обогащение:** Добавление заголовков, контекста.
6. **Декларативный аудит событий:** Если для сырых событий (от устройств) требуется аудит, это настраивается YAML-правилом (`action: audit`). Нет правила — нет аудита.
7. **Dead Letter Queue (DLQ):** Невалидные или необработанные события уходят в DLQ для последующего анализа и реплея.

### 9.3. Хранение и управление правилами

**НОВОЕ в v4.0**

Правила маршрутизации хранятся в коллекции `routing_rules` в SurrealDB и управляются через админский UI (SDUI).

#### Модель данных

```rust
pub struct RoutingRule {
    pub id: Uuid,
    pub code: String,              // Уникальный код правила (например, "rate_limit_telemetry")
    pub name: String,              // Человекочитаемое название
    pub description: Option<String>,
    pub priority: i32,             // Приоритет выполнения (меньше = раньше)
    pub is_active: bool,           // Активно ли правило
    pub rule_yaml: String,         // YAML-представление правила (условие + действие)
    pub company_id: Option<Uuid>,  // None для глобальных правил
    pub version: u32,              // Версия правила (для OCC)
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

#### Формат правила (YAML)

```yaml
name: "rate_limit_telemetry"
condition:
  field: "event_type"
  operator: "eq"
  value: "TemperatureNormal"
action:
  type: "drop"
rate_limit:
  limit: 1
  per_seconds: 60
  key: "source.id"
```

#### Команды управления

- `routing_rule.create` — создание правила (требует `routing_rule.manage`)
- `routing_rule.update` — обновление правила (требует `routing_rule.manage`)
- `routing_rule.delete` — удаление правила (требует `routing_rule.manage`)
- `routing_rule.list` — список всех правил (требует `routing_rule.read`)
- `routing_rule.get` — получение правила по коду (требует `routing_rule.read`)
- `routing_rule.export` — экспорт правила в JSON (требует `routing_rule.read`)
- `routing_rule.import` — импорт правила из JSON (требует `routing_rule.manage`)
- `routing_rule.export_all` — экспорт всех правил в JSON-файл (требует `routing_rule.read`)
- `routing_rule.import_all` — импорт всех правил из JSON-файла (требует `routing_rule.manage`)

#### UI для управления правилами

Администратор через SDUI-интерфейс может:
- Просматривать список всех правил (каталог)
- Создавать новые правила через форму (редактор YAML)
- Редактировать существующие правила
- Активировать/деактивировать правила
- Экспортировать правила в JSON-файл
- Импортировать правила из JSON-файла

**Примечание:** Редактор YAML — это текстовое поле с подсветкой синтаксиса (CodeMirror или аналог). Валидация YAML происходит на сервере при сохранении.

#### Экспорт/импорт

**Экспорт:**
```json
{
  "version": "1.0",
  "exported_at": "2026-09-16T10:00:00Z",
  "rules": [
    {
      "code": "rate_limit_telemetry",
      "name": "Rate limit telemetry",
      "priority": 10,
      "is_active": true,
      "rule_yaml": "..."
    }
  ]
}
```

**Импорт:**
- Валидация структуры JSON
- Проверка уникальности кодов
- Ensure-семантика: если правило с таким кодом уже существует и версия ниже — обновить, иначе пропустить
- Запись в аудит `routing_rule.imported`

### 9.4. Формат правил

Правила описываются в YAML:

```yaml
rules:
  - name: "reject_invalid_device_event"
    condition:
      all:
        - field: "source.type"
          operator: "eq"
          value: "device"
        - field: "payload.weight_kg"
          operator: "lt"
          value: 0
    action:
      type: "reject"
      requeue: false

  - name: "route_critical_alerts"
    condition:
      field: "priority"
      operator: "eq"
      value: "critical"
    action:
      type: "publish"
      channel: "critical_priority_queue"

  - name: "rate_limit_telemetry"
    condition:
      field: "event_type"
      operator: "eq"
      value: "TemperatureNormal"
    action:
      type: "drop"
    rate_limit:
      limit: 1
      per_seconds: 60
      key: "source.id"

  - name: "audit_critical_device_events"
    condition:
      all:
        - field: "kind"
          operator: "eq"
          value: "event"
        - field: "priority"
          operator: "eq"
          value: "critical"
    action:
      type: "audit"
      audit_action: "device.event.received"
```

### 9.5. Dead Letter Queue (DLQ)

События, не прошедшие валидацию или обработку, уходят в коллекцию `dead_letters`:

```json
{
  "id": "uuid",
  "original_event": { ... },
  "reason": "validation_failed",
  "error_details": "payload.weight_kg is required",
  "received_at": "2026-09-16T10:00:00Z"
}
```

Админ через UI просматривает DLQ, анализирует причину, и может выполнить `dlq.replay` для возврата события в основной поток.

---

## 10. РАБОЧЕЕ МЕСТО (Workplace)

**НОВОЕ в v4.0**

### 10.1. Назначение

Логическая или физическая точка, объединяющая контекст действия. Решает проблему идентификации «тупых» устройств и сменных дежурных.

### 10.2. Модель данных

```rust
pub struct Workplace {
    pub id: Uuid,
    pub company_id: Uuid,
    pub code: String,
    pub name: String,
    pub location: Option<String>,
    pub assigned_user_id: Option<Uuid>,
    pub device_ids: Vec<Uuid>,
    pub version: u32,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

### 10.3. Сценарии использования

- **Пост охраны с тревожной кнопкой:** USB-кнопка не умеет аутентифицироваться, но клиент рабочего места знает `workplace_id` и подставляет его в `context`.
- **Касса с подключенным сканером:** Сканер шлёт события с `device_id`, клиент обогащает `workplace_id`.
- **Серверная стойка с датчиками:** Датчики шлют телеметрию, шлюз обогащает `workplace_id` на основе метаданных.

### 10.4. Управление через UI и команды

**НОВОЕ в v4.0**

Workplace хранится в коллекции `workplaces` в SurrealDB и управляется через админский UI (SDUI).

#### Команды управления

- `workplace.create` — создание рабочего места (требует `workplace.manage`)
- `workplace.update` — обновление рабочего места (требует `workplace.manage`)
- `workplace.delete` — удаление рабочего места (требует `workplace.manage`)
- `workplace.list` — список всех рабочих мест (требует `workplace.read`)
- `workplace.get` — получение рабочего места по коду (требует `workplace.read`)
- `workplace.export` — экспорт рабочего места в JSON (требует `workplace.read`)
- `workplace.import` — импорт рабочего места из JSON (требует `workplace.manage`)
- `workplace.export_all` — экспорт всех рабочих мест в JSON-файл (требует `workplace.read`)
- `workplace.import_all` — импорт всех рабочих мест из JSON-файла (требует `workplace.manage`)

#### UI для управления рабочими местами

Администратор через SDUI-интерфейс может:
- Просматривать список всех рабочих мест (каталог)
- Создавать новые рабочие места через форму
- Редактировать существующие рабочие места (привязка устройств, назначение пользователя)
- Экспортировать рабочие места в JSON-файл
- Импортировать рабочие места из JSON-файла

#### Экспорт/импорт

**Экспорт:**
```json
{
  "version": "1.0",
  "exported_at": "2026-09-16T10:00:00Z",
  "workplaces": [
    {
      "code": "security-post-03",
      "name": "Пост охраны №3",
      "location": "Главный вход",
      "assigned_user_id": "uuid-user-123",
      "device_ids": ["uuid-device-456", "uuid-device-789"]
    }
  ]
}
```

**Импорт:**
- Валидация структуры JSON
- Проверка уникальности кодов
- Ensure-семантика: если рабочее место с таким кодом уже существует и версия ниже — обновить, иначе пропустить
- Запись в аудит `workplace.imported`

---

## 11. ВНЕШНИЕ УСТРОЙСТВА И ИНТЕГРАЦИИ

**НОВОЕ в v4.0**

### 11.1. Device (Устройство)

#### Модель данных

```rust
pub struct Device {
    pub id: Uuid,
    pub company_id: Uuid,
    pub code: String,
    pub name: String,
    pub device_type: String, // "sensor", "scanner", "printer", "button"
    pub protocol: String, // "mqtt", "http", "serial", "hid"
    pub auth_method: String, // "api_key", "certificate", "none"
    pub auth_credentials: serde_json::Value, // зашифрованные ключи
    pub workplace_id: Option<Uuid>,
    pub metadata_schema: serde_json::Value, // JSON Schema для телеметрии
    pub version: u32,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

#### Аутентификация

API-ключ в заголовке `X-Device-Key` или сертификат.

#### Управление через UI

Device управляется через SDUI-интерфейс:
- Каталог устройств
- Форма создания/редактирования (с выбором типа, протокола, метода аутентификации)
- Привязка к рабочему месту
- Генерация API-ключа
- Экспорт/импорт устройств

#### Команды управления

- `device.create` — создание устройства (требует `device.manage`)
- `device.update` — обновление устройства (требует `device.manage`)
- `device.delete` — удаление устройства (требует `device.manage`)
- `device.list` — список устройств (требует `device.read`)
- `device.get` — получение устройства по коду (требует `device.read`)
- `device.export` — экспорт устройства в JSON (требует `device.read`)
- `device.import` — импорт устройства из JSON (требует `device.manage`)

#### Экспорт/импорт

Экспорт/импорт работает аналогично правилам маршрутизации (раздел 9.3). При импорте проверяется уникальность кода, ensure-семантика, запись в аудит.

### 11.2. Integration (Внешняя система)

#### Модель данных

```rust
pub struct Integration {
    pub id: Uuid,
    pub company_id: Uuid,
    pub code: String,
    pub name: String,
    pub integration_type: String, // "webhook", "rest_api", "graphql"
    pub auth_method: String, // "hmac", "oauth", "api_key"
    pub auth_credentials: serde_json::Value,
    pub webhook_url: Option<String>,
    pub metadata_schema: serde_json::Value,
    pub version: u32,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

#### Аутентификация

HMAC-подпись в заголовке `X-Signature` или OAuth token.

#### Управление через UI

Integration управляется через SDUI-интерфейс:
- Каталог интеграций
- Форма создания/редактирования
- Настройка webhook URL
- Генерация HMAC-ключа
- Экспорт/импорт интеграций

#### Команды управления

- `integration.create` — создание интеграции (требует `integration.manage`)
- `integration.update` — обновление интеграции (требует `integration.manage`)
- `integration.delete` — удаление интеграции (требует `integration.manage`)
- `integration.list` — список интеграций (требует `integration.read`)
- `integration.get` — получение интеграции по коду (требует `integration.read`)
- `integration.export` — экспорт интеграции в JSON (требует `integration.read`)
- `integration.import` — импорт интеграции из JSON (требует `integration.manage`)

#### Экспорт/импорт

Экспорт/импорт работает аналогично правилам маршрутизации (раздел 9.3). При импорте проверяется уникальность кода, ensure-семантика, запись в аудит.

---

## 12. ВНУТРЕННИЙ ТРАНСПОРТ (EventBus)

**НОВОЕ в v4.0**

### 12.1. Назначение

In-process транспорт на базе Tokio channels для передачи UniversalEvent между компонентами ядра.

### 12.2. Реализация

```rust
pub struct EventBus {
    command_tx: mpsc::Sender<UniversalEvent>,
    event_tx: mpsc::Sender<UniversalEvent>,
    broadcast_tx: broadcast::Sender<UniversalEvent>,
}

impl EventBus {
    // Для команд (идут в CommandProcessor)
    pub async fn send_command(&self, event: UniversalEvent) {
        self.command_tx.send(event).await;
    }
    
    // Для сырых событий (идут напрямую в EventStore)
    pub async fn send_event(&self, event: UniversalEvent) {
        self.event_tx.send(event).await;
    }
    
    // Для broadcast (ServerPush клиентам)
    pub async fn broadcast(&self, event: UniversalEvent) {
        self.broadcast_tx.send(event);
    }
}
```

### 12.3. Почему не очереди (RabbitMQ/Kafka)?

- Внутри ядра — in-process быстрее (нет сетевых вызовов).
- У нас уже есть Tokio runtime.
- Очереди нужны для **внешних интеграций** (если мы хотим подключить RabbitMQ для внешних систем).

---

## 13. КОМАНДЫ И СОБЫТИЯ (EVENT SOURCING + CQRS)

### 13.1. Event Sourcing

События хранятся в коллекции `events`. Это append-only журнал.

Поля события:
- `_id` (UUID)
- `stream_type` (тип потока — см. ниже)
- `stream_id` (ID объекта)
- `event_type` (тип события: `object.created`, `document.posted`, `company.updated`, и т.д.)
- `version` (порядковый номер в потоке)
- `payload` (данные события)
- `metadata` (`actor_user_id`, `actor_login`, `actor_full_name`, `actor_position`, `actor_company_id`, `ip_address`, `trace_id`)
- `company_id`
- `correlation_id` (сквозной ID бизнес-операции)
- `causation_id` (ID события-причины)
- `occurred_at` (UTC)

### 13.2. Типы потоков (StreamType)

Каждая сущность с собственной историей изменений имеет свой тип потока:

| StreamType | Назначение | Примеры событий |
|---|---|---|
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
| `Workplace` | Рабочие места (НОВОЕ в v4.0) | `workplace.created`, `workplace.updated`, `workplace.imported` |
| `Device` | Внешние устройства (НОВОЕ в v4.0) | `device.created`, `device.telemetry_received`, `device.imported` |
| `Integration` | Внешние системы (НОВОЕ в v4.0) | `integration.created`, `integration.webhook_received`, `integration.imported` |
| `RoutingRule` | Правила маршрутизации (НОВОЕ в v4.0) | `routing_rule.created`, `routing_rule.updated`, `routing_rule.imported` |

### 13.3. CQRS

Проекции (Projections) слушают события и обновляют материализованные представления (`objects`, `ledger_balances`). В v0.1 проекции применяются синхронно внутри транзакции SurrealDB.

### 13.4. Снимок исполнителя (Actor Snapshot)

```rust
pub struct ActorSnapshot {
    pub user_id: Option<Uuid>,      // None для системного актора
    pub login: String,
    pub full_name: String,
    pub position: Option<String>,
    pub company_id: Option<Uuid>,
}
```

Системный актор: Для операций, выполняемых системой, используется `ActorSnapshot::system()` с `login: "system"`, `full_name: "Система"`.

### 13.5. Оптимистичная блокировка (OCC)

Каждый объект имеет поле `version` (u64). При обновлении клиент передаёт `expected_version`. Если текущая версия в БД не совпадает с ожидаемой, операция отклоняется с ошибкой `CONFLICT_ERROR`.

---

## 14. АУДИТ ДЕЙСТВИЙ (audit_log)

### 14.1. Event Store и audit_log — две разные подсистемы

- **Event Store:** Бизнес-события (для восстановления состояния, CQRS). Append-only, никогда не удаляются.
- **Audit Log:** Операционные действия (для безопасности, compliance, отладки). Могут архивироваться/удаляться по retention policy.

### 14.2. Структура AuditEntry

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

### 14.3. Что логируется в audit_log

- Аутентификация: `user.login`, `user.login_failed`, `user.logout`.
- Управление модулями: `module.install`, `module.uninstall`, `module.enable`.
- Управление правами: `permission.granted`, `role.created`.
- Системные операции: `system.backup`, `system.migration`.
- Декларативная регистрация: `seed_metadata`, `seed_metadata_skipped`.
- Вызовы команд: `command.executed`, `permission.denied` (через `CommandExecutionPipeline`).
- **События от устройств (НОВОЕ в v4.0):** `device.event.received` (декларативный аудит через Rules Router).
- **Управление правилами маршрутизации (НОВОЕ в v4.0):** `routing_rule.created`, `routing_rule.updated`, `routing_rule.imported`.
- **Управление устройствами (НОВОЕ в v4.0):** `device.created`, `device.updated`, `device.imported`.
- **Управление интеграциями (НОВОЕ в v4.0):** `integration.created`, `integration.updated`, `integration.imported`.
- **Управление рабочими местами (НОВОЕ в v4.0):** `workplace.created`, `workplace.updated`, `workplace.imported`.

### 14.4. Декларативный аудит событий (НОВОЕ в v4.0)

Для сырых событий (от устройств) аудит настраивается через YAML-правила в Rules Router:

```yaml
- name: "audit_critical_device_events"
  condition:
    all:
      - field: "kind"
        operator: "eq"
        value: "event"
      - field: "priority"
        operator: "eq"
        value: "critical"
  action:
    type: "audit"
    audit_action: "device.event.received"
```

**Принцип:** Нет правила — нет аудита. Включил правило для критичных датчиков — пишутся. Для мусорной телеметрии — не пишутся.

---

## 15. ПРАВА ДОСТУПА (RBAC)

### 15.1. Модель

`User` → `Role` → `PermissionPolicy`. Группы не используются. Наследование ролей не используется (вместо него — «создать на основании» / клонирование).

### 15.2. Модели данных

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
    pub version: u32,                    // Версия политики (для OCC)
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
    pub version: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

### 15.3. Алгоритм проверки прав (PermissionManager)

Сервис `PermissionManager` в `core-application` реализует детерминированную проверку:

1. Получить роли пользователя в компании.
2. Собрать все `permission_policy_codes` из этих ролей.
3. Загрузить политики из `AppRegistry.permission_registry`.
4. Фильтрация:
   - `scope_type` покрывает запрошенный модуль/платформу.
   - `entity_type` совпадает или является wildcard (`None`).
   - `actions` содержит запрошенное действие или `"*"`.
5. Сортировка совпавших политик по `priority` (убывание).
6. Deny overrides allow: Если среди совпавших есть политика с `deny: true` и наивысшим приоритетом → доступ запрещён.
7. Если есть политика с `deny: false` и наивысшим приоритетом → доступ разрешён.
8. Deny-by-default: Если ничего не совпало → доступ запрещён.

### 15.4. CommandExecutionPipeline (Middleware)

Чтобы команда в `CommandRegistry` не могла «забыть» проверить права, проверка встроена в конвейер выполнения:

1. **Audit Start:** Записать в `audit_log` действие `command.executed` (статус: `started`).
2. **Permission Check:** `PermissionManager` проверяет `required_permission` из метаданных команды. Если отказ → записать в `audit_log` `permission.denied` и вернуть `PERMISSION_ERROR`.
3. **Execute:** Выполнение бизнес-логики команды.
4. **Audit End:** Обновить запись в `audit_log` (статус: `success` или `failed`).

### 15.5. Системные роли

При инициализации компании автоматически создаются системные роли (`is_system: true`):

- `admin`: Полный доступ ко всем действиям (`All`).
- `staff`: Базовые права на чтение и создание своих документов (`Owned`/`ByCompany`).
- `guest`: Только чтение (`Owned`/`ByCompany`).
- `archived`: Роль для уволенных. Даёт право `read` на объекты, созданные этим пользователем, но запрещает создание новых или изменение чужих.

### 15.6. Экспорт/импорт политик прав (НОВОЕ в v4.0)

Политики прав (`PermissionPolicy`) поддерживают экспорт/импорт для переноса между компаниями или окружениями.

#### Команды

- `permission_policy.export` — экспорт политики в JSON (требует `permission_policy.read`)
- `permission_policy.import` — импорт политики из JSON (требует `permission_policy.manage`)
- `permission_policy.export_all` — экспорт всех политик в JSON-файл (требует `permission_policy.read`)
- `permission_policy.import_all` — импорт всех политик из JSON-файла (требует `permission_policy.manage`)

#### Формат экспорта

```json
{
  "version": "1.0",
  "exported_at": "2026-09-16T10:00:00Z",
  "policies": [
    {
      "code": "invoice.create",
      "name": "Создание счетов",
      "scope_type": { "module": "invoice" },
      "actions": ["create"],
      "record_access": "ByCompany",
      "deny": false,
      "priority": 50
    }
  ]
}
```

#### Импорт

- Валидация структуры JSON
- Проверка уникальности кодов
- Ensure-семантика: если политика с таким кодом уже существует и версия ниже — обновить, иначе пропустить
- Запись в аудит `permission_policy.imported`

---

## 16. МОДУЛЬНАЯ СИСТЕМА (WASM + EXTISM)

### 16.1. Жизненный цикл модуля

- **Установка:** Валидация WASM + `get_info()`. Манифест = единственный источник правды. Декларативная регистрация: хост сам применяет `object_schemas` / `permissions` / `navigation` — код плагина НЕ запускается.
- **Включение компании:** `preload_company_modules` из БД, загрузка и повторная декларативная регистрация (идемпотентно).
- **Выгрузка:** `uninstall`/`disable` → удаление из памяти, регистров и меню.

### 16.2. Манифест `get_info()`

Полный дескриптор v2 включает: `code`, `version`, `display_name`, `api_version`, `capabilities`, `commands`, `permissions`, `object_schemas`, `print_templates`, `scripts`, `metadata_version`, `handles_documents`, `navigation`, `demo`, `settings_schema`, `dependencies`.

### 16.3. Декларативная регистрация (ensure-семантика)

При `install`/`enable`/`preload` хост применяет манифест сам. `permissions` → политики `permission_policies` по кодам (`scope` из манифеста). Каждое применение метаданных пишется в аудит (`seed_metadata`, актор «система/модуль»).

### 16.4. Трёхслойная архитектура

- **Ядро:** Базовые сущности, метаданные, безопасность (`permission_policy`, signing), инфраструктура (`tx`, `events`, `audit`, `plugin_manager`).
- **Базовые модули:** `core.nomenclature`, `core.counterparty`, `core.chart_of_accounts`.
- **Прикладные модули:** `plugin.stock`, `plugin.trade`, `plugin.accounting`, `plugin.requests`.

### 16.5. Ресурсные лимиты

- Топливо: 10 000 000 инструкций.
- Память: 256 страниц (~16 МБ).
- Таймаут: 10 с внутри плагина, 30 с на весь `plugin_call`.
- Доступ к ФС/сети: нет.

---

## 17. ТРАНСПОРТНЫЙ СЛОЙ (RPCMESSAGE)

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

## 18. КЛИЕНТСКАЯ ЧАСТЬ (FLUTTER + SDUI)

### 18.1. Server-Driven UI (SDUI)

Клиент использует паттерн SDUI. Универсальный движок рендерит каталоги и формы на лету на основе `object_schemas` и `forms` из манифеста модуля.

### 18.2. Реестр виджетов

Превращает JSON-описание в нативный виджет.

### 18.3. Кастомизация

Если универсального виджета недостаточно, регистрируется кастомный виджет под кодом (например, `production.gantt_chart`).

### 18.4. Оффлайн-режим

Локальная БД (Isar/Hive). При изменении данных клиент записывает событие локально. При наличии сети — отправляет на сервер (`EventBatch`).

---

## 19. ОФФЛАЙН-СИНХРОНИЗАЦИЯ И РАЗРЕШЕНИЕ КОНФЛИКТОВ

### 19.1. Optimistic Concurrency Control (OCC)

Клиент передаёт `expected_version`. Сервер сверяет версию с текущей в БД. При несовпадении сервер немедленно прерывает транзакцию и возвращает `RpcMessage::Error` с кодом `CONFLICT_ERROR` и деталями (`actual_version`, `modified_by`, `modified_at`).

Автоматического мерджа нет. Пользователь должен вручную разрешить конфликт.

---

## 20. СКРИПТОВЫЙ ДВИЖОК RHAI

Типы скриптов: `formula`, `validator`, `before_action`, `after_action`, `report`, `event_handler`.

Ограничения: таймаут, лимит операций, запрет файловой системы, запрет сети по умолчанию, доступ только через capability API.

Ядро предоставляет набор быстрых, скомпилированных Rust-функций (Core API) для Rhai.

---

## 21. КРИПТОПОДПИСЬ

Основной провайдер первой очереди: КриптоПро CSP на Linux через библиотеку `cpcsp-rs`.

Платформа не является СКЗИ, не хранит приватные ключи. Подписывается канонический снимок версии объекта.

При включённой политике подписываются: `create non-draft`, `update non-draft`, `post`, `cancel`, `restore_version`. Для черновиков подпись не обязательна.

---

## 22. УВЕДОМЛЕНИЯ, ЭКСПОРТ, ПЕЧАТЬ

- **Уведомления:** Первая очередь: inapp, email (SMTP).
- **Экспорт:** Основной формат: CSV (UTF-8, разделитель `;` по умолчанию).
- **Печатные формы:** Формируются как HTML-страницы со встроенными стилями. Печатная форма всегда светлая.

---

## 23. НАБЛЮДАЕМОСТЬ (OBSERVABILITY)

**НОВОЕ в v4.0**

### 23.1. Structured Logging

JSON-формат логов с `trace_id`, `event_type`, `rule_matched`:

```json
{
  "timestamp": "2026-09-16T10:00:00Z",
  "level": "info",
  "trace_id": "abc-123-def",
  "event_type": "DocumentCreated",
  "source": "user-42",
  "rule_matched": "validate_document",
  "action": "publish",
  "duration_ms": 2,
  "result": "success"
}
```

### 23.2. Metrics

Ключевые метрики:
- `rules_engine_evaluations_total{rule_name="..."}` — сколько раз правило оценивалось
- `rules_engine_matches_total{rule_name="..."}` — сколько раз сработало
- `rules_engine_dlq_total{reason="..."}` — сколько ушло в DLQ
- `rules_engine_latency_seconds` — гистограмма времени обработки
- `rules_engine_rate_limit_hits_total{rule_name="..."}` — сколько сработал rate limit

**Инструменты:** Prometheus + Grafana.

### 23.3. Distributed Tracing

Каждое событие получает `trace_id`, который передаётся через все слои (Rules Engine → Rhai → WASM → БД).

**Инструменты:** OpenTelemetry + Jaeger/Zipkin.

---

## 24. ЭТАПЫ РАЗРАБОТКИ v0.1

Обоснование порядка: Инфраструктура безопасности (аудит и права) реализуется ДО бизнес-логики (объектов), чтобы все объекты сразу создавались с проверкой прав и записью в аудит.

1. Каркас проекта, подключение к SurrealDB, диагностика. ✅ Done
2. Компании, расширенная модель пользователей, роли. ✅ Done
3. Метаданные (`entity_types`, `fields`, `states`). ✅ Done
4. Аудит действий (`audit_log`) и `AuditRepository`. ✅ Done
5. Права доступа (`permission_policies`), `PermissionManager`, `CommandExecutionPipeline`, системные роли. ✅ Done
6. Объекты, CRUD, оптимистичная блокировка. ✅ Done
7. События, версии, снимки исполнителя. ✅ Done
8. CommandRegistry, AppRegistry, 5 регистров с ensure-семантикой. ✅ Done
9. WASM-модули через Extism, манифест, декларативная регистрация. ✅ Done
10. Транспортный слой (`RpcMessage`), REST + WebSocket. ✅ Done
11. Flutter-клиент, SDUI, тёмная тема. ✅ Done
12. Оффлайн-синхронизация, Optimistic Concurrency Control.
13. Rhai-скрипты, редактор, Core API. ✅ Done
14. Модуль управленческого учёта, проводки, ОСВ, баланс. ✅ Done
15. CSV-экспорт, HTML-печатные формы.
16. Уведомления inapp + e-mail.
17. Криптоподпись через `cpcsp-rs` (Linux).
18. Пакет диагностики, логирование, маскирование ПД.
19. Тесты и документация.

**Новые фазы v4.0 (добавляются после Фазы 15):**

20. Rules Router (интеграция `sis_service` в режиме `minimal`) — RulesEngine как middleware перед CommandProcessor/EventStore.
21. Коллекция `routing_rules` — хранение правил в БД, команды CRUD.
22. UI для управления правилами — SDUI-интерфейс для просмотра/создания/редактирования правил.
23. Экспорт/импорт правил — команды `routing_rule.export`/`routing_rule.import`.
24. Коллекция `workplaces` — хранение рабочих мест в БД, команды CRUD.
25. UI для управления рабочими местами — SDUI-интерфейс для просмотра/создания/редактирования рабочих мест.
26. Экспорт/импорт рабочих мест — команды `workplace.export`/`workplace.import`.
27. Коллекция `devices` — хранение устройств в БД, команды CRUD.
28. UI для управления устройствами — SDUI-интерфейс для просмотра/создания/редактирования устройств.
29. Экспорт/импорт устройств — команды `device.export`/`device.import`.
30. Коллекция `integrations` — хранение интеграций в БД, команды CRUD.
31. UI для управления интеграциями — SDUI-интерфейс для просмотра/создания/редактирования интеграций.
32. Экспорт/импорт интеграций — команды `integration.export`/`integration.import`.
33. Экспорт/импорт политик прав — команды `permission_policy.export`/`permission_policy.import`.
34. EventBus (внутренний транспорт на Tokio channels) — замена прямых вызовов на шину событий.
35. Наблюдаемость (structured logging, метрики, трассировка) — интеграция tracing/metrics/opentelemetry.

---

## 25. КРИТЕРИИ ГОТОВНОСТИ v0.1

Система готова, если:

- Можно подключиться к SurrealDB по URI.
- Можно создать компанию и пользователя с ФИО, контактами и рабочим профилем.
- Можно настроить новый тип документа без программирования.
- Можно создавать и редактировать документы через UI (SDUI).
- Видна история версий, можно восстановить версию.
- Каждое действие попадает в аудит со снимком исполнителя.
- Операционный аудит (`audit_log`) записывает действия пользователей, установку модулей, изменение прав.
- Любая команда без явного права доступа блокируется `CommandExecutionPipeline` с записью `permission.denied` в аудит.
- Работают Rhai-скрипты в песочнице.
- Работает управленческий учёт, проведение создаёт события и проводки.
- ОСВ, журнал проводок, карточка счёта и баланс работают.
- CSV-экспорт работает с выбором разделителя и кодировки.
- Печатные формы открываются и печатаются (всегда светлые).
- Тёмная тема работает в интерфейсе.
- Уведомления inapp и e-mail работают.
- Криптоподпись на Linux работает через `cpcsp-rs`.
- WASM-модули устанавливаются, регистрируют команды, вызываются через `RpcMessage`.
- Оффлайн-синхронизация работает, конфликты возвращают `CONFLICT_ERROR`.
- Пакет диагностики формируется, контакты в нём маскируются.
- Логи и документация на русском языке.
- **Rules Router работает с YAML-правилами, хранящимися в БД.**
- **UniversalEvent поддерживает kind (command/event) и source identity.**
- **Workplace управляются через админский UI с экспортом/импортом.**
- **Device / Integration регистрируются через метаданные и управляются через UI с экспортом/импортом.**
- **Правила маршрутизации управляются через админский UI с экспортом/импортом.**
- **Устройства управляются через админский UI с экспортом/импортом.**
- **Интеграции управляются через админский UI с экспортом/импортом.**
- **Политики прав поддерживают экспорт/импорт.**
- **Наблюдаемость: логи, метрики, трассировка работают.**

---

## 26. ОТЛОЖЕНО

Откладываются:

- Визуальный конструктор форм.
- Marketplace модулей.
- Группы пользователей.
- Пакетное проведение.
- LIFO.
- Полноценная мультивалютность.
- Полевые права.
- CRL/OCSP.
- Telegram / Max / SMS.
- Мобильные клиенты (v0.3).
- Нативные динамические библиотеки (dylib).
- Retention policy для `audit_log` (архивация/удаление старых записей).
- Circuit Breaker и Canary Release для Rules Router (v0.4).

---

## 27. ИТОГ

**Eventra FluxCore** — это не «ещё одна учётная система», а универсальная событийно-ориентированная платформа, где:

- **Всё есть событие (UniversalEvent).** Пользователь, устройство, API, cron — все генерируют события с единым контрактом.
- **Метаданные управляют всем.** UI (SDUI), маршрутизация (Rules Router), бизнес-логика (WASM/Rhai).
- **Rules Router обеспечивает гибкую маршрутизацию и защиту.** Декларативные YAML-правила, хранящиеся в БД, управляемые через UI с экспортом/импортом.
- **Workplace + Source Identity дают полный контекст действия.** Каждое событие имеет источник, рабочее место, метод ввода.
- **Device / Integration / Workplace / Permission управляются декларативно.** Все конфигурационные сущности хранятся в БД, управляются через админский UI, поддерживают экспорт/импорт.
- **Модульность через WASM-плагины с изоляцией.** Каждый плагин изолирован, падение одного не роняет ядро.
- **Наблюдаемость на всех уровнях.** Логи, метрики, трассировка.

---

## ПРИЛОЖЕНИЯ

### Приложение №1. CommandRegistry

Динамический реестр команд с `tokio::sync::RwLock`, поддерживающий `register`/`unregister`/`execute`/`list`/`remove_by_prefix`.

### Приложение №2. AppRegistry

Группирующая структура из 5 регистров с ensure-семантикой.

### Приложение №3. DependencySpec

Спецификация зависимостей модулей с semver-проверкой.

### Приложение №4. Host-функции (26) и Capabilities

Полный список host-fn и правил минимизации.

### Приложение №5. Конверт host-функций и Оркестрация документов

`unwrap_host`, коды ошибок, `handles_documents`, `$ref`-связывание.

### Приложение №6. Модель аудита действий (audit_log)

Структура `AuditEntry`, `AuditTarget`, `AuditResult`, `AuditFilter`, `AuditRepository`.

### Приложение №7. Модель прав доступа (Permissions)

Структура `PermissionPolicy`, `Role`, алгоритм `PermissionManager::check_access`, `CommandExecutionPipeline`. Поддерживается экспорт/импорт политик прав (команды `permission_policy.export`/`import`).

### Приложение №8. Модель Workplace (НОВОЕ в v4.0)

Структура `Workplace`, сценарии использования, SDUI-рендеринг. Команды управления: `workplace.create`/`update`/`delete`/`list`/`get`/`export`/`import`. Поддерживается экспорт/импорт рабочих мест.

### Приложение №9. Модель Device (НОВОЕ в v4.0)

Структура `Device`, аутентификация, мета-модель телеметрии, команды управления, экспорт/импорт.

### Приложение №10. Модель Integration (НОВОЕ в v4.0)

Структура `Integration`, аутентификация, webhook-приёмник, команды управления, экспорт/импорт.

### Приложение №11. Модель RoutingRule (НОВОЕ в v4.0)

Структура `RoutingRule`, формат YAML-правил, операторы, действия, rate limiting, DLQ, команды управления, экспорт/импорт.

### Приложение №12. Наблюдаемость (НОВОЕ в v4.0)

Structured logging, метрики, трассировка, инструменты (Prometheus, Grafana, OpenTelemetry, Jaeger).

---

**Конец документа.**