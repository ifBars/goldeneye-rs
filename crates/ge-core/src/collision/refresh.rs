//! Mutable candidate-hierarchy bounds refresh over explicit source storage.
use super::{bounds::Bounds, world::StoredBounds};
use anyhow::{Result, ensure};
use serde::Serialize;

/// Addressing and initial placement preparation stay in the adapter. No method
/// reads membership flags: refresh includes every addressed leaf placement.
pub trait RefreshHierarchy {
    type Node: Copy;
    fn child_count(&mut self, node: Self::Node) -> Result<u8>;
    fn leaf_count(&mut self, node: Self::Node) -> Result<u16>;
    fn leaf_placement(&mut self, node: Self::Node, index: u16) -> Result<u16>;
    /// Already transformed BASE header bounds using the serialized initial
    /// placement matrix. None means entity lookup failed, not empty geometry.
    /// Component-expanded or current live instance bounds are different inputs.
    fn placement_base_bounds(&mut self, placement: u16) -> Result<Option<StoredBounds>>;
    fn branch_bounds(&mut self, node: Self::Node, index: u8) -> Result<StoredBounds>;
    fn branch_target(&mut self, node: Self::Node, index: u8) -> Result<Self::Node>;
    fn write_branch_bounds(
        &mut self,
        node: Self::Node,
        index: u8,
        bounds: StoredBounds,
    ) -> Result<()>;
}

#[derive(Debug, Clone, Copy)]
pub struct RefreshLimits {
    /// Helper entries, including repeated shared-node or cyclic entries.
    pub node_visits: usize,
    /// Combined processed branch records and leaf entries.
    pub entry_tests: usize,
}

#[derive(Debug, Default, Serialize)]
pub struct RefreshStats {
    pub node_visits: usize,
    pub child_count_reads: usize,
    pub leaf_count_reads: usize,
    pub branch_entries: usize,
    pub branch_bounds_written: usize,
    pub leaf_entries: usize,
    pub resolved_placements: usize,
    pub unresolved_placements: usize,
}

#[derive(Clone, Copy)]
enum Phase {
    Enter,
    Branches,
    Leaves,
}
struct Frame<N> {
    node: N,
    phase: Phase,
    next: u16,
    accumulator: Bounds,
}
fn unpack(bounds: StoredBounds) -> Bounds {
    Bounds {
        minimum: [bounds.minimum[0], bounds.minimum[1], bounds.minimum[2], 0.],
        maximum: [bounds.maximum[0], bounds.maximum[1], bounds.maximum[2], 0.],
    }
}
fn packed(bounds: Bounds) -> StoredBounds {
    StoredBounds {
        minimum: [bounds.minimum[0], bounds.minimum[1], bounds.minimum[2]],
        maximum: [bounds.maximum[0], bounds.maximum[1], bounds.maximum[2]],
    }
}

/// Grow existing child bounds in depth-first storage order. A child accumulator
/// starts from its stored packed bound, never a fresh inverted box. Completion
/// writes the child's XYZ bounds BEFORE unioning its four-lane result into the
/// parent. Root unions are committed to `accumulator` as they happen; errors do
/// not roll back earlier root updates or callback writes. Native recovery from
/// exceptional data is unestablished; host errors never imply a complete refresh.
pub fn refresh_hierarchy<S: RefreshHierarchy>(
    source: &mut S,
    root: S::Node,
    accumulator: &mut Bounds,
    limits: RefreshLimits,
) -> Result<RefreshStats> {
    ensure!(
        [limits.node_visits, limits.entry_tests]
            .into_iter()
            .all(|v| v > 0 && v <= 1_000_000),
        "invalid hierarchy refresh host budget"
    );
    let mut stats = RefreshStats::default();
    let mut frames = vec![Frame {
        node: root,
        phase: Phase::Enter,
        next: 0,
        accumulator: *accumulator,
    }];
    loop {
        let top = frames.len() - 1;
        let node = frames[top].node;
        let next = frames[top].next;
        let complete = match frames[top].phase {
            Phase::Enter => {
                ensure!(
                    stats.node_visits < limits.node_visits,
                    "hierarchy refresh node-visit budget exhausted"
                );
                stats.node_visits += 1;
                stats.child_count_reads += 1;
                frames[top].phase = if source.child_count(node)? == 0 {
                    Phase::Leaves
                } else {
                    Phase::Branches
                };
                false
            }
            Phase::Leaves => {
                stats.leaf_count_reads += 1;
                if next >= source.leaf_count(node)? {
                    true
                } else {
                    ensure!(
                        stats.branch_entries + stats.leaf_entries < limits.entry_tests,
                        "hierarchy refresh entry-test budget exhausted"
                    );
                    stats.leaf_entries += 1;
                    let placement = source.leaf_placement(node, next)?;
                    if let Some(bounds) = source.placement_base_bounds(placement)? {
                        stats.resolved_placements += 1;
                        frames[top].accumulator = frames[top].accumulator.union(unpack(bounds))?;
                        if top == 0 {
                            *accumulator = frames[top].accumulator;
                        }
                    } else {
                        stats.unresolved_placements += 1;
                    }
                    frames[top].next += 1;
                    false
                }
            }
            Phase::Branches => {
                // Entry classification is separate. The native internal loop
                // reloads its child count before every record and termination.
                stats.child_count_reads += 1;
                if next >= u16::from(source.child_count(node)?) {
                    true
                } else {
                    ensure!(
                        stats.branch_entries + stats.leaf_entries < limits.entry_tests,
                        "hierarchy refresh entry-test budget exhausted"
                    );
                    stats.branch_entries += 1;
                    let seed = unpack(source.branch_bounds(node, next as u8)?);
                    let child = source.branch_target(node, next as u8)?;
                    frames.push(Frame {
                        node: child,
                        phase: Phase::Enter,
                        next: 0,
                        accumulator: seed,
                    });
                    false
                }
            }
        };
        if complete {
            let finished = frames.pop().unwrap();
            let Some(parent) = frames.last_mut() else {
                *accumulator = finished.accumulator;
                return Ok(stats);
            };
            source.write_branch_bounds(
                parent.node,
                parent.next as u8,
                packed(finished.accumulator),
            )?;
            stats.branch_bounds_written += 1;
            parent.accumulator = parent.accumulator.union(finished.accumulator)?;
            parent.next += 1;
            if frames.len() == 1 {
                *accumulator = frames[0].accumulator;
            }
        }
    }
}

#[cfg(test)]
#[path = "../tests/collision_refresh.rs"]
mod tests;
