# 2C Platform — Технический отчёт о состоянии фронтенда (Flutter-клиент)

> **Рабочий документ.** Локальный, не версионируется в git (в `.gitignore`).
> Ведётся параллельно `doc/technical_report.md` (бэкенд), но по задачам клиента.
> Обновляется по результатам каждой подфазы работ над `2c_client/`.

- **Дата:** 2026-09-11
- **Репозиторий:** локальный git в `/home/mikhail/develop/2C`
- **Проект:** `2c_client/` (Flutter 3.x, desktop-first: Linux/Windows)

---

## 1. Статус

Клиент в стадии Фазы 11a (каркас). Серверная инфраструктура готова (Фаза 10):
`POST /rpc` (Command/Query/EventBatch → `RpcMessage`), `GET /ws?token=<JWT>`
(ServerPush через PushHub), `user.login`/`user.logout` (JWT HS256, Argon2id),
идемпотентность Command `(actor_user_id, request_id)` TTL 5 мин.

**Согласовано с ведущим (внешняя сверка контрактов):**
- Модель `RpcMessage` — 6 вариантов (`command`/`query`/`event_batch`/`response`/
  `error`/`server_push`), тег `type`, значения `snake_case` (`rpc_message.rs`).
- Серверных кодов ошибок ровно 5: `NOT_FOUND_ERROR`, `CONFLICT_ERROR`,
  `VALIDATION_ERROR`, `PERMISSION_ERROR`, `STORAGE_ERROR`. `AUTH_REQUIRED` —
  клиентская концепция (трактовка `PERMISSION_ERROR` на защищённом экране).
- Refresh-токенов на сервере нет; с Фазы 11a access TTL JWT = **8 часов**
  (изменение сервера, коммит `63b290c`). При истечении — повторный вход.
- HTTP-клиент — `http` (не dio); refresh-механизм — в Фазе 12 (оффлайн).

---

## 2. Фазы клиента

| ID | Фаза | Статус | Комментарий | Коммит |
|---|---|---|---|---|
| 11a | Каркас Flutter-клиента | 🔄 In progress | desktop (Linux/Windows), RpcMessage (6), RpcClient, WsClient+reconnect, JWT-логин, go_router+Riverpod, тёмная тема, экраны login/home/settings, маппинг ошибок, тесты | — |

---

## 3. Структура проекта

```text
2c_client/
├── pubspec.yaml
├── lib/
│   ├── main.dart                    # входная точка, инициализация
│   ├── app.dart                     # MaterialApp, тема, роутер
│   ├── core/
│   │   ├── config.dart              # адрес сервера, настройки
│   │   ├── theme.dart               # тёмная тема
│   │   └── router.dart              # go_router
│   ├── models/
│   │   ├── rpc_message.dart         # RpcMessage конверт (ТЗ §10)
│   │   ├── auth_token.dart          # AuthToken + JWT claims
│   │   ├── server_error.dart        # маппинг кодов ошибок
│   │   └── event.dart               # Event / ActorSnapshot (для EventBatch)
│   ├── services/
│   │   ├── rpc_client.dart          # HTTP POST /rpc
│   │   ├── ws_client.dart           # WebSocket GET /ws + reconnect
│   │   └── auth_service.dart        # login/logout/tryRestore/refresh
│   ├── providers/
│   │   ├── auth_provider.dart       # состояние аутентификации
│   │   ├── connection_provider.dart # статус WS-соединения
│   │   └── push_provider.dart       # входящие ServerPush
│   └── screens/
│       ├── login_screen.dart        # экран логина
│       ├── home_screen.dart         # drawer + заглушка (SDUI-слот, 11b)
│       └── settings_screen.dart     # настройки (адрес сервера)
└── test/
    ├── models/…                     # round-trip сериализации
    └── services/…                   # mock http, auth
```

---

## 4. Ключевые решения

- **Стейт:** Riverpod 2.x (ТЗ §3); **роутинг:** go_router (deep-link-ready).
- **HTTP:** `http` (один POST + таймаут; dio не нужен до Фазы 12).
- **WS:** `web_socket_channel`, reconnect с exponential backoff (старт 1 c, cap 30 c).
- **Токен:** `flutter_secure_storage` (libsecret на Linux: пакеты
  `libsecret-1-dev` + `jsoncpp`).
- **Токен НЕ хранить** в plaintext; пароль не хранить вовсе.
- **Идемпотентность:** уникальный UUID в `id` каждого Command.
- **`flutter build windows`** — только на Windows-хосте (нет кросс-компиляции).

---

## 5. Критерии приёмки Фазы 11a

1. `flutter build linux` собирается; `flutter build windows` — на Windows-машине.
2. Модель `RpcMessage` — все 6 вариантов, round-trip совпадает с сервером.
3. Логин через `user.login` → JWT в secure storage.
4. Команды через `POST /rpc` → `Response`/`Error`, русские сообщения об ошибках.
5. WebSocket подключён → ServerPush принимается; индикатор соединения.
6. Обрыв WS → автопереподключение.
7. Тёмная тема (Material 3).
8. Ошибки сервера → понятные сообщения (5 кодов + клиентский `AUTH_REQUIRED`).
9. `flutter test` и `flutter analyze` без ошибок.
10. Автовосстановление сессии из secure storage при старте.

---

## 6. Журнал снимков

| Коммит | Дата | Что зафиксировано | Разделы отчёта |
|---|---|---|---|
| `63b290c` | 2026-09-11 | Сервер: access TTL JWT 1ч → 8ч (`main.rs`); сделан под 11a (не клиентский коммит, но зависимость) | §1 |