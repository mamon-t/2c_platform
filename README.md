# 2C Platform

Конфигурируемая документо-событийная платформа для малого и среднего бизнеса.

## Стек

Rust/Tokio/MongoDB(replSet)/Tauri v2 · Svelte 5 + TS + Vite 8 + Tailwind CSS v4 + Skeleton UI · Extism/wasmtime (WASM плагины) · Rhai (скрипты) · КриптоПро CSP 5.0 (cpcsp-rs)

## Требования

- Rust stable (edition 2021)
- Node.js ≥ 18 + npm
- MongoDB 7.0+ **replica set** (для транзакций)
- КриптоПро CSP 5.0 (Linux: `/opt/cprocsp/`) — опционально, для ЭЦП
- libudev-dev + pkg-config (для tokio-serial)

## Быстрый старт

```sh
# 1. Установить зависимости
npm install

# 2. Настроить .env (или скопировать .env.example)
echo 'MONGODB_URI=mongodb://localhost:27017/2c_platform?replicaSet=rs0' > .env
echo 'MONGODB_DATABASE=2c_platform' >> .env
echo 'JWT_SECRET=dev-secret-change-me' >> .env

# 3. Запустить в dev-режиме
npm run tauri:dev

# 4. В окне приложения: подключиться к MongoDB → войти admin/admin
```

## Сборка WASM-плагинов

```sh
cd wasm-modules/requests && cargo build --target wasm32-unknown-unknown --release
cd wasm-modules/stock   && cargo build --target wasm32-unknown-unknown --release
cd wasm-modules/trade   && cargo build --target wasm32-unknown-unknown --release
cd wasm-modules/convert && cargo build --target wasm32-unknown-unknown --release
```

> Если линковщик ругается на `-fuse-ld=lld` — проверьте `~/.cargo/config.toml`.
> Локальные `.cargo/config.toml` в каждом wasm-модуле перекрывают глобальный флаг.

## Тестирование

```sh
cd src-tauri

# Unit + plugin тесты (без БД)
cargo test

# Интеграционные тесты (нужна живая MongoDB replica set)
TX_TEST_MONGO=1 cargo test --test ledger_test
TX_TEST_MONGO=1 cargo test --test stock_engine_test
TX_TEST_MONGO=1 cargo test --test tx_executor_test
TX_TEST_MONGO=1 cargo test --test stock_orchestrator_test
TX_TEST_MONGO=1 cargo test --test trade_orchestrator_test
```

### Проверки качества

```sh
cargo check                    # компиляция backend
npx svelte-check               # типы фронтенда
npm run build                  # production build фронтенда
```

## Модули

| Модуль | Тип | Описание |
|---|---|---|
| Заявки | WASM | Маршруты согласования, ЭЦП |
| Склад | Native | FIFO, подотчёт, инвентаризация |
| Торговля | WASM | Поступления, реализации, возвраты |
| Учёт | Native | Двойная запись, план счетов РСБУ, ОСВ |
| Устройства | Native | Сканеры, весы (ККМ — v0.3) |

Документация по модулям: `docs/`
