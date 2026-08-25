# MongoDB для разработки и тестов

Платформе нужен MongoDB в режиме **replica set** — транзакции (`tx_exec`,
склад+учёт атомарно) работают только с ним. Автотесты помечены гейтом
`TX_TEST_MONGO=1` и читают строку подключения из `MONGODB_URI`.

Минимальная версия сервера: **6.0+** (проверялось на 7.0).

---

## Вариант 1 — docker-compose (рекомендуется)

Одна нода, инициализация реплики автоматическая.

**docker-compose.yml**

```yaml
services:
  mongo:
    image: mongo:7
    container_name: 2c-mongo
    command: ["--replSet", "rs0", "--bind_ip_all"]
    ports:
      - "27017:27017"
    volumes:
      - mongo_data:/data/db
    healthcheck:
      test: mongosh --eval "try { rs.status().ok } catch (e) { rs.initiate() }" --quiet
      interval: 5s
      timeout: 10s
      retries: 12

volumes:
  mongo_data:
```

```sh
docker compose up -d
# ждём healthcheck (~15 c), затем проверка транзакции:
mongosh mongodb://localhost:27017/2c_platform?replicaSet=rs0 --quiet \
  --eval 'const s=db.getMongo().startSession(); s.startTransaction(); \
          s.getDatabase("2c_platform").t.insertOne({ok:1}); s.commitTransaction(); \
          print("TXN OK")'
```

Строка подключения:

```
MONGODB_URI="mongodb://localhost:27017/2c_platform?replicaSet=rs0"
```

> Без авторизации — для локальной разработки. Для сети добавьте
> `MONGO_INITDB_ROOT_USERNAME/PASSWORD` и `authSource=admin`.

---

## Вариант 2 — локальный mongod с replica set

Для уже установленного сервера (Linux).

1. Включить реплику в `/etc/mongod.conf`:

```yaml
replication:
  replSetName: rs0
# при необходимости слушать на внешнем интерфейсе:
# net:
#   bindIp: 0.0.0.0
```

2. Перезапустить и инициализировать (однократно):

```sh
sudo systemctl restart mongod
mongosh --eval 'rs.initiate()'
```

3. Пользователь (если нужен):

```js
// mongosh
use admin
db.createUser({ user: "db_user", pwd: "***",
  roles: [{ role: "readWrite", db: "2c_platform" }] })
```

Строка подключения:

```
MONGODB_URI="mongodb://db_user:***@192.168.x.x:27017/2c_platform?replicaSet=rs0&authSource=admin"
```

---

## Подключение проекта

`.env` в корне (не коммитится):

```
MONGODB_URI="<строка из варианта выше>"
MONGODB_DATABASE=2c_platform
JWT_SECRET=<произвольная строка>
```

⚠️ Если правите `.env` вручную, берите URI **в кавычки**: символ `&` в
query-параметрах без кавычек ломает `source ../.env` в шелле.

## Запуск интеграционных тестов

```sh
cd src-tauri
set -a; source ../.env; set +a
TX_TEST_MONGO=1 cargo test        # весь набор, включая E2E wasm-оркестраторов
```

Тесты создают одноразовые БД (`*_e2e_<timestamp>_<hex>`), коллекции удаляют
по одной — `dropDatabase` не требуется.
