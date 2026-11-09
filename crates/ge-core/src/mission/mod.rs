//! Pure mission catalog and selection state. Loading resources, creating a
//! world, and initializing a player remain separate runtime responsibilities.
pub mod actor_definitions;
pub mod animator;
pub mod catalog;
pub mod loader;
pub mod maps;
pub mod player;
pub mod post_trigger;
pub mod registration;
pub mod residency;
pub mod resource_state;
pub mod session;
pub mod source_rng;
pub mod spawn;
pub mod triggers;
pub mod world;
pub mod world_hooks;

pub use catalog::{
    Budget, Difficulty, DuplicateRow, InitialSelection, LevelRegistry, LevelRow, SelectionOutcome,
    TableKind, TuningRow,
};
