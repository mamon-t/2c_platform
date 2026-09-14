use core_domain::audit::{AuditEntry, AuditFilter};
use core_domain::company::Company;
use core_domain::error::DomainError;
use core_domain::event::{ActorSnapshot, Event, StreamType};
use core_domain::metadata::{
    EntityAction, EntityField, EntityForm, EntityRelation, EntityState, EntityTransition, EntityType,
};
use core_domain::module::{ModuleRecord, PluginCallContext};
use core_domain::permission::PermissionPolicy;
use core_domain::object::{Object, ObjectSnapshot};
use core_domain::role::Role;
use core_domain::script::Script;
use core_domain::types::{AggregateId, Version};
use core_domain::wasm_manifest::ModuleManifest;
use core_domain::user::{Person, User, UserCertificate, UserCompanyProfile, UserContact};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;
use uuid::Uuid;
use chrono::{DateTime, Utc};

use crate::script_context::ScriptContext;

/// Пинованный boxed-футур для dyn-совместимых методов портов.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Хранилище событий с журналом только с добавлением — Труба в концепции «Трубы и Доски».
/// Реализовано в `core-infrastructure` поверх коллекции `events`.
pub trait EventStore: Send + Sync {
    /// Добавляет события атомарно, по порядку, с ключами по потокам. Должен быть идемпотентным
    /// для каждого ID события, чтобы повторная отправка пакета не создавала дублей.
    ///
    /// Методы возвращают `BoxFuture` вместо `impl Future`, чтобы трейт был
    /// объектно-безопасным (позволяет `Arc<dyn EventStore>` на транспортном слое).
    fn append<'a>(&'a self, events: &[Event]) -> BoxFuture<'a, Result<(), DomainError>>;

    /// Загружает полную историю потока, упорядоченную по `version`.
    fn read_stream<'a>(
        &'a self,
        stream_type: StreamType,
        stream_id: &str,
    ) -> BoxFuture<'a, Result<Vec<Event>, DomainError>>;
}

/// Хранилище материализованных объектов — Доска. Поддерживает OCC (оптимистичную блокировку) через `version`.
///
/// Методы записи сохраняют запись Доски, снимок её новой версии и переданные
/// `events` атомарно в единой транзакции SurrealDB (Труба и
/// Доска продвигаются вместе, согласно ТЗ).
pub trait ObjectRepository: Send + Sync {
    /// Получает объект вместе с его текущей версией для проверок оптимистичной
    /// блокировки.
    ///
    /// # Ошибки
    ///
    /// Возвращает `DomainError::NotFound`, если объект не существует.
    fn get_with_version(
        &self,
        id: &AggregateId,
    ) -> BoxFuture<'_, Result<(Object, Version), DomainError>>;

    /// Получает объект по id.
    fn get(&self, id: &AggregateId) -> BoxFuture<'_, Result<Object, DomainError>>;

    /// Создаёт объект с `version == 1`, записывает его начальный снимок и
    /// добавляет события в одной транзакции.
    ///
    /// Если объект является документом без `number`, в той же транзакции
    /// присваивается атомарный номер документа.
    ///
    /// # Ошибки
    ///
    /// Возвращает `DomainError::NotFound`, если объект уже существует.
    fn create(
        &self,
        obj: &Object,
        events: &[Event],
    ) -> BoxFuture<'_, Result<Object, DomainError>>;

    /// Обновляет объект с применением OCC: сохранённая версия должна совпадать
    /// с `obj.version` (ожидаемой версией вызывающей стороны), иначе возвращается
    /// `DomainError::VersionConflict`. При успехе объект сохраняется
    /// с `version + 1` и записывается новый снимок.
    fn update(
        &self,
        obj: &Object,
        events: &[Event],
    ) -> BoxFuture<'_, Result<Object, DomainError>>;

    /// Атомарно применяет пачку обновлений объектов в одной транзакции
    /// SurrealDB: обороты, снимки и события всех записей коммитятся вместе,
    /// а сбой любого обновления (включая `DomainError::VersionConflict`)
    /// откатывает всю пачку целиком.
    ///
    /// OCC проверяется для каждой пары `(Object, events)` по её `version`;
    /// возвращённые объекты содержат сохранённые версии (`current + 1`).
    fn update_batch(
        &self,
        ops: &[(Object, Vec<Event>)],
    ) -> BoxFuture<'_, Result<Vec<Object>, DomainError>>;

    /// Физически удаляет черновик без истории изменений (`version == 1`).
    ///
    /// # Ошибки
    ///
    /// Возвращает `DomainError::ValidationError`, если у объекта есть история,
    /// `DomainError::NotFound`, если он не существует.
    fn delete(
        &self,
        id: &AggregateId,
        events: &[Event],
    ) -> BoxFuture<'_, Result<(), DomainError>>;

    /// Перечисляет объекты типа сущности в компании, сначала самые новые,
    /// ограниченные по `limit`.
    fn list(
        &self,
        entity_type: &str,
        company_id: &str,
        limit: usize,
    ) -> BoxFuture<'_, Result<Vec<Object>, DomainError>>;

    /// Возвращает общее количество объектов типа сущности в компании
    /// (без применения `limit`); используется для пагинации и `total_count`.
    fn count(
        &self,
        entity_type: &str,
        company_id: &str,
    ) -> BoxFuture<'_, Result<u64, DomainError>>;

    /// Перечисляет историю версий объекта, сначала самые старые.
    fn get_snapshots(
        &self,
        object_id: &AggregateId,
    ) -> BoxFuture<'_, Result<Vec<ObjectSnapshot>, DomainError>>;

    /// Восстанавливает объект к данным/состоянию `version`, создавая новую
    /// версию объекта (`current + 1`) со свежим снимком; история
    /// никогда не перезаписывается.
    fn restore_snapshot(
        &self,
        object_id: &AggregateId,
        version: Version,
        events: &[Event],
    ) -> BoxFuture<'_, Result<Object, DomainError>>;

    /// Атомарно наращивает счётчик для пары (тип сущности, компания) и возвращает
    /// форматированный номер документа `{entity_type}-{YYYY}-{sequential:04}`.
    fn next_document_number(
        &self,
        entity_type: &str,
        company_id: &str,
    ) -> BoxFuture<'_, Result<String, DomainError>>;
}

/// Хост для WASM-модулей (Extism): загрузка, исполнение и выгрузка модуля.
/// Host-функции (`whoami`, KV, объекты и т.д.) выполняются в namespace
/// `ExtismHost` и подчиняются capabilities манифеста (Приложения №4-№6 ТЗ v3.0).
pub trait WasmHost: Send + Sync {
    /// Загружает WASM-модуль: компилирует байты, вызывает `get_info()`,
    /// валидирует манифест v2 и подготавливает host-функции. Повторная
    /// загрузка того же кода заменяет ранее загруженную версию.
    ///
    /// # Ошибки
    ///
    /// Возвращает `DomainError::ValidationError`, если манифест не проходит
    /// проверку, либо `DomainError::Storage` при ошибке компиляции плагина.
    fn load_module(
        &self,
        code: &str,
        wasm_bytes: &[u8],
    ) -> BoxFuture<'_, Result<ModuleManifest, DomainError>>;

    /// Вызывает экспортированную функцию `function` модуля с входными
    /// байтами `input` и возвращает байты вывода. Хост соблюдает ресурсные
    /// лимиты (топливо, память, таймаут) и capabilities модуля.
    ///
    /// # Ошибки
    ///
    /// Возвращает `DomainError::NotFound`, если модуль не загружен, или
    /// `DomainError::ValidationError` при ошибке исполнения.
    fn call_function(
        &self,
        code: &str,
        function: &str,
        input: &[u8],
    ) -> BoxFuture<'_, Result<Vec<u8>, DomainError>>;

    /// Вызывает функцию модуля с предоставленным контекстом единичного вызова
    /// (компания, исполнитель, capabilities, настройки). Используется
    /// командами `plugin.{code}.{name}` из сценария 9b.
    fn invoke_with_context(
        &self,
        code: &str,
        function: &str,
        input: &[u8],
        ctx: PluginCallContext,
    ) -> BoxFuture<'_, Result<Vec<u8>, DomainError>>;

    /// Выгружает модуль из памяти; не влияет на установку в БД.
    fn unload_module(&self, code: &str) -> BoxFuture<'_, Result<(), DomainError>>;

    /// Возвращает `true`, если модуль с заданным кодом загружен в память.
    fn is_loaded(&self, code: &str) -> BoxFuture<'_, bool>;
}

/// Хранилище материализованных компаний.
///
/// Методы записи сохраняют и запись Доски, и переданные `events`
/// атомарно в единой транзакции SurrealDB (Труба и Доска
/// продвигаются вместе, согласно ТЗ).
pub trait CompanyRepository: Send + Sync {
    /// Создаёт компанию и добавляет её события в одной транзакции.
    ///
    /// # Ошибки
    ///
    /// Возвращает `DomainError::ValidationError`, если `code` уже занят.
    fn create(
        &self,
        company: &Company,
        events: &[Event],
    ) -> impl Future<Output = Result<(), DomainError>> + Send;

    /// Получает компанию по id.
    ///
    /// # Ошибки
    ///
    /// Возвращает `DomainError::NotFound`, если компания не существует.
    fn get(&self, id: &Uuid) -> impl Future<Output = Result<Company, DomainError>> + Send;

    /// Перечисляет все компании, упорядоченные по `code`.
    fn list(&self) -> impl Future<Output = Result<Vec<Company>, DomainError>> + Send;

    /// Обновляет компанию и добавляет её события в одной транзакции.
    ///
    /// # Ошибки
    ///
    /// Возвращает `DomainError::ValidationError`, если `code` совпадает
    /// с другой компанией, `DomainError::NotFound`, если компания отсутствует.
    fn update(
        &self,
        company: &Company,
        events: &[Event],
    ) -> impl Future<Output = Result<(), DomainError>> + Send;
}

/// Хранилище материализованных пользователей, персон, контактов, профилей и
/// сертификатов. Методы записи сохраняют запись Доски и переданные
/// `events` атомарно в единой транзакции SurrealDB.
///
/// Методы используют `BoxFuture` для dyn-совместимости: трейт используется
/// как `Arc<dyn UserRepository>` в транспортном слое (`core-api`).
pub trait UserRepository: Send + Sync {
    /// Создаёт пользователя вместе с персоной и добавляет события в одной транзакции.
    ///
    /// # Ошибки
    ///
    /// Возвращает `DomainError::ValidationError`, если `login` уже занят.
    fn create(
        &self,
        user: &User,
        person: &Person,
        events: &[Event],
    ) -> BoxFuture<'_, Result<(), DomainError>>;

    /// Получает пользователя по id.
    fn get(&self, id: &Uuid) -> BoxFuture<'_, Result<User, DomainError>>;

    /// Получает пользователя по логину; используется для аутентификации и проверок
    /// дубликатов логинов.
    fn get_by_login(&self, login: &str) -> BoxFuture<'_, Result<User, DomainError>>;

    /// Перечисляет всех пользователей, упорядоченных по `login`.
    fn list(&self) -> BoxFuture<'_, Result<Vec<User>, DomainError>>;

    /// Перечисляет активных (не `Archived`) пользователей с назначенной ролью.
    ///
    /// Роли ограничены компаниями, а `User` не хранит `company_id`, поэтому
    /// принадлежность проверяется через роль: если роль с `role_id` не существует
    /// или принадлежит другой компании — возвращается пустой список (не ошибка).
    fn list_by_role(
        &self,
        role_id: Uuid,
        company_id: Uuid,
    ) -> BoxFuture<'_, Result<Vec<User>, DomainError>>;

    /// Обновляет пользователя и добавляет события в одной транзакции.
    fn update(&self, user: &User, events: &[Event]) -> BoxFuture<'_, Result<(), DomainError>>;

    /// Получает запись персоны пользователя.
    fn get_person(&self, user_id: &Uuid) -> BoxFuture<'_, Result<Person, DomainError>>;

    /// Добавляет канал связи и соответствующее ему событие в одной транзакции.
    fn add_contact(
        &self,
        contact: &UserContact,
        events: &[Event],
    ) -> BoxFuture<'_, Result<(), DomainError>>;

    /// Перечисляет каналы связи пользователя.
    fn list_contacts(&self, user_id: &Uuid) -> BoxFuture<'_, Result<Vec<UserContact>, DomainError>>;

    /// Добавляет профиль трудоустройства и соответствующее ему событие в одной транзакции.
    fn add_profile(
        &self,
        profile: &UserCompanyProfile,
        events: &[Event],
    ) -> BoxFuture<'_, Result<(), DomainError>>;

    /// Перечисляет профили трудоустройства пользователя.
    fn list_profiles(
        &self,
        user_id: &Uuid,
    ) -> BoxFuture<'_, Result<Vec<UserCompanyProfile>, DomainError>>;

    /// Добавляет сертификат и соответствующее ему событие в одной транзакции.
    fn add_certificate(
        &self,
        certificate: &UserCertificate,
        events: &[Event],
    ) -> BoxFuture<'_, Result<(), DomainError>>;

    /// Перечисляет сертификаты пользователя.
    fn list_certificates(
        &self,
        user_id: &Uuid,
    ) -> BoxFuture<'_, Result<Vec<UserCertificate>, DomainError>>;
}

/// Хранилище материализованных ролей. Методы используют `BoxFuture` для
/// dyn-совместимости (трейт используется как `Arc<dyn RoleRepository>` в
/// [`PermissionManager`](crate::PermissionManager)).
pub trait RoleRepository: Send + Sync {
    /// Создаёт роль и добавляет её событие в одной транзакции.
    ///
    /// # Ошибки
    ///
    /// Возвращает `DomainError::ValidationError`, если `code` занят в компании.
    fn create(&self, role: &Role, events: &[Event]) -> BoxFuture<'_, Result<(), DomainError>>;

    /// Получает роль по id.
    fn get(&self, id: &Uuid) -> BoxFuture<'_, Result<Role, DomainError>>;

    /// Получает роль по коду в рамках компании; используется при ensure-сидинге.
    fn get_by_code(&self, company_id: &Uuid, code: &str) -> BoxFuture<'_, Result<Role, DomainError>>;

    /// Обновляет роль и добавляет событие в одной транзакции.
    ///
    /// # Ошибки
    ///
    /// Возвращает `DomainError::NotFound`, если роль отсутствует.
    fn update(&self, role: &Role, events: &[Event]) -> BoxFuture<'_, Result<(), DomainError>>;

    /// Перечисляет все роли, упорядоченные по `code`.
    fn list(&self) -> BoxFuture<'_, Result<Vec<Role>, DomainError>>;

    /// Собирает коды политик доступа всех ролей пользователя в компании:
    /// загружает роли через `user.role_ids`, фильтрует по `company_id`,
    /// объединяет и дедуплицирует `permission_policy_codes`.
    fn get_policies_for_user(
        &self,
        user_id: &Uuid,
        company_id: &Uuid,
    ) -> BoxFuture<'_, Result<Vec<String>, DomainError>>;
}

/// Хранилище политик доступа (`permission_policies`, Приложение №7 ТЗ v3.1).
/// Использует `BoxFuture` для dyn-совместимости (см. [`RoleRepository`]).
pub trait PermissionPolicyRepository: Send + Sync {
    /// Вносит политику с ensure-семантикой по коду: вставка идемпотентна,
    /// существующая политика не перезаписывается.
    fn upsert(&self, policy: &PermissionPolicy) -> BoxFuture<'_, Result<(), DomainError>>;

    /// Получает политику по коду.
    fn get_by_code(&self, code: &str) -> BoxFuture<'_, Result<PermissionPolicy, DomainError>>;

    /// Получает политики по списку кодов, пропуская отсутствующие.
    fn get_by_codes(&self, codes: &[String]) -> BoxFuture<'_, Result<Vec<PermissionPolicy>, DomainError>>;
}

/// Полный декларативный снимок типа сущности: сам тип, а также его
/// поля, состояния, переходы, формы, действия и связи. Собирается методом
/// `MetadataRepository::get_schema`, используется в потоках создания/обновления.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySchema {
    pub entity_type: EntityType,
    pub fields: Vec<EntityField>,
    pub states: Vec<EntityState>,
    pub transitions: Vec<EntityTransition>,
    pub forms: Vec<EntityForm>,
    pub actions: Vec<EntityAction>,
    pub relations: Vec<EntityRelation>,
}

/// Хранилище метаданных сущностей (метатип-модель), согласно разделу 7 ТЗ.
///
/// Типы сущностей ключуются по `code` в рамках компании (`company_id == ""` для
/// общеплатформенных типов). Методы записи следуют ensure-семантике (раздел 9):
/// `create_entity_type` создаёт отсутствующие ресурсы и обновляет существующие по
/// коду только когда переданный `metadata_version` новее; ресурсы с
/// не более новой версией остаются без изменений (пользовательские правки сохраняются).
pub trait MetadataRepository: Send + Sync {
    /// Регистрирует тип сущности и его ресурсы с ensure-семантикой,
    /// добавляя переданные события в той же транзакции.
    ///
    /// # Ошибки
    ///
    /// Возвращает `DomainError::ValidationError`, когда переход ссылается
    /// на отсутствующее состояние, `DomainError::Storage` при сбое сохранения.
    fn create_entity_type(
        &self,
        schema: &EntitySchema,
        events: &[Event],
    ) -> BoxFuture<'_, Result<(), DomainError>>;

    /// Получает тип сущности по id.
    ///
    /// # Ошибки
    ///
    /// Возвращает `DomainError::NotFound`, если тип не существует.
    fn get_entity_type(&self, id: &Uuid) -> BoxFuture<'_, Result<EntityType, DomainError>>;

    /// Получает тип сущности по коду в рамках компании.
    fn get_entity_type_by_code(
        &self,
        company_id: &str,
        code: &str,
    ) -> BoxFuture<'_, Result<EntityType, DomainError>>;

    /// Перечисляет все типы сущностей, упорядоченные по `code`.
    fn list_entity_types(&self) -> BoxFuture<'_, Result<Vec<EntityType>, DomainError>>;

    /// Повторно регистрирует тип сущности и его ресурсы, добавляя переданные
    /// события в той же транзакции. Существующие ресурсы обновляются по коду;
    /// пользовательские правки имеют собственные версии и сохраняются.
    fn update_entity_type(
        &self,
        schema: &EntitySchema,
        events: &[Event],
    ) -> BoxFuture<'_, Result<(), DomainError>>;

    /// Собирает полный снимок схемы типа сущности компании.
    fn get_schema(
        &self,
        company_id: &str,
        entity_type: &str,
    ) -> BoxFuture<'_, Result<EntitySchema, DomainError>>;
}

/// Хранилище установленных WASM-модулей: каталог `modules` (глобальная запись
/// с манифестными данными) и проекция `company_modules` (включение для
/// конкретной компании). Методы записи транзакционно продвигают Трубу
/// (события `module.*`) и Доску (записи таблиц) вместе, согласно концепции.
pub trait ModuleRepository: Send + Sync {
    /// Устанавливает модуль в каталог `modules` с ensure-семантикой по коду.
    ///
    /// # Ошибки
    ///
    /// Возвращает `DomainError::ValidationError`, если модуль с таким кодом
    /// уже установлен.
    fn install(
        &self,
        record: &ModuleRecord,
        events: &[Event],
    ) -> BoxFuture<'_, Result<(), DomainError>>;

    /// Помечает модуль как `Uninstalled` и возвращает обновлённую запись.
    ///
    /// # Ошибки
    ///
    /// Возвращает `DomainError::NotFound`, если модуль не установлен.
    fn uninstall(
        &self,
        code: &str,
        events: &[Event],
    ) -> BoxFuture<'_, Result<ModuleRecord, DomainError>>;

    /// Получает запись установленного модуля по коду.
    ///
    /// # Ошибки
    ///
    /// Возвращает `DomainError::NotFound`, если модуль не установлен.
    fn get(&self, code: &str) -> BoxFuture<'_, Result<ModuleRecord, DomainError>>;

    /// Перечисляет все записи каталога модулей, упорядоченные по коду.
    fn list(&self) -> BoxFuture<'_, Result<Vec<ModuleRecord>, DomainError>>;

    /// Включает модуль для компании (строго один раз на пару компания+код).
    fn enable_for_company(
        &self,
        company_id: &str,
        code: &str,
        events: &[Event],
    ) -> BoxFuture<'_, Result<(), DomainError>>;

    /// Отключает модуль для компании. Отсутствие пары — не ошибка.
    fn disable_for_company(
        &self,
        company_id: &str,
        code: &str,
        events: &[Event],
    ) -> BoxFuture<'_, Result<(), DomainError>>;

    /// Возвращает установленные модули, включённые для компании.
    fn list_enabled_for_company(
        &self,
        company_id: &str,
    ) -> BoxFuture<'_, Result<Vec<ModuleRecord>, DomainError>>;

    /// Перечисляет компании, для которых модуль включён. Пары упорядочены
    /// по `company_id`.
    fn list_enabled_companies(&self, code: &str) -> BoxFuture<'_, Result<Vec<String>, DomainError>>;

    /// Проверяет, включён ли модуль для компании.
    fn is_enabled_for_company(
        &self,
        company_id: &str,
        code: &str,
    ) -> BoxFuture<'_, Result<bool, DomainError>>;
}

/// Хранилище операционного аудита (`audit_log`) — отдельная подсистема,
/// отличная от Event Store (раздел 8.5 ТЗ v3.1). Записи только добавляются
/// (append-only); физическое удаление и архивация — вне области действия
/// этого порта. Использует `BoxFuture` для dyn-совместимости (хранится как
/// `Arc<dyn AuditRepository>` в `CommandExecutionPipeline`).
pub trait AuditRepository: Send + Sync {
    /// Добавляет новую запись аудита. Идентификатор формируется вызывающей
    /// стороной; повторная передача той же записи идемпотентна.
    fn log(&self, entry: AuditEntry) -> BoxFuture<'_, Result<(), DomainError>>;

    /// Возвращает записи, удовлетворяющие фильтру, в порядке убывания
    /// `timestamp` (самые новые первыми).
    fn query(&self, filter: AuditFilter) -> BoxFuture<'_, Result<Vec<AuditEntry>, DomainError>>;
}

/// Выпущенный токен доступа. Сериализуется в ответе `user.login`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AuthToken {
    pub access_token: String,
    pub expires_at: DateTime<Utc>,
    pub token_type: &'static str,
}

/// Издаёт и проверяет токены доступа (ТЗ v3.1, §10b).
pub trait TokenManager: Send + Sync {
    /// Выпускает токен для данного исполнителя.
    ///
    /// # Errors
    ///
    /// Возвращает `DomainError::Storage` при сбое подписи/кодирования.
    fn issue(&self, actor: &ActorSnapshot) -> Result<AuthToken, DomainError>;

    /// Разбирает токен и восстанавливает исполнителя.
    ///
    /// # Errors
    ///
    /// Возвращает `DomainError::PermissionDenied`, если токен невалиден, просрочен
    /// или подписан неизвестным ключом.
    fn parse(&self, token: &str) -> Result<ActorSnapshot, DomainError>;
}

/// Хранилище скриптов Rhai (`scripts`) — Доска для модели `Script`.
/// События `script.created/updated/deleted` пишутся в Трубу через `events`.
pub trait ScriptRepository: Send + Sync {
    /// Создаёт скрипт. Код уникален в рамках компании (`None` — глобальный).
    fn create<'a>(
        &'a self,
        script: &'a Script,
        events: &'a [Event],
    ) -> BoxFuture<'a, Result<Script, DomainError>>;

    /// Возвращает скрипт по идентификатору.
    fn get<'a>(&'a self, id: &'a Uuid) -> BoxFuture<'a, Result<Option<Script>, DomainError>>;

    /// Возвращает скрипт по коду (в рамках компании; `company_id == None` — поиск
    /// по глобальным скриптам). Коллизии в глобальном пространстве исключены
    /// UNIQUE-индексом на `(code, coalesce(company_id,'global'))`.
    fn get_by_code<'a>(
        &'a self,
        code: &'a str,
        company_id: Option<&'a Uuid>,
    ) -> BoxFuture<'a, Result<Option<Script>, DomainError>>;

    /// Перечисляет скрипты. При `company_id == Some` — скрипты компании
    /// и глобальные; при `None` — только глобальные.
    fn list<'a>(
        &'a self,
        company_id: Option<&'a Uuid>,
    ) -> BoxFuture<'a, Result<Vec<Script>, DomainError>>;

    /// Полностью заменяет скрипт (OCC по переданной записи не выполняется;
    /// версия задаётся вызывающей стороной через события).
    fn update<'a>(
        &'a self,
        script: &'a Script,
        events: &'a [Event],
    ) -> BoxFuture<'a, Result<Script, DomainError>>;

    /// Удаляет скрипт по идентификатору. Отсутствие — не ошибка.
    fn delete<'a>(
        &'a self,
        id: &'a Uuid,
        events: &'a [Event],
    ) -> BoxFuture<'a, Result<(), DomainError>>;

    /// Удаляет все скрипты модуля (используется при `unregister` модуля).
    /// Отсутствие скриптов — не ошибка.
    fn delete_by_module<'a>(
        &'a self,
        module_code: &'a str,
        events: &'a [Event],
    ) -> BoxFuture<'a, Result<(), DomainError>>;
}

/// Исполнитель скриптов Rhai с песочницей (ТЗ v3.1 §15). Реализация
/// `RhaiScriptEngine` живёт в `core-infrastructure`; этот порт держит
/// `core-application` и `core-infrastructure` независимыми от Rhai.
pub trait ScriptEngine: Send + Sync {
    /// Выполняет скрипт с заданным контекстом и возвращает результат как JSON.
    ///
    /// # Errors
    ///
    /// Возвращает `DomainError::ValidationError` при ошибке компиляции/выполнения и
    /// `DomainError::Storage` при превышении лимита операций или таймаута.
    fn execute<'a>(
        &'a self,
        source: String,
        context: &'a ScriptContext,
    ) -> BoxFuture<'a, Result<serde_json::Value, DomainError>>;

    /// Компилирует скрипт без выполнения. Используется командой `script.validate`.
    ///
    /// # Errors
    ///
    /// Возвращает `DomainError::ValidationError` при синтаксической ошибке.
    fn validate(&self, source: &str) -> Result<(), DomainError>;
}