//! Ordered vertex-pair cache and raw XZ boundary predicate from static SJBE52 evidence.
use anyhow::{Context, Result, ensure};
use serde::Serialize;

pub struct BoundaryMesh<'a> {
    pub vertex_count: usize,
    pub faces: &'a [[usize; 3]],
    pub neighbors: &'a [[Option<usize>; 3]],
    pub flags: &'a [Option<u32>],
}
#[derive(Debug, Clone, Copy, Serialize)]
pub struct BoundaryEdge {
    pub vertices: [usize; 2],
    /// Rust diagnostic provenance; the original cache stores only the pair.
    pub source_face: usize,
}
#[derive(Debug, Default, Serialize)]
pub struct BoundaryStats {
    pub edges: usize,
    pub flagged_faces: usize,
    pub one_neighbor_faces: usize,
    pub two_neighbor_faces: usize,
    pub isolated_faces: usize,
    pub interior_faces: usize,
    pub missing_exclusions: usize,
}
pub struct BoundaryCache {
    edges: Vec<BoundaryEdge>,
    stats: BoundaryStats,
}
#[derive(Debug, Serialize)]
pub struct BoundaryHit {
    pub edge: usize,
    pub source_face: usize,
    pub vertices: [usize; 2],
    /// Collinear acceptance returns the initialized zero vector, not a geometric point.
    pub position: [f32; 3],
    pub collinear: bool,
}

/// Initial cache only: preserve face order, neighbor-slot compaction, edge orientation
/// and duplicate pairs. This is not generic extraction of all mesh boundary edges.
pub fn build_boundary_cache(mesh: BoundaryMesh<'_>) -> Result<BoundaryCache> {
    ensure!(
        mesh.faces.len() == mesh.neighbors.len() && mesh.faces.len() == mesh.flags.len(),
        "boundary table length mismatch"
    );
    ensure!(
        mesh.faces.iter().flatten().all(|&v| v < mesh.vertex_count)
            && mesh
                .neighbors
                .iter()
                .flatten()
                .all(|n| n.is_none_or(|n| n < mesh.faces.len())),
        "boundary source index out of bounds"
    );
    let mut cache = BoundaryCache {
        edges: Vec::new(),
        stats: BoundaryStats::default(),
    };
    cache.edges.try_reserve(
        mesh.faces
            .len()
            .checked_mul(3)
            .context("boundary cache capacity overflow")?,
    )?;
    for (source_face, &face) in mesh.faces.iter().enumerate() {
        let mut emit = |a: usize, b: usize| -> Result<()> {
            ensure!(
                a < mesh.vertex_count && b < mesh.vertex_count,
                "boundary construction produced an invalid vertex"
            );
            cache.edges.push(BoundaryEdge {
                vertices: [a, b],
                source_face,
            });
            Ok(())
        };
        if mesh.flags[source_face].unwrap_or(0) & 1 != 0 {
            cache.stats.flagged_faces += 1;
            for i in 0..3 {
                emit(face[i], face[(i + 1) % 3])?;
            }
            continue;
        }
        // All three slots are examined; a sentinel is skipped, not a terminating prefix.
        let neighbors = mesh.neighbors[source_face]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        match neighbors.as_slice() {
            [] => {
                cache.stats.isolated_faces += 1;
            }
            &[neighbor] => {
                cache.stats.one_neighbor_faces += 1;
                // Last current vertex missing from the neighbor, not first.
                let exclusive = face
                    .into_iter()
                    .rfind(|v| !mesh.faces[neighbor].contains(v))
                    .context("boundary one-neighbor case has no exclusive vertex")?;
                let others = face
                    .into_iter()
                    .filter(|&v| v != exclusive)
                    .collect::<Vec<_>>();
                ensure!(
                    others.len() == 2,
                    "boundary one-neighbor case lacks two initialized other vertices"
                );
                emit(exclusive, others[0])?;
                emit(exclusive, others[1])?;
            }
            &[a, b] => {
                cache.stats.two_neighbor_faces += 1;
                let common = mesh.faces[a]
                    .into_iter()
                    .filter(|v| mesh.faces[b].contains(v))
                    .collect::<Vec<_>>();
                let excluded = if common.len() == 1 {
                    common.first().copied()
                } else {
                    face.into_iter()
                        .find(|v| common.iter().take(2).any(|c| c == v))
                };
                if excluded.is_none() {
                    cache.stats.missing_exclusions += 1;
                }
                let others = face
                    .into_iter()
                    .filter(|v| Some(*v) != excluded)
                    .collect::<Vec<_>>();
                ensure!(
                    others.len() >= 2,
                    "boundary two-neighbor case lacks initialized endpoints"
                );
                // With no exclusion the third vertex overwrites endpoint one in the source.
                emit(others[0], *others.last().unwrap())?;
            }
            _ => {
                cache.stats.interior_faces += 1;
            }
        }
    }
    cache.stats.edges = cache.edges.len();
    Ok(cache)
}

impl BoundaryCache {
    pub fn edges(&self) -> &[BoundaryEdge] {
        &self.edges
    }
    pub fn stats(&self) -> &BoundaryStats {
        &self.stats
    }

    /// Return the first blocking edge. Inputs are used UNCHANGED against local
    /// mesh vertices, as in the observed wrapper; no inferred translation or Y test.
    pub fn first_blocker(
        &self,
        vertices: &[[f32; 3]],
        start: [f32; 3],
        end: [f32; 3],
    ) -> Result<Option<BoundaryHit>> {
        ensure!(
            start.into_iter().chain(end).all(f32::is_finite),
            "nonfinite boundary segment"
        );
        for (index, edge) in self.edges.iter().enumerate() {
            let a = *vertices
                .get(edge.vertices[0])
                .context("boundary vertex out of bounds")?;
            let b = *vertices
                .get(edge.vertices[1])
                .context("boundary vertex out of bounds")?;
            if let Some((position, collinear)) = intersection(start, end, a, b)? {
                return Ok(Some(BoundaryHit {
                    edge: index,
                    source_face: edge.source_face,
                    vertices: edge.vertices,
                    position,
                    collinear,
                }));
            }
        }
        Ok(None)
    }
}

fn intersection(
    s: [f32; 3],
    t: [f32; 3],
    a: [f32; 3],
    b: [f32; 3],
) -> Result<Option<([f32; 3], bool)>> {
    ensure!(
        s.into_iter().chain(t).chain(a).chain(b).all(f32::is_finite),
        "nonfinite boundary coordinates"
    );
    let dx = t[0] - s[0];
    let dz = t[2] - s[2];
    let ex = b[0] - a[0];
    let ez = b[2] - a[2];
    let rx = s[0] - a[0];
    let rz = s[2] - a[2];
    // Separate products/subtractions: these instructions do not fuse.
    let determinant = ez * dx - ex * dz;
    let first = ex * rz - ez * rx;
    let second = dx * rz - dz * rx;
    ensure!(
        [determinant, first, second].into_iter().all(f32::is_finite),
        "boundary intersection overflow"
    );
    if determinant.abs() < f32::MIN_POSITIVE {
        // Collinear line test accepts even disjoint segments; preserve the source quirk.
        return Ok(
            (first.abs() < f32::MIN_POSITIVE && second.abs() < f32::MIN_POSITIVE)
                .then_some(([0.; 3], true)),
        );
    }
    let u = first / determinant;
    let v = second / determinant;
    ensure!(
        u.is_finite() && v.is_finite(),
        "boundary parameter overflow"
    );
    if !(0. ..=1.).contains(&u) || !(0. ..=1.).contains(&v) {
        return Ok(None);
    }
    let position = [s[0] + u * dx, 0., s[2] + u * dz];
    ensure!(
        position.into_iter().all(f32::is_finite),
        "boundary hit position overflow"
    );
    Ok(Some((position, false)))
}

#[cfg(test)]
#[path = "../tests/boundary.rs"]
mod tests;
