//! Ordered placed-object candidate traversal over explicit hierarchy state.
use super::world::StoredBounds;
use anyhow::{Result, ensure};
use serde::Serialize;

/// Addressing remains in the adapter. The split reads preserve the native
/// disabled-node, pruned-branch and disabled-leaf short circuits.
pub trait CandidateHierarchy {
    type Node: Copy;
    fn node_flags(&mut self, node: Self::Node) -> Result<u8>;
    fn child_count(&mut self, node: Self::Node) -> Result<u8>;
    fn leaf_count(&mut self, node: Self::Node) -> Result<u16>;
    fn branch_bounds(&mut self, node: Self::Node, index: u8) -> Result<StoredBounds>;
    fn branch_target(&mut self, node: Self::Node, index: u8) -> Result<Self::Node>;
    fn leaf_flags(&mut self, node: Self::Node, index: u16) -> Result<u8>;
    fn leaf_placement(&mut self, node: Self::Node, index: u16) -> Result<u16>;
}

#[derive(Debug, Clone, Copy)]
pub struct CandidateLimits {
    /// Number of node-enable/header checks, including recursive rechecks.
    pub node_visits: usize,
    /// Combined branch-bound and leaf-flag tests.
    pub entry_tests: usize,
    pub output_indices: usize,
}

#[derive(Debug, Default, Serialize)]
pub struct CandidateResult {
    /// Native u16 placement indices are appended into a u32 list. Repeats stay.
    pub placements: Vec<u32>,
    /// Enable/header checks, not unique nodes: an internal child is checked by
    /// its parent and checked again on recursive helper entry.
    pub nodes_tested: usize,
    pub branch_tests: usize,
    pub leaf_tests: usize,
}

enum Work<N> {
    Enter(N),
    Branches { node: N, next: u8, count: u8 },
    Leaves { node: N, next: u16 },
}

fn header<S: CandidateHierarchy>(
    source: &mut S,
    node: S::Node,
    result: &mut CandidateResult,
    node_budget: usize,
) -> Result<Option<u8>> {
    ensure!(
        result.nodes_tested < node_budget,
        "candidate node-visit budget exhausted"
    );
    result.nodes_tested += 1;
    if source.node_flags(node)? & 8 == 0 {
        return Ok(None);
    }
    Ok(Some(source.child_count(node)?))
}

/// Depth-first storage-order traversal, preserving node and leaf enable bit 0x08.
/// The origin/radius are only inspected when a branch bound is actually tested.
/// Shared nodes and cycles are not globally deduplicated; host budget exhaustion
/// is an error, never an empty or truncated successful candidate list.
pub fn collect_candidates<S: CandidateHierarchy>(
    source: &mut S,
    root: S::Node,
    origin: [f32; 3],
    radius: f32,
    limits: CandidateLimits,
) -> Result<CandidateResult> {
    collect_candidates_with_bounds(source, root, limits, |bounds| {
        bounds.contains_with_margin(origin, radius)
    })
}

/// Same source traversal, with the caller's ordered branch-bound operation.
/// Line queries use their distinct broad-overlap and packed-face gates here.
pub fn collect_candidates_with_bounds<S: CandidateHierarchy>(
    source: &mut S,
    root: S::Node,
    limits: CandidateLimits,
    mut overlaps: impl FnMut(StoredBounds) -> Result<bool>,
) -> Result<CandidateResult> {
    ensure!(
        [
            limits.node_visits,
            limits.entry_tests,
            limits.output_indices
        ]
        .into_iter()
        .all(|value| value > 0 && value <= 1_000_000),
        "invalid candidate traversal host budget"
    );
    let mut result = CandidateResult::default();
    let mut work = vec![Work::Enter(root)];
    while let Some(item) = work.pop() {
        match item {
            Work::Enter(node) => {
                let Some(count) = header(source, node, &mut result, limits.node_visits)? else {
                    continue;
                };
                if count == 0 {
                    work.push(Work::Leaves { node, next: 0 });
                } else {
                    work.push(Work::Branches {
                        node,
                        next: 0,
                        count,
                    });
                }
            }
            Work::Branches { node, next, count } => {
                ensure!(
                    result.branch_tests + result.leaf_tests < limits.entry_tests,
                    "candidate entry-test budget exhausted"
                );
                result.branch_tests += 1;
                if next + 1 < count {
                    work.push(Work::Branches {
                        node,
                        next: next + 1,
                        count,
                    });
                }
                if overlaps(source.branch_bounds(node, next)?)? {
                    // The source pointer is addressed only after its bound passes.
                    let child = source.branch_target(node, next)?;
                    let Some(count) = header(source, child, &mut result, limits.node_visits)?
                    else {
                        continue;
                    };
                    if count == 0 {
                        work.push(Work::Leaves {
                            node: child,
                            next: 0,
                        });
                    } else {
                        // Native internal recursion reads the same flags/count
                        // again; do not fold the parent's precheck into entry.
                        work.push(Work::Enter(child));
                    }
                }
            }
            Work::Leaves { node, next } => {
                // The source reloads this count at every loop condition,
                // including the final termination check. It is not a snapshot.
                if next >= source.leaf_count(node)? {
                    continue;
                }
                ensure!(
                    result.branch_tests + result.leaf_tests < limits.entry_tests,
                    "candidate entry-test budget exhausted"
                );
                result.leaf_tests += 1;
                if source.leaf_flags(node, next)? & 8 != 0 {
                    let placement = source.leaf_placement(node, next)?;
                    ensure!(
                        result.placements.len() < limits.output_indices,
                        "candidate output-index budget exhausted"
                    );
                    result.placements.push(u32::from(placement));
                }
                work.push(Work::Leaves {
                    node,
                    next: next + 1,
                });
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
#[path = "../tests/collision_candidates.rs"]
mod tests;
