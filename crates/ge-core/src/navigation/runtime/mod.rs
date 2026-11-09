//! Fresh navigation-wrapper binding and its initial integer caches.
//!
//! The host retains the actual level resource and loaded map instances. These
//! functions own no asset bytes, perform no queries and do not advance the world
//! loader. A successful owner is one initialized component, not a ready world.
//! Repeated native manager history and appending to an existing boundary cache
//! are outside this explicitly fresh construction scope.

mod binding;
mod owner;

pub use binding::{
    Binding, Entity, MapView, Placement, PlacementLocation, SourceInstanceKey, resolve_binding,
};
pub use owner::Owner;

/// Explicit host bounds, not source validation or original allocator policy.
/// These limits apply to one binding/build call. The retaining host also bounds
/// its owner count and aggregate retained faces, cache storage and categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    pub maps: usize,
    pub placements: usize,
    pub faces: usize,
    /// Bounds the existing boundary builder's worst-case allocation (3 * faces),
    /// rather than checking only its final edge count after allocation.
    pub cache_edges: usize,
    pub category_members: usize,
}

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
