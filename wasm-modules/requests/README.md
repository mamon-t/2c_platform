# requests-plugin — модуль «Заявки»

WASM-плагин платформы 2C: заявки с маршрутами согласования,
криптоподписью решений (ГОСТ Р 34.10-2012 через CryptoPro) и уведомлениями.

## Сборка

```sh
./build.sh
# → target/wasm32-unknown-unknown/release/requests_plugin.wasm (~400KB)
```

> RUSTFLAGS="" в скрипте перекрывает глобальный `-fuse-ld=lld`
> из ~/.cargo/config.toml, который ломает rust-lld для wasm32.

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
run_script(source, ctx_json), notify_user(recipient, subject, body),
whoami(), now_ms(), module_settings(), log_message(msg), get_entity_type,
list_entity_fields.

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
