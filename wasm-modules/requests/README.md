# requests-plugin — модуль «Заявки»

WASM-плагин платформы 2C: заявки с маршрутами согласования,
криптоподписью решений (ГОСТ Р 34.10-2012 через CryptoPro) и уведомлениями.

## Сборка

```sh
cargo build --target wasm32-unknown-unknown --release
# → target/wasm32-unknown-unknown/release/requests_plugin.wasm (~400KB)
```

> Локальный `.cargo/config.toml` задаёт явный linker + rustflags для wasm32,
> перекрывая глобальный `-fuse-ld=lld` из ~/.cargo/config.toml, который
> ломает rust-lld (gcc-style аргумент в wasm-flavor lld). Пустой массив
> cargo игнорирует — переопределение работает только с реальным содержимым.

## Манифест (get_info)

Модуль сам декларирует всё о себе — хост ничего не хардкодит:

| Поле | Значение |
|---|---|
| code | `requests` |
| api_version | `1.0` |
| capabilities | objects.read, objects.update, storage, scripts, notifications, logging |
| permissions | requests.create / read / read_all / submit / approve / reject / cancel / manage_routes |

Политики RBAC создаются хостом автоматически при установке.

## Функции

| Функция | Описание |
|---|---|
| routes_list / routes_save / routes_delete | CRUD маршрутов согласования |
| submit(request_id, route_code, signature_der) | Отправка черновика на согласование; подпись инициатора обязательна |
| approve_step / reject_step(request_id, comment, signature_der) | Решение утверждающего текущего этапа; подпись обязательна |
| cancel_request(request_id) | Только инициатор; заявка остаётся черновиком |
| approval_get(request_id) | Процедура + timeline этапов |
| pending_approvals() | Активные согласования, где текущий этап мой |
| all_approvals() | Все процедуры компании |

## Контракт host-функций (будущий Plugin SDK)

Каждая host-функция возвращает конверт:

```
успех:  {"ok": true,  "data": ...}
ошибка: {"ok": false, "error": {"code": "...", "message": "..."}}
```

Коды ошибок: NO_DATABASE, NO_COMPANY, INVALID_COMPANY, NO_USER, INVALID_USER,
NO_MODULE_CODE, INVALID_UUID, INVALID_JSON, INVALID_VERSION, INVALID_ACTION,
NOT_FOUND, DB_ERROR, SCRIPT_FAILED, CAPABILITY_DENIED.

Доступные функции: create_object, list_objects, get_object, update_object,
transition_object(post|cancel), kv_put/kv_get/kv_list/kv_delete,
kv_put_if_absent(key, value) — атомарная вставка (гонки),
run_script(source, ctx_json), notify_user(recipient, subject, body),
users_by_role(role_id) — члены роли для рассылаемых этапов,
whoami() — включая role_ids[] (мультипрофиль),
now_ms(), module_settings(), log_message(msg), get_entity_type,
list_entity_fields, emit_event(stream_id, event_type, payload_json),
cms_verify(data_b64, sig_b64) — верификация CMS через КриптоПро
(возвращает valid/signer_subject/signer_sha1),
signature_required(module, action, object_id),
**tx_begin / tx_add_op / tx_commit** (capability `transactions`).

Аудит: kv_put/kv_delete пишут AuditEntry (ModuleKvPut/Delete) —
«кто что записал» сохраняется навсегда.

### Транзакционные пачки (Plugin SDK ≥1.1)

Сборка атомарной пачки из песочницы — три вызова, Mongo-транзакция
открывается только на commit:

```rhai
// псевдо-код гостя
let h   = tx_begin("receipt-2024-001");        // business_key = ключ идемпотентности
let id1 = tx_add_op(h, "stock.issue", ...);    // op_id раздаёт ядро: "op_1"
let id2 = tx_add_op(h, "object.post", ...);    // "op_2"
tx_commit(h);                                  // атомарно; повтор по ключу безопасен
```

- op_id присваивает хост → связывание через `{"$ref": "op_1.field"}`;
- брошенная сессия вычищается через 10 минут;
- права: право на пачку не требуется, каждый обработчик проверяет свой
  subsystem.action по свежим политикам роли вызывающего.

### KV-хранилище

Изоляция гарантируется хостом: полный ключ = `{company}:{module}:{key}`.
Модуль оперирует только своим ключом. Конвенции модуля:
`route:{code}`, `approval:{request_id}`.

### Хуки Rhai

Админ кладёт скрипты в настройки модуля (modules_update_settings):

| Ключ | Момент | Строгий |
|---|---|---|
| before_submit | перед отправкой | да — throw отменяет операцию |
| after_approve | после этапа | нет |
| on_reject | при отклонении | нет |
| on_complete | после проведения | нет |

Контекст скрипта: переменная `ctx` (JSON): caller, request/approval.

### Подпись

По ТЗ разд. 12: submit/approve/reject требуют CMS-подпись (DER, base64).
Фронтенд: list_crypto_certificates → sign_document → plugin_call.
Плагин проверяет наличие и сохраняет DER в этапе согласования;
проверка подлинности — verify_document_signature (host-side).


## Подписи: верификация и слепок (SDK ≥1.2)

Подписываемая строка — КАНОНИЧНАЯ, собирается одинаково фронтом и плагином:

```
submit:  requests.submit|{id}|{version}|{state}
decide:  requests.decide|{id}|{approve|reject}|{comment}
```

Плагин перед записью шага вызывает cms_verify; расхождение →
SIGNATURE_INVALID, операция отменяется. В шаге хранится:
payload ЦЕЛИКОМ + payload_sha256 + signer_sha1 + signer_subject +
verified. Данные заявки не входят в строку напрямую — их неизменность
гарантирует версионный замок (снимок версии фиксируется вместе с решением).

## Задел v0.2

- `timeout_hours` / `is_required` этапа — под механизм эскалаций
- `all_approvals`: pushdown фильтра на хост при >1000 процедур
  (kv_list сейчас отдаёт всё с префиксом)
- Outbox для гарантированной доставки уведомлений
