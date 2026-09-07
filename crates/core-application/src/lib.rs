pub mod app_registry;
pub mod command_registry;
pub mod permission_manager;
pub mod ports;
pub mod registry;
pub mod seed;

pub use app_registry::AppRegistry;
pub use command_registry::{CommandExecutionCtx, CommandMetadata, CommandRegistry};
pub use permission_manager::PermissionManager;
pub use ports::{
    AuditRepository, CompanyRepository, EntitySchema, EventStore, MetadataRepository,
    ObjectRepository, PermissionPolicyRepository, RoleRepository, UserRepository, WasmHost,
};
pub use registry::CodeRegistry;
pub use seed::seed_system_roles_and_policies;