# 2C Platform — Technical Report

**Дата:** 25.08.2026 · **Версия платформы:** 0.1.0 · **Состояние:** рабочий прототип, все ключевые подсистемы работают

Назначение документа — точный снимок того, что реализовано и работает на текущий момент, для сверки с ТЗ v2.2/v2.3. Составлен по коду (не по планам): имена файлов, команд, коллекций и цифры взяты из репозитория.

---

## 1. Обзор

| Показатель | Значение |
|---|---|
| Backend | Rust (edition 2021), Tauri v2, Tokio, MongoDB 3.x driver |
| Backend модулей | **32** в `src-tauri/src/` (7 публичных) |
| IPC-команд (Tauri) | **135** зарегистрировано в `invoke_handler` |
| Host-функций для WASM | **25** |
| MongoDB-коллекций | **38** |
| Capabilities плагинов | **14** |
| Seed-политик RBAC | **64** (20 подсистем) |
| Вариантов аудита | **52** (`AuditableAction`) |
| Операций в реестре tx_exec | **13** |
| Фронтенд | Svelte 5 (runes) + TS, ~5 700 строк; **16 компонентов**, 115 API-методов |
| Тестов | **56** функций (22 unit + 34 интеграционных на живой БД) |

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
| `permission_policy` | RBAC: политики, seed 64 шт., deny-by-default `check_access` (приоритет, wildcard `*`, entity_type) |
| `meta` | Метамодель: 6 коллекций (типы/поля/состояния/переходы/формы/действия), 17 FieldKind, 8 EntityKind |
| `objects` | Универсальное хранилище: CRUD, версии/снимки, проведение/отмена, валидация данных по метаполям; делегирование post/cancel оркестраторам (`handles_documents`) |
| `events` | Event Store: append-only, `StreamType{Object,User,Module,Device}`, ActorSnapshot, version-in-stream |
| `audit` | Журнал действий: 52 действия, `AuditChanges` (old→new), фильтры, 4 составных индекса |
| `tx` | **Механизм транзакционных пачек**: исполнитель (валидация → идемпотентность → txn → `$ref` → журнал → commit), реестр операций, сессия-строитель для плагинов, идемпотентность через `tx_journal` |
| `plugin_manager` | Extism-рантайм: 25 host-fn, загрузка с лимитами, dispatch `plugin_call`, KV-хранилище модулей (`module_store`) с аудитором, workflow-fns |
| `modules` | Жизненный цикл WASM-модулей: install/enable/disable/settings per company; манифест = источник правды (capabilities/permissions/handles_documents); авто-seed RBAC при установке |
| `notify` | Outbox in-app уведомлений (`notifications`) |
| `numbering` | Атомарные номера `{prefix}-{entity_code}-{seq}}` per company+type |
| `print` | Шаблоны печатных форм (HTML+Handlebars-подобный синтаксис) + рендер |
| `rhai` | Песочница скриптов (max_ops, без ФС/сети) |
| `signing` | КриптоПро CMS: список серт. MY-store, sign attached/detached, verify, verify_detached, тестовый самоподписанный сертификат |
| `devices` | Внешнее оборудование: сканеры (wedge/serial), весы (regex-протокол), насос событий → Event Store + UI-push |
| `stock` | Движок склада (инварианты FIFO/сторно) + обработчики tx_exec + политики подписи + seed метаданных |
| `trade` | Оркестратор торговли: seed метаданных (контрагенты/цены/документы), частичные индексы objects, цена на дату |
| `ledger` | **Учёт — двойная запись**: план счетов, проводки (пары Дт/Кт), периоды (открытие/закрытие/переоткрытие), обороты по счетам, обработчики tx_exec |
| `crypto` | Абстракция провайдеров ЭЦП (заготовка под будущий выбор провайдера) |

### 2.2 IPC-команды (135)

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
| Учёт | ledger_accounts_list, ledger_account_create/update, ledger_periods_list, ledger_period_set_state, ledger_osv, ledger_journal, ledger_card | 8 |
| Торговля | trade_seed_metadata, trade_get_price | 2 |
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

**Реестр операций (13):**

| Операция | Право | Домен |
|---|---|---|
| `test.noop` | — | Отладка |
| `object.post` / `object.cancel` | documents.approve/cancel | Документы |
| `stock.receipt` / `stock.issue` / `stock.transfer` / `stock.handover` / `stock.handover_return` / `stock.count` / `stock.balances` / `stock.reverse` | stock.use / stock.read | Склад |
| `accounting.post` / `accounting.reverse_by_doc` | accounting.post | Учёт |

Для WASM-плагинов — сессия-строитель: `tx_begin(business_key)→handle`, `tx_add_op(handle, op, params)→op_id` (op_id раздаёт хост), `tx_commit(handle)`; Mongo-txn открывается только в commit; брошенные сессии чистятся (TTL 10 мин). Права на момент коммита = объединение политик всех активных ролей пользователя в компании.

### 2.4 Plugin SDK (WASM)

- Рантайм Extism: timeout 10 c / fuel 10M / memory 256 стр.; каждый host-call проходит capability-check → `{ok,data|error{code,message}}`.
- Манифест модуля (`get_info()`): code, name, version, api_version, author, description, capabilities[], permissions[] ("subsystem.action" → авто-seed RBAC при install), handles_documents[], functions[]. Хост ничего не хардкодит.
- Делегирование проведения: если entity_type документа входит в `handles_documents` включённого модуля, команды `post_object/cancel_object` вызывают экспортируемые `on_post/on_cancel` — плагин собирает пачку (складские операции + учётные проводки + object.post/cancel) и всё атомарно.

**Host-функции (25):**

| Группа | Функции | Capability |
|---|---|---|
| Объекты | create_object, list_objects, get_object, update_object, transition_object(post\|cancel) | objects.create/read/update |
| Метаданные | get_entity_type, list_entity_fields | metadata.read |
| KV-хранилище | kv_put, kv_get, kv_list, kv_delete, **kv_put_if_absent**(атомарная вставка для гонок) | storage |
| Workflow | transition_object, run_script(source, ctx_json) | objects.update / scripts |
| Уведомления | notify_user(recipient, subject, body), users_by_role(role_id) | notifications |
| Контекст | whoami() — включая role_ids[], now_ms(), module_settings() | — |
| События | emit_event(stream_id, event_type, payload_json) | events.emit |
| Подпись | signature_required(module, action, object_id), cms_verify(data_b64, sig_b64) | signature |
| Учёт | accounting.post/reverse — через tx_exec, не отдельная host-fn | transactions |
| Лог | log_message(msg) | logging |
| TX | tx_begin(business_key), tx_add_op(handle, op, params), tx_commit(handle) | transactions |

### 2.5 Заявки (WASM `requests`, референс SDK ≥1.2)

Полный жизненный цикл согласования: маршруты (этапы user/role, `requires_signature` на маршруте), submit→этапы→approve/reject→completed, cancel инициатором.

**Серверная верификация подписей**: каноничные строки (`requests.submit|id|version|state`, `requests.decide|id|decision|comment`) собираются фронтом и плагином идентично; плагин вызывает cms_verify перед записью шага — расхождение = SIGNATURE_INVALID, операция отменяется. В шаге фиксируется: payload целиком + sha256 + signer_sha1 + signer_subject + verified=true.

Хуки Rhai из настроек модуля: before_submit (strict — throw отменяет операцию) / after_approve / on_reject / on_complete (warn-and-forget). События lifecycle → Труба (emit_event). Уведомления user и role-рассылка (users_by_role). Гонка submit разрешена атомарной вставкой (kv_put_if_absent + уникальный индекс ns_key). Мультироли утверждающего (пересечение role_ids[] профиля).

Аудит KV: kv_put/kv_delete пишут AuditEntry (ModuleKvPut/Delete) — «кто что записал» сохраняется навсегда.

### 2.6 Склад

**Движок (нативно, session-aware)**: receipt / issue_fifo (FIFO по receipt_date, атомарный условный декремент партии, наборы раскладываются в компоненты, услуги пропускаются) / transfer (цена и дата партии переезжают; handover-вариант фиксирует ответственного и срок возврата, себестоимость не списывается) / count (излишек/недостача) / balances / reverse_document (строгое сторно: расходные возвращают в ту же партию, приходные удаляются только нетронутые; движения помечаются reversed + reversed_by — защита от двойного сторно). Отрицательные остатки запрещены по умолчанию (настройка компании `app_settings.stock.allow_negative`), ошибка «Недостаточно X: нужно N, есть M».

Коллекции: `stock_movements` (лента, 8 видов движений, actor), `stock_batches` (FIFO-партии, частичный индекс живых), `stock_balances` (unique company+location+nomenclature). Индексы: карточка товара, локация, doc_id, подотчёт (responsible_user_id+expected_return_date).

**Оркестратор** — WASM `stock`: манифест `handled_documents=[MOVE,COUNT,HANDOVER,HANDOVER_RETURN]`; on_post строит пачку [складская операция по строкам → object.post] одной транзакцией; on_cancel — [stock.reverse → object.cancel]. Проведение присваивает номер нумерацией в той же транзакции.

**Подотчёт**: «что у кого на руках» (остатки custodian-локаций + данные выдачи), просроченные возвраты. Напоминания — существующий механизм хуков/уведомлений.

### 2.7 Криптоподпись

- `signing`: список сертификатов MY-хранилища, sign (attached/detached, ГОСТ Р 34.11-2012_256), verify, verify_detached; генерация тестового самоподписанного сертификата (settings.manage).
- **Политики подписи** (`signature_policies`): {module, action, condition, required}; condition v0.1 — `{"nomenclature_category": X}` (применимо, если строка документа ведёт к номенклатуре категории X); default OFF. Оценка: host-fn `signature_required` + IPC `signature_required_for_doc`. Интегрировано в HANDOVER склада и заявки.
- **Верификация**: host-fn `cms_verify(data_b64, sig_b64)` → КриптоПро verify_detached; используется плагинами для серверной проверки подписей перед записью решения.

### 2.8 Учёт

Двойная запись нативно (session-aware — пишут через сессию исполнителя tx_exec):

- **План счетов** (`ledger_accounts`): код уникален в рамках компании, parent_code (иерархия), AccountType (Asset/Liability/Equity/Revenue/Expense/OffBalance); seed типового торгового плана (41/44/50/51/60/62/90.1/90.2); CRUD IPC (read/manage).
- **Проводки** (`ledger_entries`): пара Дт/Кт = документ; posting_id группирует; date + period_key ("YYYY-MM"); nomenclature_id — измерение для построчной себестоимости; doc_kind/doc_id — ссылка на документ-источник; is_reversal + reversed_by — защита от двойного сторно.
- **Обороты** (`ledger_balances`): unique (company, period_key, account_id); debit_turnover / credit_turnover наращиваются при каждой записи; сальдо вычисляется читателем через AccountType::balance_sign.
- **Периоды** (`accounting_periods`): month-key "YYYY-MM"; ensure_period апсерт opened=true при первой проводке; period_set_state close/reopen (reopen = accounting.manage); проводка в закрытый период → отказ.
- **Операции tx_exec**: `accounting.post` (проверки: Σ>0, Дт≠Кт, счета активны, период открыт → записи + обороты обеих сторон пары) и `accounting.reverse_by_doc(target_doc_id)` (зеркальный постинг, исходники помечаются reversed_by).
- **Отчёты**: ОСВ (обороты + сальдо по типам счетов за период), журнал проводок (фильтры счёт/документ/дата), карточка счёта (нарастающий остаток).

### 2.9 Торговля

Оркестратор поверх склада и учёта — WASM `trade`. Не хранит остатки и не считает себестоимость.

- Манифест `handled_documents=[PURCHASE,SALES,CUSTOMER_RETURN,SUPPLIER_RETURN]`.
- Справочники — в универсальной коллекции objects: COUNTERPARTY (name/legal_name/type(enum)/inn/contacts[]/bank_accounts[]/manager_id), PRICE_TYPE (code/name/purpose(enum)/order), PRICE (price_type_id/nomenclature_id/value/valid_from/valid_to — история через закрытие периодов).
- Частичные индексы objects: counterparty(data.name, data.inn), price_type(unique code), price(nom+ptype+valid_from DESC).
- `trade_get_price(nom, ptype, date)` — нативное чтение цены на дату по главному индексу.
- on_post по коду типа:
  - PURCHASE → stock.receipt (unit_cost = цена + доп.расходы пропорционально сумме строк) → accounting.post (D:41 C:60) → object.post;
  - SALES → stock.issue (FIFO себестоимость) → accounting.post (D:90.2 C:41 COGS построчно; D:62 C:90.1 выручка) → object.post;
  - CUSTOMER_RETURN → stock.receipt (обратный приём) → accounting.post (реверс выручки) → object.post;
  - SUPPLIER_RETURN → stock.issue → accounting.post (D:60 C:41) → object.post.
- on_cancel: [stock.reverse → accounting.reverse_by_doc → object.cancel] — полное зеркальное сторно.
- Если use_accounting=false в настройках модуля — пачка без проводок.
- События: trade.purchase_posted/sales_posted/customer_returned/supplier_returned/doc_cancelled в Трубу.

### 2.10 Устройства

Сканеры: keyboard-wedge (фронт, поле-локально) и Serial (tokio-serial, человекочитаемые ошибки Linux — dialout/занят/нет порта). Весы: regex-протокол из настроек устройства, стабильность показаний. Насос: mpsc → Event Store (StreamType::Device, системный actor) → опциональный Rhai scan_handler из настроек устройства → Tauri-push «device-event» в UI. FiscalPrinter/LabelPrinter — задел (v0.3).

### 2.11 RBAC и аудит

- Deny-by-default; приоритет deny>allow; wildcard actions; entity_type wildcard; record_scope («company»/«own») проверяется middleware для Scope::Object.
- Seed 64 политик / 20 подсистем: platform(1), companies(4), users(4), roles(4), contacts(5), documents(6), metadata(4), catalogs(4), reports(3), scripts(3), audit(1), settings(2), print(4), plugins(3), numbering(2), modules(2), devices(3), stock(3), accounting(3), trade(3).
- Аудит: 52 действия с label/icon/target_type; AuditEntry (11 полей incl. signature_ref); 4 индекса; запись warn-and-forget в POST-фазе middleware и после tx-коммитов. ModuleKvPut/Delete — аудит KV-хранилища модулей.

---

## 3. Frontend (Svelte 5)

### 3.1 Каркас

- `App.svelte` (~1000 строк): boot (theme.init → getMe → restore сессии → permissions), экран подключения к БД → логин → shell. Sidebar справа, свёртываемый; 20 пунктов навигации, фильтруемых по правам (`requiredPermission`), групповые заголовки (секция «Настройки»: devices, settings); переключатель компаний; тёмная/светлая тема (localStorage `2c-theme`).
- Companies/Users/Roles — inline-разделы App.svelte (Users с детальной карточкой: контакты/профили/сертификаты/смена пароля).

### 3.2 Страницы (16 импортированных + inline)

| Раздел | Компонент | Особенности |
|---|---|---|
| Объекты/Документы/Справочники | ObjectsPage (одна на 3 nav-кода) + **ObjectEditor** | Динамическая форма по метамодели: 17 видов полей (integer/money — кнопка «вес с весов» из device-event), reference-select, table/json/array, formula/computed read-only; вкладки Форма/JSON/История версий; переходы с RBAC (documents.approve/cancel); восстановление версий; делегирование проведения оркестратору (post_object → plugin on_post) |
| Заявки | RequestsPage | Табы Мои/На согласовании/Все/Маршруты; создание; отправка = маршрут+сертификат (если политика требует) → каноничная строка → cms-подпись; timeline этапов со слепками подписи (payload/sha256/signer); отозвать; редактор маршрутов (user/role, чекбокс ЭЦП) |
| Склад | StockPage | Остатки (фильтр по месту учёта, отрицательные красным), Подотчёт «что у кого», Просрочки; кнопка seed метаданных |
| Торговля | **TradePage** | ОСВ (обороты + сальдо по типам счетов за период), Журнал проводок (фильтры дата/счёт/документ); кнопка seed метаданных торговли |
| Устройства | DevicesPage | Карточки устройств, выбор COM-порта, connect/test, живой журнал device-event, wedge-тестовое поле |
| Модули | ModulesPage | Установка .wasm, enable/disable, capabilities/permissions/handles_documents модуля |
| Прочее | MetadataPage (дизайнер метамодели), EventsPage, AuditPage (registry 37 действий), PrintPage (шаблоны+превью), NumberingPage, ScriptsPage (Rhai playground), ReportsPage (дашборд), SettingsPage (контактные типы) | |

### 3.3 Инфраструктура фронта

- **Адаптеры транспорта** (`adapters/transport.ts`): единый `invoke()` поверх трёх реализаций — Tauri IPC / HTTP (`POST /api/{command}`) / Mock. Все 115 методов api.ts транспорт-агностичны.
- **api.ts** (~1200 строк): 115 методов по доменам, ~75 экспортированных типов, registry-константы (AUDIT_ACTION_META×37, FIELD_KIND_META×17, OBJECT_STATE_META×6, ENTITY_KIND_META×8, EVENT_TYPE_META×7), `PluginEnvelope<T>`+`unwrapPlugin`.
- **pluginCall<T>(module, fn, args)**: универсальный мост; разворачивает конверт `{ok,data|error}` если гость его вернул, иначе возвращает сырой JSON (совместимость с convert-плагином).
- **Stores**: auth (AuthUser{userId, companyId, roleId, permissions…}, hasPermission(subsystem, action)=deny-aware, localStorage persistence), navigation (NavItem+group), devices (lastWeight/lastScan ← Tauri event `device-event`), theme.
- **Utils**: `barcodeField` — svelte-action клавиатурного сканера: активен ТОЛЬКО в фокусе поля, пауза 80 мс сбрасывает буфер, Enter фиксирует код ≥4 символов; `requestSignatures` — каноничные строки подписи заявок (единый контракт с плагином).
- **RBAC на фронте**: скрытие nav/кнопок по hasPermission (documents.create/update/approve/cancel, requests.*, stock.read, trade.read, accounting.read, settings.manage и др.) — защита глубиной к серверным проверкам.
- **Live**: Tauri-event `device-event` → журнал устройств + lastWeight/lastScan (ObjectEditor подставляет вес кнопкой).

### 3.4 Состояние качества

- `npm run build` ✓ · `svelte-check --tsconfig tsconfig.app.json` → **0 errors** (67 warnings: a11y-мелочи и устаревание node10-resolution).
- ConvertPage.svelte (372 строки) — осиротевший компонент раннего этапа (не импортируется; функциональность конвертации доступна через ModulesPage/plugin_call).
- Уведомления: backend outbox + IPC готовы, UI-поллинг не подключён (осознанно, v0.2).

---

## 4. Тесты (56 функций)

| Набор | Кол-во | Что покрывает | Гейт |
|---|---|---|---|
| unit (lib) | 22 | COMMAND_MAP×4, tx::validate/$ref×14, tx::session×4 | — |
| requests_plugin_test | 20 | Реальный requests.wasm + моки хоста: маршруты, submit/approve/reject/cancel, подписи (valid/invalid/contract), мультироли, гонка submit (kv_put_if_absent), рассылки роли, хуки | — |
| stock_engine_test | 4 | FIFO-математика демо ТЗ, недостаточно, сторно (частично съеденная партия отклоняется, двойное сторно блокируется), перенос цены, гонка списаний | TX_TEST_MONGO=1 |
| tx_executor_test | 5 | Идемпотентный повтор, конкурентный ключ (результат победителя), rollback, $ref-цепочка, deny-by-default | TX_TEST_MONGO=1 |
| stock_orchestrator_test | 2 | E2E: реальный stock.wasm проводит/отменяет MOVE на живой БД; политика подписи по категории | TX_TEST_MONGO=1 |
| ledger_test | 2 | Постинг + балансы + реверс + повторное сторно отклонено; закрытый период блокирует проводку | TX_TEST_MONGO=1 |
| trade_orchestrator_test | 1 | E2E демо п.11 ТЗ: поступление→реализация→COGS→сторно, живая БД | TX_TEST_MONGO=1 |

Инфраструктура live-тестов: отдельные БД `tx_test_*`/`stock_test_*`/`stock_orch_*`/`trade_e2e_*`/`ledger_test_*` с очисткой; переменные `TX_TEST_MONGO=1` + `MONGODB_URI`.

---

## 5. Известные ограничения и задел (честно)

| Область | Ограничение | План |
|---|---|---|
| План счетов | Только типовой торговый seed; нет произвольного создания иерархии через UI | Расширение MetadataPage или отдельный UI |
| ОСВ | Без входящих сальдо (платформа молодая, история с нуля) | По мере накопления данных |
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

- Автотесты: `cargo test` (unit+plugin), `TX_TEST_MONGO=1 cargo test` (+live: tx, stock, ledger, e2e-оркестраторы) — всего 56.
- Склад: `docs/testing-stock.md` — демо из 11 шагов ТЗ (п.15) + политики подписи.
- Торговля: `docs/testing-trade.md` — демо п.11 ТЗ (8 шагов) + автотест E2E.
- Заявки: сценарии согласования двух ролей, подписные/безподписные маршруты — `wasm-modules/requests/README.md`.

---

## 7. Коммит-история (ключевые коммиты)

| Хеш | Что |
|---|---|
| `81dd529` | U1/U2: базовый сервис учёта + accounting.post/reverse в tx_exec |
| `00f1849` | T1: торговля-базис (seed, индексы, цена на дату) |
| `ab311e6` | T2: WASM-плагин торговли (298KB) |
| `90876ca` | T3/U3: E2E демо п.11 + отчёты учёта IPC |
| `d4381c2` | T3/U3: TradePage frontend |
| `b3272b3` | RQ1: серверная верификация подписей + слепок |
| `f48a2cb` | RQ3: мультироли утверждающего |
| `9e31676` | RQ4+RQ5+RQ2: гонка submit, рассылки роли, аудит KV |
| `f4a6edf` | S1–S5: модуль оборудования (devices) |
| `157e581` | Инфраструктура Plugin SDK (KV-storage, tx_exec, notify, RBAC-seed) |
