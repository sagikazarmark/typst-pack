//! Asynchronous storage integration using caller-supplied OpenDAL operators.
//!
//! Backend services, credentials, transports, TLS, executors, runtimes, layers,
//! retry policy, and polling remain the caller's responsibility.

pub mod location;

pub use ::opendal::Operator;
pub use location::{
    Location, LocationError, LocationRoleError, OperatorBinding, OperatorBindingError,
    OperatorBindings, OperatorBindingsError, OperatorBindingsResolveError, OperatorResolver,
};
