//! Asynchronous storage integration using caller-supplied OpenDAL operators.
//!
//! Backend services, credentials, transports, TLS, executors, runtimes, layers,
//! retry policy, and polling remain the caller's responsibility.

#[allow(dead_code)]
mod acquisition;
#[allow(dead_code)]
mod compilation;
pub mod location;
pub mod pack_archive;
pub mod pack_assembly;
#[allow(dead_code)]
pub mod publication;

#[cfg(test)]
#[allow(dead_code)]
#[allow(clippy::collapsible_if)]
#[path = "../tests/support/opendal.rs"]
mod scripted_service;

pub use ::opendal::Operator;
pub use location::{
    Location, LocationError, LocationRoleError, OperatorBinding, OperatorBindingError,
    OperatorBindings, OperatorBindingsError, OperatorBindingsResolveError, OperatorResolver,
};
