//! Initialized static hierarchy snapshots. Live membership and clearance are separate.
use anyhow::{Context, Result, ensure};
use ge_assets::{
    candidates::{Entries, Hierarchy},
    edb::Edb,
    scene::Placement,
};
use ge_core::collision::{
    bounds::Bounds,
    candidates::{CandidateHierarchy, CandidateLimits, collect_candidates},
    refresh::{RefreshHierarchy, RefreshLimits, refresh_hierarchy},
    world::StoredBounds,
};
use serde::Serialize;
use std::{collections::BTreeMap, path::PathBuf};

pub struct Runtime<'a> {
    pub hierarchy: Hierarchy,
    edb: &'a Edb,
    placements: &'a [Placement],
    base: BTreeMap<u16, Option<StoredBounds>>,
}
impl<'a> Runtime<'a> {
    pub fn new(hierarchy: Hierarchy, edb: &'a Edb, placements: &'a [Placement]) -> Self {
        Self {
            hierarchy,
            edb,
            placements,
            base: BTreeMap::new(),
        }
    }
    fn node(&self, node: usize) -> Result<&ge_assets::candidates::Node> {
        self.hierarchy
            .nodes
            .get(&node)
            .context("candidate snapshot node missing")
    }
    fn branch(&self, node: usize, index: u8) -> Result<&ge_assets::candidates::Branch> {
        let Entries::Branches(entries) = &self.node(node)?.entries else {
            anyhow::bail!("candidate node is not internal")
        };
        entries
            .get(usize::from(index))
            .context("candidate branch outside node")
    }
    fn leaf(&self, node: usize, index: u16) -> Result<&ge_assets::candidates::Leaf> {
        let Entries::Leaves(entries) = &self.node(node)?.entries else {
            anyhow::bail!("candidate node is not a leaf")
        };
        entries
            .get(usize::from(index))
            .context("candidate leaf index outside node")
    }
    pub fn query(
        &mut self,
        origin: [f32; 3],
        radius: f32,
    ) -> Result<ge_core::collision::candidates::CandidateResult> {
        let root = self.hierarchy.root;
        collect_candidates(
            self,
            root,
            origin,
            radius,
            CandidateLimits {
                node_visits: 1_000_000,
                entry_tests: 1_000_000,
                output_indices: 1_000_000,
            },
        )
    }
    pub fn line_query(
        &mut self,
        world_bounds: StoredBounds,
        origin: [f32; 3],
        delta: [f32; 3],
    ) -> Result<ge_core::collision::candidates::CandidateResult> {
        let root = self.hierarchy.root;
        ge_core::collision::candidates::collect_candidates_with_bounds(
            self,
            root,
            CandidateLimits {
                node_visits: 1_000_000,
                entry_tests: 1_000_000,
                output_indices: 1_000_000,
            },
            |bounds| {
                Ok(
                    ge_core::collision::line_bounds::overlap(bounds, world_bounds)?
                        && ge_core::collision::line_bounds::packed_segment_overlaps(
                            bounds, origin, delta,
                        )?,
                )
            },
        )
    }
    pub fn refresh(
        &mut self,
        accumulator: &mut Bounds,
    ) -> Result<ge_core::collision::refresh::RefreshStats> {
        let root = self.hierarchy.root;
        refresh_hierarchy(
            self,
            root,
            accumulator,
            RefreshLimits {
                node_visits: 1_000_000,
                entry_tests: 1_000_000,
            },
        )
    }
}
impl CandidateHierarchy for Runtime<'_> {
    type Node = usize;
    fn node_flags(&mut self, n: usize) -> Result<u8> {
        Ok(self.node(n)?.flags_raw)
    }
    fn child_count(&mut self, n: usize) -> Result<u8> {
        Ok(match &self.node(n)?.entries {
            Entries::Branches(v) => v.len() as u8,
            Entries::Leaves(_) => 0,
        })
    }
    fn leaf_count(&mut self, n: usize) -> Result<u16> {
        let Entries::Leaves(v) = &self.node(n)?.entries else {
            anyhow::bail!("candidate node is not a leaf")
        };
        Ok(v.len() as u16)
    }
    fn branch_bounds(&mut self, n: usize, i: u8) -> Result<StoredBounds> {
        let b = self.branch(n, i)?.bounds;
        Ok(StoredBounds {
            minimum: b[0],
            maximum: b[1],
        })
    }
    fn branch_target(&mut self, n: usize, i: u8) -> Result<usize> {
        Ok(self.branch(n, i)?.target)
    }
    fn leaf_flags(&mut self, n: usize, i: u16) -> Result<u8> {
        Ok(self.leaf(n, i)?.flags_raw)
    }
    fn leaf_placement(&mut self, n: usize, i: u16) -> Result<u16> {
        Ok(self.leaf(n, i)?.placement)
    }
}
impl RefreshHierarchy for Runtime<'_> {
    type Node = usize;
    fn child_count(&mut self, n: usize) -> Result<u8> {
        CandidateHierarchy::child_count(self, n)
    }
    fn leaf_count(&mut self, n: usize) -> Result<u16> {
        CandidateHierarchy::leaf_count(self, n)
    }
    fn leaf_placement(&mut self, n: usize, i: u16) -> Result<u16> {
        CandidateHierarchy::leaf_placement(self, n, i)
    }
    fn placement_base_bounds(&mut self, index: u16) -> Result<Option<StoredBounds>> {
        if let Some(value) = self.base.get(&index) {
            return Ok(*value);
        }
        let placement = self
            .placements
            .get(usize::from(index))
            .context("candidate placement outside initial array")?;
        let result = if let Some(reference) = self.edb.entity_reference(placement.object_hash)? {
            let header = self.edb.entity_spatial_at(reference.directory_index)?;
            let local = Bounds {
                minimum: [
                    header.bounds[0][0],
                    header.bounds[0][1],
                    header.bounds[0][2],
                    0.,
                ],
                maximum: [
                    header.bounds[1][0],
                    header.bounds[1][1],
                    header.bounds[1][2],
                    0.,
                ],
            };
            let world = local.transformed(&placement.model)?;
            Some(StoredBounds {
                minimum: world.minimum[..3].try_into().unwrap(),
                maximum: world.maximum[..3].try_into().unwrap(),
            })
        } else {
            None
        };
        self.base.insert(index, result);
        Ok(result)
    }
    fn branch_bounds(&mut self, n: usize, i: u8) -> Result<StoredBounds> {
        CandidateHierarchy::branch_bounds(self, n, i)
    }
    fn branch_target(&mut self, n: usize, i: u8) -> Result<usize> {
        CandidateHierarchy::branch_target(self, n, i)
    }
    fn write_branch_bounds(&mut self, n: usize, i: u8, b: StoredBounds) -> Result<()> {
        let node = self
            .hierarchy
            .nodes
            .get_mut(&n)
            .context("candidate write node missing")?;
        let Entries::Branches(entries) = &mut node.entries else {
            anyhow::bail!("candidate write is not internal")
        };
        entries
            .get_mut(usize::from(i))
            .context("candidate write branch outside node")?
            .bounds = [b.minimum, b.maximum];
        Ok(())
    }
}

#[derive(clap::Args)]
pub struct Options {
    disc: PathBuf,
    resource: String,
    #[arg(long,value_parser=crate::parse_hash)]
    map: u32,
    #[arg(long)]
    zone: usize,
    #[arg(long, num_args = 3, allow_hyphen_values = true, required = true)]
    origin: Vec<f32>,
    #[arg(long, allow_hyphen_values = true)]
    radius: f32,
    #[arg(long)]
    pub output: Option<PathBuf>,
}
pub fn run(options: &Options) -> Result<serde_json::Value> {
    ensure!(
        options.origin.iter().all(|v| v.is_finite()) && options.radius.is_finite(),
        "candidate query values must be finite"
    );
    let origin = options
        .origin
        .as_slice()
        .try_into()
        .context("candidate origin needs three coordinates")?;
    let mut assets = ge_assets::resource::GameAssets::open(&options.disc, "Filelist.bin")?;
    let edb = assets.load_edb(&options.resource)?;
    let tree = edb
        .map_candidate_hierarchy(options.map, options.zone)?
        .context("candidate hierarchy is absent, not an empty-world result")?;
    let placements = edb.map_placements(options.map)?;
    let mut runtime = Runtime::new(tree, &edb, &placements);
    let mut bound = Bounds::MISSING_ENTITY;
    let refresh = runtime.refresh(&mut bound)?;
    let result = runtime.query(origin, options.radius)?;
    Ok(
        serde_json::json!({"map":options.map,"zone":options.zone,"origin":origin,"radius":options.radius,"refresh":refresh,"initial_zone_bounds":bound,"query":result,"reference_compared":false,
        "scope":"Initial serialized membership snapshot with native seeded hierarchy-bound refresh. Ordered candidate indices only; no geometry/contact query, live membership or original-runtime comparison."}),
    )
}

#[derive(Default, Serialize)]
pub struct Audit {
    pub maps: usize,
    pub zones: usize,
    pub absent_hierarchies: usize,
    pub refreshed_hierarchies: usize,
    pub nodes: usize,
    pub branch_bounds_written: usize,
    pub leaf_entries_refreshed: usize,
    pub missing_placement_entities: usize,
    pub queries: usize,
    pub stable_queries: usize,
    pub ambiguous_queries: usize,
    pub candidate_indices: usize,
    pub maximum_candidates: usize,
}
impl Audit {
    pub fn include(
        &mut self,
        edb: &Edb,
        map: u32,
        placements: &[Placement],
        spatial: &ge_assets::spatial::MapSpatial,
    ) -> Result<()> {
        self.maps += 1;
        for (zone, metadata) in spatial.zones.iter().enumerate() {
            self.zones += 1;
            let Some(tree) = edb.map_candidate_hierarchy(map, zone)? else {
                self.absent_hierarchies += 1;
                continue;
            };
            self.nodes += tree.nodes.len();
            let flags = membership(&tree);
            let mut runtime = Runtime::new(tree, edb, placements);
            let mut bound = Bounds::MISSING_ENTITY;
            let refresh = runtime.refresh(&mut bound)?;
            ensure!(
                membership(&runtime.hierarchy) == flags,
                "hierarchy refresh changed membership metadata"
            );
            self.refreshed_hierarchies += 1;
            self.branch_bounds_written += refresh.branch_bounds_written;
            self.leaf_entries_refreshed += refresh.leaf_entries;
            self.missing_placement_entities += refresh.unresolved_placements;
            if zone >= 8 {
                continue;
            }
            let center = std::array::from_fn(|i| {
                ((f64::from(metadata.bounds[0][i]) + f64::from(metadata.bounds[1][i])) * 0.5) as f32
            });
            for origin in [center, metadata.bounds[0], metadata.bounds[1]] {
                let actual = runtime.query(origin, 1.)?;
                let (expected, ambiguous) = oracle(&runtime.hierarchy, origin, 1.)?;
                self.queries += 1;
                self.candidate_indices += actual.placements.len();
                self.maximum_candidates = self.maximum_candidates.max(actual.placements.len());
                if ambiguous {
                    self.ambiguous_queries += 1;
                } else {
                    ensure!(
                        actual.placements == expected,
                        "candidate list differs from independent f64 traversal"
                    );
                    self.stable_queries += 1;
                }
            }
        }
        Ok(())
    }
}
type Membership = Vec<(usize, u8, Vec<(u16, u8, u8)>)>;
fn membership(tree: &Hierarchy) -> Membership {
    tree.nodes
        .iter()
        .map(|(&at, node)| {
            (
                at,
                node.flags_raw,
                match &node.entries {
                    Entries::Branches(_) => Vec::new(),
                    Entries::Leaves(v) => v
                        .iter()
                        .map(|l| (l.placement, l.flags_raw, l.unknown_byte))
                        .collect(),
                },
            )
        })
        .collect()
}

#[cfg(test)]
#[path = "tests/candidates.rs"]
mod tests;
fn oracle(tree: &Hierarchy, origin: [f32; 3], radius: f32) -> Result<(Vec<u32>, bool)> {
    let mut stack = vec![tree.root];
    let mut placements = Vec::new();
    let mut ambiguous = false;
    let mut work = 0;
    let mut entry_work = 0usize;
    while let Some(at) = stack.pop() {
        work += 1;
        ensure!(work <= 1_000_000, "independent candidate budget exhausted");
        let node = tree
            .nodes
            .get(&at)
            .context("independent candidate missing node")?;
        if node.flags_raw & 8 == 0 {
            continue;
        }
        match &node.entries {
            Entries::Leaves(entries) => {
                entry_work += entries.len();
                ensure!(
                    entry_work <= 1_000_000,
                    "independent candidate entry budget exhausted"
                );
                for leaf in entries.iter().filter(|l| l.flags_raw & 8 != 0) {
                    ensure!(
                        placements.len() < 1_000_000,
                        "independent candidate output budget exhausted"
                    );
                    placements.push(u32::from(leaf.placement));
                }
            }
            Entries::Branches(entries) => {
                entry_work += entries.len();
                ensure!(
                    entry_work <= 1_000_000,
                    "independent candidate entry budget exhausted"
                );
                let mut passed = Vec::new();
                for branch in entries {
                    let mut contains = true;
                    for (side, sign) in [(0, 1.), (1, -1.)] {
                        for (axis, &coordinate) in origin.iter().enumerate() {
                            let difference =
                                f64::from(branch.bounds[side][axis]) - f64::from(coordinate);
                            let limit = sign * f64::from(radius);
                            let bound = 1e-6
                                + 4. * f64::from(f32::EPSILON)
                                    * (f64::from(branch.bounds[side][axis]).abs()
                                        + f64::from(coordinate).abs()
                                        + f64::from(radius).abs());
                            ambiguous |= (difference - limit).abs() <= bound;
                            contains &= if side == 0 {
                                difference <= limit
                            } else {
                                difference >= limit
                            };
                        }
                    }
                    if contains {
                        passed.push(branch.target);
                    }
                }
                stack.extend(passed.into_iter().rev());
            }
        }
    }
    Ok((placements, ambiguous))
}
