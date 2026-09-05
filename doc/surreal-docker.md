# Развертывание SurrealDB в Docker для 2C Platform (v3.0)

Инструкция для развёртывания SurrealDB в локальном или тестовом окружении (в том числе на удалённой машине в ЛВС). Документация соответствует архитектурным требованиям ТЗ v3.0 (ACID-транзакции, Event Sourcing, изоляция данных).

## 1. Требования
- Docker Engine 20.10+ или Docker Desktop
- Свободный порт `8000` на хост-машине (стандартный порт SurrealDB)
- Минимум 2 ГБ свободной оперативной памяти (рекомендуется для стабильной работы RocksDB и WASM-модулей)

## 2. Рекомендуемый способ: Docker Compose
Этот способ обеспечивает персистентное хранение данных на движке RocksDB, удобное управление логами и сетевую изоляцию для будущего расширения (например, добавления OCR-сервиса).

Создайте файл `docker-compose.yml` в корне проекта:

```yaml
version: '3.8'

services:
  surrealdb:
    image: surrealdb/surrealdb:latest
    container_name: 2c-surrealdb
    restart: unless-stopped
    
    networks:
      - 2c-platform-net
    
    command:
      - start
      - --log=info
      - --user=${DB_USER:-root}
      - --pass=${DB_PASSWORD:-root}
      - rocksdb:/mydata/2c_platform.db
    
    environment:
      - SURREAL_NO_BANNER=true
    
    ports:
      - "8000:8000"
    
    volumes:
      - /opt/2c-platform/surrealdb_data:/mydata

    deploy:
      resources:
        limits:
          memory: 1G
        reservations:
          memory: 256M

networks:
  2c-platform-net:
    name: 2c-platform-net
    driver: bridge
```

## 3. Управление и мониторинг (для сисадмина)

### Запуск и остановка
```bash
# Запуск в фоновом режиме (создаст сеть и том автоматически)
docker compose up -d

# Просмотр статуса контейнера
docker compose ps

# Корректная остановка (контейнер удаляется, данные в томе 2cplatform_v3_0_data сохраняются!)
docker compose down

# Полное удаление (ВНИМАНИЕ: удаляет том с данными безвозвратно! Использовать только для сброса dev-окружения)
docker compose down -v
```

### Работа с логами
```bash
# Просмотр последних 50 строк лога в реальном времени
docker compose logs -f --tail=50 surrealdb

# Поиск ошибок или предупреждений в логах
docker compose logs surrealdb | grep -iE "error|warn"

# Экспорт логов в файл для анализа или отчёта
docker compose logs surrealdb > surrealdb_logs_$(date +%F).txt
```

### Резервное копирование и восстановление (Event Store)
Поскольку мы используем Event Sourcing, целостность данных критична. Используйте встроенные утилиты SurrealDB для экспорта/импорта.

```bash
# Экспорт всей БД (namespace: main, database: main) в SQL-файл на хост-машине
docker compose exec surrealdb /surreal export \
  --endpoint http://localhost:8000 \
  --username root --password root \
  --namespace main --database main \
  > backup_$(date +%F).sql

# Импорт из SQL-файла (например, после сбоя или для развёртывания копии)
docker compose exec -T surrealdb /surreal import \
  --endpoint http://localhost:8000 \
  --username root --password root \
  --namespace main --database main \
  < backup_2026-09-05.sql
```

## 4. Проверка подключения (согласно ТЗ v3.0)

### Через CLI внутри контейнера
```bash
# Подключение через встроенный CLI (интерактивный режим)
docker compose exec surrealdb /surreal sql \
  --endpoint http://localhost:8000 \
  --username root --password root \
  --namespace main --database main --pretty
```
Выполните тестовый запрос (таблица создаётся автоматически при первой записи):
```sql
CREATE company:test SET name = "Тестовая компания 2C", created_at = time::now();
SELECT * FROM company;
```

### Через Curl (REST API с хоста)
```bash
curl -s http://localhost:8000/sql \
  -H "Authorization: Bearer root:root" \
  -H "Content-Type: application/json" \
  -d '{"statement": "SELECT * FROM company;"}'
```

### Через Rust SDK (как в ядре 2C Platform)
```rust
use surrealdb::engine::remote::ws::Client;
use surrealdb::Surreal;

let db = Surreal::new::<Client>("ws://localhost:8000").await?;
db.signin(surrealdb::opt::auth::Root { username: "root", password: "root" }).await?;
db.use_ns("main").use_db("main").await?;
```

## 5. Подключение из других контейнеров в той же сети
Если вы запускаете ядро 2C Platform или будущий OCR-сервис в том же `docker-compose.yml` (или в той же сети `2c-platform-net`), используйте внутреннее имя сервиса вместо `localhost`:
- URI для подключения: `ws://surrealdb:8000` или `http://surrealdb:8000`

## 6. Частые операции и устранение неполадок
- **Конфликт порта 8000**: Если порт занят, измените проброс в `docker-compose.yml` на `- "8001:8000"`. Внутренний порт контейнера всегда остаётся `8000`.
- **Очистка кэша или полный сброс**: Остановите контейнер (`docker compose down`), удалите том (`docker volume rm 2cplatform_v3_0_data`), запустите заново.
- **Проверка целостности тома**: `docker volume inspect 2cplatform_v3_0_data`

---
*Документ актуален для ТЗ v3.0. Движок хранения: RocksDB. Сетевая изоляция: включена.*
