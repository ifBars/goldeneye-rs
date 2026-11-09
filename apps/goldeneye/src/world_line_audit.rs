//! Bounded authored full-map line diagnostics; never a reference-runtime receipt.
use anyhow::{Context, Result, ensure};
use ge_assets::{collision::CollisionMesh, edb::Edb, spatial::MapSpatial};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Default, Serialize)]
pub struct Audit {
    pub maps: usize,
    pub sample_origins: usize,
    pub queries: usize,
    pub nearest_queries: usize,
    pub any_hit_queries: usize,
    pub mode_comparisons: usize,
    pub hits: usize,
    pub misses: usize,
    pub zones_selected: usize,
    pub zone_capacity_reached: usize,
    pub maximum_zones_selected: usize,
    pub bsp_nodes_tested: usize,
    pub roots_queried: usize,
    pub mesh_queries: usize,
    pub resident_meshes: usize,
    pub planes_generated: usize,
    pub planes_reused: usize,
    pub planes_prepared: usize,
    pub cache_generated: usize,
    pub cache_reused: usize,
    pub cache_skipped_format: usize,
    pub cache_no_included_batches: usize,
    pub bounds_rejections: usize,
    pub coarse_queries: usize,
    pub coarse_node_reads: usize,
    pub coarse_sphere_tests: usize,
    pub coarse_leaf_tests: usize,
    pub coarse_maximum_fanout: Option<usize>,
    pub batch_bounds_tests: usize,
    pub batches_rejected: usize,
    pub triangles_tested: usize,
    pub triangles_rejected: usize,
    pub flag_words_advanced: usize,
    pub plane_records_advanced: usize,
    pub mesh_hits_validated: usize,
    pub final_hits_validated: usize,
    pub provenance_meshes_decoded: usize,
    pub f64_plane_checks: usize,
    pub f64_degenerate_triangles: usize,
    pub f64_short_normal_triangles: usize,
    /// Diagnostic distance to the raw triangle plane; no reference tolerance claim.
    pub maximum_f64_plane_distance: Option<f64>,
}
impl Audit {
    pub fn include(&mut self, edb: &Edb, map: u32, spatial: &MapSpatial) -> Result<()> {
        ensure!(
            spatial.map_hash == map,
            "line audit map/spatial identity differs"
        );
        self.maps += 1;
        let mut meshes = BTreeMap::new();
        for zone in spatial.zones.iter().take(8) {
            let [minimum, maximum] = zone.bounds;
            let center = std::array::from_fn::<_, 3, _>(|i| {
                ((f64::from(minimum[i]) + f64::from(maximum[i])) * 0.5) as f32
            });
            let top = maximum[1] + 1.;
            let bottom = minimum[1] - 1.;
            let vertical = snapshot([center[0], top, center[2]], [0., bottom - top, 0.], false);
            let nearest = crate::world_line::query_map(edb, map, &vertical)?;
            self.observe(&nearest, false, |hit| {
                validate_hit(load(edb, &mut meshes, hit)?, hit).map(|_| ())
            })?;
            self.plane_diagnostic(&nearest, &meshes)?;
            let any = crate::world_line::query_map(
                edb,
                map,
                &crate::world_line::LineSnapshot {
                    stop_first_raw: 1,
                    ..vertical
                },
            )?;
            self.observe(&any, true, |hit| {
                validate_hit(load(edb, &mut meshes, hit)?, hit).map(|_| ())
            })?;
            self.plane_diagnostic(&any, &meshes)?;
            compare_modes(&nearest, &any)?;
            self.mode_comparisons += 1;
            let diagonal = snapshot(
                minimum,
                std::array::from_fn(|i| maximum[i] - minimum[i]),
                false,
            );
            let result = crate::world_line::query_map(edb, map, &diagonal)?;
            self.observe(&result, false, |hit| {
                validate_hit(load(edb, &mut meshes, hit)?, hit).map(|_| ())
            })?;
            self.plane_diagnostic(&result, &meshes)?;
            self.sample_origins += 1;
        }
        self.provenance_meshes_decoded += meshes.len();
        Ok(())
    }
    fn observe(
        &mut self,
        result: &Value,
        any: bool,
        mut provenance: impl FnMut(&Value) -> Result<()>,
    ) -> Result<()> {
        ensure!(
            result["reference_compared"] == false,
            "line audit cannot promote reference evidence"
        );
        ensure!(
            count(&result["request"], "stop_first_raw")? == usize::from(any),
            "line audit query mode differs"
        );
        let binding = &result["zone_binding"];
        let zones = binding["zones"]
            .as_array()
            .context("line audit selected zones missing")?;
        self.zones_selected += zones.len();
        self.maximum_zones_selected = self.maximum_zones_selected.max(zones.len());
        self.zone_capacity_reached += usize::from(boolean(binding, "capacity_reached")?);
        self.bsp_nodes_tested += count(binding, "node_tests")?;
        self.resident_meshes += count(result, "resident_meshes")?;
        let roots = result["roots"]
            .as_array()
            .context("line audit root probes missing")?;
        self.roots_queried += roots.len();
        for root in roots {
            finite_array::<4>(&root["working_origin"])?;
            finite_array::<4>(&root["working_delta"])?;
            for coarse in root["child_coarse"]
                .as_array()
                .context("line audit child coarse probes missing")?
            {
                self.observe_coarse(&coarse["result"])?;
            }
            for report in root["meshes"]
                .as_array()
                .context("line audit mesh probes missing")?
            {
                self.mesh_queries += 1;
                self.observe_coarse(&report["coarse"])?;
                let mesh = &report["mesh"];
                match mesh["cache_preparation"]["disposition"]
                    .as_str()
                    .context("line cache disposition missing")?
                {
                    "generated" => self.cache_generated += 1,
                    "reused" => self.cache_reused += 1,
                    "skipped_format" => self.cache_skipped_format += 1,
                    "no_included_batches" => self.cache_no_included_batches += 1,
                    _ => anyhow::bail!("line audit unknown cache disposition"),
                }
                let generated = boolean(mesh, "planes_generated")?;
                let plane_count = count(mesh, "planes_count")?;
                if generated {
                    self.planes_generated += 1;
                    self.planes_prepared += plane_count;
                } else if plane_count > 0
                    && !boolean(mesh, "bounds_rejected")?
                    && !boolean(mesh, "coarse_rejected")?
                {
                    self.planes_reused += 1;
                }
                self.bounds_rejections += usize::from(boolean(mesh, "bounds_rejected")?);
                self.batch_bounds_tests += count(mesh, "batch_bounds_tests")?;
                self.batches_rejected += count(mesh, "batches_rejected")?;
                self.triangles_tested += count(mesh, "triangles_tested")?;
                self.triangles_rejected += count(mesh, "triangles_rejected")?;
                self.flag_words_advanced += count(mesh, "flag_words_advanced")?;
                self.plane_records_advanced += count(mesh, "plane_records_advanced")?;
                if !mesh["hit"].is_null() {
                    ensure!(
                        count(mesh, "geometry_key")? == count(&mesh["hit"], "geometry_key")?,
                        "line mesh hit geometry differs"
                    );
                    validate_finite_hit(&mesh["hit"])?;
                    provenance(&mesh["hit"])?;
                    self.mesh_hits_validated += 1;
                }
            }
        }
        let line = &result["line"];
        if line["hit"].is_null() {
            ensure!(
                line["binding"].is_null()
                    && line["world_normal"].is_null()
                    && line["parameter_point"].is_null(),
                "line miss has hit binding data"
            );
            self.misses += 1;
        } else {
            let hit = &line["hit"];
            validate_finite_hit(hit)?;
            provenance(hit)?;
            ensure!(
                number(line, "maximum_parameter")? == number(hit, "parameter")?,
                "line maximum differs from final hit"
            );
            finite_array::<3>(&line["parameter_point"])?;
            winning_root(result)?;
            if any {
                ensure!(
                    line["world_normal"].is_null(),
                    "any-hit line unexpectedly finalized world normal"
                );
            } else {
                let normal = finite_array::<4>(&line["world_normal"])?;
                let plane = finite_array::<4>(&hit["local_plane"])?;
                ensure!(
                    (normal[3] as f32).to_bits() == (plane[3] as f32).to_bits(),
                    "line world normal changed local plane W"
                );
            }
            self.final_hits_validated += 1;
            self.hits += 1;
        }
        self.queries += 1;
        if any {
            self.any_hit_queries += 1;
        } else {
            self.nearest_queries += 1;
        }
        Ok(())
    }
    fn observe_coarse(&mut self, value: &Value) -> Result<()> {
        if value.is_null() {
            return Ok(());
        }
        self.coarse_queries += 1;
        self.coarse_node_reads += count(value, "node_reads")?;
        self.coarse_sphere_tests += count(value, "sphere_tests")?;
        self.coarse_leaf_tests += count(value, "leaf_tests")?;
        let fanout = count(value, "maximum_fanout")?;
        self.coarse_maximum_fanout = Some(
            self.coarse_maximum_fanout
                .map_or(fanout, |old| old.max(fanout)),
        );
        Ok(())
    }
    fn plane_diagnostic(
        &mut self,
        result: &Value,
        meshes: &BTreeMap<u32, CollisionMesh>,
    ) -> Result<()> {
        let hit = &result["line"]["hit"];
        if hit.is_null() {
            return Ok(());
        }
        let key = u32::try_from(count(hit, "geometry_key")?)?;
        let mesh = meshes
            .get(&key)
            .context("line diagnostic mesh not source-validated")?;
        let index = validate_hit(mesh, hit)?;
        let vertices = mesh
            .triangle_vertices(index)?
            .context("winning line triangle disabled")?;
        let root = winning_root(result)?;
        let origin = finite_array::<4>(&root["working_origin"])?;
        let delta = finite_array::<4>(&root["working_delta"])?;
        let parameter = number(hit, "parameter")?;
        let point = std::array::from_fn(|i| origin[i] + delta[i] * parameter);
        let diagnostic = plane_distance(vertices, point)?;
        match diagnostic {
            PlaneDistance::Degenerate => self.f64_degenerate_triangles += 1,
            PlaneDistance::Short => self.f64_short_normal_triangles += 1,
            PlaneDistance::Distance(distance) => {
                self.f64_plane_checks += 1;
                self.maximum_f64_plane_distance = Some(
                    self.maximum_f64_plane_distance
                        .map_or(distance, |old| old.max(distance)),
                );
            }
        }
        Ok(())
    }
}
fn snapshot(origin: [f32; 3], delta: [f32; 3], any: bool) -> crate::world_line::LineSnapshot {
    crate::world_line::LineSnapshot {
        origin,
        delta,
        constructor_delta: None,
        maximum_parameter: 1.,
        filter_slot: 0,
        filter_flags: 0,
        required_any: 0,
        excluded_any: 0,
        stop_first_raw: u8::from(any),
    }
}
fn compare_modes(nearest: &Value, any: &Value) -> Result<()> {
    let near = &nearest["line"]["hit"];
    let first = &any["line"]["hit"];
    ensure!(
        near.is_null() == first.is_null(),
        "nearest/any-hit line existence differs"
    );
    if !near.is_null() {
        ensure!(
            number(near, "parameter")? <= number(first, "parameter")?,
            "nearest line parameter exceeds first-hit parameter"
        );
    }
    Ok(())
}
fn winning_root(result: &Value) -> Result<&Value> {
    let line = &result["line"];
    let key = count(&line["binding"], "root_key")?;
    for root in result["roots"]
        .as_array()
        .context("line winning root list missing")?
        .iter()
        .rev()
    {
        if count(root, "root_key")? == key
            && root["meshes"]
                .as_array()
                .context("line winning mesh list missing")?
                .iter()
                .any(|m| m["mesh"]["hit"] == line["hit"])
        {
            return Ok(root);
        }
    }
    anyhow::bail!("line final binding has no matching accepted root mesh")
}
fn load<'a>(
    edb: &Edb,
    meshes: &'a mut BTreeMap<u32, CollisionMesh>,
    hit: &Value,
) -> Result<&'a CollisionMesh> {
    let key = u32::try_from(count(hit, "geometry_key")?)?;
    if let std::collections::btree_map::Entry::Vacant(entry) = meshes.entry(key) {
        entry.insert(edb.collision_mesh_at_address(key as usize)?);
    }
    meshes.get(&key).context("line provenance mesh missing")
}
fn validate_finite_hit(hit: &Value) -> Result<()> {
    let parameter = number(hit, "parameter")?;
    ensure!(
        (0. ..=1.).contains(&parameter),
        "line audit hit outside inclusive segment"
    );
    finite_array::<4>(&hit["local_plane"])?;
    Ok(())
}
fn validate_hit(mesh: &CollisionMesh, hit: &Value) -> Result<usize> {
    ensure!(
        mesh.address == count(hit, "geometry_key")?,
        "line provenance geometry address differs"
    );
    let corner = count(hit, "first_corner_key")?;
    let flag = count(hit, "flag_index")?;
    let batch_index = count(hit, "batch_index")?;
    let ordinal = count(hit, "triangle_in_strip")?;
    let flags = count(hit, "triangle_flags_raw")?;
    let material = count(hit, "material_raw")?;
    for (index, triangle) in mesh.triangles.iter().enumerate() {
        if triangle.first_corner_offset != corner
            || triangle.flag_index != flag
            || triangle.batch_index != batch_index
            || triangle.triangle_in_strip != ordinal
            || usize::from(triangle.flags_raw) != flags
        {
            continue;
        }
        let batch = mesh
            .batches
            .get(triangle.batch_index)
            .context("line provenance batch missing")?;
        if triangle.enabled()
            && batch.included()
            && usize::from(batch.material_raw) == material
            && index >= batch.first_triangle
            && index - batch.first_triangle < batch.triangle_count
        {
            return Ok(index);
        }
    }
    anyhow::bail!("line hit provenance differs from enabled source triangle/material/ordinals")
}
enum PlaneDistance {
    Degenerate,
    Short,
    Distance(f64),
}
fn plane_distance(vertices: [[f32; 3]; 3], point: [f64; 3]) -> Result<PlaneDistance> {
    let vertices = vertices.map(|v| v.map(f64::from));
    let a = std::array::from_fn::<_, 3, _>(|i| vertices[1][i] - vertices[0][i]);
    let b = std::array::from_fn::<_, 3, _>(|i| vertices[2][i] - vertices[0][i]);
    let normal = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    let squared = normal.iter().map(|v| v * v).sum::<f64>();
    ensure!(
        squared.is_finite(),
        "nonfinite independent line plane geometry"
    );
    if squared == 0. {
        return Ok(PlaneDistance::Degenerate);
    }
    if squared <= f64::from(f32::EPSILON) {
        return Ok(PlaneDistance::Short);
    }
    let numerator = (0..3)
        .map(|i| normal[i] * (point[i] - vertices[0][i]))
        .sum::<f64>();
    let distance = numerator.abs() / squared.sqrt();
    ensure!(
        distance.is_finite(),
        "nonfinite independent line plane distance"
    );
    Ok(PlaneDistance::Distance(distance))
}
fn count(value: &Value, key: &str) -> Result<usize> {
    usize::try_from(
        value[key]
            .as_u64()
            .with_context(|| format!("line audit count {key} missing"))?,
    )
    .context("line audit count exceeds host")
}
fn boolean(value: &Value, key: &str) -> Result<bool> {
    value[key]
        .as_bool()
        .with_context(|| format!("line audit bool {key} missing"))
}
fn number(value: &Value, key: &str) -> Result<f64> {
    let number = value[key]
        .as_f64()
        .with_context(|| format!("line audit number {key} missing"))?;
    ensure!(number.is_finite(), "nonfinite line audit number");
    Ok(number)
}
fn finite_array<const N: usize>(value: &Value) -> Result<[f64; N]> {
    let values = value
        .as_array()
        .context("line audit numeric array missing")?;
    ensure!(values.len() == N, "line audit array length differs");
    let mut result = [0.; N];
    for (out, value) in result.iter_mut().zip(values) {
        *out = value
            .as_f64()
            .context("line audit array component missing")?;
        ensure!(out.is_finite(), "nonfinite line audit array");
    }
    Ok(result)
}

#[cfg(test)]
#[path = "tests/world_line_audit.rs"]
mod tests;
