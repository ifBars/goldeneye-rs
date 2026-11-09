//! Checked mission data adapters. Runtime selection belongs to ge-core;
//! residency, world registration and player initialization belong to the app.
pub mod common_shapes;
pub mod common_triggers;
mod levels;
pub mod map_runtime;
pub mod maps;
pub mod startpoints;
mod triggers;
pub mod world;

pub use levels::{Budget, LevelCatalog, RowLocations};
pub use triggers::Locator as TriggerLocator;

pub const CATALOG_RESOURCE: u32 = 0x0109_0006;
pub const PRIMARY_SHEET: u32 = 0x1400_0011;
pub const SECONDARY_SHEET: u32 = 0x1400_003a;
