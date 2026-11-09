//! Asset-to-runtime adapter and geometric smoke probes; no original code execution.
use anyhow::{Context, Result, ensure};
use ge_assets::graph::TriangleGraph;
use ge_core::navigation::{
    SegmentSearch, SegmentSearchResult, SurfaceHit, SurfaceRay, SurfaceTriangle,
    connected_segment_hit, first_surface_hit, group_range, nearest_surface_hit,
    triangle_segment_hit,
};
use serde::Serialize;
use std::ops::Range;

#[derive(Debug, Clone, Copy, Serialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Selection {
    /// First accepted triangle in storage order, strictly 0 < t < maximum.
    First,
    /// Least nonnegative parameter below maximum; storage order breaks ties.
    Nearest,
}

pub struct SurfaceData {
    triangles: Vec<SurfaceTriangle>,
    groups: Vec<Range<usize>>,
    neighbors: Vec<[Option<usize>; 3]>,
}
impl SurfaceData {
    pub fn new(graph: &TriangleGraph) -> Result<Self> {
        let mut triangles = Vec::new();
        triangles.try_reserve_exact(graph.triangles.len())?;
        for (index, triangle) in graph.triangles.iter().enumerate() {
            let mut points = [[0.; 3]; 3];
            for (i, &vertex) in triangle.vertices.iter().enumerate() {
                points[i] = *graph
                    .vertices
                    .get(vertex as usize)
                    .context("navigation vertex index out of bounds")?;
            }
            triangles.push(SurfaceTriangle {
                index,
                vertices: points,
            });
        }
        let groups = graph
            .groups
            .iter()
            .map(|g| {
                let start = g.first_triangle as usize;
                let end = start
                    .checked_add(g.triangle_count as usize)
                    .context("navigation group range overflow")?;
                ensure!(end <= triangles.len(), "navigation group out of bounds");
                Ok(start..end)
            })
            .collect::<Result<Vec<_>>>()?;
        let neighbors: Vec<_> = graph
            .triangles
            .iter()
            .map(|t| t.neighbors.map(|n| n.map(|n| n as usize)))
            .collect();
        ensure!(
            neighbors
                .iter()
                .flatten()
                .all(|n| n.is_none_or(|n| n < triangles.len())),
            "navigation neighbor index out of bounds"
        );
        Ok(Self {
            triangles,
            groups,
            neighbors,
        })
    }
    pub fn query(
        &self,
        ray: SurfaceRay,
        group: i32,
        selection: Selection,
    ) -> Result<Option<SurfaceHit>> {
        let range = group_range(self.triangles.len(), &self.groups, group)?;
        let triangles = self.triangles[range].iter().copied();
        match selection {
            Selection::First => first_surface_hit(ray, triangles),
            Selection::Nearest => nearest_surface_hit(ray, triangles),
        }
    }
    pub fn segment(
        &self,
        origin: [f32; 3],
        displacement: [f32; 3],
        triangle: usize,
    ) -> Result<Option<SurfaceHit>> {
        let triangle = *self
            .triangles
            .get(triangle)
            .context("navigation triangle index out of bounds")?;
        triangle_segment_hit(origin, displacement, triangle)
    }
    pub fn connected_segment(&self, request: SegmentSearch) -> Result<SegmentSearchResult> {
        connected_segment_hit(request, &self.triangles, &self.neighbors)
    }
    pub fn probe(&self) -> Result<ProbeStats> {
        let mut stats = ProbeStats::default();
        let stride = self.triangles.len().div_ceil(32).max(1);
        for index in (0..self.triangles.len()).step_by(stride).take(32) {
            let points = self.triangles[index].vertices.map(|p| p.map(f64::from));
            let normal = cross(sub(points[1], points[0]), sub(points[2], points[0]));
            let squared = dot(normal, normal);
            if squared <= f64::from(f32::EPSILON) {
                stats.skipped_small_samples += 1;
                continue;
            }
            let normal = normal.map(|v| v / squared.sqrt());
            let center: [f64; 3] =
                std::array::from_fn(|i| (points[0][i] + points[1][i] + points[2][i]) / 3.);
            let ray = SurfaceRay {
                origin: std::array::from_fn(|i| (center[i] + normal[i]) as f32),
                direction: normal.map(|v| -v as f32),
                max_parameter: 2.,
            };
            let own_group = self
                .groups
                .iter()
                .position(|r| r.contains(&index))
                .context("triangle has no navigation group")? as i32;
            let mut full_hits = [None; 2];
            for group in [-1, own_group, i32::MAX] {
                let first = self.query(ray, group, Selection::First)?;
                let nearest = self.query(ray, group, Selection::Nearest)?;
                stats.first_queries += 1;
                stats.nearest_queries += 1;
                if let (Some(first), Some(nearest)) = (first, nearest) {
                    ensure!(
                        nearest.parameter <= first.parameter,
                        "nearest query returned a farther intersection than storage-first"
                    );
                    if group == -1 && nearest.triangle != first.triangle {
                        stats.nearest_differs_from_first += 1;
                    }
                }
                for (variant, result) in [first, nearest].into_iter().enumerate() {
                    stats.queries += 1;
                    let Some(hit) = result else {
                        stats.misses += 1;
                        continue;
                    };
                    let range = group_range(self.triangles.len(), &self.groups, group)?;
                    ensure!(
                        range.contains(&hit.triangle),
                        "query escaped selected navigation group"
                    );
                    if group == -1 {
                        full_hits[variant] = Some(hit);
                    }
                    if group == i32::MAX {
                        ensure!(
                            full_hits[variant] == Some(hit),
                            "invalid navigation group did not preserve full-mesh result"
                        );
                    }
                    self.validate_hit(hit, &mut stats)?;
                }
            }
            stats.queries += 1;
            stats.segment_queries += 1;
            if let Some(hit) = self.segment(ray.origin, ray.direction.map(|v| v * 2.), index)? {
                ensure!(
                    hit.triangle == index && (0. ..=1.).contains(&hit.parameter),
                    "selected-triangle segment returned an invalid result"
                );
                self.validate_hit(hit, &mut stats)?;
            } else {
                stats.misses += 1;
            }
            // Begin at a distinct reciprocal neighbor, so this exercises a transition
            // toward the sampled face rather than trivially hitting its starting face.
            let start = self.neighbors[index]
                .iter()
                .flatten()
                .copied()
                .find(|&neighbor| {
                    neighbor != index && self.neighbors[neighbor].contains(&Some(index))
                });
            if let Some(start_triangle) = start {
                stats.queries += 1;
                stats.connected_queries += 1;
                let found = self.connected_segment(SegmentSearch {
                    origin: ray.origin,
                    displacement: ray.direction.map(|v| v * 2.),
                    start_triangle,
                    previous_triangle: None,
                    max_depth: 1,
                    max_face_tests: 4,
                })?;
                ensure!(
                    (1..=4).contains(&found.face_tests) && found.maximum_depth_reached <= 1,
                    "connected-segment query exceeded its depth/work contract"
                );
                stats.connected_face_tests += found.face_tests;
                stats.connected_policy_checks += found.policy_checks;
                if let Some(hit) = found.hit {
                    ensure!(
                        (0. ..=1.).contains(&hit.parameter),
                        "connected-segment hit escaped segment"
                    );
                    if hit.triangle != start_triangle {
                        stats.connected_neighbor_hits += 1;
                    }
                    self.validate_hit(hit, &mut stats)?;
                } else {
                    stats.misses += 1;
                }
            } else {
                stats.skipped_connected_samples += 1;
            }
        }
        ensure!(
            stats.queries > 0,
            "navigation mesh has no ordinary surface probe samples"
        );
        ensure!(
            stats.misses == 0,
            "navigation surface probes missed {} expected intersections",
            stats.misses
        );
        Ok(stats)
    }

    fn validate_hit(&self, hit: SurfaceHit, stats: &mut ProbeStats) -> Result<()> {
        stats.hits += 1;
        let p = self.triangles[hit.triangle]
            .vertices
            .map(|p| p.map(f64::from));
        let ab = sub(p[1], p[0]);
        let ac = sub(p[2], p[0]);
        let relative = sub(hit.position.map(f64::from), p[0]);
        let n = cross(ab, ac);
        let norm = dot(n, n);
        if norm <= f64::from(f32::EPSILON) {
            stats.fallback_hits += 1;
            return Ok(());
        }
        let error = dot(n, relative).abs() / norm.sqrt();
        stats.max_plane_error = stats.max_plane_error.max(error);
        let aa = dot(ab, ab);
        let cc = dot(ac, ac);
        let bc = dot(ab, ac);
        let ar = dot(ab, relative);
        let cr = dot(ac, relative);
        let determinant = aa * cc - bc * bc;
        let u = (cc * ar - bc * cr) / determinant;
        let v = (aa * cr - bc * ar) / determinant;
        ensure!(
            error < 1e-3 && u >= -1e-3 && v >= -1e-3 && u + v <= 1.001,
            "navigation probe failed independent f64 plane/barycentric check"
        );
        Ok(())
    }
}

#[derive(Default, Serialize)]
pub struct ProbeStats {
    pub queries: usize,
    pub first_queries: usize,
    pub nearest_queries: usize,
    pub segment_queries: usize,
    pub connected_queries: usize,
    pub connected_face_tests: usize,
    pub connected_policy_checks: usize,
    pub connected_neighbor_hits: usize,
    pub skipped_connected_samples: usize,
    /// Full-range sample positions whose two selection methods return different faces.
    pub nearest_differs_from_first: usize,
    pub hits: usize,
    pub misses: usize,
    pub skipped_small_samples: usize,
    pub fallback_hits: usize,
    pub max_plane_error: f64,
}
impl ProbeStats {
    pub fn include(&mut self, other: Self) {
        self.queries += other.queries;
        self.first_queries += other.first_queries;
        self.nearest_queries += other.nearest_queries;
        self.segment_queries += other.segment_queries;
        self.connected_queries += other.connected_queries;
        self.connected_face_tests += other.connected_face_tests;
        self.connected_policy_checks += other.connected_policy_checks;
        self.connected_neighbor_hits += other.connected_neighbor_hits;
        self.skipped_connected_samples += other.skipped_connected_samples;
        self.nearest_differs_from_first += other.nearest_differs_from_first;
        self.hits += other.hits;
        self.misses += other.misses;
        self.skipped_small_samples += other.skipped_small_samples;
        self.fallback_hits += other.fallback_hits;
        self.max_plane_error = self.max_plane_error.max(other.max_plane_error);
    }
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| a[i] * b[i]).sum()
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[cfg(test)]
#[path = "tests/navigation.rs"]
mod tests;
