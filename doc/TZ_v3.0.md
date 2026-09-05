# 2C Platform / 2С Техническое задание v3.0

**Статус:** Рабочая архитектурная спецификация v3.0 (Living Document)  
**Дата:** 05.09.2026  
**Владелец:** Миха Алексеев  
**Основание:** Исходное ТЗ v2.2, обсуждение архитектуры, решения по модулям, событиям, транспорту и клиенту.

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
6. Полный аудит, версионирование, криптоподпись.
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
   - **Доска (Projections)** — это текущее состояние. Материализованные представления (коллекция objects, остатки, ОСВ), которые строятся на основе событий.
5. **Команды и События:**
   - **Команда (Command)** — это намерение (например, «Провести документ»). Она может быть отклонена. В базу не пишется.
   - **Событие (Event)** — это свершившийся факт (например, «Документ проведён»). Пишется в базу навсегда.
6. **Модульность через WASM:**
   - Все предметные модули — это WASM-плагины, исполняемые через Extism.
   - Модули регистрируют команды, метаданные, права декларативно.
   - Хост (ядро) обеспечивает изоляцию, безопасность, ресурсные лимиты.
7. **Физическое удаление бизнес-объектов и пользователей с историей запрещено.**
8. **Гибкость управляемая, не анархичная.**

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
21. `audit_log`
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

1. **users (Учётная запись).** Хранит логин, password_hash, статус (invited, active, disabled, locked, archived), role_ids, параметры безопасности (failed_login_count, locked_until, must_change_password), locale, timezone.
2. **persons (Персона).** Хранит last_name, first_name, middle_name, display_name. Отображается в интерфейсе, аудите и печатных формах.
3. **user_contacts (Контактные каналы).** Хранит несколько e-mail, телефонов, telegram. Поля: channel_type, value, is_primary, is_verified, purposes (login, notifications, recovery).
4. **user_company_profiles (Рабочие профили).** Хранит привязку к компаниям. Поля: company_id, employee_number, position, department, is_primary, is_active, valid_from, valid_to.
5. **user_certificates (Сертификаты).** Хранит связь с сертификатами КриптоПро. Поля: provider_code, certificate_ref, subject, issuer, fingerprint, is_active.

В событиях и аудите сохраняется снимок исполнителя (actor_login, actor_full_name, actor_position, actor_company_id), чтобы история оставалась читаемой даже при смене фамилии или увольнении.

---

## 7. МЕТАДАННЫЕ И ОБЪЕКТЫ

**Тип сущности** описывается в `entity_types`. Виды: document, catalog, register, task, contract, project, setting, custom.

**Поля** описываются в `entity_fields`. Типы: string, text, integer, money, date, datetime, boolean, enum, reference, array, table, json, file, user, company, formula, computed.

**Состояния и переходы** описываются в `entity_states` и `entity_transitions`.

**Универсальная коллекция `objects`** хранит все сущности. Поля: _id, entity_type, kind, company_id, state, data, computed, number, date, parent_id, version, created_by, updated_by, created_at, updated_at.

Документ — это объект с kind = document. Высоконагруженные модули могут использовать собственные коллекции (dedicated storage).

**Нумерация документов** уникальна в пределах типа и компании. Номер присваивается атомарно при проведении. Сброс нумерации делает администратор.

---

## 8. КОМАНДЫ И СОБЫТИЯ (EVENT SOURCING + CQRS)

### Event Sourcing
События хранятся в коллекции `events`. Это append-only журнал.

**Поля события:**
1. `_id` (UUID)
2. `stream_type` (тип потока: object, user, person, user_contact, user_profile, user_cert, company, role, module)
3. `stream_id` (ID объекта)
4. `event_type` (тип события: object.created, document.posted)
5. `version` (порядковый номер в потоке)
6. `payload` (данные события)
7. `metadata` (actor_user_id, actor_login, actor_full_name, ip_address)
8. `company_id`
9. `correlation_id` (сквозной ID бизнес-операции)
10. `causation_id` (ID события-причины)
11. `occurred_at` (UTC)

**Индексы для events:**
1. `{ stream_type, stream_id, version }`
2. `{ event_type, occurred_at }`
3. `{ company_id, occurred_at }`
4. `{ correlation_id }`

### CQRS
**Проекции (Projections)** слушают события и обновляют материализованные данные (objects, ledger_balances). В v0.1 проекции применяются синхронно внутри транзакции SurrealDB.

---

## 9. МОДУЛЬНАЯ СИСТЕМА (WASM + EXTISM)

### Жизненный цикл модуля
**Установка (админ, ModulesPage):**
- Валидация: полная загрузка WASM + вызов `get_info()`.
- Манифест = единственный источник правды.
- `api_version` сверяется с хостом (`SUPPORTED_API_VERSION "2.0"`); несовпадение = отказ установки.
- Декларативная регистрация: хост сам применяет `object_schemas` / `permissions` / `navigation` — код плагина НЕ запускается.
- Сохранение в БД (`modules` + `company_modules`, `enabled`).
- Байты → локальный кэш `~/.cache/2c-platform/modules/{code}-{sha256:16}.wasm`.
- Модуль сразу загружен в память сессии — перезапуск не нужен.

**Включение компании / старт / смена компании:**
- `preload_company_modules`: мета из БД (без бинарников).
- Загрузка и повторная декларативная регистрация (идемпотентно).
- Динамическая навигация модуля попадает в меню пользователя (RBAC).

**Выгрузка:** `uninstall`/`disable` → удаление из памяти, регистров и меню; повторный `install` того же кода отклоняется (сначала `uninstall`).

Кэш пер-машинный: другая машина при первом старте докачает бинарь один раз, дальше офлайн-устойчива. Обновление модуля = новый хэш = докачивается только он.

### Манифест `get_info()`
Единственная экспортируемая функция без параметров. Вызывается хостом при установке и каждой загрузке. Полный дескриптор v2 (8 ресурсных блоков):

| Поле | Тип | Назначение |
|------|-----|------------|
| `code`, `version`, `display_name` | string | Обязательны |
| `description`, `author` | string? | Метаданные |
| `api_version` | string? | Контракт API; сейчас `"2.0"` |
| `capabilities` | string[] | Технические гранты на host-fn |
| `commands` | Command[] | Команды единого вызова: `{name, label, description, required_permission?, params_schema}` |
| `permissions` | Permission[] | RBAC-политики: `{code, label?, group?, scope?}` |
| `object_schemas` | ObjectSchema[] | Мета-модель сущностей: entity_type, kind, fields, states, transitions, forms, indexes, relations |
| `print_templates` | PrintTemplate[] | Печатные формы модуля |
| `scripts` | Script[] | Rhai-скрипты модуля |
| `metadata_version` | u32 | Версия декларативной схемы (ensure-семантика) |
| `handles_documents` | string[] | Коды entity_type: проведение делегируется модулю |
| `navigation` | Nav[] | Пункты меню: `{code, title, icon, view, permission?, order}` |
| `demo` | Demo? | `{seed_fn}` — демо-данные по запросу |
| `settings_schema` | JSON Schema? | Настройки модуля (UI) |
| `dependencies` | DependencySpec[] | Зависимости от других модулей (код, версия semver, обязательность) |

### Декларативная регистрация (ensure-семантика)
При `install`/`enable`/`preload` хост применяет манифест сам, не запуская код плагина. Каждый блок — с ensure-семантикой:

- `object_schemas` → `entity_types` + `fields/states/transitions/forms` по кодам: сущности/поля нет — создать; есть и `metadata_version` манифеста выше сохранённого — обновить по кодам (новые добавить, изменённые обновить, пользовательские добавки не удалять); версия не выше — не трогать.
- `permissions` → политики `permission_policies` по кодам.
- `print_templates` / `scripts` → регистры хоста по коду.
- `commands` → регистрируются как `plugin.{code}.{name}` в CommandRegistry.

Идемпотентность обязательна: двойной `preload` не создаёт дублей. Каждое применение метаданных пишется в аудит (`seed_metadata`, актор «система/модуль»).

### Трёхслойная архитектура
**Слой 1: Ядро (обязательное, всегда есть)**
- Базовые сущности: company, user, role, person, user_contact, user_profile, user_certificate, settings.
- Метаданные и объекты: objects (CRUD через метамодель), meta (6 типов метамодели).
- Безопасность: permission_policy (deny-by-default), signing (КриптоПро CMS), crypto.
- Инфраструктура: tx (tx_exec), events (Event Store), audit, plugin_manager (26 host-fn), modules, notify, rhai, numbering, messaging, print, devices.
- Справочники (базовые, всегда нужны): currency (валюты, курсы), uom (единицы измерения).

**Слой 2: Базовые модули (поставляются с платформой, технически — WASM-плагины)**
- `core.nomenclature` — номенклатура, категории, штрихкоды, артикулы.
- `core.counterparty` — контрагенты, договоры, банки, реквизиты.
- `core.chart_of_accounts` — план счетов, типы счетов, seed торговли.

**Слой 3: Прикладные модули (зависят от базовых)**
- `plugin.stock` — склад (зависит от core.nomenclature; uom в ядре).
- `plugin.trade` — торговля (зависит от core.nomenclature, core.counterparty; core.chart_of_accounts — опционально).
- `plugin.accounting` — бухгалтерия (зависит от core.chart_of_accounts).
- `plugin.requests` — заявки (зависит от core.counterparty).

### Ресурсные лимиты
Хост принудительно ограничивает каждый модуль:

| Лимит | Значение |
|-------|----------|
| Топливо (инструкции) | 10 000 000 на вызов |
| Память | 256 страниц (~16 МБ) |
| Таймаут вызова | 10 с внутри плагина, 30 с на весь plugin_call |
| Доступ к файловой системе/сети | нет (только host-fn) |

---

## 10. ТРАНСПОРТНЫЙ СЛОЙ (RPCMESSAGE)

### Конверт сообщений
Все сообщения между клиентом и сервером оборачиваются в единый конверт `RpcMessage`:

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RpcMessage {
    /// 1. COMMAND: Клиент просит изменить состояние (Write).
    Command {
        id: String,          // Корреляционный ID (для трекинга и идемпотентности)
        module: String,      // Имя WASM-модуля (напр., "warehouse")
        action: String,      // Имя команды (напр., "post_document")
        payload: Value,      // Данные команды
    },

    /// 2. QUERY: Клиент запрашивает данные (Read).
    Query {
        id: String,
        module: String,
        action: String,      // Напр., "get_document_by_id"
        payload: Value,
    },

    /// 3. EVENT_BATCH: Клиент отправляет события на сервер (для оффлайн-синхронизации).
    EventBatch {
        id: String,
        module: String,
        events: Vec<Value>,  // Массив сырых событий для записи в Event Store
    },

    /// 4. RESPONSE: Сервер успешно обработал Command или Query.
    Response {
        id: String,          // Тот же ID, что пришел в запросе
        payload: Value,      // Результат
    },

    /// 5. ERROR: Сервер не смог обработать запрос.
    Error {
        id: String,
        code: String,        // Машиночитаемый код (напр., "CONFLICT_ERROR")
        message: String,     // Человекочитаемое описание
        details: Option<Value>,
    },

    /// 6. SERVER_PUSH: Сервер сам инициирует отправку сообщения клиенту (WebSocket/SSE).
    ServerPush {
        id: String,          // ID события (для идемпотентности на клиенте)
        module: String,      // От какого модуля пришло
        event_type: String,  // Тип события (напр., "stock_depleted")
        payload: Value,      // Данные события
    },
}
```

### Транспорт
- **REST/HTTP** — базовый обмен (Command/Query/EventBatch).
- **WebSocket/SSE** — ServerPush и тяжёлые EventBatch (если поток большой).

---

## 11. КЛИЕНТСКАЯ ЧАСТЬ (FLUTTER + SDUI)

### Server-Driven UI (SDUI)
Клиент использует паттерн Server-Driven UI. Универсальный движок рендерит каталоги и формы на лету на основе `object_schemas` и `forms` из манифеста модуля.

**Как это работает:**
1. **Реестр виджетов (Widget Factory) на клиенте:** Во Flutter пишется не сами формы, а *движок рендеринга*. Он знает, как превратить JSON-описание в нативный виджет. Например, тип `"string"` с `widget: "text_input"` рендерит `TextField`, а `"enum"` рендерит `DropdownButton`.
2. **Универсальные представления:** Клиент запрашивает у сервера метаданные сущности (`object_schemas`). Сервер отдаёт JSON со списком полей, их типами, правилами валидации и доступными состояниями (`states`/`transitions`).
3. **Динамические формы:** Клиент на лету строит сложные многошаговые формы с валидацией прямо из JSON Schema.
4. **Кастомизация:** Если для какой-то сущности универсального виджета недостаточно, регистрируется кастомный виджет под кодом `production.gantt_chart`, и сервер в метаданных указывает именно его. Но 80% справочников, журналов документов и простых форм закрываются универсальным движком бесплатно.

**Формат `view`:**
- `requests.main` — выделенная страница модуля (компонент фронтенда).
- `generic.catalog:<entity_type>` — универсальная страница каталога (CRUD бесплатно из метамодели).

### Оффлайн-режим
Flutter-клиент умеет работать локально:
- Локальная БД (Isar/Hive) для кэша.
- При изменении данных клиент записывает событие в локальную БД.
- При наличии сети — отправляет на сервер.
- При отсутствии сети — событие остается в локальной БД, ждем восстановления связи.

---

## 12. ОФФЛАЙН-СИНХРОНИЗАЦИЯ И РАЗРЕШЕНИЕ КОНФЛИКТОВ

### Optimistic Concurrency Control (OCC)
В каждом документе (объекте) есть системное поле `version` (или `expected_version`).

**Алгоритм:**
1. Когда Flutter-клиент формирует `EventBatch` или команду `update` после оффлайн-работы, он **обязан** передать `expected_version`, который он запомнил при последней синхронизации.
2. Серверный WASM-модуль при выполнении `tx_begin` или `update_object` сверяет эту версию с текущей в БД.
3. Если версии не совпадают (кто-то уже провел/изменил документ), сервер **немедленно прерывает транзакцию** и возвращает клиенту `RpcMessage::Error` с кодом `CONFLICT_ERROR`.
4. **Payload ошибки** содержит детали: `{ "actual_version": 5, "modified_by": "user_id_123", "modified_at": "2026-09-05T14:30:00Z" }`.
5. Flutter-клиент показывает пользователю понятное окно: *"Документ уже был изменен пользователем X в Y. Ваши изменения не применены. Обновите данные и повторите попытку"*.

**Автоматического мерджа нет.** Пользователь должен вручную разрешить конфликт.

---

## 13. ПРАВА ДОСТУПА

**Модель:** User -> Role -> PermissionPolicy. Группы не используются.

**Уровни контроля:**
1. Доступ к компании.
2. Доступ к модулю/подсистеме.
3. Доступ к типу объекта.
4. Доступ к действию.
5. Доступ к записям (own / assigned / company / all).

**Две независимые оси:**
- **Capabilities модуля** — что модулю технически разрешено вызывать. Статичны, проверяются на каждом host-вызове.
- **RBAC пользователя** — политики из `permissions[]` манифеста создаются хостом при регистрации (ensure по кодам, `scope` из манифеста) и привязываются ролям админом. Определяют, кто может вызывать функции модуля.

Команда плагина несёт `required_permission` — и хост проверяет его при каждом едином вызове и при вызове `plugin.{code}.{name}`. Пользователь без права не получит результат ни через UI, ни напрямую.

---

## 14. МОДУЛЬ УПРАВЛЕНЧЕСКОГО УЧЁТА

Первый предметный модуль. Не является полноценной бухгалтерией РСБУ.

**Сущности:** accounts, ledger_entries, ledger_balances, accounting_periods.

**План счетов:** редактируемый, иерархический, привязан к компании. Типы: Asset, Liability, Equity, Revenue, Expense, OffBalance.

**Проводки** создаются только через API модуля. Скрипты не пишут проводки напрямую, они выражают намерение.

Перед сохранением проверяется: существование счетов, активность счетов, равенство дебета и кредита, права, дата, период.

**Отмена проведения** не удаляет проводки. Используется сторно или обратные записи.

**Обязательные отчёты:** оборотно-сальдовая ведомость, журнал проводок, карточка счёта, баланс.

---

## 15. СКРИПТОВЫЙ ДВИЖОК RHAI

**Типы скриптов:** formula, validator, before_action, after_action, report, event_handler.

**Контекст:** ctx.user, ctx.company, ctx.entity_type, ctx.action, ctx.object, ctx.changes, ctx.settings, ctx.log, ctx.db, ctx.ledger, ctx.emit, ctx.notify.

**Ограничения:** таймаут, лимит операций, запрет файловой системы, запрет сети по умолчанию, доступ только через capability API.

Ядро предоставляет набор быстрых, скомпилированных Rust-функций (Core API) для Rhai. Функции не должны паниковать (возвращают Result). API пакетный для снижения стоимости перехода границы.

**Редактор скриптов:** CodeMirror 6 (или аналог для Flutter), тёмная тема, подсветка синтаксиса.

---

## 16. КРИПТОПОДПИСЬ

**Основной провайдер первой очереди:** КриптоПро CSP на Linux через библиотеку cpcsp-rs.

Платформа не является СКЗИ, не хранит приватные ключи, не управляет токенами. Платформа вызывает безопасный Rust API cpcsp-rs для подписи хэша.

Подписывается канонический снимок версии объекта.

При включённой политике подписываются: create non-draft, update non-draft, post, cancel, restore_version. Для черновиков подпись не обязательна.

Проверка CRL/OCSP в первой очереди не выполняется. Проверяются криптографическая корректность, срок действия и доверенный издатель.

---

## 17. УВЕДОМЛЕНИЯ, ЭКСПОРТ, ПЕЧАТЬ

### Уведомления
1. Первая очередь: inapp, email (SMTP).
2. Архитектура расширяемая через интерфейс NotificationChannel.

### Экспорт
1. Основной формат: CSV.
2. Параметры по умолчанию: кодировка UTF-8, разделитель точка с запятой.
3. В диалоге экспорта можно выбрать разделитель (;, ,, Tab) и кодировку (UTF-8, UTF-8 with BOM, Windows-1251).

### Печатные формы
1. Формируются как HTML-страницы со встроенными стилями.
2. Печатная форма всегда светлая, тёмная тема интерфейса на неё не влияет.
3. Печать выполняется системным диалогом или через внешний браузер (Print-to-PDF).

---

## 18. ЭТАПЫ РАЗРАБОТКИ v0.1

1. Каркас проекта, подключение к SurrealDB, диагностика.
2. Компании, расширенная модель пользователей, роли.
3. Метаданные (entity_types, fields, states).
4. Объекты, CRUD, оптимистичная блокировка.
5. События, версии, аудит, снимки исполнителя.
6. Права доступа (permission_policies).
7. CommandRegistry, AppRegistry, 5 регистров с ensure-семантикой.
8. WASM-модули через Extism, манифест, декларативная регистрация.
9. Транспортный слой (RpcMessage), REST + WebSocket.
10. Flutter-клиент, SDUI, тёмная тема.
11. Оффлайн-синхронизация, Optimistic Concurrency Control.
12. Rhai-скрипты, редактор, Core API.
13. Модуль управленческого учёта, проводки, ОСВ, баланс.
14. CSV-экспорт, HTML-печатные формы.
15. Уведомления inapp + e-mail.
16. Криптоподпись через cpcsp-rs (Linux).
17. Пакет диагностики, логирование, маскирование ПД.
18. Тесты и документация.

---

## 19. КРИТЕРИИ ГОТОВНОСТИ v0.1

Система готова, если:
1. Можно подключиться к SurrealDB по URI.
2. Можно создать компанию и пользователя с ФИО, контактами и рабочим профилем.
3. Можно настроить новый тип документа без программирования.
4. Можно создавать и редактировать документы через UI (SDUI).
5. Видна история версий, можно восстановить версию.
6. Каждое действие попадает в аудит со снимком исполнителя.
7. Работают Rhai-скрипты в песочнице.
8. Работает управленческий учёт, проведение создаёт события и проводки.
9. ОСВ, журнал проводок, карточка счёта и баланс работают.
10. CSV-экспорт работает с выбором разделителя и кодировки.
11. Печатные формы открываются и печатаются (всегда светлые).
12. Тёмная тема работает в интерфейсе.
13. Уведомления inapp и e-mail работают.
14. Криптоподпись на Linux работает через cpcsp-rs.
15. WASM-модули устанавливаются, регистрируют команды, вызываются через RpcMessage.
16. Оффлайн-синхронизация работает, конфликты возвращают CONFLICT_ERROR.
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

---

## 21. ИТОГ

2C Platform — это не «ещё одна самописная бухгалтерия», а конфигурируемая платформа, где:
1. Ядро нейтрально и работает по принципу «Труба и Доска».
2. Документы являются частным случаем объектов.
3. Метаданные позволяют настраивать сущности и генерировать UI (SDUI).
4. События дают историю и основу интеграций (Event Sourcing + CQRS).
5. Модульность через WASM-плагины с изоляцией и capability-моделью.
6. Криптоподпись реализована через нативную библиотеку cpcsp-rs.
7. Frontend кроссплатформенный и современный (Flutter + Dart).
8. Оффлайн-работа с Optimistic Concurrency Control.
9. Первая очередь реалистична и ограничена по объёму.

---

# ПРИЛОЖЕНИЯ

---

## Приложение №1. CommandRegistry

### Назначение
Динамический реестр команд, заменяющий статическую регистрацию. Поддерживает регистрацию/удаление команд в рантайме (включая WASM-модули).

### Структура
```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;

type CommandHandler = Arc<dyn Fn(Value) -> Pin<Box<dyn Future<Output = Result<Value, String>> + Send>> + Send + Sync>;

pub struct CommandRegistry {
    handlers: RwLock<HashMap<String, CommandHandler>>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            handlers: RwLock::new(HashMap::new()),
        }
    }

    pub async fn register<F, Fut>(&self, name: &str, handler: F)
    where
        F: Fn(Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Value, String>> + Send + 'static,
    {
        let mut map = self.handlers.write().await;
        map.insert(name.to_string(), Arc::new(move |params| Box::pin(handler(params))));
    }

    pub async fn unregister(&self, name: &str) {
        let mut map = self.handlers.write().await;
        map.remove(name);
    }

    pub async fn execute(&self, name: &str, params: Value) -> Result<Value, String> {
        let map = self.handlers.read().await;
        match map.get(name) {
            Some(handler) => handler(params).await,
            None => Err(format!("Unknown command: {}", name)),
        }
    }

    pub async fn list(&self) -> Vec<String> {
        let map = self.handlers.read().await;
        map.keys().cloned().collect()
    }
}
```

### Ключевые свойства
1. **Внутренняя синхронизация:** `tokio::sync::RwLock<HashMap<String, CommandHandler>>` — async-совместимый lock для read-heavy workload.
2. **Шаринг:** В AppState хранится как `Arc<CommandRegistry>` — Clone, Send, Sync.
3. **Async-обработчики:** Команды — async-замыкания: `async fn(Value) -> Result<Value, String>`.
4. **Параметры как JSON:** `serde_json::Value`, десериализуются в явные структуры Args на стороне обработчика.

### Почему `tokio::sync::RwLock`, а не `std::sync::Mutex`
- `std::sync::Mutex` блокирует поток — в async-среде это может заблокировать executor.
- `tokio::sync::RwLock` — async-совместимый, позволяет множественным читателям параллельно выполнять команды.
- Read-heavy workload (много execute, мало register) — RwLock идеален.

---

## Приложение №2. AppRegistry

### Назначение
Группирующая структура, содержащая 5 регистров. Упрощает API, инкапсулирует логику ensure-семантики.

### Структура
```rust
use std::sync::Arc;

pub struct AppRegistry {
    pub commands: Arc<CommandRegistry>,
    pub permissions: Arc<PermissionRegistry>,
    pub object_schemas: Arc<ObjectSchemaRegistry>,
    pub print_templates: Arc<PrintTemplateRegistry>,
    pub scripts: Arc<ScriptRegistry>,
}

impl AppRegistry {
    pub fn new() -> Self {
        Self {
            commands: Arc::new(CommandRegistry::new()),
            permissions: Arc::new(PermissionRegistry::new()),
            object_schemas: Arc::new(ObjectSchemaRegistry::new()),
            print_templates: Arc::new(PrintTemplateRegistry::new()),
            scripts: Arc::new(ScriptRegistry::new()),
        }
    }

    pub async fn register_module(&self, manifest: &ModuleManifest) -> Result<()> {
        // ensure-семантика для всех 5 регистров
        self.commands.ensure_commands(&manifest.commands).await?;
        self.permissions.ensure_permissions(&manifest.permissions).await?;
        self.object_schemas.ensure_schemas(&manifest.object_schemas).await?;
        self.print_templates.ensure_templates(&manifest.print_templates).await?;
        self.scripts.ensure_scripts(&manifest.scripts).await?;
        Ok(())
    }

    pub async fn unregister_module(&self, code: &str) -> Result<()> {
        // удаляет из всех 5 регистров
        self.commands.remove_module_commands(code).await?;
        self.permissions.remove_module_permissions(code).await?;
        self.object_schemas.remove_module_schemas(code).await?;
        self.print_templates.remove_module_templates(code).await?;
        self.scripts.remove_module_scripts(code).await?;
        Ok(())
    }
}

pub struct AppState {
    pub registry: Arc<AppRegistry>,
    pub db: Arc<SurrealClient>,
    pub config: Arc<AppConfig>,
    pub event_bus: Arc<EventBus>,
    // ... другие поля
}
```

### Преимущества
1. **Семантическая группировка:** Все 5 регистров — логически единая группа.
2. **Упрощение API:** Вместо передачи 5 параметров — один `Arc<AppRegistry>`.
3. **Инкапсуляция логики:** Хелпер-методы `register_module` / `unregister_module` инкапсулируют всю логику ensure-семантики.
4. **Расширяемость:** Если понадобится 6-й регистр — добавляем поле в `AppRegistry`, а не плодим поля в `AppState`.
5. **Тестирование:** Можно создать мок `AppRegistry` целиком для тестов.

---

## Приложение №3. DependencySpec

### Назначение
Спецификация зависимости модуля от других модулей. Проверяется при install/uninstall.

### Структура
```rust
pub struct DependencySpec {
    pub code: String,              // Код модуля (например, "core.nomenclature")
    pub version: String,           // Требуемая версия (semver: "^1.0.0")
    pub required: bool,            // Обязательность
    pub description: Option<String>, // Описание для UI
}
```

### Проверка при install
```rust
pub async fn check_dependencies_on_install(
    manifest: &ModuleManifest,
    installed_modules: &[ModuleInfo],
) -> Result<(), DependencyError> {
    for dep in &manifest.dependencies {
        let found = installed_modules.iter().find(|m| m.code == dep.code);
        
        match found {
            Some(module) => {
                // Проверяем версию через semver
                let req = VersionReq::parse(&dep.version)?;
                let ver = Version::parse(&module.version)?;
                
                if !req.matches(&ver) {
                    return Err(DependencyError::VersionMismatch {
                        required: dep.version.clone(),
                        actual: module.version.clone(),
                        module: dep.code.clone(),
                    });
                }
            }
            None => {
                if dep.required {
                    return Err(DependencyError::MissingRequired {
                        module: dep.code.clone(),
                    });
                }
            }
        }
    }
    Ok(())
}
```

### Проверка при uninstall
```rust
pub async fn check_dependencies_on_uninstall(
    module_code: &str,
    installed_modules: &[ModuleInfo],
) -> Result<(), DependencyError> {
    let dependents: Vec<_> = installed_modules
        .iter()
        .filter(|m| m.dependencies.iter().any(|d| d.code == module_code && d.required))
        .map(|m| m.code.clone())
        .collect();
    
    if !dependents.is_empty() {
        return Err(DependencyError::HasDependents {
            module: module_code.to_string(),
            dependents,
        });
    }
    Ok(())
}
```

### UI для зависимостей
- На странице установки модуля показывается список зависимостей и их статус.
- На странице списка модулей показывается количество зависимостей и зависимых модулей.
- Выделяются обязательные/опциональные зависимости.

---

## Приложение №4. Host-функции (26)

Доступны как импорты в namespace `ExtismHost`. Перед каждой — проверка capability модуля (`CAPABILITY_DENIED` при отсутствии гранта).

### Объекты (Доски)
| Функция | Сигнатура | Capability |
|---------|-----------|------------|
| create_object | (entity_type_id, data_json) → {id} | objects.create |
| list_objects | (entity_type_id, limit) → {objects[], total_count} | objects.read |
| get_object | (id) → объект {id,number,date,state,version,data,…} | objects.read |
| update_object | (id, data_json, version) → {id, version} | objects.update |
| transition_object | (id, action, params_json) — post/cancel через движок | objects.update |
| stock_doc_cost | (doc_id) → себестоимость списаний документа по строкам | objects.read |

### Метаданные
| Функция | Сигнатура | Capability |
|---------|-----------|------------|
| get_entity_type | (id) → {id,code,name,kind} | metadata.read |
| list_entity_fields | (entity_type_id) → {fields[]} | metadata.read |

### KV-хранилище модуля (изолированное по коду модуля)
| Функция | Сигнатура | Capability |
|---------|-----------|------------|
| kv_put / kv_put_if_absent | (key, value) | storage |
| kv_get / kv_list / kv_delete | (key) / (prefix) / (key) | storage |

### Workflow и события
| Функция | Сигнатура | Capability |
|---------|-----------|------------|
| run_script | (source_rhai, ctx_json) — Rhai в песочнице | scripts |
| notify_user | (recipient_user_id, subject, body) | notifications |
| users_by_role | (role_id) → список пользователей | notifications |
| emit_event | (stream_id, event_type, payload_json) — в Event Store | events.emit |

### Контекст и сервис
| Функция | Сигнатура | Capability |
|---------|-----------|------------|
| whoami | () → {user_id, login, display_name, role_id, role_ids[]} | — |
| now_ms | () → unix-миллисекунды хоста | — |
| module_settings | () → настройки модуля для компании | — |
| log_message | (msg) — структурированный лог [Module:{code}] | logging |

### Подпись (КриптоПро)
| Функция | Сигнатура | Capability |
|---------|-----------|------------|
| signature_required | (module_code, action, object_id) → политика | signature |
| cms_verify | (data_b64, sig_b64) — серверная проверка CMS ГОСТ | signature |

### Транзакции (tx_exec для оркестраторов)
| Функция | Сигнатура | Capability |
|---------|-----------|------------|
| tx_begin | (business_key) → {handle} — идемпотентный ключ пачки | transactions |
| tx_add_op | (handle, op, params_json) → {op_id} | transactions |
| tx_commit | (handle) — атомарный коммит всех операций | transactions |

**Операции `tx_add_op`:** `object.post/cancel`, `stock.receipt/issue/transfer/handover/handover_return/count/balances/reverse`, `accounting.post/reverse_by_doc`, `test.noop`.

**`$ref`-связывание:** Параметры операции могут ссылаться на результат предыдущей: `{"$ref": "op_id.path.to.field"}` — разрешается перед исполнением. Так COGS проводка торговли берёт себестоимость прямо из результата `stock.issue`: `{"$ref": "{issue_op}.total_cost"}`.

---

## Приложение №5. Capabilities

Гранты перечисляет сам модуль в манифесте; неизвестная capability = отказ установки. Полный набор:

- `objects.create`
- `objects.read`
- `objects.update`
- `objects.delete`
- `metadata.read`
- `events.emit`
- `numbering.next`
- `logging`
- `notifications`
- `storage`
- `scripts`
- `transactions`
- `signature`

**Правило минимизации:** Запрашивать только используемые host-fn.

---

## Приложение №6. Конверт host-функций

Любой вызов хоста возвращает строку-конверт:
```json
{ "ok": true,  "data": ... }
{ "ok": false, "error": { "code": "...", "message": "..." } }
```

**Эталонная развёртка (Rust):**
```rust
fn unwrap_host(raw: String) -> anyhow::Result<serde_json::Value> {
    let v: serde_json::Value = serde_json::from_str(&raw)?;
    if v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false) {
        Ok(v.get("data").cloned().unwrap_or(serde_json::Value::Null))
    } else {
        let code = v["error"]["code"].as_str().unwrap_or("UNKNOWN");
        let msg  = v["error"]["message"].as_str().unwrap_or("");
        Err(anyhow::anyhow!("{code}: {msg}"))
    }
}
```

**Коды ошибок хоста:** `NO_DATABASE, NO_COMPANY, INVALID_COMPANY, NO_USER, INVALID_USER, NO_MODULE_CODE, INVALID_UUID, INVALID_JSON, INVALID_VERSION, INVALID_ACTION, NOT_FOUND, DB_ERROR, SCRIPT_FAILED, CAPABILITY_DENIED, CONFLICT_ERROR`.

**Исключения без конверта** — простые сервисные функции с голым значением: `now_ms()` → `"1732…"` (строка мс), `log_message(msg)` → пустая строка. Все остальные (данные, KV, транзакции, скрипты…) соблюдают конверт.

---

## Приложение №7. Оркестрация документов (`handles_documents`)

Если код документа из `handles_documents`, то `post_object`/`cancel_object` делегируются функциям `on_post` / `on_cancel` модуля вместо стандартного перехода.

**Контракт:**
```rust
#[derive(Deserialize)]
pub struct PostInput {
    pub id: String,                      // UUID объекта
    pub expected_version: Option<i64>,   // оптимистичная блокировка
}
// on_post:  tx_begin → ops(stock/accounting/object.post) → tx_commit
// on_cancel: tx_begin → stock.reverse + accounting.reverse_by_doc + object.cancel
```

Если `handles_documents` не заполнен, хост берёт коды из `object_schemas[].entity_type`.

**Паттерн атомарности:** Все изменения документа, склада и проводок — одна транзакция `tx_exec`; конкурентный конфликт версии → результат «победителя» (идемпотентность по business_key `post-{id}` / `cancel-{id}`).

---

**Конец документа.**