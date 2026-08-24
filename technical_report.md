# 2C Platform — Technical Report

**Дата:** 25.08.2026 · **Версия:** 0.1.0 · **Состояние:** рабочий прототип

Снимок реализованного по коду для сверки с ТЗ v2.2/v2.3.

---

## 1. Обзор

| Показатель | Значение |
|---|---|
| Backend | Rust / Tauri v2 / Tokio / MongoDB(replSet) |
| Backend модулей | **32** |
| IPC-команд | **135** |
| Host-fn для WASM | **25** |
| Mongo-коллекций | **38** |
| Capabilities плагинов | **14** |
| RBAC политик | **64** (20 подсистем) |
| Аудит действий | **52** |
| Операций tx_exec | **13** |
| Фронтенд | Svelte 5 + TS · 16 компонентов · 115 API-методов |
| Тестов | **56** (22 unit + 34 интеграционных) |

Стек: Rust/Tokio/MongoDB/Tauri2 · Svelte5+TS+Vite8+Tailwind4+Skeleton · Extism/wasmtime · Rhai · Argon2id+JWT · КриптоПро CSP 5.0.

Архитектура «Труба и Доски»: Event Store (неизменяемая лента) + Objects (материализованное состояние через метамодель). Гибридная модульность: ядро нейтрально; тяжёлые инварианты — нативные Rust-модули; оркестрация — WASM-плагины через `tx_exec`.

---

## 2. Backend

### 2.1 Модули (32)

| Группа | Модули |
|---|---|
| Ядро | core(+middleware), db, actions, auth, events, audit |
| Данные | objects(+validation), meta(6 коллекций), company, user, person, role, user_contact, user_profile, user_certificate, settings |
| Безопасность | permission_policy(deny-by-default), signing(КриптоПро CMS) |
| Плагины | plugin_manager(25 host-fn), modules(lifecycle), notify(outbox+projection), rhai(sandbox) |
| Инфраструктура | tx(tx_exec), numbering(атомарные номера), print(шаблоны), devices(оборудование) |
| Прикладные | stock(движок), trade(оркестратор), ledger(двойная запись), crypto(заготовка ЭЦП), commands(IPC hub) |

### 2.2 IPC-команды (135)

| Домен | Кол-во | Примеры |
|---|---|---|
| Метамодель CRUD | 32 | entity_types/fields/states/transitions/forms/actions + validate_entity_transition + execute_entity_action |
| Диагностика/БД/Auth | 6 | get_diagnostics, connect_db, authenticate, get_me, switch_company, get_my_permissions |
| Компании | 5 | CRUD |
| Пользователи/Персона/Профили/Контакты/Сертификаты | 19 | CRUD + contacts types + certificates |
| Роли | 4 | CRUD |
| Политики доступа | 3 | list/create/delete |
| Объекты | 8 | list/get/create/update/post/cancel/restore/list_versions |
| Event Store | 3 | list_events/get_event/list_stream_events |
| Аудит | 2 | list_audit_logs/get_audit_entry |
| Rhai | 2 | validate/execute |
| Печать | 6 | templates×5 + render |
| Нумерация | 4 | list/get/update_format/reset |
| WASM модули | 11 | wasm_load/unload/list/plugin_call + modules_list/get/install/uninstall/enable/disable/update_settings |
| Подпись | 4 | list_crypto_certificates/sign_document/verify_document_signature/create_test_certificate |
| Склад | 8 | seed_metadata/balances/report_handover/report_overdue/signature_policies×3/signature_required_for_doc |
| Учёт | 8 | accounts_list/account_create/update/periods_list/period_set_state/osv/journal/card |
| Торговля | 2 | trade_seed_metadata/trade_get_price |
| Устройства | 9 | list/get/save/delete/connect/disconnect/test/list_ports/wedge_scan |
| Уведомления | 7 | list/mark_read/count_unread/subscriptions×2/templates_list + notify_user host |
| Настройки приложения | 4 | get/save_app_config/get/save_contact_types |

### 2.3 tx_exec — транзакционные пачки

**Реестр операций (13):**
`object.post`, `object.cancel`, `stock.receipt`, `stock.issue`, `stock.transfer`, `stock.handover`, `stock.handover_return`, `stock.count`, `stock.balances`, `stock.reverse`, `accounting.post`, `accounting.reverse_by_doc`, `test.noop`

Фазы: валидация → идемпотентный повтор (`tx_journal` unique `(company_id, idempotency_key)`) → права пачки → txn → последовательное выполнение с `$ref`-подстановкой → журнал внутри txn → commit. Конкурентный дубликат E11000 → результат победителя.

Лимиты: ≤100 ops / 30 c timeout. TransientTransactionError retry ×3.

### 2.4 Plugin SDK ≥1.2

**Host-fn (25):**

| Группа | Функции | Capability |
|---|---|---|
| Объекты | create_object, list_objects, get_object, update_object | objects.create/read/update |
| Переходы | transition_object(post\|cancel) | objects.update |
| Метаданные | get_entity_type, list_entity_fields | metadata.read |
| KV | kv_put, kv_get, kv_list, kv_delete, **kv_put_if_absent** | storage |
| Workflow | run_script(source, ctx_json) | scripts |
| Уведомления | notify_user(recipient, subject, body), users_by_role(role_id) | notifications |
| Контекст | whoami() → {user_id, login, display_name, role_id, **role_ids[]**}, now_ms() | — |
| Настройки | module_settings() → CompanyModule.settings JSON | — |
| События | emit_event(stream_id, event_type, payload) | events.emit |
| Подпись | signature_required(module, action, object_id), **cms_verify**(data_b64, sig_b64) | signature |
| TX | tx_begin(key), tx_add_op(handle, op, params), tx_commit(handle) | transactions |
| Лог | log_message(msg) | logging |

**Capabilities (14):** objects.create/read/update/delete, metadata.read, events.emit, numbering.next, logging, notifications, storage, scripts, transactions, signature

**Манифест** = единственный источник правды: code/name/version/api_version/capabilities[]/permissions[]/handles_documents[]/functions[]. Хост ничего не хардкодит. `handles_documents` → post_object делегирует on_post/on_cancel плагину.

**Аудит KV**: kv_put/kv_delete пишут AuditEntry (ModuleKvPut/Delete).

### 2.5 Заявки (WASM `requests`, референс SDK)

Маршруты согласования (этапы user/role, `requires_signature`). Серверная верификация CMS (cms_verify против каноничных строк). Слепок payload целиком + sha256 + сертификат подписанта. Хуки Rhai из настроек. События lifecycle в Трубу. Уведомления user и role-рассылка (users_by_role). Гонка submit через kv_put_if_absent. Мультироли утверждающего (пересечение role_ids[]). Отмена инициатором.

### 2.6 Склад

Нативный движок session-aware: receipt (партии+движения+балансы), issue_fifo (FIFO, атомарный условный декремент, наборы→компоненты), transfer (цена/дата партии переезжают, handover фиксирует ответственного), count (излишек/недостача), balances, reverse_document (строгое сторно, защита от двойного сторно через reversed_by).

Коллекции: stock_movements/batches/balances. Отрицательные остатки запрещены по умолчанию. Оркестратор — WASM `stock` (handled_documents=[MOVE,COUNT,HANDOVER,HANDOVER_RETURN]). Подотчёт: отчёты «что у кого» и просрочки.

### 2.7 Учёт

Двойная запись нативно (session-aware):
- **План счетов** (`ledger_accounts`): код уникален per company, AccountType определяет знак сальдо, seed торговли (41/44/50/51/60/62/90.1/90.2)
- **Проводки** (`ledger_entries`): пара Дт/Кт = документ; posting_id группирует; nomenclature_id — измерение для возвратов
- **Обороты** (`ledger_balances`): unique (company, period_key, account_id); сальдо через AccountType::balance_sign
- **Периоды** (`accounting_periods`): ensure при первой проводке, close/reopen (reopen = accounting.manage)
- Операции tx_exec: `accounting.post` (Σ>0, Дт≠Кт, счета активны, период открыт) / `accounting.reverse_by_doc`
- Отчёты: ОСВ, журнал проводок, карточка счёта

### 2.8 Торговля

WASM-оркестратор поверх склада и учёта. Не хранит остатки.
- Справочники на Досках: COUNTERPARTY, PRICE_TYPE, PRICE (история цен через закрытие valid_to)
- Частичные индексы objects (по entity_type UUID)
- Документы: PURCHASE/SALES/CUSTOMER_RETURN/SUPPLIER_RETURN
- on_post: [stock.receipt/issue (+доп.расходы пропорционально) → accounting.post (счета из module_settings) → object.post] одной пачкой
- use_accounting=false → без проводок
- trade_get_price(nom, ptype, date) — нативное чтение цены на дату

### 2.9 Криптоподпись

КриптоПро CSP 5.0 (cpcsp-rs). CMS attached/detached ГОСТ Р 34.11-2012_256. Политики подписи (signature_policies): condition по категории номенклатуры, default OFF. cms_verify для серверной верификации. signature_ref в AuditEntry.

### 2.10 Устройства

Сканеры (wedge/serial), весы (regex из настроек). Насос: mpsc → Event Store (StreamType::Device) → Rhai scan_handler → Tauri-push. FiscalPrinter — v0.3.

### 2.11 RBAC

Deny-by-default. Seed 64 политик / 20 подсистем: platform(1), companies(4), users(4), roles(4), contacts(5), documents(6), metadata(4), catalogs(4), reports(3), scripts(3), audit(1), settings(2), print(4), plugins(3), numbering(2), modules(2), devices(3), stock(3), accounting(3), trade(3), notifications(2). record_scope («company»/«own»).

### 2.12 Аудит

52 действия. AuditEntry (11 полей incl. signature_ref, ModuleKvPut/Delete для аудита KV модулей). Запись warn-and-forget после коммита.

---

## 3. Frontend (Svelte 5)

16 компонентов · 20 nav пунктов · 115 api.ts методов · ~75 типов.

| Раздел | Компонент |
|---|---|
| Объекты/Документы | ObjectsPage + ObjectEditor (17 FieldKind, transitions, версии, вес с весов) |
| Заявки | RequestsPage (маршруты, ЭЦП, timeline) |
| Склад | StockPage (остатки, подотчёт, просрочки) |
| Торговля | TradePage (ОСВ, журнал проводок) |
| Устройства | DevicesPage (сканеры/весы, wedge, device-event журнал) |
| Модули | ModulesPage (install/enable/disable, настройки счетов) |
| Прочее | MetadataPage, EventsPage, AuditPage, PrintPage, NumberingPage, ScriptsPage, ReportsPage, SettingsPage |

Инфраструктура: адаптеры транспорта (Tauri/HTTP/Mock), stores (auth/navigation/devices/theme), utils (barcodeField/requestSignatures). pluginCall<T> — универсальный мост к WASM. RBAC фильтрация навигации и кнопок. Live device-events через Tauri event.

---

## 4. Тесты (56)

| Набор | Кол-во | Покрытие | Гейт |
|---|---|---|---|
| unit | 22 | COMMAND_MAP, tx::validate/$ref, tx::session | — |
| requests_plugin | 20 | Реальный wasm: маршруты/submit/approve/reject/подписи/мультироли/гонка/хуки | — |
| stock_engine | 4 | FIFO, сторно, гонка списаний, перенос цены | TX_TEST_MONGO=1 |
| tx_executor | 5 | Идемпотентность, конкурентность, rollback, $ref, deny-by-default | TX_TEST_MONGO=1 |
| stock_orchestrator | 2 | E2E stock.wasm: MOVE post/cancel, политики подписи | TX_TEST_MONGO=1 |
| ledger_test | 2 | Постинг+балансы+реверс, закрытый период | TX_TEST_MONGO=1 |
| trade_orchestrator | 1 | E2E trade.wasm: демо п.11 ТЗ (поступление→реализация→COGS→сторно) | TX_TEST_MONGO=1 |

---

## 5. Ограничения и задел

| Область | Статус | План |
|---|---|---|
| ОСВ | Без входящих сальдо | По мере накопления |
| ККМ | Не реализована | v0.3, права заложены |
| Серийные номера | Количественный учёт | v0.2 |
| Уведомления UI | Backend готов, бейдж/dropdown нет | v0.2 |
| Email/Push каналы | InApp только | v0.2-v0.3 |
| Messaging (чаты) | Не реализовано | v0.2 |
| Эскалации | Хранятся, не исполняются | v0.2 |
| WebSocket real-time | Поллинг | v0.3 |
| object.patch op | Для записи себестоимости в строки документа | v0.2 |
| Windows | Linux-first | По плану |

---

## 6. Как проверить

- Автотесты: `cargo test` + `TX_TEST_MONGO=1 cargo test` (live)
- Склад: `docs/testing-stock.md`
- Торговля: `docs/testing-trade.md`
- Заявки: `wasm-modules/requests/README.md`
