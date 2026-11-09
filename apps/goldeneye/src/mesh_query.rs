//! Static collected-record mesh contacts for explicit authored request snapshots.
use anyhow::{Context, Result, ensure};
use ge_assets::{collision::CollisionMesh, edb::Edb, resource::GameAssets};
use ge_core::collision::{
    OrientedBox,
    batch::{self, BatchDescriptor, CacheLimits, StaticBatchCache, TriangleDescriptor},
    coarse::{self, CoarseLimits, CoarseSource},
    collector::{CollectedRecord, GeometryFilter},
    dispatch::QuerySphere,
    frame::FrameCache,
    mesh::{self, ContactBuffer, MeshRequest, MeshTriangle, RecordQuery, StaticMeshSource},
    world::StoredBounds,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs::File, io::Read, path::PathBuf};

#[derive(clap::Args)]
pub struct Options {
    disc: PathBuf,
    resource: String,
    #[arg(long,value_parser=crate::parse_hash)]
    map: u32,
    #[arg(long)]
    zone: usize,
    #[arg(long)]
    snapshot: PathBuf,
    #[arg(long)]
    pub output: Option<PathBuf>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub world_origin: [f32; 3],
    pub radius: f32,
    pub world_from_query: [f32; 16],
    pub boxes: Vec<OrientedBox>,
    pub filter_slot: u32,
    pub filter_flags: u32,
    pub record_capacity: usize,
    pub contact_capacity: usize,
}
impl Snapshot {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.world_origin
                .into_iter()
                .chain([self.radius])
                .chain(self.world_from_query)
                .all(f32::is_finite),
            "mesh snapshot query/frame values must be finite"
        );
        ensure!(
            self.boxes.len() <= 256
                && self.record_capacity <= 1_000_000
                && self.contact_capacity <= 1_000_000,
            "mesh snapshot exceeds host count budget"
        );
        for boxed in &self.boxes {
            ensure!(
                boxed
                    .center
                    .into_iter()
                    .chain(boxed.axes.into_iter().flatten())
                    .chain(boxed.half_extents)
                    .all(f32::is_finite)
                    && boxed.half_extents.into_iter().all(|v| v >= 0.),
                "invalid mesh snapshot oriented box"
            );
        }
        Ok(())
    }
}
pub fn run(options: &Options) -> Result<serde_json::Value> {
    let mut bytes = Vec::new();
    File::open(&options.snapshot)
        .context("opening mesh query snapshot")?
        .take(1_048_577)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 1_048_576, "mesh snapshot exceeds 1 MiB");
    let snapshot: Snapshot =
        serde_json::from_slice(&bytes).context("parsing mesh query snapshot")?;
    snapshot.validate()?;
    let mut assets = GameAssets::open(&options.disc, "Filelist.bin")?;
    let edb = assets.load_edb(&options.resource)?;
    query_map(&edb, options.map, options.zone, &snapshot)
}
struct Resident {
    mesh: CollisionMesh,
    batches: Vec<BatchDescriptor>,
    format: u32,
    cache: Option<StaticBatchCache>,
}
struct Source<'a>(&'a CollisionMesh);
impl StaticMeshSource for Source<'_> {
    fn triangle(&self, index: usize) -> Result<MeshTriangle> {
        let triangle = self
            .0
            .triangles
            .get(index)
            .context("mesh triangle index outside body")?;
        Ok(MeshTriangle {
            descriptor: TriangleDescriptor {
                flags_raw: triangle.flags_raw,
                position_indices: triangle.position_indices,
                strip_in_batch: triangle.strip_in_batch,
                triangle_in_strip: triangle.triangle_in_strip,
            },
            first_corner_key: u32::try_from(triangle.first_corner_offset)
                .context("mesh corner exceeds resource-local key range")?,
        })
    }
    fn coordinate(&self, position: u16, axis: usize) -> Result<f32> {
        self.0
            .positions
            .get(usize::from(position))
            .and_then(Option::as_ref)
            .and_then(|point| point.get(axis))
            .copied()
            .context("static mesh position was not decoded")
    }
}
impl Resident {
    fn new(mesh: CollisionMesh) -> Self {
        let batches = mesh
            .batches
            .iter()
            .map(|b| BatchDescriptor {
                flags_raw: b.flags_raw,
                material_raw: b.material_raw,
                declared_triangles: b.declared_triangles,
                first_triangle: b.first_triangle,
                triangle_count: b.triangle_count,
            })
            .collect();
        Self {
            format: mesh.index_format_raw,
            mesh,
            batches,
            cache: None,
        }
    }
    fn prepare(&mut self) -> Result<batch::CachePreparationStats> {
        let source = Source(&self.mesh);
        batch::prepare_static_cache(
            &mut self.format,
            &mut self.cache,
            &self.batches,
            |i| Ok(source.triangle(i)?.descriptor),
            |p, a| source.coordinate(p, a),
            CacheLimits::default(),
        )
    }
}
pub(crate) struct CoarseAdapter<'a>(pub ge_assets::coarse::View<'a>);
impl CoarseSource for CoarseAdapter<'_> {
    type Node = usize;
    fn node_count(&mut self, n: usize) -> Result<u16> {
        self.0.node_count(n)
    }
    fn pool_index(&mut self, n: usize) -> Result<u16> {
        self.0.pool_index(n)
    }
    fn sphere(&mut self, index: u16, slot: u16) -> Result<[f32; 4]> {
        self.0.sphere(index, slot)
    }
    fn child_target(&mut self, n: usize, slot: u16) -> Result<usize> {
        self.0.child_target(n, slot)
    }
    fn root_bounds(&mut self) -> Result<StoredBounds> {
        let b = self.0.root_bounds()?;
        Ok(StoredBounds {
            minimum: b.minimum,
            maximum: b.maximum,
        })
    }
    fn leaf_bounds(&mut self, index: u16) -> Result<StoredBounds> {
        let b = self.0.leaf_bounds(index)?;
        Ok(StoredBounds {
            minimum: b.minimum,
            maximum: b.maximum,
        })
    }
}
pub fn query_map(
    edb: &Edb,
    map: u32,
    zone: usize,
    snapshot: &Snapshot,
) -> Result<serde_json::Value> {
    snapshot.validate()?;
    let collection = crate::static_collect::snapshot(
        edb,
        map,
        zone,
        QuerySphere {
            origin: snapshot.world_origin,
            radius: snapshot.radius,
        },
        GeometryFilter {
            current_slot: snapshot.filter_slot,
            flags_raw: snapshot.filter_flags,
        },
        snapshot.record_capacity,
    )?;
    query_collection(edb, collection, snapshot)
}

/// Consume the complete ordered collection once, retaining frame/resident/contact
/// state across all zone passes. The collection owns its map/zone diagnostics.
pub fn query_collection(
    edb: &Edb,
    collection: serde_json::Value,
    snapshot: &Snapshot,
) -> Result<serde_json::Value> {
    snapshot.validate()?;
    let records: Vec<CollectedRecord> = serde_json::from_value(collection["records"].clone())?;
    let request = MeshRequest::new(
        snapshot.world_origin,
        snapshot.radius,
        &snapshot.world_from_query,
        &snapshot.boxes,
    )?;
    let mut contacts = ContactBuffer::new(snapshot.contact_capacity)?;
    let mut frames = FrameCache::new(None);
    let mut preparations = 0;
    let mut residents = BTreeMap::new();
    let mut probes = Vec::new();
    for (index, record) in records.iter().enumerate() {
        ensure!(
            record.optional_context_key == 0,
            "initialized mesh query cannot infer optional dynamic position context"
        );
        let frame = frames.resolve(record.frame_key(), || {
            preparations += 1;
            Ok((record.matrix, record.flags_raw))
        })?;
        if let std::collections::btree_map::Entry::Vacant(entry) =
            residents.entry(record.geometry_key)
        {
            entry.insert(Resident::new(
                edb.collision_mesh_at_address(record.geometry_key as usize)?,
            ));
        }
        let resident = residents
            .get_mut(&record.geometry_key)
            .context("resident mesh missing")?;
        let cache = resident.prepare()?;
        let mut coarse_stats = None;
        let stats = mesh::query_record(
            &Source(&resident.mesh),
            &resident.batches,
            resident.cache.as_ref(),
            RecordQuery {
                record,
                cached_frame: frame,
                collected_record_key: (index + 1) as u32,
            },
            &request,
            &mut contacts,
            |origin, radius| {
                let Some(view) = edb.collision_coarse_at_address(record.geometry_key as usize)?
                else {
                    return Ok(None);
                };
                let root = view.root;
                let mut source = CoarseAdapter(view);
                let result = coarse::query(
                    &mut source,
                    root,
                    origin,
                    radius,
                    CoarseLimits {
                        node_reads: 1_000_000,
                        sphere_tests: 1_000_000,
                        fanout: 16,
                    },
                )?;
                let overlaps = result.overlaps;
                coarse_stats = Some(result);
                Ok(Some(overlaps))
            },
        )
        .with_context(|| {
            format!(
                "collected record {index} mesh at {:#x}",
                record.geometry_key
            )
        })?;
        probes.push(serde_json::json!({"record_key":index+1,"geometry_key":record.geometry_key,"cache":cache,"coarse":coarse_stats,"mesh":stats}));
    }
    Ok(
        serde_json::json!({"resource_hash":edb.header.hash,"map":collection["map"],"zone":collection.get("zone"),"request":snapshot,"collection":collection,"resident_meshes":residents.len(),"inverse_preparations":preparations,
        "probes":probes,"contacts":contacts.contacts(),"reference_compared":false,
        "scope":"Initialized static-map type-three box contacts from authored query/filter/frame/capacities. Resource-local mesh/corner keys and one-based collected-record keys are host identities. Source membership is retained; live mutation, dynamic positions, live actor/world-manager integration and original-runtime/FP estimate parity remain unverified."}),
    )
}

#[cfg(test)]
#[path = "tests/mesh_query.rs"]
mod tests;
