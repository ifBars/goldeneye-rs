//! Initial static-map collection with explicit query/filter/capacity, not contacts.
use anyhow::{Context, Result, ensure};
use ge_assets::{edb::Edb, resource::GameAssets};
use ge_core::collision::{
    bounds::Bounds,
    collector::{CollectionContext, GeometryFilter, RecordBuffer, map_placement_flags},
    dispatch::{DispatchLimits, DispatchRequest, QuerySphere, collect},
};
use std::path::PathBuf;

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
    /// Explicit request filter slot; not inferred from render LOD.
    #[arg(long)]
    filter_slot: u32,
    /// Explicit request filter flags, in hexadecimal.
    #[arg(long,value_parser=crate::parse_hash)]
    filter_flags: u32,
    /// Authored host record capacity; saturation does not establish clear space.
    #[arg(long, default_value_t = 4096)]
    capacity: usize,
    #[arg(long)]
    pub output: Option<PathBuf>,
}
pub fn run(options: &Options) -> Result<serde_json::Value> {
    ensure!(
        options.origin.iter().all(|v| v.is_finite()) && options.radius.is_finite(),
        "collection query values must be finite"
    );
    ensure!(
        options.capacity <= 1_000_000,
        "collection capacity exceeds host budget"
    );
    let origin = options
        .origin
        .as_slice()
        .try_into()
        .context("collection origin needs three coordinates")?;
    let mut assets = GameAssets::open(&options.disc, "Filelist.bin")?;
    let edb = assets.load_edb(&options.resource)?;
    snapshot(
        &edb,
        options.map,
        options.zone,
        QuerySphere {
            origin,
            radius: options.radius,
        },
        GeometryFilter {
            current_slot: options.filter_slot,
            flags_raw: options.filter_flags,
        },
        options.capacity,
    )
}
pub fn snapshot(
    edb: &Edb,
    map: u32,
    zone: usize,
    query: QuerySphere,
    filter: GeometryFilter,
    capacity: usize,
) -> Result<serde_json::Value> {
    let mut records = RecordBuffer::new(capacity)?;
    let mut result = append_zone(edb, map, zone, query, filter, &mut records)?;
    result["records"] = serde_json::to_value(records.records())?;
    Ok(result)
}

/// One source-ordered zone pass into the request's existing record buffer.
/// Candidate ordinals restart in each zone; records and capacity do not reset.
pub fn append_zone(
    edb: &Edb,
    map: u32,
    zone: usize,
    query: QuerySphere,
    filter: GeometryFilter,
    records: &mut RecordBuffer,
) -> Result<serde_json::Value> {
    let record_start = records.records().len();
    let zone_reference = edb.map_zone_entity_reference(map, zone)?;
    let mut source = crate::geometry_source::Source(edb.geometry());
    let identity = [
        1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
    ];
    let mut outcomes = Vec::new();
    if let Some(node) = source.reference(zone_reference)?
        && let Some(node) = source.filter_node(node, filter)?
    {
        outcomes.push(collect(
            &mut source,
            node,
            &DispatchRequest {
                query,
                context: CollectionContext::map_zone(identity),
                optional_context_key: 0,
                traversal_flags: 0,
                collection_flags: 0,
            },
            records,
            DispatchLimits::default(),
        )?);
    }
    let mut refresh = None;
    let mut candidates = None;
    let mut bounds_rejected = 0;
    let mut entity_missing = 0;
    let mut filter_rejected = 0;
    if filter.flags_raw & 0x800 == 0 {
        let placements = edb.map_placements(map)?;
        let tree = edb
            .map_candidate_hierarchy(map, zone)?
            .context("collection hierarchy absent, not clear space")?;
        let mut runtime = crate::candidates::Runtime::new(tree, edb, &placements);
        let mut accumulator = Bounds::MISSING_ENTITY;
        refresh = Some(runtime.refresh(&mut accumulator)?);
        let selected = runtime.query(query.origin, query.radius)?;
        for (ordinal, &index) in selected.placements.iter().enumerate() {
            let placement = placements
                .get(index as usize)
                .context("collected candidate outside placement array")?;
            if !crate::initial_bounds::placement(edb, placement)?
                .contains_with_margin(query.origin, query.radius)?
            {
                bounds_rejected += 1;
                continue;
            }
            let Some(reference) = edb.entity_reference(placement.object_hash)? else {
                entity_missing += 1;
                continue;
            };
            let Some(node) = source.filter_node(reference.entity.address, filter)? else {
                filter_rejected += 1;
                continue;
            };
            outcomes.push(collect(
                &mut source,
                node,
                &DispatchRequest {
                    query,
                    context: CollectionContext::map_placement(placement.model, ordinal as u32),
                    optional_context_key: 0,
                    traversal_flags: 0,
                    collection_flags: map_placement_flags(0, placement.engine_flags),
                },
                records,
                DispatchLimits::default(),
            )?);
        }
        candidates = Some(selected);
    }
    Ok(
        serde_json::json!({"map":map,"zone":zone,"origin":query.origin,"radius":query.radius,"filter_slot":filter.current_slot,"filter_flags":filter.flags_raw,"capacity":records.capacity(),
        "refresh":refresh,"candidates":candidates,"placement_bounds_rejected":bounds_rejected,"placement_entity_missing":entity_missing,"placement_filter_rejected":filter_rejected,
        "dispatch":outcomes,"record_start":record_start,"record_end":records.records().len(),"reference_compared":false,
        "scope":"Initialized static map snapshot with source membership flags, explicit authored query/filter/capacity, seeded hierarchy refresh and ordered recursive geometry records. No triangle contacts, live membership, gameplay clearance or original-runtime comparison."}),
    )
}

#[derive(Default, serde::Serialize)]
pub struct Audit {
    pub maps: usize,
    pub queries: usize,
    pub records_collected: usize,
    pub maximum_records: usize,
    pub sphere_rejections: usize,
    pub bounds_rejections: usize,
    pub full_rejections: usize,
    pub frame_origins_prepared: usize,
    pub inverse_preparations: usize,
}
impl Audit {
    pub fn include(
        &mut self,
        edb: &Edb,
        map: u32,
        spatial: &ge_assets::spatial::MapSpatial,
    ) -> Result<()> {
        self.maps += 1;
        for (zone, metadata) in spatial.zones.iter().enumerate().take(8) {
            let origin = std::array::from_fn(|i| {
                ((f64::from(metadata.bounds[0][i]) + f64::from(metadata.bounds[1][i])) * 0.5) as f32
            });
            let result = snapshot(
                edb,
                map,
                zone,
                QuerySphere { origin, radius: 8. },
                GeometryFilter {
                    current_slot: 0,
                    flags_raw: 0,
                },
                4096,
            )?;
            self.queries += 1;
            let records = result["records"]
                .as_array()
                .context("static collection records missing")?;
            self.records_collected += records.len();
            self.maximum_records = self.maximum_records.max(records.len());
            for dispatch in result["dispatch"]
                .as_array()
                .context("static collection dispatch missing")?
            {
                self.sphere_rejections += dispatch["sphere_rejections"]
                    .as_u64()
                    .context("sphere rejection count missing")?
                    as usize;
                self.bounds_rejections += dispatch["bounds_rejections"]
                    .as_u64()
                    .context("bounds rejection count missing")?
                    as usize;
                self.full_rejections += dispatch["full_rejections"]
                    .as_u64()
                    .context("full rejection count missing")?
                    as usize;
            }
            let mut cache = ge_core::collision::frame::FrameCache::new(None);
            for record in records {
                let matrix: [f32; 16] = serde_json::from_value(record["matrix"].clone())?;
                let flags = record["flags_raw"]
                    .as_u64()
                    .context("collected flags missing")? as u16;
                let key = ge_core::collision::frame::FrameKey {
                    owner_key: record["owner_key"]
                        .as_u64()
                        .context("collected owner missing")? as u32,
                    slot: record["slot"].as_i64().context("collected slot missing")? as i16,
                };
                let frame = cache.resolve(key, || {
                    self.inverse_preparations += 1;
                    Ok((matrix, flags))
                })?;
                frame.query_origin(origin, [matrix[12], matrix[13], matrix[14]])?;
                self.frame_origins_prepared += 1;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests/static_collect.rs"]
mod tests;
