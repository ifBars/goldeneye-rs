//! Retained mission resources and host loading adapters. Catalog selection is
//! pure core state; this module never equates resource reads with a ready world.
pub mod cli;
mod context;
mod metadata;
mod player;
mod registration;
mod residency;
pub(crate) mod resource;
mod spawn;
pub(crate) mod world;

pub use context::MissionContext;

#[cfg(test)]
#[path = "tests/fixtures.rs"]
mod fixtures;
