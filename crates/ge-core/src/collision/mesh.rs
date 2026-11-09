//! Static collected-mesh type-three traversal; live positions are not inferred.
use super::{
    OrientedBox,
    batch::{self, BatchDescriptor, QueryCube, StaticBatchCache, TriangleDescriptor},
    collector::CollectedRecord,
    frame::CollisionFrame,
    result_binding::{self, BoundContact, ContactProvenance},
};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

pub struct MeshTriangle {
    pub descriptor: TriangleDescriptor,
    pub first_corner_key: u32,
}
/// Resident static coordinates remain stable for the duration of a query.
pub trait StaticMeshSource {
    fn triangle(&self, index: usize) -> Result<MeshTriangle>;
    fn coordinate(&self, position: u16, axis: usize) -> Result<f32>;
}
pub struct MeshRequest<'a> {
    pub world_origin: [f32; 3],
    pub radius: f32,
    pub world_from_query: &'a [f32; 16],
    pub query_from_world: [f32; 16],
    pub boxes: &'a [OrientedBox],
    pub triangle_budget: usize,
    pub primitive_budget: usize,
}
impl<'a> MeshRequest<'a> {
    pub fn new(
        world_origin: [f32; 3],
        radius: f32,
        world_from_query: &'a [f32; 16],
        boxes: &'a [OrientedBox],
    ) -> Result<Self> {
        // The request-frame helper is always rigid, independently of each
        // collected record's general-inverse flag.
        let query_from_world = CollisionFrame::from_record(world_from_query, 0)?.local_from_world;
        Ok(Self {
            world_origin,
            radius,
            world_from_query,
            query_from_world,
            boxes,
            triangle_budget: 4_000_000,
            primitive_budget: 1_000_000,
        })
    }
}
pub struct RecordQuery<'a> {
    pub record: &'a CollectedRecord,
    pub cached_frame: &'a CollisionFrame,
    pub collected_record_key: u32,
}
pub struct ContactBuffer {
    capacity: u32,
    contacts: Vec<BoundContact>,
}
impl ContactBuffer {
    pub fn new(capacity: usize) -> Result<Self> {
        ensure!(
            capacity <= 1_000_000,
            "contact capacity exceeds host budget"
        );
        Ok(Self {
            capacity: capacity as u32,
            contacts: Vec::new(),
        })
    }
    pub fn contacts(&self) -> &[BoundContact] {
        &self.contacts
    }
    pub fn capacity(&self) -> u32 {
        self.capacity
    }
}
#[derive(Default, Debug, Serialize)]
pub struct MeshStats {
    pub coarse_tests: usize,
    pub coarse_rejections: usize,
    pub capacity_mesh_skips: usize,
    pub included_batches: usize,
    pub batch_rejections: usize,
    pub flag_words_consumed: usize,
    pub triangle_tests: usize,
    pub triangle_rejections: usize,
    pub primitive_queries: usize,
    pub primitive_contacts: usize,
    pub appended: usize,
    pub post_hit_full: usize,
}
fn concat(left: &[f32; 16], right: &[f32; 16]) -> Result<[f32; 16]> {
    ensure!(
        left.iter().chain(right).all(|v| v.is_finite()),
        "nonfinite mesh frame input"
    );
    let matrix = std::array::from_fn(|i| {
        let row = i % 4;
        let col = i / 4;
        let x = left[row] * right[col * 4];
        let y = left[4 + row].mul_add(right[col * 4 + 1], x);
        let z = left[8 + row].mul_add(right[col * 4 + 2], y);
        left[12 + row].mul_add(right[col * 4 + 3], z)
    });
    ensure!(
        matrix.iter().all(|v| v.is_finite()),
        "mesh frame concatenation overflow"
    );
    Ok(matrix)
}
fn point(matrix: &[f32; 16], point: [f32; 3]) -> Result<[f32; 3]> {
    ensure!(
        point.into_iter().all(f32::is_finite),
        "nonfinite mesh triangle point"
    );
    let result = std::array::from_fn(|row| {
        let x = matrix[row] * point[0];
        let y = matrix[4 + row].mul_add(point[1], x);
        let z = matrix[8 + row].mul_add(point[2], y);
        matrix[12 + row].mul_add(1., z)
    });
    ensure!(
        result.iter().all(|v| v.is_finite()),
        "mesh point transform overflow"
    );
    Ok(result)
}
/// Cache preparation must precede this call. `coarse` lazily returns None for
/// absence, never false for unavailable data. Flag 0x2 bypasses it entirely.
/// Shapes stay in the request frame while passing triangles are transformed.
pub fn query_record(
    source: &impl StaticMeshSource,
    batches: &[BatchDescriptor],
    cache: Option<&StaticBatchCache>,
    record: RecordQuery<'_>,
    request: &MeshRequest<'_>,
    contacts: &mut ContactBuffer,
    coarse: impl FnOnce([f32; 4], f32) -> Result<Option<bool>>,
) -> Result<MeshStats> {
    ensure!(
        record.record.optional_context_key == 0,
        "static mesh query needs a resolved optional position context"
    );
    ensure!(
        request.triangle_budget > 0
            && request.triangle_budget <= 4_000_000
            && request.primitive_budget > 0
            && request.primitive_budget <= 1_000_000
            && batches.len() <= 65535
            && request.boxes.len() <= 65536,
        "invalid mesh traversal host budgets"
    );
    let mut stats = MeshStats::default();
    let matrix = &record.record.matrix;
    let local_origin = record
        .cached_frame
        .query_origin(request.world_origin, [matrix[12], matrix[13], matrix[14]])?;
    if record.record.flags_raw & 2 == 0
        && let Some(overlaps) = coarse(local_origin, request.radius)?
    {
        stats.coarse_tests += 1;
        if !overlaps {
            stats.coarse_rejections += 1;
            return Ok(stats);
        }
    }
    let query_from_mesh = concat(&request.query_from_world, matrix)?;
    // Native cube arithmetic occurs BEFORE the reserved-slot mesh gate.
    let cube = QueryCube::new(local_origin[..3].try_into().unwrap(), request.radius)?;
    if !result_binding::can_begin_mesh(contacts.contacts.len() as u32, contacts.capacity) {
        stats.capacity_mesh_skips += 1;
        return Ok(stats);
    }
    let mut cache_slot = 0;
    for batch in batches {
        if !batch.included() {
            continue;
        }
        stats.included_batches += 1;
        let passes = if let Some(cache) = cache {
            let bound = cache
                .bounds
                .get(cache_slot)
                .context("included batch cache slot missing")?;
            batch::cache_passes(bound, &cube)?
        } else {
            true
        };
        cache_slot += 1;
        if !passes {
            stats.batch_rejections += 1;
            stats.flag_words_consumed += batch::skipped_flag_words(batch);
            continue;
        }
        ensure!(
            batch.triangle_count == usize::from(batch.declared_triangles),
            "addressed mesh triangle count differs from declared count"
        );
        for index in batch.first_triangle
            ..batch
                .first_triangle
                .checked_add(batch.triangle_count)
                .context("mesh triangle range overflow")?
        {
            ensure!(
                stats.triangle_tests < request.triangle_budget,
                "mesh triangle-test budget exhausted"
            );
            stats.triangle_tests += 1;
            let triangle = source.triangle(index)?;
            stats.flag_words_consumed += 1;
            if !batch::triangle_passes(&triangle.descriptor, &cube, |position, axis| {
                source.coordinate(position, axis)
            })? {
                stats.triangle_rejections += 1;
                continue;
            }
            let mut vertices = [[0.; 3]; 3];
            for (vertex, &position) in vertices
                .iter_mut()
                .zip(&triangle.descriptor.position_indices)
            {
                let local = [
                    source.coordinate(position, 0)?,
                    source.coordinate(position, 1)?,
                    source.coordinate(position, 2)?,
                ];
                *vertex = point(&query_from_mesh, local)?;
            }
            for (shape_index, shape) in request.boxes.iter().enumerate() {
                ensure!(
                    stats.primitive_queries < request.primitive_budget,
                    "mesh primitive-query budget exhausted"
                );
                stats.primitive_queries += 1;
                let result = shape.query_triangle(vertices)?;
                let Some(contact) = result.contact else {
                    continue;
                };
                stats.primitive_contacts += 1;
                if !result_binding::can_append(contacts.contacts.len() as u32, contacts.capacity) {
                    stats.post_hit_full += 1;
                    return Ok(stats);
                }
                contacts.contacts.push(result_binding::bind_type_three(
                    &contact,
                    vertices,
                    request.world_from_query,
                    ContactProvenance {
                        collected_record_key: record.collected_record_key,
                        first_corner_key: triangle.first_corner_key,
                        position_indices: triangle.descriptor.position_indices,
                        triangle_flags_raw: triangle.descriptor.flags_raw,
                        material_raw: batch.material_raw,
                        query_shape_index: shape_index as u32,
                        triangle_in_strip: u32::try_from(triangle.descriptor.triangle_in_strip)
                            .context("mesh strip ordinal overflow")?,
                    },
                )?);
                stats.appended += 1;
            }
        }
    }
    Ok(stats)
}

#[cfg(test)]
#[path = "../tests/collision_mesh.rs"]
mod tests;
