# 2C Platform — Technical Report

**Дата:** 25.08.2026 · **Версия:** 0.1.4 · **Состояние:** desktop MVP

Снимок реализованного по коду для сверки с ТЗ v2.2/v2.3.

---

## 1. Обзор

| Показатель | Значение |
|---|---|
| Backend | Rust / Tauri v2 / Tokio / MongoDB (Atlas, replSet) |
| Backend модулей | **32** |
| IPC-команд | **~150** |
| Host-fn для WASM | **26** |
| Mongo-коллекций | **38** |
| Capabilities плагинов | **14** |
| RBAC политик | **64+** (20 подсистем) |
| Аудит действий | **55+** |
| Операций tx_exec | **13** |
| Фронтенд | Svelte 5 + TS · 25+ компонентов · 125+ API-методов |
| Тестов | **63** (23 unit + 40 интеграционных) |

Стек: Rust/Tokio/MongoDB(Tls 0.9)/Tauri2 · Svelte5+TS+Vite8+Tailwind4+Skeleton(nosh) · Extism/wasmtime · Rhai · Argon2id+JWT · КриптоПро CSP 5.0 · Font Awesome 6 Free.

Архитектура «Труба и Доски»: Event Store + Objects через метамодель. Гибридная модульность: ядро нейтрально; инварианты — нативные Rust; оркестрация — WASM через `tx_exec`. MongoDB Atlas (M0 Free tier, шард-0 replica set).

---

## 2. Backend (32 модуля)

### 2.1 Модули

| Группа | Модули |
|---|---|
| Ядро | core(+middleware), db, actions(COMMAND_MAP), auth(JWT), events(Event Store), audit(55 действия) |
| Данные | objects(+validation), meta(6 типов метамодели), company, user, person, role, user_contact, user_profile, user_certificate, settings |
| Безопасность | permission_policy(deny-by-default), signing(КриптоПро CMS), crypto(абстракция ЭЦП) |
| Плагины | plugin_manager(25 host-fn), modules(lifecycle), notify(projection engine), rhai(sandbox) |
| Инфраструктура | tx(tx_exec), numbering, print(шаблоны), devices(сканеры/весы), messaging(чаты) |
| Прикладные | stock(FIFO движок), trade(оркестратор), ledger(двойная запись), commands(IPC hub) |

### 2.2 tx_exec — реестр операций (13)

`object.post/cancel`, `stock.receipt/issue/transfer/handover/handover_return/count/balances/reverse`, `accounting.post/reverse_by_doc`, `test.noop`

Фазы: валидация → идемпотентный повтор → права пачки → txn → последовательное выполнение с `$ref` → журнал внутри txn → commit. Конкурентный E11000 → результат победителя. Лимиты ≤100 ops / 30 c.

### 2.3 Plugin SDK ≥1.2

**Host-fn (25):**

| Группа | Функции | Capability |
|---|---|---|
| Объекты | create_object, list_objects, get_object, update_object, transition_object(post\|cancel) | objects.* |
| Метаданные | get_entity_type, list_entity_fields | metadata.read |
| KV | kv_put, kv_get, kv_list, kv_delete, kv_put_if_absent | storage |
| Workflow | run_script(source, ctx_json) | scripts |
| Уведомления | notify_user(recipient, subject, body), users_by_role(role_id) | notifications |
| Контекст | whoami() → {user_id, login, display_name, role_id, **role_ids[]**}, now_ms() | — |
| Настройки | module_settings() | — |
| События | emit_event(stream_id, event_type, payload_json) — подключён projection engine | events.emit |
| Подпись | signature_required(module, action, object_id), **cms_verify**(data_b64, sig_b64) | signature |
| TX | tx_begin(key), tx_add_op(handle, op, params)→op_id, tx_commit(handle) | transactions |
| Склад | **stock_doc_cost(doc_id)** — себестоимость списаний документа из движений (по line_ref) | objects.read |
| Лог | log_message(msg) | logging |

**Манифест** = источник правды: code/version/api_version/capabilities[]/permissions[]/**handles_documents[]**/functions[]. `handles_documents` → post_object делегирует on_post/on_cancel плагину атомарно.

### 2.4 Заявки (WASM `requests`)

Маршруты согласования (этапы user/role, `requires_signature`). Серверная верификация CMS (cms_verify против каноничных строк). Слепок payload + sha256 + сертификат подписанта в шаге. Хуки Rhai из настроек модуля. События lifecycle в Трубу. Уведомления user и role-рассылка. Мультироли утверждающего. Гонка submit через kv_put_if_absent. Отмена инициатором.

### 2.5 Склад

Нативный движок session-aware: receipt / issue_fifo / transfer (+handover) / count / balances / reverse_document. FIFO по receipt_date, атомарный условный декремент. Строгое сторно с защитой от двойного сторно (`reversed_by`). Отрицательные остатки запрещены по умолчанию.

Оркестратор — WASM `stock`: handled_documents=[MOVE,COUNT,HANDOVER,HANDOVER_RETURN]. Подотчёт: отчёты «что у кого» и просрочки.

### 2.6 Учёт

Двойная запись нативно (session-aware):
- **План счетов** (`ledger_accounts`): код per company, AccountType, seed торговли (41/44/50/51/60/62/90.1/90.2), CRUD IPC (`ledger_account_create`, `ledger_account_update`)
- **Проводки** (`ledger_entries`): пара Дт/Кт = документ; posting_id группирует; nomenclature_id — измерение для возвратов; doc_kind/doc_id — ссылка на документ
- **Обороты** (`ledger_balances`): unique (company, period_key, account_id); **натуральное хранение сальдо**: активы/расходы ≥ 0, пассивы/капитал/доходы ≤ 0. Правило ввода одно для всех мест: пользователь всегда указывает **положительное число на стороне счёта** (дебетовый остаток для активов, кредитовый — для пассивов); `save_opening_balances` переводит его во внутреннее представление (инвертирует пассивные счета). Внутри системы формула единая: closing = opening + Дт − Кт
- **Периоды** (`accounting_periods`): ensure при первой проводке, close/reopen (reopen = accounting.manage); проводка в закрытый → отказ; правка входящих сальдо закрытого периода → отказ; carry-forward не перезаписывает opening закрытого следующего периода; валидация period_key (месяц 01–12, год 2000–2100)
- **Входящие сальдо**: `save_opening_balances` (массовый ввод), `get_opening_balances` (чтение), `close_period_with_carry_forward` (при закрытии: **closing = opening + (Дт − Кт)** единообразно для всех типов счетов → следующий период)
- **ОСВ**: обороты + сальдо по каждому счёту за диапазон периодов; opening берётся из **самого раннего периода диапазона** (не сумма — после carry-forward поздние периоды уже кумулятивны); balance = Дт − Кт; closing = opening + balance
- **Карточка счёта**: running_balance стартует с `opening_balance` последнего периода ≤ date_from (кумулятивного) и прибавляет ±amount по стороне счёта; фильтры дат journal/card через `$and` (работают обе границы)
- Операции tx_exec: `accounting.post` (Σ>0, Дт≠Кт, счета активны, период открыт) / `accounting.reverse_by_doc`
- Отчёты: ОСВ (отдельная страница), журнал проводок, карточка счёта, входящие сальдо

### 2.7 Торговля

WASM-оркестратор поверх склада и учёта.
- Справочники на Досках: COUNTERPARTY, PRICE_TYPE, PRICE (история цен через закрытие valid_to)
- Частичные индексы objects (по entity_type UUID): counterparty(data.name, data.inn), price(unique code), price(nom+ptype+valid_from DESC)
- Документы: PURCHASE/SALES/CUSTOMER_RETURN/SUPPLIER_RETURN; строки документов несут `line_id` → `line_ref` движений
- on_post: [stock.receipt/issue (+доп.расходы пропорционально сумме строк) → accounting.post → object.post] одной пачкой. **COGS — из результата stock.issue через `$ref` (`{op}.total_cost`)**
- **Себестоимость = проекция движений**: не пишется в строки документа; читается по `(doc_id, line_ref)` через host-fn/IPC `stock_doc_cost`; при отмене исчезает вместе с движениями (фильтры is_reversal/reversed)
- **Возврат от покупателя** (`source_sales_id`): себестоимость возврата = средняя по списаниям строки исходной реализации; одна цифра в партии возврата и проводке Дт41 Кт90.2
- on_cancel: [stock.reverse → accounting.reverse_by_doc → object.cancel]
- use_accounting=false → без проводок; нулевые суммы проводок отбрасываются accounting.post (сервисные строки)
- trade_get_price(nom, ptype, date) — нативное чтение цены на дату
- События: trade.purchase_posted/sales_posted/customer_returned/supplier_returned/doc_cancelled

### 2.8 Криптоподпись

КриптоПро CSP 5.0. CMS attached/detached ГОСТ Р 34.11-2012_256. Политики подписи (signature_policies) по категории номенклатуры, default OFF. cms_verify host-fn для серверной проверки. signature_ref в AuditEntry. Тестовый самоподписанный сертификат.

### 2.9 Устройства

Сканеры (wedge/serial), весы (regex из настроек). Насос: mpsc → Event Store (StreamType::Device) → Rhai scan_handler → Tauri-push + notifications collection. `devices_test` выполняет serial I/O **без глобального лока AppState** (зависшее устройство не блокирует UI). FiscalPrinter — v0.3.

### 2.10 RBAC и аудит

Deny-by-default. Seed 64+ политик / 20 подсистем. record_scope («company»/«own»). Все мутации пишут в audit_log (audit_log! макрос) **после успешного выполнения** — при отказе операции ложная запись не создаётся. ExecuteTransaction аудит после tx_exec коммита. ModuleKvPut/Delete для аудита KV модулей.

**Audit coverage**: Login (bootstrap + regular), SwitchCompany, CRUD пользователей/компаний/ролей/политик, CRUD документов, проводки/реверсы, настройки оборудования, шаблоны печати, сертификаты, KV модулей, seed метаданных торговли, сохранение входящих сальдо, автозагрузка модулей.

**RBAC fallback**: при пустых policies после входа — автоматическая загрузка всех политик из БД (защита от seed-гонки при первом входе).

**Дисциплина блокировок**: тяжёлые операции не держат глобальный Mutex<AppState> во время I/O — crypto-генерация сертификата (spawn_blocking) и serial-тест устройства выполняются после release лока; post_object/cancel_object читают user+company одним захватом (устранён TOCTOU со switch_company); StdMutex плагина захватывается только вне глобального лока.

### 2.11 Уведомления

Расширенная модель: severity(info/warning/critical), entity_ref({type,id}), channels[], metadata. Projection engine: событие из Трубы → шаблон → подписка → уведомление. Host-fn: notify_user, users_by_role. Devices: error/disconnect события → notifications collection. IPC: list/mark_read/count_unread/subscriptions/templates. UI: колокольчик + бейдж + dropdown панель (поллинг 30с).

### 2.12 Автозагрузка модулей

После логина/смены компании — автоматическая загрузка всех Enabled модулей для текущей компании. Команда `preload_company_modules`: **параллельная загрузка** через `futures::join_all` (один лок на проверку уже загруженных, один на вставку результатов). Бинарники тянутся через **локальный кэш байтов**: список берётся без `wasm_bytes` (`list_enabled_meta`), каждый модуль читается из `~/.cache/2c-platform/modules/{code}-{hash16}.wasm`; промах кэша → разовый fetch из БД + запись. Хэш SHA-256 фиксируется при install; обновление модуля докачивает только его, старые версии кэша удаляются. Тайминги этапов в логах: `[Pre-load] list_enabled_meta`, `[Plugin compile] build=…ms`, `[Plugin init] get_info=…ms`. Ошибки показываются пользователю через toast; при успешной загрузке ≥1 модуля — аудит-запись.

### 2.13 Кэш WASM-модулей

Два уровня:

- **Кэш байтов** (`~/.cache/2c-platform/modules/`, ключ sha256): повторный вход не скачивает бинарники по сети, работает офлайн. Ленивая миграция старых записей (хэш досчитывается при первой загрузке).
- **Дисковый кэш компиляции wasmtime**: `.with_cache_config()` — TOML в `~/.cache/2c-platform/wasmtime-cache/config.toml` (создаётся автоматически, `OnceLock`). Первый запуск модуля — Cranelift-компиляция с сохранением артефакта, далее десериализация.

Замер на trade.wasm (~300 КБ): **первая компиляция ~460 мс, повторная загрузка из кэша ~9 мс (~50×)**. Недоступный каталог → graceful fallback + warn.
- **Установил → сразу использует**: после успешного `install` модуль загружается в память текущей сессии (с реальными capabilities манифеста) — перезапуск/перелогин не нужен. При повторной установке кода старая версия выгружается `uninstall`'ом.
- Инвалидация in-memory: при `uninstall`/`disable` — выгрузка из `wasm_modules` по UUID и коду.
- Юнит-тест конфига кэша (`wasmtime_cache_config_created_and_valid`).

---

## 3. Frontend (Svelte 5)

25+ компонентов · 23 nav пункта · 125+ API-методов · ~75 типов.

### 3.1 Экраны

| Экран | Компонент | Описание |
|---|---|---|
| Выбор БД | DbConnectScreen + ConnectionsDialog | Выпадающий список сохранённых подключений (1С-стиль) + редактор (добавить/изменить/удалить). Хранилище в `~/.config/2c-platform/connections.json` |
| Вход | LoginScreen | Выбор БД над логином + кнопка ⚙ для редактирования подключений |
| Главная | DashboardScreen | Диагностика, запуск демо-сценария |
| Компании | CompaniesScreen | Карточка с 5 вкладками: Основные/Реквизиты/Банк/Подписанты/Налоги. Галка УСН. Таблица: Код/Название/ИНН/Режим/Статус |
| Пользователи | UsersScreen | CRUD пользователей |
| Роли | RolesScreen | CRUD ролей, привязка политик |
| Объекты | ObjectsPage + ObjectEditor | 17 FieldKind, transitions, версии, делегирование оркестратору |
| Заявки | RequestsPage | Маршруты, ЭЦП, timeline, слепки подписи |
| Склад | StockPage | Остатки, подотчёт, просрочки, seed |
| Торговля | TradePage | Журнал проводок (фильтры: период, счёт), seed метаданных |
| **ОСВ** | **OsvPage** | **Отдельная страница: обороты, входящее/исходящее сальдо за период** |
| **План счетов** | **LedgerAccountsPage** | **CRUD: таблица счетов (код/название/тип/родитель/активен), модалка добавления/редактирования** |
| **Входящие сальдо** | **OpeningBalancesScreen** | **Массовый ввод: таблица счетов + редактируемые поля ввода (рубли), выбор периода** |
| Сообщения | MessagesPage | Rooms list, чат, групповые диалоги |
| Устройства | DevicesPage | Карточки, COM-порты, wedge-тест, device-event журнал |
| Модули | ModulesPage | Install/enable/disable, настройки счетов |
| **Конвертация** | **ConvertPage (DataMapper)** | **4-шаговый мастер: выбор файла (CSV/JSON/XML) → маппинг колонок на поля → превью → импорт/экспорт. Автоматический маппинг по похожести имён. Валидация типов (fieldValidation.ts). Референсы через searchObjectByField** |
| Прочее | MetadataPage, AuditPage (55 действий), PrintPage, NumberingPage, ScriptsPage, ReportsPage, SettingsPage |

### 3.2 Сайдбар

Группы сворачиваются кликом (▸/▾ chevron). Состояние хранится в localStorage. Администрирование свёрнута по умолчанию. Группы: Торговля · Справочники · Отчёты · **Учёт** (ОСВ, План счетов, Входящие сальдо) · Обслуживание · Администрирование.

### 3.3 Уведомления и диалоги

Колокольчик уведомлений с поллингом 30с. Toast-система (success/error/info/warning), позиция: левый верх. Confirm/prompt диалоги (нативные alert/confirm/prompt заменены на компоненты).

### 3.4 Инфраструктура

Адаптеры транспорта (Tauri/HTTP/Mock), stores (auth/navigation/devices/theme), utils (barcodeField/requestSignatures, **fieldValidation.ts** — валидация типов полей, портированная из Rust). **fileConverter.ts** — парсинг/сериализация CSV/JSON/XML. `pluginCall<T>` — мост к WASM. RBAC фильтрация навигации и кнопок. Live device-events. Переключение темы (light/dark). **searchObjectByField** — поиск объектов по точному значению поля.

---

## 4. Тесты (57)

| Набор | Кол-во | Покрытие | Гейт |
|---|---|---|---|
| unit | 23 | COMMAND_MAP×4, tx::validate/$ref×14, tx::session×4, wasmtime-cache×1 | — |
| requests_plugin | 20 | requests.wasm: маршруты/submit/approve/reject/подписи(valid/invalid)/мультироли/гонка/рассылки/хуки | — |
| stock_engine | 4 | FIFO математика ТЗ, сторно (частично съеденная партия отклоняется, двойное блокируется), перенос цены, гонка списаний | TX_TEST_MONGO=1 |
| tx_executor | 5 | Идемпотентность, конкурентность, rollback, $ref-цепочка, deny-by-default | TX_TEST_MONGO=1 |
| stock_orchestrator | 2 | E2E stock.wasm: MOVE post/cancel, политика подписи | TX_TEST_MONGO=1 |
| ledger_test | 2 | Постинг+балансы+реверс+повтор отклонён; закрытый период | TX_TEST_MONGO=1 |
| trade_orchestrator | 4 | E2E trade.wasm: демо п.11 ТЗ (поступление→реализация→COGS→сторно); себестоимость двух строк из разных партий по line_ref; частичный возврат по средней строки исходника (склад=бухгалтерия); отмена → проекция себестоимости пуста | TX_TEST_MONGO=1 |
| example_hello | 3 | Пример Plugin API v1 (`example/hello`) против реального хоста: манифест, эхо, сырой host-fn — гвардия совместимости доки | — |

Live-тесты: любой MongoDB с replica set (локальный или облако). `TX_TEST_MONGO=1` + `MONGODB_URI` в окружении; рецепты поднятия — `docs/mongodb-setup.md`.

---

## 5. Хранилище и конфигурация

- `.env` — dev-only: MONGODB_URI, MONGODB_DATABASE, JWT_SECRET (в `.gitignore`)
- `~/.config/2c-platform/connections.json` — список сохранённых подключений к БД (1С-стиль)
- `~/.cache/2c-platform/wasmtime-cache/` — дисковый кэш компиляции WASM-модулей
- MongoDB — строка подключения в `.env` (`MONGODB_URI`, не коммитится): Atlas или локальный replica set (рецепты: `docs/mongodb-setup.md`)
- Старый локальный backup: `.env.bak.31host`

---

## 6. Ограничения и задел

| Область | Ограничение | План |
|---|---|---|
| ККМ | Не реализована | v0.3, права заложены |
| Серийные номера | Количественный учёт | v0.2 |
| Уведомления Email/Push | InApp только | v0.2-v0.3 |
| Messaging real-time | Поллинг 15с | WebSocket v0.3 |
| Эскалации | timeout_hours/is_required хранятся, **не исполняются** — в UI маршрутов помечено «не исполняется» | v0.2 |
| Возврат «с конца» партии строки | Сейчас средняя по строке исходной реализации | улучшение |
| Windows | Linux-first | По плану |

**Дизайн-решение**: себестоимость реализаций читается из движений склада по `doc_id` (+`line_ref`), в строки документа не пишется; object.patch для стоимости не нужен (см. `docs/testing-trade.md`).

---

## 7. Как проверить

- Автотесты: `cargo test` + `TX_TEST_MONGO=1 cargo test` (live)
- Склад: `docs/testing-stock.md`
- Торговля: `docs/testing-trade.md`
- Заявки: `wasm-modules/requests/README.md`
- Разработка модулей: `example/README.md` (Plugin API v1) + минимальный образец `example/hello`
- Демо: кнопка «Заполнить демо» на Dashboard (создаёт «ООО ЛесТорг» в текущей БД)

### Выбор базы данных

При запуске приложения доступны два сохранённых подключения:
1. **Чистая (2c_platform)** — пустая база для реальной работы
2. **Демо — ЛесТорг (2c_platform_demo)** — заполняется через кнопку на Dashboard

Переключение: на экране входа → выпадающий список «База данных» → кнопка ⚙ для редактирования списка подключений.

### Входящие сальдо

1. «План счетов» → создать нужные счета (или принять seed по умолчанию)
2. «Входящие сальдо» → выбрать период → ввести суммы (в рублях) → Сохранить. Активные счета — дебетовые остатки, пассивные — кредитовые; знак система учитывает автоматически
3. При закрытии периода — сальдо автоматически переносятся в следующий (closing = opening + Дт − Кт)
4. ОСВ показывает входящее/исходящее сальдо по каждому счёту

### DataMapper (Конвертация)

1. «Конвертация» → Экспорт: выбрать entity_type → Скачать CSV/JSON/XML
2. Импорт: выбрать файл → сопоставить колонки (автоматический маппинг) → Превью → Импорт
