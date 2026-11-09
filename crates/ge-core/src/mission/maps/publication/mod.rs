//! The fresh, base-only path through post-AddMap publication. This owns actual
//! zone bindings and placement bounds, not all-geometry or native stream readiness.
use super::sections::{Owner, Status};
use crate::{
    collision::{
        bounds::{Bounds, EntityBounds, prepare_instance_bounds},
        refresh::{RefreshHierarchy, RefreshLimits, refresh_hierarchy},
    },
    mission::world::Key,
};
use anyhow::{Context, Result, ensure};
use std::cell::RefCell;

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub zones: usize,
    pub placements: usize,
    pub binding_records: usize,
    pub placement_links: usize,
    pub refresh_nodes: usize,
    pub refresh_entries: usize,
}
impl Default for Budget {
    fn default() -> Self {
        Self {
            zones: 65536,
            placements: 1_000_000,
            binding_records: 1_000_000,
            placement_links: 1_000_000,
            refresh_nodes: 1_000_000,
            refresh_entries: 1_000_000,
        }
    }
}

/// Derived from actual scoped world/component owners, not guessed from a level.
#[derive(Debug, Clone, Copy)]
pub struct FreshScope {
    pub live_contexts: usize,
    pub non_null_components: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct Root {
    /// Root +54. Fresh records each own two source-zero constructor words.
    pub binding_count: u8,
    /// Root +55. Its nonzero dynamic consumer remains unsupported here.
    pub dynamic: u8,
}

/// Adapter retains actual source owners. Unrequested zones must not cause root
/// or placement payload reads. Hierarchy callbacks supply serialized INITIAL
/// base bounds, distinct from the component-expanded live bounds committed here.
pub trait Source: RefreshHierarchy {
    fn key(&self) -> Key;
    fn fresh_scope(&self) -> FreshScope;
    fn zone_section_index(&mut self, zone: usize) -> Result<usize>;
    fn zone_root(&mut self, zone: usize) -> Result<Root>;
    fn zone_placement_count(&mut self, zone: usize) -> Result<usize>;
    fn zone_placement(&mut self, zone: usize, ordinal: usize) -> Result<usize>;
    fn zone_tree_root(&mut self, zone: usize) -> Result<Self::Node>;
    /// None is an actual checked lookup miss. An unavailable required provider
    /// or unpublished body must remain an error, not a fabricated missing box.
    fn placement_entity(&mut self, placement: usize) -> Result<Option<EntityBounds>>;
    fn placement_matrix(&mut self, placement: usize) -> Result<[f32; 16]>;
    fn placement_component(&mut self, placement: usize, slot: u8) -> Result<Bounds>;
    fn placement_special(&mut self, placement: usize) -> Result<Option<Bounds>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Binding {
    words: [u32; 2],
}
impl Binding {
    pub fn words(&self) -> &[u32; 2] {
        &self.words
    }
}

#[derive(Debug)]
pub struct Zone {
    initialized: bool,
    bindings: Vec<Binding>,
}
impl Zone {
    /// Source byte +6a, separate from list/cache invalidation bytes +6b..6e.
    pub fn initialized(&self) -> bool {
        self.initialized
    }
    pub fn bindings(&self) -> &[Binding] {
        &self.bindings
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Receipt {
    pub section_queries: usize,
    pub base_ready: usize,
    pub unrequested: usize,
    pub initialized: usize,
    pub retained: usize,
    pub bindings_allocated: usize,
    pub placement_updates: usize,
    pub hierarchy_refreshes: usize,
    pub hierarchy_nodes: usize,
    pub hierarchy_entries: usize,
    pub branch_bounds_written: usize,
}

#[derive(Debug)]
pub struct State {
    key: Key,
    zones: Vec<Zone>,
    placement_bounds: Vec<Option<Bounds>>,
    aggregate: Option<Bounds>,
    failed: bool,
    receipt: Receipt,
}

impl State {
    pub fn from_fresh_constructor(
        key: Key,
        zones: usize,
        placements: usize,
        budget: Budget,
    ) -> Result<Self> {
        ensure!(
            zones <= budget.zones && placements <= budget.placements,
            "map publication constructor budget exceeded"
        );
        Ok(Self {
            key,
            zones: (0..zones)
                .map(|_| Zone {
                    initialized: false,
                    bindings: Vec::new(),
                })
                .collect(),
            placement_bounds: vec![None; placements],
            aggregate: None,
            failed: false,
            receipt: Receipt::default(),
        })
    }
    pub fn key(&self) -> Key {
        self.key
    }
    pub fn zones(&self) -> &[Zone] {
        &self.zones
    }
    pub fn placement_bounds(&self) -> &[Option<Bounds>] {
        &self.placement_bounds
    }
    /// Diagnostic root accumulator; not a replacement for declared map bounds.
    pub fn aggregate(&self) -> Option<Bounds> {
        self.aggregate
    }
    pub fn failed(&self) -> bool {
        self.failed
    }
    pub fn receipt(&self) -> Receipt {
        self.receipt
    }
}

/// Execute source-reached base-only operations. Errors retain reached source
/// stores; failed partial initialization cannot be retried as a successful skip.
/// Parent map owners invalidate their actual list/cache fields before this call;
/// that invalidation deliberately preserves this state's +6a and owned bindings.
pub fn run<S: Source>(
    state: &mut State,
    sections: &Owner,
    source: &mut S,
    budget: Budget,
) -> Result<Receipt> {
    ensure!(
        !state.failed,
        "map publication retains an incomplete prior operation"
    );
    ensure!(
        state.key == source.key() && state.key.wrapper == sections.wrapper(),
        "map publication owner mismatch"
    );
    let scope = source.fresh_scope();
    ensure!(
        scope.live_contexts == 0 && scope.non_null_components == 0,
        "map publication requires unsupported existing context/component processing"
    );
    ensure!(
        sections.loaded_mask().iter().all(|w| *w == 0),
        "map publication requires nonbase section history processing"
    );
    ensure!(
        state.zones.len() <= budget.zones && state.placement_bounds.len() <= budget.placements,
        "map publication state budget exceeded"
    );
    state.receipt = Receipt::default();
    let result = execute(state, sections, source, budget);
    if result.is_err() {
        state.failed = true;
    }
    result.map(|()| state.receipt)
}

fn execute<S: Source>(
    state: &mut State,
    sections: &Owner,
    source: &mut S,
    budget: Budget,
) -> Result<()> {
    let mut changed = false;
    for zone in 0..state.zones.len() {
        let section = source.zone_section_index(zone)?;
        state.receipt.section_queries += 1;
        match sections.status(section)? {
            Status::Unrequested => {
                state.receipt.unrequested += 1;
                continue;
            }
            Status::BasePublished => state.receipt.base_ready += 1,
        }
        let newly_initialized = !state.zones[zone].initialized;
        // 80316f10 reaches this flag store before its entity/provider reads.
        if newly_initialized {
            state.zones[zone].initialized = true;
        }
        let root = source.zone_root(zone)?;
        if newly_initialized {
            let count = usize::from(root.binding_count);
            ensure!(
                state
                    .receipt
                    .bindings_allocated
                    .checked_add(count)
                    .context("binding count overflow")?
                    <= budget.binding_records,
                "map publication binding budget exceeded"
            );
            state.zones[zone].bindings = vec![Binding { words: [0; 2] }; count];
            state.receipt.bindings_allocated += count;
            let links = source.zone_placement_count(zone)?;
            ensure!(
                state
                    .receipt
                    .placement_updates
                    .checked_add(links)
                    .context("placement link count overflow")?
                    <= budget.placement_links,
                "map publication placement link budget exceeded"
            );
            for ordinal in 0..links {
                let placement = source.zone_placement(zone, ordinal)?;
                ensure!(
                    placement < state.placement_bounds.len(),
                    "zone placement outside owned map"
                );
                let entity = source.placement_entity(placement)?;
                let bounds = if let Some(entity) = entity {
                    let matrix = source.placement_matrix(placement)?;
                    let shared = RefCell::new(&mut *source);
                    prepare_instance_bounds(
                        &matrix,
                        Some(entity),
                        |slot| shared.borrow_mut().placement_component(placement, slot),
                        || shared.borrow_mut().placement_special(placement),
                    )?
                } else {
                    Bounds::MISSING_ENTITY
                };
                state.placement_bounds[placement] = Some(bounds);
                state.receipt.placement_updates += 1;
            }
            changed = true;
            state.receipt.initialized += 1;
        } else {
            state.receipt.retained += 1;
        }
        ensure!(
            root.dynamic == 0,
            "map publication dynamic root processing unsupported"
        );
    }
    if changed {
        let mut aggregate = Bounds::MISSING_ENTITY;
        // Source refreshes ALL serialized zone trees after any zone initialized.
        // These inputs remain in the base block even for unrequested geometry.
        for zone in 0..state.zones.len() {
            let root = source.zone_tree_root(zone)?;
            let nodes = budget
                .refresh_nodes
                .checked_sub(state.receipt.hierarchy_nodes)
                .context("hierarchy node budget exceeded")?;
            let entries = budget
                .refresh_entries
                .checked_sub(state.receipt.hierarchy_entries)
                .context("hierarchy entry budget exceeded")?;
            let stats = refresh_hierarchy(
                source,
                root,
                &mut aggregate,
                RefreshLimits {
                    node_visits: nodes,
                    entry_tests: entries,
                },
            )?;
            state.receipt.hierarchy_refreshes += 1;
            state.receipt.hierarchy_nodes += stats.node_visits;
            state.receipt.hierarchy_entries += stats.branch_entries + stats.leaf_entries;
            state.receipt.branch_bounds_written += stats.branch_bounds_written;
            state.aggregate = Some(aggregate);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
