pub mod aggregate;
pub mod command;
pub mod company;
pub mod error;
pub mod event;
pub mod object;
pub mod role;
pub mod types;
pub mod user;

pub use aggregate::AggregateRoot;
pub use command::Command;
pub use company::Company;
pub use error::DomainError;
pub use event::{Event, EventMetadata, StreamType};
pub use object::{Object, ObjectKind};
pub use role::Role;
pub use types::{AggregateId, Version};
pub use user::{
    ContactChannelType, ContactPurpose, Person, User, UserCertificate, UserCompanyProfile,
    UserContact, UserStatus,
};