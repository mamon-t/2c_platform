# 2C Platform — Technical Report

**Дата:** 25.08.2026 · **Версия платформы:** 0.1.0 · **Состояние:** рабочий прототип, все ключевые подсистемы работают

Назначение документа — точный снимок того, что реализовано и работает на текущий момент, для сверки с ТЗ v2.2/v2.3. Составлен по коду (не по планам): имена файлов, команд, коллекций и цифры взяты из репозитория.

---

## 1. Обзор

| Показатель | Значение |
|---|---|
| Backend | Rust (edition 2021), Tauri v2, Tokio, MongoDB 3.x driver |
| Backend модулей | **30** в `src-tauri/src/` (7 публичных) |
| IPC-команд (Tauri) | **125** зарегистрировано в `invoke_handler` |
| Host-функций для WASM | **25** |
| MongoDB-коллекций | **33** |
| Capabilities плагинов | **13** |
| Seed-политик RBAC | **58** (18 подсистем) |
| Вариантов аудита | **52** (`AuditableAction`) |
| Фронтенд | Svelte 5 (runes) + TS, ~5 700 строк; **15 компонентов**, 109 API-методов |
| Тестов | **53** функции (unit + интеграционные на живой БД) |

Стек: Rust/Tokio/MongoDB(replSet)/Tauri2 · Svelte5+TS+Vite8+Tailwind4+Skeleton(nosh) · Extism/wasmtime · Rhai · Argon2id+JWT · КриптоПро CSP 5.0 (cpcsp-rs).

Архитектура — «Труба и Доски»: неизменяемая лента событий (Event Store) + материализованные объекты (универсальная коллекция `objects` через метамодель). Гибридная модульность: ядро нейтрально к предметной области; тяжёлые инварианты — нативные Rust-модули; оркестрация — WASM-плагины поверх механизма транзакционных пачек `tx_exec`.

---

## 2. Backend

### 2.1 Карта модулей

| Модуль | Назначение |
|---|---|
| `core` (+`middleware`) | Примитивы (`Id/CompanyId/UserId/RoleId`, `PlatformError`), `CommandContext`: PRE(права→record_scope)→EXECUTE→POST(аудит) |
| `actions` | Константы `"subsystem.action"` + матрица `COMMAND_MAP` (команда→право→scope→audit) с тестами целостности |
| `auth` | JWT Claims |
| `db` | Обёртка MongoClient (connect, typed collections) |
| `commands` | Основная масса IPC-обработчиков + `AppState` (db/auth/config/current_user/current_company_id/current_role_id/current_policies/wasm_modules/devices) |
| `company`, `user`, `person`, `role`, `user_contact`, `user_profile`, `user_certificate` | Справочники: компании, пользователи, персоны, роли, контакты, рабочие профили (несколько ролей на компанию), сертификаты ЭЦП |
| `settings` | Настройки компании (`app_settings`; контактные типы; `stock.allow_negative`) |
| `permission_policy` | RBAC: политики, seed 58 шт., deny-by-default `check_access` (приоритет, wildcard `*`, entity_type) |
| `meta` | Метамодель: 6 коллекций (типы/поля/состояния/переходы/формы/действия), 17 FieldKind, 8 EntityKind |
| `objects` | Универсальное хранилище: CRUD, версии/снимки, проведение/отмена, валидация данных по метаполям |
| `events` | Event Store: append-only, `StreamType{Object,User,Module,Device}`, ActorSnapshot, version-in-stream |
| `audit` | Журнал действий: 52 действия, `AuditChanges` (old→new), фильтры, 4 составных индекса |
| `tx` | **Механизм транзакционных пачек** (см. 2.3) |
| `plugin_manager` | Extism-рантайм: 25 host-fn, загрузка с лимитами, dispatch `plugin_call`, KV-хранилище модулей, workflow-fns |
| `modules` | Жизненный цикл WASM-модулей: install/enable/disable/settings per company; манифест = источник правды (capabilities/permissions/handles_documents); авто-seed RBAC при установке |
| `notify` | Outbox in-app уведомлений (`notifications`) |
| `numbering` | Атомарные номера `{prefix}-{entity_code}-{seq}}` per company+type |
| `print` | Шаблоны печатных форм (HTML+Handlebars-подобный синтаксис) + рендер |
| `rhai` | Песочница скриптов (max_ops, без ФС/сети) |
| `signing` | КриптоПро CMS: список серт. MY-store, sign attached/detached, verify, тестовый самоподписанный сертификат |
| `devices` | Внешнее оборудование: сканеры (wedge/serial), весы (regex-протокол из настроек), насос событий |
| `stock` | Движок склада (инварианты) + обработчики tx_exec + политики подписи + seed метаданных |
| `ledger` | Управленческий учёт — модели и фабрики (заготовка, не подключён) |
| `crypto` | Абстракция провайдеров ЭЦП (заготовка под будущий выбор провайдера) |

### 2.2 IPC-команды (125)

Группировка зарегистрированных команд:

| Домен | Команды | Кол-во |
|---|---|---|
| Диагностика/БД | get_diagnostics, connect_db | 2 |
| Авторизация | authenticate, get_me, switch_company, get_my_permissions | 4 |
| Компании | CRUD ×5 | 5 |
| Пользователи | CRUD ×5 | 5 |
| Роли | CRUD ×4 | 4 |
| Персона/Контакты/Профили/Сертификаты | get/update_person; contacts ×4(+types ×2); profiles ×4; certificates ×2 | 14 |
| Политики доступа | list/create/delete | 3 |
| Настройки приложения | get/save_app_config, get/save_contact_types | 4 |
| Аудит | list_audit_logs, get_audit_entry | 2 |
| Event Store | list_events, get_event, list_stream_events | 3 |
| Rhai | validate_rhai_script, execute_rhai_script | 2 |
| Метамодель | CRUD ×6 типов сущностей (30) + validate_entity_transition + execute_entity_action | 32 |
| Объекты | list/get/create/update/post/cancel_object, restore/list_object_versions | 8 |
| Печать | templates ×5 + render | 6 |
| Нумерация | list/get/update_format/reset | 4 |
| WASM | wasm_load/unload/list, plugin_call | 4 |
| Прикладные модули | modules_list/get/install/uninstall/enable/disable/update_settings | 7 |
| Подпись | list_crypto_certificates, sign_document, verify_document_signature, create_test_certificate | 4 |
| Склад | stock_seed_metadata, stock_balances, stock_report_handover, stock_report_overdue, signature_policies_list/upsert/delete, signature_required_for_doc | 8 |
| Устройства | devices_list/get/save/delete/connect/disconnect/test/list_ports/wedge_scan | 9 |
| Уведомления | notifications_list/mark_read | 2 |

### 2.3 tx_exec — механизм транзакционных пачек

Центральный механизм оркестрации (склад, торговля, учёт).

**Пакет:** `TransactionPackage { idempotency_key, required_permission?, operations[], context{company, actor, policies}, created_at, expires_at }`; операция `{op_id, op, params}`.

**Фазы исполнителя:**
1. Валидация структуры → идемпотентный повтор (журнал `tx_journal` по уникальному индексу `(company_id, idempotency_key)` — при повторе возвращается сохранённый результат, txn не открывается) → право на пачку;
2. Открытие Mongo-транзакции;
3. Строго последовательное выполнение: `$ref`-подстановка (`{"$ref":"op_id.path"}`) из результатов предыдущих операций → проверка права обработчика → вызов из реестра (все записи через сессию исполнителя);
4. Запись журнала **внутри той же транзакции** → commit; конкурентный дубликат (E11000) → откат проигравшего и возврат результата победителя;
5. Ошибка → откат + `TxError{message, failed_op}`.

Лимиты: ≤100 операций, таймаут 30 c. TransientTransactionError ретраится ×3 (безопасно благодаря идемпотентности). Аудит `execute_transaction` после коммита.

**Реестр операций (11):** `test.noop`, `object.post`, `object.cancel`, `stock.receipt`, `stock.issue`, `stock.transfer`, `stock.handover`, `stock.handover_return`, `stock.count`, `stock.balances`, `stock.reverse`. Каждый обработчик объявляет свой `subsystem.action` — второй уровень проверки прав.

Для WASM-плагинов — сессия-строитель: `tx_begin(business_key)→handle`, `tx_add_op(handle, op, params)→op_id` (op_id раздаёт хост), `tx_commit(handle)`; Mongo-txn открывается только в commit; брошенные сессии чистятся (TTL 10 мин). Права на момент коммита = объединение политик всех активных ролей пользователя в компании.

### 2.4 Plugin SDK (WASM)

- Рантайм Extism: timeout 10 c / fuel 10M / memory 256 стр.; каждый host-call проходит capability-check → `{ok,data|error{code,message}}`.
- Манифест модуля (`get_info()`): code, name, version, api_version, author, description, capabilities[], permissions[] ("subsystem.action" → авто-seed RBAC при install), handles_documents[], functions[]. Хост ничего не хардкодит.
- Делегирование проведения: если entity_type документа входит в `handles_documents` включённого модуля, команды `post_object/cancel_object` вызывают экспортируемые `on_post/on_cancel` — плагин собирает пачку (складские операции + object.post/cancel) и всё атомарно.

### 2.5 Заявки (WASM `requests`, референс SDK)

Полный жизненный цикл согласования: маршруты (этапы user/role, `requires_signature` на маршруте), submit→этапы→approve/reject→completed, cancel инициатором. Серверная верификация CMS-подписей (cms_verify против каноничных строк `requests.submit|id|version|state` и `requests.decide|id|decision|comment`), слепок payload целиком + sha256 + сертификат подписанта фиксируются в шаге. Хуки Rhai из настроек модуля: before_submit (strict) / after_approve / on_reject / on_complete. События lifecycle → Труба (`emit_event`). Уведомления (user и role-рассылка через users_by_role). Гонка submit разрешена kv_put_if_absent. Мультироли утверждающего (пересечение ролей профиля).

### 2.6 Склад

**Движок (нативно, session-aware)**: receipt / issue_fifo (FIFO по receipt_date, атомарный условный декремент партии, наборы раскладываются в компоненты, услуги пропускаются) / transfer (цена и дата партии переезжают; handover-вариант фиксирует ответственного и срок возврата, себестоимость не списывается) / count (излишек/недостача) / balances / reverse_document (строгое сторно: расходные возвращают в ту же партию, приходные удаляются только нетронутые; движения помечаются reversed). Отрицательные остатки запрещены по умолчанию (настройка компании `app_settings.stock.allow_negative`), ошибка «Недостаточно X: нужно N, есть M».

Коллекции: `stock_movements` (лента, 8 видов движений, actor), `stock_batches` (FIFO-партии, частичный индекс живых), `stock_balances` (unique company+location+nomenclature). Индексы: карточка товара, локация, doc_id, подотчёт (responsible_user_id+expected_return_date).

**Оркестратор** — WASM `stock`: манифест `handled_documents=[MOVE,COUNT,HANDOVER,HANDOVER_RETURN]`; on_post строит пачку [складская операция по строкам → object.post] одной транзакцией; on_cancel — [stock.reverse → object.cancel]. Проведение присваивает номер нумерацией в той же транзакции.

**Подотчёт**: «что у кого на руках» (остатки custodian-локаций + данные выдачи), просроченные возвраты. Напоминания — существующий механизм хуков/уведомлений.

### 2.7 Криптоподпись

- `signing`: список сертификатов MY-хранилища, sign (attached/detached, ГОСТ Р 34.11-2012_256), verify, verify_detached; генерация тестового самоподписанного сертификата (settings.manage).
- **Политики подписи** (`signature_policies`): {module, action, condition, required}; condition v0.1 — `{"nomenclature_category": X}` (применимо, если строка документа ведёт к номенклатуре категории X); default OFF. Оценка: хост-fn `signature_required` + IPC `signature_required_for_doc`. Интегрировано в HANDOVER склада и заявки.

### 2.8 Устройства

Сканеры: keyboard-wedge (фронт, поле-локально) и Serial (tokio-serial, человекочитаемые ошибки Linux — dialout/занят/нет порта). Весы: regex-протокол из настроек устройства, стабильность показаний. Насос: mpsc → Event Store (StreamType::Device, системный actor) → опциональный Rhai scan_handler из настроек устройства → Tauri-push «device-event» в UI. FiscalPrinter/LabelPrinter — задел (v0.3).

### 2.9 RBAC и аудит

- Deny-by-default; приоритет deny>allow; wildcard actions; entity_type wildcard; record_scope («company»/«own») проверяется middleware для Scope::Object.
- Seed 58 политик / 18 подсистем: platform, companies, users, roles, contacts, documents, metadata, catalogs, reports, scripts, audit, settings, print, plugins, numbering, modules, devices, stock.
- Аудит: 52 действия с label/icon/target_type; AuditEntry (11 полей incl. signature_ref); 4 индекса; запись warn-and-forget в POST-фазе middleware и после tx-коммитов.

---

## 3. Frontend (Svelte 5)

### 3.1 Каркас

- `App.svelte` (~1000 строк): boot (theme.init → getMe → restore сессии → permissions), экран подключения к БД → логин → shell. Sidebar справа, свёртываемый; 19 пунктов навигации, фильтруемых по правам (`requiredPermission`), групповые заголовки (секция «Настройки»: devices, settings); переключатель компаний; тёмная/светлая тема (localStorage `2c-theme`).
- Companies/Users/Roles — inline-разделы App.svelte (Users с детальной карточкой: контакты/профили/сертификаты/смена пароля).

### 3.2 Страницы (13 импортированных + inline)

| Раздел | Компонент | Особенности |
|---|---|---|
| Объекты/Документы/Справочники | ObjectsPage (одна на 3 nav-кода) + **ObjectEditor** | Динамическая форма по метамодели: 17 видов полей (integer/money — кнопка «вес с весов» из device-event), reference-select, table/json/array, formula/computed read-only; вкладки Форма/JSON/История версий; переходы с RBAC (documents.approve/cancel); восстановление версий |
| Заявки | RequestsPage | Табы Мои/На согласовании/Все/Маршруты; создание; отправка = маршрут+сертификат (если политика требует) → каноничная строка → cms-подпись; timeline этапов со слепками подписи; отозвать; редактор маршрутов (user/role, чекбокс ЭЦП) |
| Склад | StockPage | Остатки (фильтр по месту учёта, отрицательные красным), Подотчёт «что у кого», Просрочки; кнопка seed метаданных |
| Устройства | DevicesPage | Карточки устройств, выбор COM-порта, connect/test, живой журнал device-event, wedge-тестовое поле |
| Модули | ModulesPage | Установка .wasm, enable/disable, capabilities/permissions модуля |
| Прочее | MetadataPage (дизайнер метамодели), EventsPage, AuditPage (registry 37 действий), PrintPage (шаблоны+превью), NumberingPage, ScriptsPage (Rhai playground), ReportsPage (дашборд), SettingsPage (контактные типы) | |

### 3.3 Инфраструктура фронта

- **Адаптеры транспорта** (`adapters/transport.ts`): единый `invoke()` поверх трёх реализаций — Tauri IPC / HTTP (`POST /api/{command}`) / Mock. Все 109 методов api.ts транспорт-агностичны.
- **api.ts** (~1150 строк): 108 методов по доменам (см. backend-домены), ~69 экспортированных типов, registry-константы (AUDIT_ACTION_META×37, FIELD_KIND_META×17, OBJECT_STATE_META×6, ENTITY_KIND_META×8, EVENT_TYPE_META×7), `PluginEnvelope<T>`+`unwrapPlugin`.
- **pluginCall<T>(module, fn, args)**: универсальный мост; разворачивает конверт `{ok,data|error}` если гость его вернул, иначе возвращает сырой JSON (совместимость с convert-плагином).
- **Stores**: auth (AuthUser{userId, companyId, roleId, permissions…}, hasPermission(subsystem, action)=deny-aware, localStorage persistence), navigation (NavItem+group), devices (lastWeight/lastScan ← Tauri event `device-event`), theme.
- **Utils**: `barcodeField` — svelte-action клавиатурного сканера: активен ТОЛЬКО в фокусе поля, пауза 80 мс сбрасывает буфер, Enter фиксирует код ≥4 символов; `requestSignatures` — каноничные строки подписи заявок (единый контракт с плагином).
- **RBAC на фронте**: скрытие nav/кнопок по hasPermission (documents.create/update/approve/cancel, requests.*, stock.read, settings.manage и др.) — защита глубиной к серверным проверкам.
- **Live**: Tauri-event `device-event` → журнал устройств + lastWeight/lastScan (ObjectEditor подставляет вес кнопкой).

### 3.4 Состояние качества

- `npm run build` ✓ · `svelte-check --tsconfig tsconfig.app.json` → **0 errors** (67 warnings: a11y-мелочи и устаревание node10-resolution).
- ConvertPage.svelte (372 строки) — осиротевший компонент раннего этапа (не импортируется; функциональность конвертации доступна через ModulesPage/plugin_call).
- Уведомления: backend outbox + IPC готовы, UI-поллинг не подключён (осознанно, v0.2).

---

## 4. Тесты (53 функции)

| Набор | Кол-во | Что покрывает | Гейт |
|---|---|---|---|
| unit (lib) | 22 | COMMAND_MAP×4, tx::validate/$ref×14, tx::session×4 | — |
| requests_plugin_test | 20 | Реальный requests.wasm + моки хоста: маршруты, submit/approve/reject/cancel, подписи (valid/invalid/contract), мультироли, гонка submit, рассылки, хуки | — |
| stock_engine_test | 4 | FIFO-математика демо ТЗ, недостаточно, сторно (частично съеденная партия отклоняется), перенос цены, гонка списаний | TX_TEST_MONGO=1 |
| tx_executor_test | 5 | Идемпотентный повтор, конкурентный ключ (результат победителя), rollback, $ref-цепочка, deny-by-default | TX_TEST_MONGO=1 |
| stock_orchestrator_test | 2 | E2E: реальный stock.wasm проводит/отменяет MOVE на живой БД; политика подписи по категории | TX_TEST_MONGO=1 |

Инфраструктура live-тестов: отдельные БД `tx_test_*`/`stock_test_*`/`stock_orch_*` с очисткой; переменные `TX_TEST_MONGO=1` + `MONGODB_URI`.

---

## 5. Известные ограничения и задел (честно)

| Область | Ограничение | План |
|---|---|---|
| Учёт (ledger) | Только модели/фабрики, нет БД и проводок | Операция `accounting.post` ляжет в реестр tx_exec торговлей |
| ККМ | Не реализована | v0.3; точечные права `devices.fiscal_*` и signature_ref ФН заложены в конвенциях devices/mod.rs |
| Серийные номера | Учёт только количеством | v0.2 |
| Уведомления | In-app outbox есть, UI-поллинга нет; email-канал не отправляется | UI + outbox-доставка v0.2 |
| Эскалации | timeout_hours/is_required хранятся, но не исполняются | v0.2 |
| all_approvals | Полный kv_list с клиентским фильтром | Pushdown на хост при >1000 процедур |
| Устройства TCP / udev-правила / circuit breaker | Не реализованы | По потребности |
| record_scope="own" | Работает только для Scope::Object через middleware | Расширять по мере необходимости |
| Windows | cpcsp-rs Linux-only; devices — Linux-first | По плану платформы |
| ConvertPage.svelte | Осиротевший компонент | Удалить либо вернуть в навигацию |
| user_company (модуль) | Мёртвый код на диске, не в lib.rs | Удалить |

---

## 6. Как проверить руками

- Автотесты: `cargo test` (unit+plugin), `TX_TEST_MONGO=1 cargo test` (+live: tx, stock, e2e-оркестратор) — всего 53.
- Склад: `docs/testing-stock.md` — демо из 11 шагов ТЗ (п.15) + политики подписи.
- Заявки: сценарии согласования двух ролей, подписные/безподписные маршруты — `wasm-modules/requests/README.md`.
