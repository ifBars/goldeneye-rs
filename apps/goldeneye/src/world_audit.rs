//! Authored single/expanded initialized-world requests and ordered cohort checks.
use anyhow::{Context, Result, ensure};
use ge_assets::{collision::CollisionMesh, edb::Edb, spatial::MapSpatial};
use ge_core::collision::{OrientedBox, collector::GeometryFilter, dispatch::QuerySphere};
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default, Serialize)]
pub struct Audit {
    pub maps: usize,
    pub sample_origins: usize,
    pub single_queries: usize,
    pub expanded_queries: usize,
    pub zones_selected: usize,
    pub duplicate_zones: usize,
    pub maximum_zones_selected: usize,
    pub bsp_nodes_tested: usize,
    pub portal_tests: usize,
    pub records_collected: usize,
    pub contacts: usize,
    pub contacts_validated: usize,
    pub provenance_meshes_decoded: usize,
    pub queries_with_contacts: usize,
    pub resident_mesh_loads: usize,
    pub inverse_preparations: usize,
    pub record_capacity_rejections: usize,
    pub contact_capacity_mesh_skips: usize,
    pub contact_capacity_after_hit_full: usize,
    pub direct_single_record_checks: usize,
    pub direct_single_contact_checks: usize,
    pub expanded_record_sequence_checks: usize,
}
impl Audit {
    pub fn include(&mut self, edb: &Edb, map: u32, spatial: &MapSpatial) -> Result<()> {
        ensure!(
            spatial.map_hash == map,
            "world audit map/spatial identity differs"
        );
        self.maps += 1;
        let mut meshes = BTreeMap::new();
        for metadata in spatial.zones.iter().take(8) {
            let origin = std::array::from_fn(|axis| {
                ((f64::from(metadata.bounds[0][axis]) + f64::from(metadata.bounds[1][axis])) * 0.5)
                    as f32
            });
            let snapshot = crate::mesh_query::Snapshot {
                world_origin: origin,
                radius: 8.,
                world_from_query: [
                    1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
                ],
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
            let single = crate::world_query::query_map(edb, map, 0, &snapshot)?;
            let initial = count(&single["zone_binding"], "initial_zone")?;
            let direct = crate::mesh_query::query_map(edb, map, initial, &snapshot)?;
            compare_single(&single, &direct)?;
            self.direct_single_record_checks += 1;
            self.direct_single_contact_checks += 1;
            let validated = validate_contacts(edb, &single, &mut meshes)?;
            self.observe(&single, true, validated)?;

            let expanded = crate::world_query::query_map(edb, map, 2, &snapshot)?;
            ensure!(
                count(&expanded["zone_binding"], "initial_zone")? == initial,
                "world expanded initial zone differs from single binding"
            );
            let zones = selected_zones(&expanded)?;
            let mut zone_records = BTreeMap::new();
            zone_records.insert(initial, records(&single)?.clone());
            let expected = concatenate(&zones, snapshot.record_capacity, |zone| {
                if let std::collections::btree_map::Entry::Vacant(entry) = zone_records.entry(zone)
                {
                    let report = crate::static_collect::snapshot(
                        edb,
                        map,
                        zone,
                        QuerySphere {
                            origin,
                            radius: snapshot.radius,
                        },
                        GeometryFilter {
                            current_slot: snapshot.filter_slot,
                            flags_raw: snapshot.filter_flags,
                        },
                        snapshot.record_capacity,
                    )?;
                    entry.insert(
                        report["records"]
                            .as_array()
                            .context("world audit direct zone records missing")?
                            .clone(),
                    );
                }
                Ok(zone_records
                    .get(&zone)
                    .context("world audit zone cohort missing")?
                    .clone())
            })?;
            ensure!(
                records(&expanded)? == &expected,
                "expanded records differ from ordered capped single-zone cohorts"
            );
            let prefix = records(&single)?;
            ensure!(
                records(&expanded)?.starts_with(prefix),
                "expanded records lost initial-zone prefix"
            );
            self.expanded_record_sequence_checks += 1;
            let validated = validate_contacts(edb, &expanded, &mut meshes)?;
            self.observe(&expanded, false, validated)?;
            self.sample_origins += 1;
        }
        self.provenance_meshes_decoded += meshes.len();
        Ok(())
    }
    fn observe(&mut self, result: &Value, single: bool, validated: usize) -> Result<()> {
        ensure!(
            result["reference_compared"] == false,
            "world audit is not an authored diagnostic"
        );
        let binding = &result["zone_binding"];
        ensure!(
            count(binding, "request_flags_raw")? == if single { 0 } else { 2 },
            "world audit request flags differ"
        );
        let zones = selected_zones(result)?;
        let initial = count(binding, "initial_zone")?;
        ensure!(
            zones.first() == Some(&initial),
            "world audit initial zone is not first"
        );
        if single {
            ensure!(
                zones.len() == 1,
                "single-zone request expanded unexpectedly"
            );
        }
        let passes = result["collection"]["zones"]
            .as_array()
            .context("world audit zone passes missing")?;
        let records = records(result)?;
        let contacts = result["contacts"]
            .as_array()
            .context("world audit contacts missing")?;
        let probes = result["probes"]
            .as_array()
            .context("world audit mesh probes missing")?;
        ensure!(
            passes.len() == zones.len() && probes.len() == records.len(),
            "world audit pass/probe counts differ"
        );
        ensure!(
            validated == contacts.len(),
            "world audit contact validation count differs"
        );
        let mut end = 0;
        for (&zone, pass) in zones.iter().zip(passes) {
            ensure!(
                count(pass, "zone")? == zone && count(pass, "record_start")? == end,
                "world audit pass order/range start differs"
            );
            let next = count(pass, "record_end")?;
            ensure!(
                next >= end && next <= records.len(),
                "world audit pass range end is invalid"
            );
            let mut appended = 0;
            for dispatch in pass["dispatch"]
                .as_array()
                .context("world audit dispatch missing")?
            {
                appended += count(dispatch, "appended")?;
                self.record_capacity_rejections += count(dispatch, "full_rejections")?;
            }
            ensure!(
                appended == next - end,
                "world audit dispatch/record range differs"
            );
            end = next;
        }
        ensure!(
            end == records.len(),
            "world audit zone passes do not cover collected records"
        );
        for probe in probes {
            self.contact_capacity_mesh_skips += count(&probe["mesh"], "capacity_mesh_skips")?;
            self.contact_capacity_after_hit_full += count(&probe["mesh"], "post_hit_full")?;
        }
        self.resident_mesh_loads += count(result, "resident_meshes")?;
        self.inverse_preparations += count(result, "inverse_preparations")?;
        self.bsp_nodes_tested += count(binding, "bsp_nodes_tested")?;
        self.portal_tests += count(binding, "portal_tests")?;
        self.records_collected += records.len();
        self.contacts += contacts.len();
        self.contacts_validated += validated;
        self.queries_with_contacts += usize::from(!contacts.is_empty());
        self.zones_selected += zones.len();
        self.maximum_zones_selected = self.maximum_zones_selected.max(zones.len());
        self.duplicate_zones += zones.len() - zones.iter().copied().collect::<BTreeSet<_>>().len();
        if single {
            self.single_queries += 1;
        } else {
            self.expanded_queries += 1;
        }
        Ok(())
    }
}
fn records(result: &Value) -> Result<&Vec<Value>> {
    result["collection"]["records"]
        .as_array()
        .context("world audit records missing")
}
fn selected_zones(result: &Value) -> Result<Vec<usize>> {
    result["zone_binding"]["zones"]
        .as_array()
        .context("world audit selected zones missing")?
        .iter()
        .map(|zone| {
            usize::try_from(zone.as_u64().context("world audit zone index missing")?)
                .context("world audit zone index exceeds host size")
        })
        .collect()
}
fn count(value: &Value, key: &str) -> Result<usize> {
    usize::try_from(
        value[key]
            .as_u64()
            .with_context(|| format!("world audit count {key} missing"))?,
    )
    .with_context(|| format!("world audit count {key} exceeds host size"))
}
fn compare_single(world: &Value, direct: &Value) -> Result<()> {
    ensure!(
        records(world)? == records(direct)?,
        "single world records differ from direct selected-zone cohort"
    );
    ensure!(
        world["contacts"] == direct["contacts"],
        "single world contacts differ from direct selected-zone cohort"
    );
    Ok(())
}
fn concatenate(
    zones: &[usize],
    capacity: usize,
    mut records: impl FnMut(usize) -> Result<Vec<Value>>,
) -> Result<Vec<Value>> {
    let mut result = Vec::new();
    for &zone in zones {
        let values = records(zone)?;
        let remaining = capacity - result.len();
        result.extend(values.into_iter().take(remaining));
    }
    Ok(result)
}
fn validate_contacts(
    edb: &Edb,
    result: &Value,
    meshes: &mut BTreeMap<usize, CollisionMesh>,
) -> Result<usize> {
    let source_records = records(result)?;
    let contacts = result["contacts"]
        .as_array()
        .context("world audit contacts missing")?;
    for contact in contacts {
        let key = count(contact, "collected_record_key")?;
        ensure!(
            (1..=source_records.len()).contains(&key),
            "world audit contact record key is invalid"
        );
        ensure!(
            count(contact, "query_shape_index")? == 0 && count(contact, "flags_raw")? == 0,
            "world audit contact shape/flags differ"
        );
        for field in ["normal", "witness_a", "witness_b"] {
            let vector = contact[field]
                .as_array()
                .context("world audit contact vector missing")?;
            ensure!(
                vector.len() == 4
                    && vector
                        .iter()
                        .all(|v| v.as_f64().is_some_and(f64::is_finite)),
                "world audit contact vector is nonfinite"
            );
            ensure!(
                vector[3].as_f64() == Some(0.),
                "world audit identity-frame contact W differs"
            );
        }
        ensure!(
            contact["depth"].as_f64().is_some_and(f64::is_finite),
            "world audit contact depth is nonfinite"
        );
        let geometry = count(&source_records[key - 1], "geometry_key")?;
        if let std::collections::btree_map::Entry::Vacant(entry) = meshes.entry(geometry) {
            entry.insert(edb.collision_mesh_at_address(geometry)?);
        }
        crate::mesh_audit::validate_provenance(
            meshes
                .get(&geometry)
                .context("world audit provenance mesh missing")?,
            contact,
        )?;
    }
    Ok(contacts.len())
}

#[cfg(test)]
#[path = "tests/world_audit.rs"]
mod tests;
