use core_domain::company::Company;
use core_domain::error::DomainError;
use core_domain::event::{Event, StreamType};
use core_domain::object::Object;
use core_domain::role::Role;
use core_domain::types::{AggregateId, Version};
use core_domain::user::{Person, User, UserCertificate, UserCompanyProfile, UserContact};
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