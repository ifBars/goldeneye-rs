//! Static-contract triangle-path selection. Original-runtime/numeric parity is unverified.
use super::{SurfaceTriangle, TraversalEdge};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct PathSearch {
    pub start_triangle: usize,
    pub goal_triangle: usize,
    /// Independent host guard; exhaustion is an error, not a completed no-path result.
    pub max_expansions: usize,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct GoalPathSearch {
    pub start_triangle: usize,
    pub max_expansions: usize,
}
#[derive(Debug, Serialize)]
pub struct GoalPathResult {
    pub search: PathSearchResult,
    pub goal_tests: usize,
}

/// The actor cost-policy heuristic at 0x8007f690 returns raw squared 3D distance,
/// with x/y products summed before fused z-square addition. No sqrt or EPS cutoff.
pub fn squared_goal_heuristic(center: [f32; 3], point: [f32; 3]) -> Result<f32> {
    ensure!(
        center.into_iter().chain(point).all(f32::is_finite),
        "nonfinite goal heuristic position"
    );
    let d: [f32; 3] = std::array::from_fn(|i| point[i] - center[i]);
    checked_cost(d[2].mul_add(d[2], d[0] * d[0] + d[1] * d[1]))
}

/// Alternate search: test goals only after queue removal. The heuristic receives
/// None for the original -1 destination sentinel; there is no safe default centroid
/// heuristic for that sentinel. Actor predicates and cost policies remain explicit.
pub fn select_triangle_path_to_goal_with_policy(
    request: GoalPathSearch,
    neighbors: &[[Option<usize>; 3]],
    mut goal: impl FnMut(usize) -> Result<bool>,
    allow_edge: impl FnMut(TraversalEdge) -> Result<bool>,
    edge_cost: impl FnMut(TraversalEdge) -> Result<f32>,
    mut heuristic: impl FnMut(usize, Option<usize>) -> Result<f32>,
) -> Result<GoalPathResult> {
    let mut goal_tests = 0;
    let search = run_search(
        request,
        neighbors,
        |face| {
            goal_tests += 1;
            goal(face)
        },
        allow_edge,
        edge_cost,
        |face| heuristic(face, None),
    )?;
    Ok(GoalPathResult { search, goal_tests })
}

#[derive(Clone, Copy)]
pub struct PathGraph<'a> {
    pub neighbors: &'a [[Option<usize>; 3]],
    pub centers: &'a [[f32; 3]],
    pub weights: &'a [Option<[f32; 3]>],
}

#[derive(Debug, Clone, Serialize)]
pub struct TrianglePath {
    /// Includes start and goal; this is not a list of movement waypoints.
    pub triangles: Vec<usize>,
    /// Final search-record g value, not a shortest-path guarantee.
    pub search_cost: f32,
}

#[derive(Debug, Default, Serialize)]
pub struct PathSearchResult {
    pub path: Option<TrianglePath>,
    pub expanded: usize,
    pub edge_checks: usize,
    pub replacements: usize,
    pub reopened: usize,
    pub peak_open: usize,
}

/// Preserve the observed ordered f32 centroid calculation when stored centers are absent.
pub fn surface_centroid(triangle: SurfaceTriangle) -> Result<[f32; 3]> {
    let [a, b, c] = triangle.vertices;
    let center = std::array::from_fn(|i| (c[i] + (a[i] + b[i])) / 3.);
    ensure!(
        center.iter().all(|v| v.is_finite()),
        "navigation centroid overflow/non-finite input"
    );
    Ok(center)
}

fn center_distance(a: [f32; 3], b: [f32; 3]) -> Result<f32> {
    let d: [f32; 3] = std::array::from_fn(|i| b[i] - a[i]);
    let squared = d[2] * d[2] + (d[0] * d[0] + d[1] * d[1]);
    ensure!(squared.is_finite(), "navigation path distance overflow");
    // Wii uses an estimate/Newton pipeline. Ordinary sqrt remains provisional.
    Ok(if squared <= f32::EPSILON {
        0.
    } else {
        squared.sqrt()
    })
}

/// Observed defaults: stored edge weights when present, otherwise center distance;
/// center distance as heuristic; reject only immediate backtracking. Queue ties are
/// FIFO, and equal-g candidates replace open/closed records. Not generic Dijkstra.
pub fn select_triangle_path(request: PathSearch, graph: PathGraph<'_>) -> Result<PathSearchResult> {
    validate_graph(graph)?;
    select_triangle_path_with_policy(
        request,
        graph.neighbors,
        |edge| Ok(Some(edge.to_triangle) != edge.previous_triangle),
        |edge| default_edge_cost(graph, edge),
        |from, goal| center_distance(graph.centers[from], graph.centers[goal]),
    )
}

/// Explicit point heuristic + default edge/cost policies. The caller supplies the
/// goal predicate and point; this does not invent an original actor's goal policy.
pub fn select_triangle_path_to_goal_with_point(
    request: GoalPathSearch,
    graph: PathGraph<'_>,
    point: [f32; 3],
    goal: impl FnMut(usize) -> Result<bool>,
) -> Result<GoalPathResult> {
    select_triangle_path_to_goal_with_point_and_edge_policy(request, graph, point, goal, |edge| {
        Ok(Some(edge.to_triangle) != edge.previous_triangle)
    })
}

pub fn select_triangle_path_to_goal_with_point_and_edge_policy(
    request: GoalPathSearch,
    graph: PathGraph<'_>,
    point: [f32; 3],
    goal: impl FnMut(usize) -> Result<bool>,
    allow_edge: impl FnMut(TraversalEdge) -> Result<bool>,
) -> Result<GoalPathResult> {
    validate_graph(graph)?;
    ensure!(
        point.into_iter().all(f32::is_finite),
        "nonfinite goal heuristic point"
    );
    select_triangle_path_to_goal_with_policy(
        request,
        graph.neighbors,
        goal,
        allow_edge,
        |edge| default_edge_cost(graph, edge),
        |face, destination| {
            debug_assert!(destination.is_none());
            squared_goal_heuristic(graph.centers[face], point)
        },
    )
}

fn validate_graph(graph: PathGraph<'_>) -> Result<()> {
    ensure!(
        graph.centers.len() == graph.neighbors.len()
            && graph.weights.len() == graph.neighbors.len(),
        "navigation path table length mismatch"
    );
    ensure!(
        graph.centers.iter().flatten().all(|v| v.is_finite()),
        "non-finite navigation path center"
    );
    ensure!(
        graph
            .weights
            .iter()
            .flatten()
            .flatten()
            .all(|v| v.is_finite() && *v >= 0.),
        "invalid navigation path weight"
    );
    Ok(())
}
fn default_edge_cost(graph: PathGraph<'_>, edge: TraversalEdge) -> Result<f32> {
    match graph.weights[edge.from_triangle] {
        Some(row) => Ok(row[edge.slot]),
        None => center_distance(
            graph.centers[edge.from_triangle],
            graph.centers[edge.to_triangle],
        ),
    }
}

/// Policies replace the observed callback boundaries. Returned costs/estimates must
/// be finite/nonnegative. First boundary ends each neighbor list. Self-edge expansion
/// and dangling original pool references remain unsupported errors, never fake paths.
pub fn select_triangle_path_with_policy(
    request: PathSearch,
    neighbors: &[[Option<usize>; 3]],
    allow_edge: impl FnMut(TraversalEdge) -> Result<bool>,
    edge_cost: impl FnMut(TraversalEdge) -> Result<f32>,
    mut heuristic: impl FnMut(usize, usize) -> Result<f32>,
) -> Result<PathSearchResult> {
    ensure!(
        request.goal_triangle < neighbors.len(),
        "navigation path endpoint out of bounds"
    );
    run_search(
        GoalPathSearch {
            start_triangle: request.start_triangle,
            max_expansions: request.max_expansions,
        },
        neighbors,
        |face| Ok(face == request.goal_triangle),
        allow_edge,
        edge_cost,
        |face| heuristic(face, request.goal_triangle),
    )
}

fn run_search(
    request: GoalPathSearch,
    neighbors: &[[Option<usize>; 3]],
    mut goal: impl FnMut(usize) -> Result<bool>,
    mut allow_edge: impl FnMut(TraversalEdge) -> Result<bool>,
    mut edge_cost: impl FnMut(TraversalEdge) -> Result<f32>,
    mut heuristic: impl FnMut(usize) -> Result<f32>,
) -> Result<PathSearchResult> {
    ensure!(
        request.max_expansions > 0,
        "navigation path requires a positive expansion budget"
    );
    ensure!(
        request.start_triangle < neighbors.len(),
        "navigation path endpoint out of bounds"
    );
    let mut records = Records::default();
    let mut state = Vec::new();
    state.try_reserve_exact(neighbors.len())?;
    state.resize(neighbors.len(), State::default());
    let h = checked_cost(heuristic(request.start_triangle)?)?;
    let start = records.allocate(Node {
        face: request.start_triangle,
        parent: None,
        g: 0.,
        score: h,
    })?;
    let mut open = vec![start];
    state[request.start_triangle] = State {
        phase: Phase::Open,
        record: Some(start),
    };
    let mut result = PathSearchResult {
        peak_open: 1,
        ..Default::default()
    };
    while !open.is_empty() {
        ensure!(
            result.expanded < request.max_expansions,
            "navigation path expansion budget exhausted after {} expansions",
            result.expanded
        );
        let current_id = open.remove(0);
        let current = *records.get(current_id)?;
        state[current.face].phase = Phase::Unseen;
        result.expanded += 1;
        if goal(current.face)? {
            let mut ids = Vec::new();
            let mut cursor = Some(current_id);
            while let Some(id) = cursor {
                ensure!(
                    ids.len() <= result.expanded,
                    "navigation path parent cycle/invalid pool chain"
                );
                let node = records.get(id)?;
                ids.try_reserve(1)?;
                ids.push(node.face);
                cursor = node.parent;
            }
            ids.reverse();
            ensure!(
                ids.first() == Some(&request.start_triangle),
                "navigation path parent chain lost its start"
            );
            result.path = Some(TrianglePath {
                triangles: ids,
                search_cost: current.g,
            });
            return Ok(result);
        }
        let previous = current
            .parent
            .map(|p| records.get(p).map(|n| n.face))
            .transpose()?;
        for (slot, neighbor) in neighbors[current.face].iter().enumerate() {
            let Some(next) = *neighbor else { break };
            ensure!(
                next < neighbors.len(),
                "navigation path neighbor out of bounds"
            );
            let edge = TraversalEdge {
                from_triangle: current.face,
                to_triangle: next,
                previous_triangle: previous,
                slot,
            };
            result.edge_checks += 1;
            if !allow_edge(edge)? {
                continue;
            }
            ensure!(
                next != current.face,
                "navigation path self-edge expansion remains unsupported"
            );
            let candidate = current.g + checked_cost(edge_cost(edge)?)?;
            checked_cost(candidate)?;
            let old = state[next];
            if old.phase != Phase::Unseen {
                let id = old
                    .record
                    .context("navigation path state lost its record")?;
                if records.get(id)?.g < candidate {
                    continue;
                }
                if old.phase == Phase::Open {
                    let position = open
                        .iter()
                        .position(|&entry| entry == id)
                        .context("navigation open record missing")?;
                    open.remove(position);
                } else {
                    result.reopened += 1;
                }
                records.release(id)?;
                result.replacements += 1;
            }
            let h = checked_cost(heuristic(next)?)?;
            let score = checked_cost(candidate + h)?;
            let id = records.allocate(Node {
                face: next,
                parent: Some(current_id),
                g: candidate,
                score,
            })?;
            // Insert after all equal scores, including equal-g replacement records.
            let mut position = 0;
            while position < open.len() && records.get(open[position])?.score <= score {
                position += 1;
            }
            open.try_reserve(1)?;
            open.insert(position, id);
            state[next] = State {
                phase: Phase::Open,
                record: Some(id),
            };
            result.peak_open = result.peak_open.max(open.len());
        }
        state[current.face] = State {
            phase: Phase::Closed,
            record: Some(current_id),
        };
    }
    Ok(result)
}

fn checked_cost(value: f32) -> Result<f32> {
    ensure!(
        value.is_finite() && value >= 0.,
        "invalid/overflowing navigation path cost"
    );
    Ok(value)
}

#[derive(Debug, Clone, Copy)]
struct Node {
    face: usize,
    parent: Option<usize>,
    g: f32,
    score: f32,
}
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Unseen,
    Open,
    Closed,
}
#[derive(Clone, Copy, Default)]
struct State {
    phase: Phase,
    record: Option<usize>,
}

// Stable logical slots model the observed 256-record pool: newest nonfull block,
// last free slot first. Parent references follow slots, not face IDs or immutable
// node generations. Host heap-address reuse after whole-block free is unverified.
const BLOCK: usize = 256;
#[derive(Default)]
struct Records {
    blocks: Vec<Block>,
    active: Vec<usize>,
}
struct Block {
    nodes: Vec<Option<Node>>,
    free: Vec<usize>,
    active: bool,
}
impl Records {
    fn allocate(&mut self, node: Node) -> Result<usize> {
        let available = self
            .active
            .iter()
            .rev()
            .copied()
            .find(|&b| !self.blocks[b].free.is_empty());
        let block = if let Some(block) = available {
            block
        } else {
            let mut nodes = Vec::new();
            nodes.try_reserve_exact(BLOCK)?;
            nodes.resize(BLOCK, None);
            let mut free = Vec::new();
            free.try_reserve_exact(BLOCK)?;
            free.extend(0..BLOCK);
            self.blocks.try_reserve(1)?;
            self.active.try_reserve(1)?;
            let block = self.blocks.len();
            self.blocks.push(Block {
                nodes,
                free,
                active: true,
            });
            self.active.push(block);
            block
        };
        let slot = self.blocks[block]
            .free
            .pop()
            .context("navigation path pool has no free slot")?;
        self.blocks[block].nodes[slot] = Some(node);
        block
            .checked_mul(BLOCK)
            .and_then(|b| b.checked_add(slot))
            .context("navigation path slot overflow")
    }
    fn get(&self, id: usize) -> Result<&Node> {
        self.blocks
            .get(id / BLOCK)
            .filter(|b| b.active)
            .and_then(|b| b.nodes.get(id % BLOCK))
            .and_then(Option::as_ref)
            .context(
                "navigation path has a dangling pool parent; original heap reuse is unverified",
            )
    }
    fn release(&mut self, id: usize) -> Result<()> {
        let block = id / BLOCK;
        let slot = id % BLOCK;
        let b = self
            .blocks
            .get_mut(block)
            .context("navigation pool block missing")?;
        ensure!(
            !b.free.contains(&slot) && b.nodes[slot].is_some(),
            "navigation pool record already free"
        );
        // The observed free routine updates the index stack outside record payloads.
        // A parent can still read this old payload until allocation overwrites it.
        b.free.push(slot);
        if b.free.len() == BLOCK {
            b.active = false;
            let at = self
                .active
                .iter()
                .position(|&b| b == block)
                .context("navigation pool active block missing")?;
            self.active.remove(at);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../tests/goal_path.rs"]
mod goal_tests;
#[cfg(test)]
#[path = "../tests/path.rs"]
mod tests;
