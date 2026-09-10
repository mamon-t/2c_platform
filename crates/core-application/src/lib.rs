pub mod app_registry;
pub mod command_registry;
pub mod module_manager;
pub mod permission_manager;
pub mod ports;
pub mod registry;
pub mod seed;
pub mod transaction_orchestrator;

pub use app_registry::AppRegistry;
pub use command_registry::{CommandExecutionCtx, CommandMetadata, CommandRegistry};
pub use module_manager::ModuleManager;
pub use permission_manager::PermissionManager;
pub use ports::{
    AuditRepository, CompanyRepository, EntitySchema, EventStore, MetadataRepository,
    ObjectRepository, PermissionPolicyRepository, RoleRepository, UserRepository, WasmHost,
};
pub use registry::CodeRegistry;
pub use seed::seed_system_roles_and_policies;
pub use transaction_orchestrator::TransactionOrchestrator;