//! Authored zone-center type-three contacts through initialized static map inputs.
use anyhow::{Context, Result, ensure};
use ge_assets::{collision::CollisionMesh, edb::Edb, spatial::MapSpatial};
use ge_core::collision::OrientedBox;
use serde::Serialize;
use serde_json::Value;

const IDENTITY: [f32; 16] = [
    1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
];

#[derive(Default, Serialize)]
pub struct Audit {
    pub maps: usize,
    pub queries: usize,
    pub records_collected: usize,
    pub maximum_records: usize,
    pub resident_mesh_loads: usize,
    pub inverse_preparations: usize,
    pub cache_generated: usize,
    pub cache_reused: usize,
    pub cache_skipped_format: usize,
    pub cache_no_included_batches: usize,
    pub cache_triangle_visits: usize,
    pub cache_coordinate_reads: usize,
    pub coarse_queries: usize,
    pub coarse_rejections: usize,
    pub coarse_absent_records: usize,
    pub coarse_bypassed_general_records: usize,
    pub coarse_node_reads: usize,
    pub coarse_sphere_tests: usize,
    pub coarse_leaf_tests: usize,
    /// None means no coarse tree was queried; Some(0) includes root-leaf queries.
    pub coarse_maximum_fanout: Option<usize>,
    pub included_batches: usize,
    pub batch_rejections: usize,
    pub triangle_tests: usize,
    pub triangle_rejections: usize,
    pub flag_words_consumed: usize,
    pub primitive_queries: usize,
    pub primitive_contacts: usize,
    pub contacts_appended: usize,
    pub contacts_validated: usize,
    pub provenance_meshes_decoded: usize,
    pub queries_with_contacts: usize,
    pub maximum_contacts: usize,
    pub record_capacity_rejections: usize,
    pub contact_capacity_mesh_skips: usize,
    pub contact_capacity_after_hit_full: usize,
}
impl Audit {
    pub fn include(&mut self, edb: &Edb, map: u32, spatial: &MapSpatial) -> Result<()> {
        self.maps += 1;
        let mut provenance = std::collections::BTreeMap::new();
        for (zone, metadata) in spatial.zones.iter().enumerate().take(8) {
            let origin = std::array::from_fn(|axis| {
                ((f64::from(metadata.bounds[0][axis]) + f64::from(metadata.bounds[1][axis])) * 0.5)
                    as f32
            });
            let snapshot = crate::mesh_query::Snapshot {
                world_origin: origin,
                radius: 8.,
                world_from_query: IDENTITY,
                boxes: vec![OrientedBox {
                    center: origin,
                    axes: [[1., 0., 0.], [0., 1., 0.]],
                    half_extents: [1.; 3],
                }],
                filter_slot: 0,
                filter_flags: 0,
                record_capacity: 4096,
                contact_capacity: 4096,
            };
            let result = crate::mesh_query::query_map(edb, map, zone, &snapshot)
                .with_context(|| format!("zone {zone} authored static box contacts"))?;
            let mut decoded = 0;
            self.observe(
                &result,
                edb.header.payload_bytes as usize,
                |contact, key| {
                    if let std::collections::btree_map::Entry::Vacant(entry) = provenance.entry(key)
                    {
                        entry.insert(edb.collision_mesh_at_address(key)?);
                        decoded += 1;
                    }
                    validate_provenance(
                        provenance.get(&key).context("provenance mesh missing")?,
                        contact,
                    )
                },
            )?;
            self.provenance_meshes_decoded += decoded;
        }
        Ok(())
    }
    fn observe(
        &mut self,
        result: &Value,
        payload_bytes: usize,
        mut provenance: impl FnMut(&Value, usize) -> Result<()>,
    ) -> Result<()> {
        ensure!(
            result["reference_compared"] == false,
            "mesh audit result is not an authored diagnostic"
        );
        let frame: [f32; 16] =
            serde_json::from_value(result["request"]["world_from_query"].clone())?;
        ensure!(frame == IDENTITY, "mesh audit expects identity query frame");
        let records = result["collection"]["records"]
            .as_array()
            .context("mesh audit records missing")?;
        let probes = result["probes"]
            .as_array()
            .context("mesh audit probes missing")?;
        let contacts = result["contacts"]
            .as_array()
            .context("mesh audit contacts missing")?;
        ensure!(
            records.len() == probes.len(),
            "mesh audit record/probe counts differ"
        );
        ensure!(
            records.len() <= 4096 && contacts.len() <= 4096,
            "mesh audit exceeds authored capacities"
        );
        let loads = count(result, "resident_meshes")?;
        let preparations = count(result, "inverse_preparations")?;
        ensure!(
            loads <= records.len() && preparations <= records.len(),
            "mesh audit preparation counts exceed records"
        );
        self.resident_mesh_loads += loads;
        self.inverse_preparations += preparations;
        self.records_collected += records.len();
        self.maximum_records = self.maximum_records.max(records.len());
        let mut appended = 0;
        for (index, (record, probe)) in records.iter().zip(probes).enumerate() {
            ensure!(
                count(probe, "record_key")? == index + 1
                    && probe["geometry_key"] == record["geometry_key"],
                "mesh audit record provenance differs"
            );
            let cache = &probe["cache"];
            match cache["disposition"]
                .as_str()
                .context("mesh audit cache disposition missing")?
            {
                "generated" => self.cache_generated += 1,
                "reused" => self.cache_reused += 1,
                "skipped_format" => self.cache_skipped_format += 1,
                "no_included_batches" => self.cache_no_included_batches += 1,
                _ => anyhow::bail!("unknown mesh audit cache disposition"),
            }
            self.cache_triangle_visits += count(cache, "triangle_visits")?;
            self.cache_coordinate_reads += count(cache, "coordinate_reads")?;
            let mesh = &probe["mesh"];
            let coarse = probe
                .get("coarse")
                .context("mesh audit coarse evidence missing")?;
            let general = count(record, "flags_raw")? & 2 != 0;
            if coarse.is_null() {
                ensure!(
                    count(mesh, "coarse_tests")? == 0 && count(mesh, "coarse_rejections")? == 0,
                    "absent coarse result has query counts"
                );
                if general {
                    self.coarse_bypassed_general_records += 1;
                } else {
                    self.coarse_absent_records += 1;
                }
            } else {
                ensure!(
                    !general && count(mesh, "coarse_tests")? == 1,
                    "coarse query violated record flag gate"
                );
                let overlaps = coarse["overlaps"]
                    .as_bool()
                    .context("mesh audit coarse result missing")?;
                ensure!(
                    count(mesh, "coarse_rejections")? == usize::from(!overlaps),
                    "coarse rejection count differs"
                );
                self.coarse_queries += 1;
                self.coarse_rejections += usize::from(!overlaps);
                self.coarse_node_reads += count(coarse, "node_reads")?;
                self.coarse_sphere_tests += count(coarse, "sphere_tests")?;
                self.coarse_leaf_tests += count(coarse, "leaf_tests")?;
                let fanout = count(coarse, "maximum_fanout")?;
                ensure!(fanout <= 16, "mesh audit coarse fanout is unsupported");
                self.coarse_maximum_fanout =
                    Some(self.coarse_maximum_fanout.unwrap_or(0).max(fanout));
            }
            self.included_batches += count(mesh, "included_batches")?;
            self.batch_rejections += count(mesh, "batch_rejections")?;
            self.triangle_tests += count(mesh, "triangle_tests")?;
            self.triangle_rejections += count(mesh, "triangle_rejections")?;
            self.flag_words_consumed += count(mesh, "flag_words_consumed")?;
            self.primitive_queries += count(mesh, "primitive_queries")?;
            self.primitive_contacts += count(mesh, "primitive_contacts")?;
            appended += count(mesh, "appended")?;
            self.contact_capacity_mesh_skips += count(mesh, "capacity_mesh_skips")?;
            self.contact_capacity_after_hit_full += count(mesh, "post_hit_full")?;
        }
        ensure!(
            appended == contacts.len(),
            "mesh audit appended/contact count differs"
        );
        for contact in contacts {
            let record_key = count(contact, "collected_record_key")?;
            ensure!(
                (1..=records.len()).contains(&record_key),
                "mesh audit contact record key is invalid"
            );
            let geometry_key = count(&records[record_key - 1], "geometry_key")?;
            ensure!(
                geometry_key < payload_bytes,
                "mesh audit geometry key outside resource"
            );
            ensure!(
                count(contact, "first_corner_key")? < payload_bytes,
                "mesh audit corner key outside resource"
            );
            ensure!(
                count(contact, "query_shape_index")? == 0 && count(contact, "flags_raw")? == 0,
                "mesh audit contact shape/flags differ"
            );
            ensure!(
                count(contact, "triangle_flags_raw")? & 0x200 == 0,
                "disabled triangle produced mesh audit contact"
            );
            let indices = contact["position_indices"]
                .as_array()
                .context("mesh audit position indices missing")?;
            ensure!(
                indices.len() == 3
                    && indices
                        .iter()
                        .all(|v| v.as_u64().is_some_and(|i| i <= u64::from(u16::MAX))),
                "mesh audit position index is invalid"
            );
            for key in ["normal", "witness_a", "witness_b"] {
                let vector = contact[key]
                    .as_array()
                    .context("mesh audit contact vector missing")?;
                ensure!(
                    vector.len() == 4
                        && vector
                            .iter()
                            .all(|v| v.as_f64().is_some_and(f64::is_finite)),
                    "mesh audit nonfinite contact vector"
                );
                ensure!(
                    vector[3].as_f64() == Some(0.),
                    "identity-frame contact W differs"
                );
            }
            ensure!(
                contact["depth"].as_f64().is_some_and(f64::is_finite),
                "mesh audit nonfinite contact depth"
            );
            let norm = contact["normal"].as_array().unwrap()[..3]
                .iter()
                .map(|v| v.as_f64().unwrap().powi(2))
                .sum::<f64>();
            ensure!(
                (norm - 1.).abs() <= 0.0001,
                "mesh audit identity-frame normal is not unit length"
            );
            provenance(contact, geometry_key)?;
            self.contacts_validated += 1;
        }
        for dispatch in result["collection"]["dispatch"]
            .as_array()
            .context("mesh audit collection dispatch missing")?
        {
            self.record_capacity_rejections += count(dispatch, "full_rejections")?;
        }
        self.contacts_appended += appended;
        self.maximum_contacts = self.maximum_contacts.max(contacts.len());
        self.queries_with_contacts += usize::from(!contacts.is_empty());
        self.queries += 1;
        Ok(())
    }
}
pub(crate) fn validate_provenance(mesh: &CollisionMesh, contact: &Value) -> Result<()> {
    let corner = count(contact, "first_corner_key")?;
    let flags = count(contact, "triangle_flags_raw")?;
    let material = count(contact, "material_raw")?;
    let actual: [u32; 3] = serde_json::from_value(contact["position_indices"].clone())?;
    for (index, triangle) in mesh
        .triangles
        .iter()
        .enumerate()
        .filter(|(_, t)| t.first_corner_offset == corner)
    {
        let batch = mesh
            .batches
            .get(triangle.batch_index)
            .context("provenance batch missing")?;
        ensure!(
            index >= batch.first_triangle && index - batch.first_triangle < batch.triangle_count,
            "provenance triangle outside batch range"
        );
        let mut expected = triangle.position_indices.map(u32::from);
        if triangle.triangle_in_strip & 1 != 0 {
            expected.swap(1, 2);
        }
        if triangle.enabled()
            && batch.included()
            && usize::from(triangle.flags_raw) == flags
            && usize::from(batch.material_raw) == material
            && expected == actual
        {
            return Ok(());
        }
    }
    anyhow::bail!("mesh audit contact provenance does not match enabled source triangle/material")
}
fn count(value: &Value, key: &str) -> Result<usize> {
    usize::try_from(
        value[key]
            .as_u64()
            .with_context(|| format!("mesh audit count {key} missing"))?,
    )
    .with_context(|| format!("mesh audit count {key} exceeds host size"))
}

#[cfg(test)]
#[path = "tests/mesh_audit.rs"]
mod tests;
