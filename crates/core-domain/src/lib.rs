pub mod aggregate;
pub mod audit;
pub mod command;
pub mod company;
pub mod error;
pub mod event;
pub mod metadata;
pub mod module;
pub mod object;
pub mod password;
pub mod permission;
pub mod role;
pub mod script;
pub mod types;
pub mod user;
pub mod wasm_manifest;

pub use aggregate::AggregateRoot;
pub use audit::{AuditEntry, AuditFilter, AuditResult, AuditTarget};
pub use command::Command;
pub use company::Company;
pub use error::DomainError;
pub use event::{ActorSnapshot, Event, StreamType};
pub use metadata::{
    EntityAction, EntityField, EntityForm, EntityKind, EntityRelation, EntityState, EntityTransition,
    EntityType, FieldType, OnDelete, RelationKind,
};
pub use module::{
    CompanyModule, CompanyModulePayload, ModuleLifecyclePayload, ModuleRecord, ModuleState,
    PluginCallContext,
};
pub use password::{hash_password, verify_password};
pub use object::{Object, ObjectKind, ObjectSnapshot};
pub use permission::{PermissionPolicy, PermissionScopeType, RecordAccessLevel};
pub use role::Role;
pub use script::{Script, ScriptType};
pub use types::{AggregateId, Version};
pub use user::{
    ContactChannelType, ContactPurpose, Person, User, UserCertificate, UserCompanyProfile,
    UserContact, UserStatus,
};
pub use wasm_manifest::{
    DependencySpec, ManifestCommand, ManifestField, ManifestNavItem, ManifestObjectSchema,
    ManifestPermission, ManifestResource, ModuleManifest, ALLOWED_CAPABILITIES,
};