//! Wii 0x607 navigation mesh data. Original traversal and gameplay behavior remain unverified.
use crate::{binary::Bytes, edb::Entity};
use anyhow::{Result, ensure};
use serde::Serialize;

const NO_NEIGHBOR: u32 = 0x000f_ffff;
const MAX_ELEMENTS: usize = 1_000_000;

#[derive(Debug, Serialize)]
pub struct Triangle {
    pub vertices: [u32; 3],
    pub vertex_high_bits_raw: [u16; 3],
    pub attribute_raw: u32,
    pub neighbors: [Option<u32>; 3],
    pub neighbor_high_bits_raw: [u16; 3],
    pub center: Option<[f32; 3]>,
    /// Optional stored face normal at +0x8c; do not synthesize or normalize absent data.
    pub normal: Option<[f32; 3]>,
    /// Boundary weights are preserved too; they are not usable graph edges.
    pub weights: Option<[f32; 3]>,
    /// Optional per-face metadata; bit zero rejects an interior shortcut test.
    pub flags_raw: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct Group {
    pub tag_raw: u8,
    pub triangle_count: u32,
    pub first_triangle: u32,
    pub start_high_bits_raw: u16,
}

#[derive(Debug, Serialize)]
pub struct TriangleGraph {
    pub entity: Entity,
    /// Stored local AABB (minimum, maximum), used by navmesh acquisition.
    pub bounds: [[f32; 3]; 2],
    /// Preserves unknown header fields, including relative pointers and sentinels.
    pub header_words: Vec<u32>,
    pub vertices: Vec<[f32; 3]>,
    pub triangles: Vec<Triangle>,
    pub groups: Vec<Group>,
    pub diagnostics: Diagnostics,
}

#[derive(Debug, Default, Serialize)]
pub struct Diagnostics {
    pub directed_links: usize,
    pub self_links: usize,
    pub neighbors_after_boundary: usize,
    pub boundary_slots: usize,
    pub non_reciprocal_links: usize,
    pub links_without_shared_edge: usize,
    pub degenerate_triangles: usize,
    pub connected_components: usize,
    pub centroid_samples: usize,
    pub distance_weight_samples: usize,
    pub flag_samples: usize,
    pub shortcut_blocked_faces: usize,
    pub normal_samples: usize,
    pub low_actor_normal_faces: usize,
    pub max_centroid_error: f64,
    pub max_distance_weight_error: f64,
}

pub(crate) fn read(data: &[u8], entity: Entity) -> Result<TriangleGraph> {
    ensure!(
        entity.entity_type == 0x607,
        "entity is not a 0x607 triangle graph"
    );
    let b = Bytes(data);
    let a = entity.address as usize;
    b.range(a, 0xc0)?;
    let bounds = [vector(b, a + 0x10)?, vector(b, a + 0x20)?];
    ensure!(
        (0..3).all(|i| bounds[0][i] <= bounds[1][i]),
        "inverted triangle graph bounds"
    );
    let vertex_count = b.u32(a + 0x6c)? as usize;
    let triangle_count = b.u32(a + 0x70)? as usize;
    let group_count = b.u32(a + 0x74)? as usize;
    ensure!(
        (1..=MAX_ELEMENTS).contains(&vertex_count)
            && (1..=MAX_ELEMENTS).contains(&triangle_count)
            && (1..=triangle_count).contains(&group_count),
        "invalid triangle graph counts"
    );
    let vertices_at = b.relative(a + 0x78)?;
    let triangles_at = b.relative(a + 0x7c)?;
    let neighbors_at = b.relative(a + 0x80)?;
    let groups_at = b.relative(a + 0x84)?;
    let centers_at = optional_pointer(b, a + 0x88)?;
    let normals_at = optional_pointer(b, a + 0x8c)?;
    let weights_at = optional_pointer(b, a + 0x90)?;
    let flags_at = optional_pointer(b, a + 0x94)?;
    for (start, count, stride) in [
        (Some(vertices_at), vertex_count, 12),
        (Some(triangles_at), triangle_count, 16),
        (Some(neighbors_at), triangle_count, 12),
        (Some(groups_at), group_count, 8),
        (centers_at, triangle_count, 12),
        (normals_at, triangle_count, 12),
        (weights_at, triangle_count, 12),
        (flags_at, triangle_count, 4),
    ] {
        let Some(start) = start else {
            continue;
        };
        b.range(start, count * stride)?;
        ensure!(
            start >= a + 0xc0,
            "triangle graph array overlaps its header"
        );
    }
    let vertices = (0..vertex_count)
        .map(|i| vector(b, vertices_at + i * 12))
        .collect::<Result<Vec<_>>>()?;
    let mut groups = Vec::with_capacity(group_count);
    let mut next_triangle = 0u64;
    for i in 0..group_count {
        let packed = b.u32(groups_at + i * 8)?;
        let count = packed & 0x00ff_ffff;
        let start_raw = b.u32(groups_at + i * 8 + 4)?;
        let first = start_raw & 0x000f_ffff;
        ensure!(
            count > 0 && u64::from(first) == next_triangle,
            "triangle graph group ranges are not contiguous"
        );
        next_triangle += u64::from(count);
        groups.push(Group {
            tag_raw: (packed >> 24) as u8,
            triangle_count: count,
            first_triangle: first,
            start_high_bits_raw: (start_raw >> 20) as u16,
        });
    }
    ensure!(
        next_triangle == triangle_count as u64,
        "triangle graph group counts do not partition triangles"
    );
    let mut triangles = Vec::with_capacity(triangle_count);
    for i in 0..triangle_count {
        let raw_indices = words(b, triangles_at + i * 16)?;
        let indices = raw_indices.map(|v| v & 0x000f_ffff);
        ensure!(
            indices.iter().all(|&index| (index as usize) < vertex_count),
            "triangle graph vertex index out of bounds"
        );
        let raw_neighbors = words(b, neighbors_at + i * 12)?;
        let neighbors = raw_neighbors.map(|n| n & 0x000f_ffff);
        ensure!(
            neighbors
                .iter()
                .all(|&index| index == NO_NEIGHBOR || (index as usize) < triangle_count),
            "triangle graph neighbor index out of bounds"
        );
        let weights = weights_at.map(|a| vector(b, a + i * 12)).transpose()?;
        ensure!(
            weights.is_none_or(|row| row.iter().all(|&v| v >= 0.)),
            "negative triangle graph weight"
        );
        triangles.push(Triangle {
            vertices: indices,
            vertex_high_bits_raw: raw_indices.map(|v| (v >> 20) as u16),
            attribute_raw: b.u32(triangles_at + i * 16 + 12)?,
            neighbors: neighbors.map(|n| (n != NO_NEIGHBOR).then_some(n)),
            neighbor_high_bits_raw: raw_neighbors.map(|n| (n >> 20) as u16),
            center: centers_at.map(|a| vector(b, a + i * 12)).transpose()?,
            normal: normals_at.map(|a| vector(b, a + i * 12)).transpose()?,
            weights,
            flags_raw: flags_at.map(|a| b.u32(a + i * 4)).transpose()?,
        });
    }
    let diagnostics = diagnose(&vertices, &triangles);
    Ok(TriangleGraph {
        entity,
        bounds,
        header_words: (0..48).map(|i| b.u32(a + i * 4)).collect::<Result<_>>()?,
        vertices,
        triangles,
        groups,
        diagnostics,
    })
}

fn words(b: Bytes<'_>, a: usize) -> Result<[u32; 3]> {
    Ok([b.u32(a)?, b.u32(a + 4)?, b.u32(a + 8)?])
}
fn vector(b: Bytes<'_>, a: usize) -> Result<[f32; 3]> {
    Ok([b.f32(a)?, b.f32(a + 4)?, b.f32(a + 8)?])
}

fn optional_pointer(b: Bytes<'_>, a: usize) -> Result<Option<usize>> {
    if b.u32(a)? == 0 {
        Ok(None)
    } else {
        Ok(Some(b.relative(a)?))
    }
}

fn diagnose(vertices: &[[f32; 3]], triangles: &[Triangle]) -> Diagnostics {
    let mut d = Diagnostics::default();
    let mut parents = (0..triangles.len()).collect::<Vec<_>>();
    for (i, t) in triangles.iter().enumerate() {
        if let Some(normal) = t.normal {
            d.normal_samples += 1;
            d.low_actor_normal_faces += usize::from(normal[1] < 0.9);
        }
        if let Some(flags) = t.flags_raw {
            d.flag_samples += 1;
            d.shortcut_blocked_faces += usize::from(flags & 1 != 0);
        }
        let points = t.vertices.map(|v| vertices[v as usize].map(f64::from));
        let ab: [f64; 3] = std::array::from_fn(|j| points[1][j] - points[0][j]);
        let ac: [f64; 3] = std::array::from_fn(|j| points[2][j] - points[0][j]);
        let cross = [
            ab[1] * ac[2] - ab[2] * ac[1],
            ab[2] * ac[0] - ab[0] * ac[2],
            ab[0] * ac[1] - ab[1] * ac[0],
        ];
        if cross.iter().all(|&v| v == 0.) {
            d.degenerate_triangles += 1;
        }
        if let Some(center) = t.center {
            d.centroid_samples += 1;
            for (axis, &coordinate) in center.iter().enumerate() {
                let centroid = (points[0][axis] + points[1][axis] + points[2][axis]) / 3.;
                d.max_centroid_error = d
                    .max_centroid_error
                    .max((centroid - f64::from(coordinate)).abs());
            }
        }
        let mut boundary_seen = false;
        for (slot, neighbor) in t.neighbors.iter().enumerate() {
            let Some(j) = neighbor.map(|n| n as usize) else {
                d.boundary_slots += 1;
                boundary_seen = true;
                continue;
            };
            if boundary_seen {
                d.neighbors_after_boundary += 1;
            }
            d.directed_links += 1;
            if i == j {
                d.self_links += 1;
            }
            let other = &triangles[j];
            if !other.neighbors.contains(&Some(i as u32)) {
                d.non_reciprocal_links += 1;
            }
            if t.vertices
                .iter()
                .filter(|v| other.vertices.contains(v))
                .count()
                != 2
            {
                d.links_without_shared_edge += 1;
            }
            if let (Some(center), Some(other_center), Some(weights)) =
                (t.center, other.center, t.weights)
            {
                d.distance_weight_samples += 1;
                let distance = center
                    .iter()
                    .zip(other_center)
                    .map(|(&a, b)| (f64::from(a) - f64::from(b)).powi(2))
                    .sum::<f64>()
                    .sqrt();
                d.max_distance_weight_error = d
                    .max_distance_weight_error
                    .max((distance - f64::from(weights[slot])).abs());
            }
            let left = root(&mut parents, i);
            let right = root(&mut parents, j);
            // Monotonic roots make the result deterministic independent of link order.
            parents[left.max(right)] = left.min(right);
        }
    }
    d.connected_components = (0..parents.len())
        .filter(|&i| root(&mut parents, i) == i)
        .count();
    d
}

fn root(parents: &mut [usize], mut i: usize) -> usize {
    while parents[i] != i {
        parents[i] = parents[parents[i]];
        i = parents[i];
    }
    i
}

#[cfg(test)]
#[path = "tests/graph.rs"]
mod tests;
