//! Resident static mesh line queries. World membership and result frames belong
//! to the caller; optional dynamic position contexts are explicitly unsupported.
use anyhow::{Context, Result, ensure};
use ge_assets::{collision::CollisionMesh, edb::Edb};
use ge_core::collision::{
    batch::{
        self, BatchDescriptor, CacheLimits, CachePreparationStats, StaticBatchCache,
        TriangleDescriptor,
    },
    line_bounds,
    line_triangle::{self, LineTriangleQuery},
    world::StoredBounds,
};
use serde::Serialize;
use std::collections::BTreeMap;

pub struct MeshQuery<'a> {
    pub origin: [f32; 3],
    pub delta: [f32; 3],
    pub local_bounds: StoredBounds,
    pub maximum_parameter: &'a mut f32,
    pub required_any: u32,
    pub excluded_any: u32,
    pub optional_context_key: u32,
    pub dispatch_flags: u32,
    pub stop_first: bool,
}
#[derive(Debug, Serialize)]
pub struct MeshHit {
    pub parameter: f32,
    pub local_plane: [f32; 4],
    pub triangle_flags_raw: u16,
    pub material_raw: u16,
    pub geometry_key: u32,
    pub first_corner_key: u32,
    pub flag_index: usize,
    pub batch_index: usize,
    pub triangle_in_strip: usize,
}
#[derive(Debug, Serialize)]
pub struct MeshProbe {
    pub geometry_key: u32,
    pub cache_preparation: CachePreparationStats,
    pub planes_generated: bool,
    pub planes_count: usize,
    pub bounds_tested: bool,
    pub bounds_rejected: bool,
    pub coarse_tested: bool,
    pub coarse_rejected: bool,
    pub batches_tested: usize,
    pub batch_bounds_tests: usize,
    pub batches_rejected: usize,
    pub triangles_tested: usize,
    pub triangles_rejected: usize,
    pub disabled_triangles: usize,
    /// Included-stream cursor advances, including skipped declared batch counts.
    pub flag_words_advanced: usize,
    pub plane_records_advanced: usize,
    pub skipped_flag_words: usize,
    pub skipped_plane_records: usize,
    pub hit: Option<MeshHit>,
}
struct Resident {
    mesh: CollisionMesh,
    batches: Vec<BatchDescriptor>,
    format: u32,
    cache: Option<StaticBatchCache>,
    planes: Option<Vec<[f32; 4]>>,
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
            planes: None,
        }
    }
    fn prepare(&mut self) -> Result<CachePreparationStats> {
        let mesh = &self.mesh;
        batch::prepare_line_cache(
            &mut self.format,
            &mut self.cache,
            &self.batches,
            |index| {
                let t = mesh
                    .triangles
                    .get(index)
                    .context("line cache triangle outside mesh")?;
                Ok(TriangleDescriptor {
                    flags_raw: t.flags_raw,
                    position_indices: t.position_indices,
                    strip_in_batch: t.strip_in_batch,
                    triangle_in_strip: t.triangle_in_strip,
                })
            },
            |position, axis| {
                mesh.positions
                    .get(usize::from(position))
                    .and_then(Option::as_ref)
                    .and_then(|p| p.get(axis))
                    .copied()
                    .context("line cache position was not decoded")
            },
            CacheLimits::default(),
        )
    }
    fn prepare_planes(&mut self, load: impl FnOnce() -> Result<CollisionMesh>) -> Result<bool> {
        if self.planes.is_some() {
            return Ok(false);
        }
        // This second view intentionally includes disabled triangles' XYZ only
        // after mesh bounds/coarse gates. Do not use triangle_vertices here.
        let mesh = load()?;
        ensure!(
            mesh.address == self.mesh.address,
            "line plane decoder returned another mesh"
        );
        let mut planes = Vec::new();
        planes
            .try_reserve(mesh.triangles.len())
            .context("allocating resident line planes")?;
        for triangle in &mesh.triangles {
            let points = vertices(&mesh, triangle.position_indices)?;
            planes.push(line_triangle::prepare_line_plane(
                points,
                triangle.triangle_in_strip,
            )?);
        }
        self.mesh = mesh;
        self.planes = Some(planes);
        Ok(true)
    }
}
fn vertices(mesh: &CollisionMesh, indices: [u16; 3]) -> Result<[[f32; 3]; 3]> {
    let mut result = [[0.; 3]; 3];
    for (point, index) in result.iter_mut().zip(indices) {
        *point = mesh
            .positions
            .get(usize::from(index))
            .copied()
            .flatten()
            .context("line triangle position was not decoded")?;
    }
    Ok(result)
}

#[derive(Default)]
pub struct Meshes {
    residents: BTreeMap<u32, Resident>,
}
impl Meshes {
    pub fn resident_count(&self) -> usize {
        self.residents.len()
    }
    pub fn query(
        &mut self,
        edb: &Edb,
        node: u32,
        request: &mut MeshQuery<'_>,
        coarse: impl FnOnce() -> Result<Option<bool>>,
    ) -> Result<MeshProbe> {
        ensure!(
            request.optional_context_key == 0,
            "static line mesh cannot infer optional dynamic position context"
        );
        if let std::collections::btree_map::Entry::Vacant(entry) = self.residents.entry(node) {
            entry.insert(Resident::new(edb.collision_mesh_at_address(node as usize)?));
        }
        let resident = self
            .residents
            .get_mut(&node)
            .context("line resident insertion failed")?;
        // Native virtual wrapper prepares mode-two bounds before virtual mesh gates.
        let preparation = resident.prepare()?;
        let mut probe = MeshProbe {
            geometry_key: node,
            cache_preparation: preparation,
            planes_generated: false,
            planes_count: resident.planes.as_ref().map_or(0, Vec::len),
            bounds_tested: false,
            bounds_rejected: false,
            coarse_tested: false,
            coarse_rejected: false,
            batches_tested: 0,
            batch_bounds_tests: 0,
            batches_rejected: 0,
            triangles_tested: 0,
            triangles_rejected: 0,
            disabled_triangles: 0,
            flag_words_advanced: 0,
            plane_records_advanced: 0,
            skipped_flag_words: 0,
            skipped_plane_records: 0,
            hit: None,
        };
        if request.dispatch_flags & 1 == 0 {
            probe.bounds_tested = true;
            let bound = edb.geometry().bounds(node as usize)?;
            if !line_bounds::overlap(
                StoredBounds {
                    minimum: bound.minimum,
                    maximum: bound.maximum,
                },
                request.local_bounds,
            )? {
                probe.bounds_rejected = true;
                return Ok(probe);
            }
            match coarse()? {
                None => {}
                Some(passes) => {
                    probe.coarse_tested = true;
                    if !passes {
                        probe.coarse_rejected = true;
                        return Ok(probe);
                    }
                }
            }
        }
        probe.planes_generated =
            resident.prepare_planes(|| edb.collision_line_mesh_at_address(node as usize))?;
        probe.planes_count = resident.planes.as_ref().map_or(0, Vec::len);
        if probe.planes_count == 0 {
            return Ok(probe);
        }
        query_triangles(resident, node, request, &mut probe)?;
        Ok(probe)
    }
}

fn query_triangles(
    resident: &Resident,
    node: u32,
    request: &mut MeshQuery<'_>,
    probe: &mut MeshProbe,
) -> Result<()> {
    let planes = resident
        .planes
        .as_ref()
        .context("line planes were not prepared")?;
    let mut compact = 0;
    let mut plane_cursor = 0;
    let mut flag_cursor = 0;
    for (batch_index, batch) in resident.mesh.batches.iter().enumerate() {
        if !batch.included() {
            continue;
        }
        probe.batches_tested += 1;
        if let Some(cache) = &resident.cache {
            let bounds = cache
                .bounds
                .get(compact)
                .context("line compact batch cache missing slot")?;
            compact += 1;
            probe.batch_bounds_tests += 1;
            ensure!(
                bounds.minimum[0].is_finite(),
                "nonfinite line cache minimum X"
            );
            let passes = bounds.minimum[0] != 999999.
                && line_bounds::segment_overlaps(
                    StoredBounds {
                        minimum: bounds.minimum,
                        maximum: bounds.maximum,
                    },
                    request.origin,
                    request.delta,
                )?;
            if !passes {
                probe.batches_rejected += 1;
                let count = usize::from(batch.declared_triangles);
                flag_cursor += count;
                plane_cursor += count;
                probe.flag_words_advanced += count;
                probe.plane_records_advanced += count;
                probe.skipped_flag_words += count;
                probe.skipped_plane_records += count;
                continue;
            }
        }
        for index in batch.first_triangle..batch.first_triangle + batch.triangle_count {
            let triangle = resident
                .mesh
                .triangles
                .get(index)
                .context("line triangle outside included batch")?;
            ensure!(
                triangle.flag_index == flag_cursor && triangle.batch_index == batch_index,
                "line included stream cursor mismatch"
            );
            let plane = *planes
                .get(plane_cursor)
                .context("line plane cursor outside resident stream")?;
            probe.triangles_tested += 1;
            if !triangle.enabled() {
                probe.disabled_triangles += 1;
            }
            let query = LineTriangleQuery {
                origin: request.origin,
                delta: request.delta,
                maximum_parameter: *request.maximum_parameter,
                required_any: request.required_any,
                excluded_any: request.excluded_any,
            };
            let hit = line_triangle::intersect_line_triangle(
                &query,
                plane,
                triangle.flags_raw,
                triangle.triangle_in_strip,
                || vertices(&resident.mesh, triangle.position_indices),
            )?;
            if let Some(hit) = hit {
                *request.maximum_parameter = hit.parameter;
                probe.hit = Some(MeshHit {
                    parameter: hit.parameter,
                    local_plane: hit.local_plane,
                    triangle_flags_raw: triangle.flags_raw,
                    material_raw: batch.material_raw,
                    geometry_key: node,
                    first_corner_key: u32::try_from(triangle.first_corner_offset)
                        .context("line corner key exceeds u32")?,
                    flag_index: triangle.flag_index,
                    batch_index,
                    triangle_in_strip: triangle.triangle_in_strip,
                });
                if request.stop_first {
                    return Ok(());
                }
            } else {
                probe.triangles_rejected += 1;
            }
            flag_cursor += 1;
            plane_cursor += 1;
            probe.flag_words_advanced += 1;
            probe.plane_records_advanced += 1;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests/line_mesh.rs"]
mod tests;
