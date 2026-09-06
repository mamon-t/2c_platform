use core_domain::company::Company;
use core_domain::error::DomainError;
use core_domain::event::{Event, StreamType};
use core_domain::metadata::{
    EntityAction, EntityField, EntityForm, EntityRelation, EntityState, EntityTransition, EntityType,
};
use core_domain::object::{Object, ObjectSnapshot};
use core_domain::role::Role;
use core_domain::types::{AggregateId, Version};
use core_domain::user::{Person, User, UserCertificate, UserCompanyProfile, UserContact};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::future::Future;
use uuid::Uuid;

/// Append-only storage of events, the Pipe in the "Pipe and Board" concept.
/// Implemented by `core-infrastructure` on top of the `events` collection.
pub trait EventStore: Send + Sync {
    /// Persists events atomically, in order, keys-by-stream. Must be idempotent
    /// per event ID so that a retried batch does not duplicate entries.
    fn append(&self, events: &[Event])
        -> impl Future<Output = Result<(), DomainError>> + Send;

    /// Loads the full history of a stream, ordered by `version`.
    fn read_stream(
        &self,
        stream_type: StreamType,
        stream_id: &str,
    ) -> impl Future<Output = Result<Vec<Event>, DomainError>> + Send;
}

/// Storage of materialized objects, the Board. Enables OCC through `version`.
///
/// Write methods persist the board record, its new version snapshot and the
/// supplied `events` atomically in a single SurrealDB transaction (the Pipe and
/// the Board advance together, per the spec).
pub trait ObjectRepository: Send + Sync {
    /// Fetches an object together with its current version for optimistic
    /// concurrency checks.
    ///
    /// # Errors
    ///
    /// Returns `DomainError::NotFound` when the object does not exist.
    fn get_with_version(
        &self,
        id: &AggregateId,
    ) -> impl Future<Output = Result<(Object, Version), DomainError>> + Send;

    /// Fetches an object by id.
    fn get(&self, id: &AggregateId) -> impl Future<Output = Result<Object, DomainError>> + Send;

    /// Creates an object with `version == 1`, writes its initial snapshot and
    /// appends the events in one transaction.
    ///
    /// When the object is a document without a `number`, an atomic document
    /// number is assigned inside the same transaction.
    ///
    /// # Errors
    ///
    /// Returns `DomainError::NotFound` when the object already exists.
    fn create(
        &self,
        obj: &Object,
        events: &[Event],
    ) -> impl Future<Output = Result<Object, DomainError>> + Send;

    /// Updates an object applying OCC: the stored version must equal
    /// `obj.version` (the caller's expected version), otherwise
    /// `DomainError::VersionConflict` is returned. On success the object is
    /// stored with `version + 1` and a new snapshot is written.
    fn update(
        &self,
        obj: &Object,
        events: &[Event],
    ) -> impl Future<Output = Result<Object, DomainError>> + Send;

    /// Physically deletes a draft without a change history (`version == 1`).
    ///
    /// # Errors
    ///
    /// Returns `DomainError::ValidationError` when the object has a history,
    /// `DomainError::NotFound` when it does not exist.
    fn delete(
        &self,
        id: &AggregateId,
        events: &[Event],
    ) -> impl Future<Output = Result<(), DomainError>> + Send;

    /// Lists objects of an entity type within a company, newest first,
    /// bounded by `limit`.
    fn list(
        &self,
        entity_type: &str,
        company_id: &str,
        limit: usize,
    ) -> impl Future<Output = Result<Vec<Object>, DomainError>> + Send;

    /// Lists the version history of an object, oldest first.
    fn get_snapshots(
        &self,
        object_id: &AggregateId,
    ) -> impl Future<Output = Result<Vec<ObjectSnapshot>, DomainError>> + Send;

    /// Restores the object to the data/state of `version`, producing a new
    /// object version (`current + 1`) with a fresh snapshot; the history is
    /// never overwritten.
    fn restore_snapshot(
        &self,
        object_id: &AggregateId,
        version: Version,
        events: &[Event],
    ) -> impl Future<Output = Result<Object, DomainError>> + Send;

    /// Atomically advances the per-(entity type, company) counter and returns
    /// the formatted document number `{entity_type}-{YYYY}-{sequential:04}`.
    fn next_document_number(
        &self,
        entity_type: &str,
        company_id: &str,
    ) -> impl Future<Output = Result<String, DomainError>> + Send;
}

/// Host capable of executing a WASM module action and returning its result.
pub trait WasmHost: Send + Sync {
    /// Executes `action` of a module. The host enforces capabilities and
    /// resource limits declared by the module manifest.
    fn execute_module(
        &self,
        module_code: &str,
        action: &str,
        payload: Value,
    ) -> impl Future<Output = Result<Value, DomainError>> + Send;
}

/// Storage of materialized companies.
///
/// Write methods persist both the board record and the supplied `events`
/// atomically in a single SurrealDB transaction (the Pipe and the Board
/// advance together, per the spec).
pub trait CompanyRepository: Send + Sync {
    /// Creates a company and appends its events in one transaction.
    ///
    /// # Errors
    ///
    /// Returns `DomainError::ValidationError` when `code` is already taken.
    fn create(
        &self,
        company: &Company,
        events: &[Event],
    ) -> impl Future<Output = Result<(), DomainError>> + Send;

    /// Fetches a company by id.
    ///
    /// # Errors
    ///
    /// Returns `DomainError::NotFound` when the company does not exist.
    fn get(&self, id: &Uuid) -> impl Future<Output = Result<Company, DomainError>> + Send;

    /// Lists all companies ordered by `code`.
    fn list(&self) -> impl Future<Output = Result<Vec<Company>, DomainError>> + Send;

    /// Updates a company and appends its events in one transaction.
    ///
    /// # Errors
    ///
    /// Returns `DomainError::ValidationError` when `code` collides with
    /// another company, `DomainError::NotFound` when the company is missing.
    fn update(
        &self,
        company: &Company,
        events: &[Event],
    ) -> impl Future<Output = Result<(), DomainError>> + Send;
}

/// Storage of materialized users, persons, contacts, profiles and
/// certificates. Write methods persist the board record and the supplied
/// `events` atomically in a single SurrealDB transaction.
pub trait UserRepository: Send + Sync {
    /// Creates a user with its person and appends events in one transaction.
    ///
    /// # Errors
    ///
    /// Returns `DomainError::ValidationError` when `login` is already taken.
    fn create(
        &self,
        user: &User,
        person: &Person,
        events: &[Event],
    ) -> impl Future<Output = Result<(), DomainError>> + Send;

    /// Fetches a user by id.
    fn get(&self, id: &Uuid) -> impl Future<Output = Result<User, DomainError>> + Send;

    /// Fetches a user by login; used for authentication lookups and duplicate
    /// login checks.
    fn get_by_login(&self, login: &str) -> impl Future<Output = Result<User, DomainError>> + Send;

    /// Lists all users ordered by `login`.
    fn list(&self) -> impl Future<Output = Result<Vec<User>, DomainError>> + Send;

    /// Updates a user and appends events in one transaction.
    fn update(
        &self,
        user: &User,
        events: &[Event],
    ) -> impl Future<Output = Result<(), DomainError>> + Send;

    /// Fetches the identity record of a user.
    fn get_person(&self, user_id: &Uuid) -> impl Future<Output = Result<Person, DomainError>> + Send;

    /// Adds a contact channel and appends its event in one transaction.
    fn add_contact(
        &self,
        contact: &UserContact,
        events: &[Event],
    ) -> impl Future<Output = Result<(), DomainError>> + Send;

    /// Lists contact channels of a user.
    fn list_contacts(
        &self,
        user_id: &Uuid,
    ) -> impl Future<Output = Result<Vec<UserContact>, DomainError>> + Send;

    /// Adds an employment profile and appends its event in one transaction.
    fn add_profile(
        &self,
        profile: &UserCompanyProfile,
        events: &[Event],
    ) -> impl Future<Output = Result<(), DomainError>> + Send;

    /// Lists employment profiles of a user.
    fn list_profiles(
        &self,
        user_id: &Uuid,
    ) -> impl Future<Output = Result<Vec<UserCompanyProfile>, DomainError>> + Send;

    /// Adds a certificate and appends its event in one transaction.
    fn add_certificate(
        &self,
        certificate: &UserCertificate,
        events: &[Event],
    ) -> impl Future<Output = Result<(), DomainError>> + Send;

    /// Lists certificates of a user.
    fn list_certificates(
        &self,
        user_id: &Uuid,
    ) -> impl Future<Output = Result<Vec<UserCertificate>, DomainError>> + Send;
}

/// Storage of materialized roles.
pub trait RoleRepository: Send + Sync {
    /// Creates a role and appends its event in one transaction.
    ///
    /// # Errors
    ///
    /// Returns `DomainError::ValidationError` when `code` is already taken.
    fn create(
        &self,
        role: &Role,
        events: &[Event],
    ) -> impl Future<Output = Result<(), DomainError>> + Send;

    /// Fetches a role by id.
    fn get(&self, id: &Uuid) -> impl Future<Output = Result<Role, DomainError>> + Send;

    /// Lists all roles ordered by `code`.
    fn list(&self) -> impl Future<Output = Result<Vec<Role>, DomainError>> + Send;
}

/// Full declarative snapshot of an entity type: the type itself plus its
/// fields, states, transitions, forms, actions and relations. Assembled by
/// `MetadataRepository::get_schema`, submitted by create/update flows.
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

/// Storage of entity metadata (the metatype model), per section 7 of the spec.
///
/// Entity types are keyed by `code` within a company (`company_id == ""` for
/// platform-wide types). Write methods follow ensure-semantics (section 9):
/// `create_entity_type` creates missing resources and updates existing ones by
/// code only when the supplied `metadata_version` is newer; resources with a
/// not-older version are left untouched (user amendments are preserved).
pub trait MetadataRepository: Send + Sync {
    /// Registers an entity type and its resources with ensure-semantics,
    /// appending the supplied events in the same transaction.
    ///
    /// # Errors
    ///
    /// Returns `DomainError::ValidationError` when a transition references
    /// a missing state, `DomainError::Storage` on persistence failure.
    fn create_entity_type(
        &self,
        schema: &EntitySchema,
        events: &[Event],
    ) -> impl Future<Output = Result<(), DomainError>> + Send;

    /// Fetches an entity type by id.
    ///
    /// # Errors
    ///
    /// Returns `DomainError::NotFound` when the type does not exist.
    fn get_entity_type(
        &self,
        id: &Uuid,
    ) -> impl Future<Output = Result<EntityType, DomainError>> + Send;

    /// Fetches an entity type by code within a company.
    fn get_entity_type_by_code(
        &self,
        company_id: &str,
        code: &str,
    ) -> impl Future<Output = Result<EntityType, DomainError>> + Send;

    /// Lists all entity types ordered by `code`.
    fn list_entity_types(&self) -> impl Future<Output = Result<Vec<EntityType>, DomainError>> + Send;

    /// Re-registers an entity type and its resources, appending the supplied
    /// events in the same transaction. Existing resources are updated by code;
    /// user amendments carry their own versions and are preserved.
    fn update_entity_type(
        &self,
        schema: &EntitySchema,
        events: &[Event],
    ) -> impl Future<Output = Result<(), DomainError>> + Send;

    /// Assembles the full schema snapshot of an entity type of a company.
    fn get_schema(
        &self,
        company_id: &str,
        entity_type: &str,
    ) -> impl Future<Output = Result<EntitySchema, DomainError>> + Send;
}