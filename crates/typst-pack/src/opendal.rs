#![doc = include_str!("../../../docs/opendal-integration.md")]

pub mod location;
pub mod pack_archive;
pub mod pack_assembly;
#[allow(dead_code)]
mod read;
#[allow(dead_code)]
pub mod write;

#[cfg(test)]
#[allow(dead_code)]
#[allow(clippy::collapsible_if)]
#[path = "../tests/support/opendal.rs"]
mod scripted_service;

pub(crate) type BoxError = Box<dyn std::error::Error + Send + Sync>;

pub use ::opendal::Operator;
pub use location::{
    Location, LocationError, LocationRoleError, OperatorBinding, OperatorBindingError,
    OperatorBindings, OperatorBindingsError, OperatorBindingsResolveError, OperatorResolver,
};
