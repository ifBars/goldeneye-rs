//! Explicit initialized world-manager snapshots, with lazy selected-resource loading.
use anyhow::{Context, Result, ensure};
use ge_assets::{edb::Edb, resource::GameAssets};
use ge_core::collision::{
    collector::{GeometryFilter, RecordBuffer},
    dispatch::QuerySphere,
    portal::{self, PortalGroup},
    world::{BspNode, MapBsp, StoredBounds, select_loaded_world},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

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
pub struct BoundsInput {
    pub minimum: [f32; 3],
    pub maximum: [f32; 3],
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct World {
    pub resource: String,
    pub map: u32,
    /// Explicit manager-stored bounds, never inferred from the resource's zones.
    pub bounds: BoundsInput,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub worlds: Vec<World>,
    pub request_flags_raw: u32,
    pub query: crate::mesh_query::Snapshot,
}
impl Snapshot {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.worlds.len() <= 65536,
            "loaded-world host count budget exceeded"
        );
        self.query.validate()
    }
}
pub(crate) fn read_snapshot<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let mut bytes = Vec::new();
    File::open(path)
        .context("opening world query snapshot")?
        .take(1_048_577)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 1_048_576, "world snapshot exceeds 1 MiB");
    serde_json::from_slice(&bytes).context("parsing world query snapshot")
}
pub fn run(options: &Options) -> Result<Value> {
    let snapshot: Snapshot = read_snapshot(&options.snapshot)?;
    query_loaded(&snapshot, |resource| {
        GameAssets::open(&options.disc, "Filelist.bin")?.load_edb(resource)
    })
}
pub fn query_loaded(snapshot: &Snapshot, load: impl FnMut(&str) -> Result<Edb>) -> Result<Value> {
    snapshot.validate()?;
    query_worlds(
        &snapshot.worlds,
        snapshot.request_flags_raw,
        &snapshot.query,
        load,
    )
}
pub fn query_worlds(
    worlds: &[World],
    request_flags_raw: u32,
    query: &crate::mesh_query::Snapshot,
    mut load: impl FnMut(&str) -> Result<Edb>,
) -> Result<Value> {
    ensure!(
        worlds.len() <= 65536,
        "loaded-world host count budget exceeded"
    );
    query.validate()?;
    let mut bounds_tested = Vec::new();
    let selected = select_loaded_world(worlds.len(), query.world_origin, |index| {
        bounds_tested.push(index);
        let bounds = &worlds[index].bounds;
        Ok(StoredBounds {
            minimum: bounds.minimum,
            maximum: bounds.maximum,
        })
    })?;
    let result = selected
        .map(|index| -> Result<Value> {
            let world = &worlds[index];
            let edb = load(&world.resource)
                .with_context(|| format!("selected world {index} resource {}", world.resource))?;
            query_map(&edb, world.map, request_flags_raw, query)
        })
        .transpose()?;
    Ok(
        json!({"world_count":worlds.len(),"selected_world":selected,"bounds_tested":bounds_tested,
        "query":result,"reference_compared":false,
        "scope":"Authored ordered world-manager snapshot and initialized static maps only. Stored world bounds and membership are explicit inputs; only the selected resource loads. One record/contact/cache sequence spans BSP/portal-selected zones. No live membership, dynamic positions, actor line queries, physics response or original-runtime/FP parity."}),
    )
}
pub fn query_map(
    edb: &Edb,
    map: u32,
    request_flags_raw: u32,
    query: &crate::mesh_query::Snapshot,
) -> Result<Value> {
    query.validate()?;
    let spatial = edb.map_spatial(map)?;
    ensure!(
        spatial.bsp_present,
        "world query BSP absent, not clear space"
    );
    let bsp = MapBsp {
        zone_count: spatial.zones.len(),
        nodes: spatial
            .nodes
            .iter()
            .map(|(&index, node)| {
                (
                    index,
                    BspNode {
                        plane: node.plane,
                        children: node.children,
                    },
                )
            })
            .collect(),
    };
    let hit = bsp.zone_at(query.world_origin, 65536)?;
    let mut portal_tests = 0;
    let zones = if request_flags_raw & 2 != 0 {
        let data = edb.map_portal_query(map)?;
        ensure!(
            data.zone_count == spatial.zones.len(),
            "world portal/BSP zone counts differ"
        );
        let groups = data
            .groups(hit.zone)?
            .iter()
            .map(|group| PortalGroup {
                target_zone: group.target_zone,
                portal_indices: group.rows.iter().map(|row| row.portal_index_raw).collect(),
            })
            .collect::<Vec<_>>();
        let mut shapes = std::collections::BTreeMap::new();
        let mut vertices_read = 0usize;
        let expanded = portal::expand(
            hit.zone,
            spatial.zones.len(),
            &groups,
            query.radius,
            1_000_000,
            65536,
            |index| {
                if let std::collections::btree_map::Entry::Vacant(entry) = shapes.entry(index) {
                    let shape = data.shape(index)?;
                    vertices_read += shape.vertices().len();
                    ensure!(
                        vertices_read <= 1_000_000,
                        "world query portal vertex budget exceeded"
                    );
                    entry.insert(shape);
                }
                let shape = shapes
                    .get(&index)
                    .context("world query portal shape missing")?;
                Ok(portal::distance(shape.vertices(), query.world_origin, 65536)?.distance)
            },
        )?;
        portal_tests = expanded.portal_tests;
        expanded.zones
    } else {
        vec![hit.zone]
    };
    let mut records = RecordBuffer::new(query.record_capacity)?;
    let mut passes = Vec::new();
    for &zone in &zones {
        passes.push(crate::static_collect::append_zone(
            edb,
            map,
            zone,
            QuerySphere {
                origin: query.world_origin,
                radius: query.radius,
            },
            GeometryFilter {
                current_slot: query.filter_slot,
                flags_raw: query.filter_flags,
            },
            &mut records,
        )?);
    }
    let collection = json!({"map":map,"zones":passes,"records":records.records(),"reference_compared":false,
        "scope":"Ordered initialized static zone passes into one fixed-capacity record buffer; duplicate zones and per-zone candidate ordinals retained. Live membership remains unverified."});
    let mut result = crate::mesh_query::query_collection(edb, collection, query)?;
    result["zone_binding"] = json!({"request_flags_raw":request_flags_raw,"initial_zone":hit.zone,"zones":zones,
        "bsp_nodes_tested":hit.nodes_tested,"portal_tests":portal_tests});
    Ok(result)
}

#[cfg(test)]
#[path = "tests/world_query.rs"]
mod tests;
