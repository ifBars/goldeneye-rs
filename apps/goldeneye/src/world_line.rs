//! Initialized static world-line requests, independent from box collection.
use anyhow::{Context, Result, ensure};
use ge_assets::{edb::Edb, resource::GameAssets, spatial::MapSpatial};
use ge_core::collision::{
    bounds::Bounds,
    bsp_segment::{self, SegmentLimits, SegmentResult, SegmentSource},
    coarse::{CoarseLimits, CoarseResult},
    collector::GeometryFilter,
    dispatch::DispatchLimits,
    line_bounds, line_coarse, line_frame,
    line_geometry::{self, LineGeometryRequest},
    world::{StoredBounds, select_loaded_world},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct Options {
    disc: PathBuf,
    #[arg(long)]
    snapshot: PathBuf,
    #[arg(long)]
    pub output: Option<PathBuf>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LineSnapshot {
    pub origin: [f32; 3],
    pub delta: [f32; 3],
    /// Original constructor delta before a caller mutation, e.g. grenade lift.
    #[serde(default)]
    pub constructor_delta: Option<[f32; 3]>,
    pub maximum_parameter: f32,
    pub filter_slot: u32,
    pub filter_flags: u32,
    pub required_any: u32,
    pub excluded_any: u32,
    pub stop_first_raw: u8,
}
impl LineSnapshot {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.origin
                .into_iter()
                .chain(self.delta)
                .chain(self.constructor_delta.into_iter().flatten())
                .chain([self.maximum_parameter])
                .all(f32::is_finite),
            "line snapshot values must be finite"
        );
        Ok(())
    }
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub worlds: Vec<crate::world_query::World>,
    pub line: LineSnapshot,
}
pub fn run(options: &Options) -> Result<Value> {
    let snapshot: Snapshot = crate::world_query::read_snapshot(&options.snapshot)?;
    query_loaded(&snapshot, |resource| {
        GameAssets::open(&options.disc, "Filelist.bin")?.load_edb(resource)
    })
}
pub fn query_loaded(snapshot: &Snapshot, load: impl FnMut(&str) -> Result<Edb>) -> Result<Value> {
    query_worlds(&snapshot.worlds, &snapshot.line, load)
}
pub fn query_worlds(
    worlds: &[crate::world_query::World],
    line: &LineSnapshot,
    mut load: impl FnMut(&str) -> Result<Edb>,
) -> Result<Value> {
    line.validate()?;
    let mut tested = Vec::new();
    let selected = select_loaded_world(worlds.len(), line.origin, |index| {
        tested.push(index);
        let bounds = &worlds[index].bounds;
        Ok(StoredBounds {
            minimum: bounds.minimum,
            maximum: bounds.maximum,
        })
    })?;
    let query = selected
        .map(|index| -> Result<Value> {
            let world = &worlds[index];
            let edb = load(&world.resource).with_context(|| {
                format!("selected line world {index} resource {}", world.resource)
            })?;
            query_map(&edb, world.map, line)
        })
        .transpose()?;
    Ok(
        json!({"selected_world":selected,"bounds_tested":tested,"world_count":worlds.len(),"query":query,
        "reference_compared":false,"scope":"Explicit ordered world-manager membership and initialized static line geometry only. No live actor pass, dynamic positions, mutation, full grenade state or original-runtime/FP parity."}),
    )
}

pub struct BspAdapter<'a>(pub &'a MapSpatial);
impl SegmentSource for BspAdapter<'_> {
    fn plane(&mut self, node: u16) -> Result<[f32; 4]> {
        Ok(self
            .0
            .nodes
            .get(&node)
            .context("line BSP plane missing")?
            .plane)
    }
    fn child(&mut self, node: u16, slot: u8) -> Result<i16> {
        self.0
            .nodes
            .get(&node)
            .and_then(|n| n.children.get(usize::from(slot)))
            .copied()
            .context("line BSP child missing")
    }
}
pub fn bind_zones(
    spatial: &MapSpatial,
    origin: [f32; 3],
    delta: [f32; 3],
) -> Result<SegmentResult> {
    ensure!(
        spatial.bsp_present,
        "line query BSP absent, not clear space"
    );
    let end = std::array::from_fn::<_, 3, _>(|i| origin[i] + delta[i]);
    bsp_segment::query(
        &mut BspAdapter(spatial),
        [origin[0], origin[1], origin[2], 1.],
        [end[0], end[1], end[2], 1.],
        spatial.zones.len(),
        SegmentLimits {
            node_tests: 1_000_000,
            leaf_visits: 1_000_000,
            pending_segments: bsp_segment::MAX_PENDING_SEGMENTS,
        },
    )
}
fn coarse(edb: &Edb, node: u32, origin: [f32; 3], delta: [f32; 3]) -> Result<Option<CoarseResult>> {
    let Some(view) = edb.geometry_line_coarse_at_address(node as usize)? else {
        return Ok(None);
    };
    let root = view.root;
    Ok(Some(line_coarse::query(
        &mut crate::mesh_query::CoarseAdapter(view),
        root,
        origin,
        delta,
        CoarseLimits {
            node_reads: 1_000_000,
            sphere_tests: 1_000_000,
            fanout: 16,
        },
    )?))
}
#[derive(Serialize)]
struct Binding {
    zone: usize,
    placement_index: Option<u32>,
    candidate_ordinal: Option<usize>,
    root_key: u32,
}
#[derive(Serialize)]
struct State {
    maximum_parameter: f32,
    world_bounds: StoredBounds,
    hit: Option<crate::line_mesh::MeshHit>,
    world_normal: Option<[f32; 4]>,
    binding: Option<Binding>,
}
struct Runtime<'a> {
    edb: &'a Edb,
    input: &'a LineSnapshot,
    state: State,
    meshes: crate::line_mesh::Meshes,
    roots: Vec<Value>,
}
impl Runtime<'_> {
    fn root(&mut self, node: u32, origin: [f32; 4], delta: [f32; 4]) -> Result<bool> {
        let origin_xyz = [origin[0], origin[1], origin[2]];
        let delta_xyz = [delta[0], delta[1], delta[2]];
        let local_bounds =
            line_bounds::endpoint_bounds(origin_xyz, delta_xyz, self.state.maximum_parameter)?;
        let request = LineGeometryRequest {
            origin: origin_xyz,
            delta: delta_xyz,
            bounds: local_bounds,
            optional_context_key: 0,
            dispatch_flags: 0,
            stop_first: self.input.stop_first_raw != 0,
        };
        let edb = self.edb;
        let input = self.input;
        let state = &mut self.state;
        let meshes = &mut self.meshes;
        let mut mesh_probes = Vec::new();
        let mut child_coarse = Vec::new();
        let mut source = crate::geometry_source::Source(edb.geometry());
        let outcome = line_geometry::query(
            &mut source,
            node,
            &request,
            DispatchLimits::default(),
            |child| {
                let result = coarse(edb, child, origin_xyz, delta_xyz)?;
                let overlaps = result.as_ref().map(|r| r.overlaps);
                child_coarse.push(json!({"geometry_key":child,"result":result}));
                Ok(overlaps)
            },
            |mesh, context, flags| {
                let mut coarse_result = None;
                let probe = meshes.query(
                    edb,
                    mesh,
                    &mut crate::line_mesh::MeshQuery {
                        origin: origin_xyz,
                        delta: delta_xyz,
                        local_bounds,
                        maximum_parameter: &mut state.maximum_parameter,
                        required_any: input.required_any,
                        excluded_any: input.excluded_any,
                        optional_context_key: context,
                        dispatch_flags: flags,
                        stop_first: input.stop_first_raw != 0,
                    },
                    || {
                        coarse_result = coarse(edb, mesh, origin_xyz, delta_xyz)?;
                        Ok(coarse_result.as_ref().map(|r| r.overlaps))
                    },
                )?;
                let report = json!({"mesh":probe,"coarse":coarse_result});
                let returned = probe.hit.is_some();
                if let Some(hit) = probe.hit {
                    state.world_bounds = line_bounds::endpoint_bounds(
                        input.origin,
                        input.delta,
                        state.maximum_parameter,
                    )?;
                    state.hit = Some(hit);
                }
                mesh_probes.push(report);
                Ok(returned)
            },
        )?;
        let returned = outcome.returned;
        self.roots.push(json!({"root_key":node,"working_origin":origin,"working_delta":delta,"local_bounds":local_bounds,
            "dispatch":outcome,"meshes":mesh_probes,"child_coarse":child_coarse}));
        Ok(returned)
    }
    fn bind(&mut self, binding: Binding, matrix: Option<&[f32; 16]>) -> Result<()> {
        let plane = self
            .state
            .hit
            .as_ref()
            .context("line root returned success without mesh result")?
            .local_plane;
        self.state.binding = Some(binding);
        if self.input.stop_first_raw == 0 {
            self.state.world_normal = Some(match matrix {
                None => plane,
                Some(m) => line_frame::world_normal(plane, m)?,
            });
        }
        Ok(())
    }
}
pub fn query_map(edb: &Edb, map: u32, input: &LineSnapshot) -> Result<Value> {
    input.validate()?;
    let spatial = edb.map_spatial(map)?;
    let binding = bind_zones(&spatial, input.origin, input.delta)?;
    let initial_bounds = line_bounds::endpoint_bounds(
        input.origin,
        input.constructor_delta.unwrap_or(input.delta),
        1.,
    )?;
    let mut runtime = Runtime {
        edb,
        input,
        state: State {
            maximum_parameter: input.maximum_parameter,
            world_bounds: initial_bounds,
            hit: None,
            world_normal: None,
            binding: None,
        },
        meshes: crate::line_mesh::Meshes::default(),
        roots: Vec::new(),
    };
    let origin = [input.origin[0], input.origin[1], input.origin[2], 1.];
    let delta = [input.delta[0], input.delta[1], input.delta[2], 0.];
    let filter = GeometryFilter {
        current_slot: input.filter_slot,
        flags_raw: input.filter_flags,
    };
    let mut zones = Vec::new();
    for &zone in &binding.zones {
        let mut pass = json!({"zone":zone,"refresh":null,"candidates":null,"zone_geometry_returned":false,
            "placement_bounds_rejected":0,"placement_segment_rejected":0,"placement_entity_missing":0,"placement_filter_rejected":0,"placement_roots_queried":0});
        let stop = (|| -> Result<bool> {
            let source = crate::geometry_source::Source(edb.geometry());
            let reference = edb.map_zone_entity_reference(map, zone)?;
            if let Some(root) = source
                .reference(reference)?
                .map(|n| source.filter_node(n, filter))
                .transpose()?
                .flatten()
                && runtime.root(root, origin, delta)?
            {
                pass["zone_geometry_returned"] = json!(true);
                runtime.bind(
                    Binding {
                        zone,
                        placement_index: None,
                        candidate_ordinal: None,
                        root_key: root,
                    },
                    None,
                )?;
                if input.stop_first_raw != 0 {
                    return Ok(true);
                }
            }
            if input.filter_flags & 0x800 != 0 {
                return Ok(false);
            }
            let placements = edb.map_placements(map)?;
            let tree = edb
                .map_candidate_hierarchy(map, zone)?
                .context("line placement hierarchy absent, not clear space")?;
            let mut candidates = crate::candidates::Runtime::new(tree, edb, &placements);
            let mut accumulator = Bounds::MISSING_ENTITY;
            pass["refresh"] = serde_json::to_value(candidates.refresh(&mut accumulator)?)?;
            let selected =
                candidates.line_query(runtime.state.world_bounds, input.origin, input.delta)?;
            for (ordinal, &index) in selected.placements.iter().enumerate() {
                let placement = placements
                    .get(index as usize)
                    .context("line candidate outside placement array")?;
                let assembled = crate::initial_bounds::placement(edb, placement)?;
                let bounds = StoredBounds {
                    minimum: [
                        assembled.minimum[0],
                        assembled.minimum[1],
                        assembled.minimum[2],
                    ],
                    maximum: [
                        assembled.maximum[0],
                        assembled.maximum[1],
                        assembled.maximum[2],
                    ],
                };
                if !line_bounds::overlap(bounds, runtime.state.world_bounds)? {
                    pass["placement_bounds_rejected"] =
                        json!(pass["placement_bounds_rejected"].as_u64().unwrap() + 1);
                    continue;
                }
                if !line_bounds::segment_overlaps(bounds, input.origin, input.delta)? {
                    pass["placement_segment_rejected"] =
                        json!(pass["placement_segment_rejected"].as_u64().unwrap() + 1);
                    continue;
                }
                let Some(entity) = edb.entity_reference(placement.object_hash)? else {
                    pass["placement_entity_missing"] =
                        json!(pass["placement_entity_missing"].as_u64().unwrap() + 1);
                    continue;
                };
                let Some(root) = source.filter_node(entity.entity.address, filter)? else {
                    pass["placement_filter_rejected"] =
                        json!(pass["placement_filter_rejected"].as_u64().unwrap() + 1);
                    continue;
                };
                let (local_origin, local_delta) =
                    line_frame::working(origin, delta, &placement.model)?;
                pass["placement_roots_queried"] =
                    json!(pass["placement_roots_queried"].as_u64().unwrap() + 1);
                if runtime.root(root, local_origin, local_delta)? {
                    runtime.bind(
                        Binding {
                            zone,
                            placement_index: Some(index),
                            candidate_ordinal: Some(ordinal),
                            root_key: root,
                        },
                        Some(&placement.model),
                    )?;
                    if input.stop_first_raw != 0 {
                        pass["candidates"] = serde_json::to_value(&selected)?;
                        return Ok(true);
                    }
                }
            }
            pass["candidates"] = serde_json::to_value(selected)?;
            Ok(false)
        })()
        .with_context(|| format!("line zone {zone}"))?;
        zones.push(pass);
        if stop {
            break;
        }
    }
    let point = runtime.state.hit.as_ref().map(|_| {
        std::array::from_fn::<_, 3, _>(|i| {
            input.origin[i] + input.delta[i] * runtime.state.maximum_parameter
        })
    });
    let mut state = serde_json::to_value(&runtime.state)?;
    state["parameter_point"] = json!(point);
    Ok(
        json!({"resource_hash":edb.header.hash,"map":map,"request":input,"initial_world_bounds":initial_bounds,
        "zone_binding":binding,"line":state,"zones":zones,"roots":runtime.roots,"resident_meshes":runtime.meshes.resident_count(),
        "reference_compared":false,"scope":"Initialized static line query over explicit world/map inputs. Constructor bounds and caller delta mutation are separate; mesh hits refresh world bounds. Source segment zones, line-specific gates, transpose frames, plane caches, equal-distance replacement and optional early stop are retained. Parameter point is derived for diagnostics. Host estimate seeds, live world/actor state, dynamic positions and original-runtime parity remain unverified."}),
    )
}

#[cfg(test)]
#[path = "tests/world_line.rs"]
mod tests;
