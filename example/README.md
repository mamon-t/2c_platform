# Plugin API v1 — внешние WASM-модули 2C Platform

Внешняя логика платформы живёт в WASM-модулях: оркестраторы документов
(торговля, склад), согласование заявок, конвертация данных. Хост — нативное
ядро на Rust/Tauri; модуль исполняется через wasmtime (Extism) с ресурсными
лимитами и capability-моделью.

**Живой минимальный пример**: [`./hello`](./hello) — эхо + время хоста.
Развитые образцы: [`../wasm-modules/trade`](../wasm-modules/trade),
[`../wasm-modules/requests`](../wasm-modules/requests).

---

## 1. Жизненный цикл

```
Установка (админ, ModulesPage)
  └─ валидация: полная загрузка WASM + вызов get_info()
     манифест = единственный источник правды
  └─ сохранение в БД (modules + company_modules, enabled)
  └─ байты → локальный кэш ~/.cache/2c-platform/modules/{code}-{sha256:16}.wasm
  └─ модуль сразу загружен в память сессии — перезапуск не нужен

Старт приложения / вход / смена компании
  └─ preload_company_modules: мета из БД (без бинарников)
  └─ байты из дискового кэша по хэшу (промах → разовая докачка из БД)
  └─ компиляция через дисковый кэш wasmtime:
     первая ~0.5 c, повторная ~9 мс (замер trade.wasm)

Выгрузка: uninstall/disable → удаление из памяти по UUID и коду;
повторный install того же кода отклоняется (сначала uninstall).
```

Кэш пер-машинный: другая машина при первом старте докачает бинарь один раз,
дальше офлайн-устойчива. Обновление модуля = новый хэш = докачивается только он.

## 2. Манифест `get_info()`

Единственная экспортируемая функция без параметров. Вызывается хостом при
установке и каждой загрузке. Формат ответа:

| Поле | Тип | Назначение |
|---|---|---|
| `name` | string | Отображаемое имя |
| `version` | string | Версия модуля |
| `code` | string? | Уникальный код (иначе берётся `name`) |
| `author`, `description` | string? | Метаданные |
| `api_version` | string? | Контракт API; сейчас `"1.0"` (дефолт) |
| `capabilities` | string[] | Гранты на host-fn (см. §4) |
| `permissions` | string[] | RBAC-политики `"subsystem.action"` — создаются при установке |
| `handled_documents` | string[] | Коды entity_type: проведение делегируется модулю |
| `functions` | Function[] | Каталог функций для UI: `{name,label,description,input_schema}` |

`input_schema` — JSON Schema входа функции (используется фронтендом).

## 3. Конверт host-функций

Любой вызов хоста возвращает строку-конверт:

```json
{ "ok": true,  "data": ... }
{ "ok": false, "error": { "code": "...", "message": "..." } }
```

Эталонная развёртка (Rust):

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

Коды ошибок хоста: `NO_DATABASE, NO_COMPANY, INVALID_COMPANY, NO_USER,
INVALID_MODULE_CODE, INVALID_UUID, INVALID_JSON, INVALID_VERSION,
INVALID_ACTION, NOT_FOUND, DB_ERROR, SCRIPT_FAILED, CAPABILITY_DENIED`.

> Исключения без конверта — простые сервисные функции с голым значением:
> `now_ms()` → `"1732…"` (строка мс), `log_message(msg)` → пустая строка.
> Все остальные (данные, KV, транзакции, скрипты…) соблюдают конверт.

## 4. Host-функции (26)

Доступны как импорты в namespace `ExtismHost`. Перед каждой — проверка
capability модуля (`CAPABILITY_DENIED` при отсутствии гранта).

### Объекты (Доски)
| Функция | Сигнатура | Capability |
|---|---|---|
| `create_object` | (entity_type_id, data_json) → `{id}` | objects.create |
| `list_objects` | (entity_type_id, limit) → `{objects[], total_count}` | objects.read |
| `get_object` | (id) → объект `{id,number,date,state,version,data,…}` | objects.read |
| `update_object` | (id, data_json, version) → `{id, version}` | objects.update |
| `transition_object` | (id, action, params_json) — post/cancel через движок | objects.update |
| `stock_doc_cost` | (doc_id) → себестоимость списаний документа по строкам | objects.read |

### Метаданные
| `get_entity_type` | (id) → `{id,code,name,kind}` | metadata.read |
|---|---|---|
| `list_entity_fields` | (entity_type_id) → `{fields[]}` | metadata.read |

### KV-хранилище модуля (изолированное по коду модуля)
| `kv_put` / `kv_put_if_absent` | (key, value) | storage |
|---|---|---|
| `kv_get` / `kv_list` / `kv_delete` | (key) / (prefix) / (key) | storage |

### Workflow и события
| `run_script` | (source_rhai, ctx_json) — Rhai в песочнице | scripts |
|---|---|---|
| `notify_user` | (recipient_user_id, subject, body) | notifications |
| `users_by_role` | (role_id) → список пользователей | notifications |
| `emit_event` | (stream_id, event_type, payload_json) — в Event Store («Труба») | events.emit |

### Контекст и сервис
| `whoami` | () → `{user_id, login, display_name, role_id, role_ids[]}` | — |
|---|---|---|
| `now_ms` | () → unix-миллисекунды хоста | — |
| `module_settings` | () → настройки модуля для компании | — |
| `log_message` | (msg) — структурированный лог `[Module:{code}]` | logging |

### Подпись (КриптоПро)
| `signature_required` | (module_code, action, object_id) → политика | signature |
|---|---|---|
| `cms_verify` | (data_b64, sig_b64) — серверная проверка CMS ГОСТ | signature |

### Транзакции (tx_exec для оркестраторов)
| `tx_begin` | (business_key) → `{handle}` — идемпотентный ключ пачки | transactions |
|---|---|---|
| `tx_add_op` | (handle, op, params_json) → `{op_id}` | transactions |
| `tx_commit` | (handle) — атомарный коммит всех операций | transactions |

Операции `tx_add_op`: `object.post/cancel`,
`stock.receipt/issue/transfer/handover/handover_return/count/balances/reverse`,
`accounting.post/reverse_by_doc`, `test.noop`.

**`$ref`-связывание**: параметры операции могут ссылаться на результат
предыдущей: `{"$ref": "op_id.path.to.field"}` — разрешается перед исполнением.
Так COGS проводка торговли берёт себестоимость прямо из результата
`stock.issue`: `{"$ref": "{issue_op}.total_cost"}`.

## 5. Capabilities

Гранты перечисляет сам модуль в манифесте; неизвестная capability = отказ
установки. Полный набор:

```
objects.create · objects.read · objects.update · objects.delete
metadata.read · events.emit · numbering.next · logging
notifications · storage · scripts · transactions · signature
```

Правило минимизации: запрашивать только используемые host-fn.

## 6. Права пользователя vs права модуля

Две независимые оси:

1. **Capabilities модуля** — что модулю *технически* разрешено вызывать.
   Статичны, проверяются на каждом host-вызове.
2. **RBAC пользователя** — политики `permissions: []` манифеста создаются
   при установке и привязываются ролям админом. Определяют, кто может
   работать с функциями модуля через UI.

`plugin_call` намеренно не проверяет права пользователя: доступ определяется
правами на объекты, над которыми работает плагин. Модуль собственных прав не
имеет (пример — convert: capabilities есть, permissions пустые).

## 7. Оркестрация документов (`handled_documents`)

Если код документа из `handled_documents`, то `post_object`/`cancel_object`
делегируются функциям **`on_post` / `on_cancel`** модуля вместо стандартного
перехода. Контракт:

```rust
#[derive(Deserialize)]
pub struct PostInput {
    pub id: String,                      // UUID объекта
    pub expected_version: Option<i64>,   // оптимистичная блокировка
}
// on_post:  tx_begin → ops(stock/accounting/object.post) → tx_commit
// on_cancel: tx_begin → stock.reverse + accounting.reverse_by_doc + object.cancel
```

Паттерн атомарности: все изменения документа, склада и проводок — одна
транзакция tx_exec; конкурентный конфликт версии → результат «победителя»
(идемпотентность по business_key `post-{id}` / `cancel-{id}`).

## 8. Ресурсные лимиты

Хост принудительно ограничивает каждый модуль:

| Лимит | Значение |
|---|---|
| Топливо (инструкции) | 10 000 000 на вызов |
| Память | 256 страниц (~16 МБ) |
| Таймаут вызова | 10 с внутри плагина, 30 с на весь plugin_call |
| Доступ к файловой системе/сети | нет (только host-fn) |

## 9. Как собрать hello

```sh
cd example/hello
cargo build --release
# артефакт: target/wasm32-unknown-unknown/release/hello_plugin.wasm
```

Затем: приложение → Модули → Установить → выбрать `.wasm`.
Модуль требует только `logging`; ничего в системе не меняет — безопасно
ставить/удалять для изучения API.

> ⚠️ Если глобальный `~/.cargo/config.toml` добавляет `-fuse-ld=lld`,
> нужен локальный `.cargo/config.toml` как в `hello/.cargo/` — иначе
> wasm-линковка падает.

## 10. Чеклист нового модуля

1. `cargo new --lib` + `crate-type = ["cdylib"]`, зависимость `extism-pdk = "1"`
2. `.cargo/config.toml` (линкер, таргет) — скопировать из `hello/`
3. `get_info()` с точным манифестом (§2) — минимум: code, api_version `"1.0"`,
   capabilities под используемые host-fn
4. Экспортируемые функции через `#[plugin_fn]`, конверты через `unwrap_host`
5. Оркестратор? → `on_post`/`on_cancel` + `handled_documents` + tx-паттерн (§7)
6. Сборка → установка через ModulesPage → модуль работает немедленно
7. Версия API поднимается хостом; несовпадение = отказ установки
