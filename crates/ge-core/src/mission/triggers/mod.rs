//! Owned source trigger membership and catalog foundations. Map data binding,
//! trigger initialization, and complete gameplay readiness remain distinct.
//! Source-proven constructors/map-init have distinct kind-specific behavior;
//! later update/event/owner callbacks are not implemented by this interface.
pub mod a3;
pub mod active91;
pub mod actor12;
pub mod ad;
pub mod aux_ac;
pub mod common8a;
mod common_shape;
pub mod generic;
mod initialization;
mod numeric;
pub mod radius03;
pub mod radius06;
pub mod radius1b;
pub mod radius74;
pub mod radius88;
pub mod range0c;
pub mod range27;
pub mod range82;
mod registry;
pub mod resource08;
pub mod shape71;
mod types;
pub mod zone;

pub use common_shape::{ShapeBinding, ShapeOutcome, ShapeRequest};
pub use registry::Registry;
pub use types::*;

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
