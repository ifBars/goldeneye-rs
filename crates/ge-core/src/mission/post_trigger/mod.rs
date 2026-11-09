//! The selected-definition operation from the post-trigger source world hook.
//!
//! Preparation consumes the retained registry once. Resource execution belongs
//! to the host; retries use the retained requests rather than selecting again.
//! Returned NULL and non-NULL source wrappers both advance the caller, but
//! neither proves section residency, native streaming completion or readiness.
mod operation;
mod types;

pub use operation::Operation;
pub use types::{PreparationError, Request, SelectedDefinition, SkipReason, TailReset};

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
