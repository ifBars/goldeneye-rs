//! Static root-mesh accounting and bounded, authored mesh-local contact probes.
use anyhow::{Context, Result, ensure};
use ge_assets::collision::CollisionMesh;
use ge_core::collision::{BoxTriangleContact, BoxTriangleResult, OrientedBox};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Default, Serialize)]
pub struct Audit {
    pub found: usize,
    pub decoded: usize,
    pub batches: usize,
    pub excluded_batches: usize,
    pub excluded_declared_triangles: usize,
    pub triangles: usize,
    pub enabled_triangles: usize,
    pub disabled_triangles: usize,
    pub degenerate_enabled_triangles: usize,
    pub declared_positions: usize,
    pub positions_read: usize,
    pub raw_triangle_flags: BTreeMap<u16, usize>,
    pub box_triangle: ContactProbe,
}

#[derive(Serialize)]
pub struct ContactProbe {
    pub samples_per_mesh_limit: usize,
    pub sampled_meshes: usize,
    pub sampled_triangles: usize,
    pub queries: usize,
    pub separating_axes: BTreeMap<u8, usize>,
    pub candidate_axes: BTreeMap<u8, usize>,
    pub contact_axes: BTreeMap<u8, usize>,
    pub candidates_without_contact: usize,
    pub far_queries: usize,
    pub far_separated: usize,
    pub plane_validated: usize,
    pub plane_ambiguous: usize,
    pub plane_degenerate: usize,
    pub maximum_plane_distance_f64: f64,
}
impl Default for ContactProbe {
    fn default() -> Self {
        Self {
            samples_per_mesh_limit: 2,
            sampled_meshes: 0,
            sampled_triangles: 0,
            queries: 0,
            separating_axes: BTreeMap::new(),
            candidate_axes: BTreeMap::new(),
            contact_axes: BTreeMap::new(),
            candidates_without_contact: 0,
            far_queries: 0,
            far_separated: 0,
            plane_validated: 0,
            plane_ambiguous: 0,
            plane_degenerate: 0,
            maximum_plane_distance_f64: 0.,
        }
    }
}
impl ContactProbe {
    fn outcome(&mut self, result: &BoxTriangleResult) {
        self.queries += 1;
        if let Some(axis) = result.separation_axis {
            *self.separating_axes.entry(axis).or_default() += 1;
        }
        if let Some(candidate) = &result.candidate {
            *self.candidate_axes.entry(candidate.axis).or_default() += 1;
            self.candidates_without_contact += usize::from(result.contact.is_none());
        }
        if let Some(contact) = &result.contact {
            *self.contact_axes.entry(contact.axis).or_default() += 1;
        }
    }
    fn plane(&mut self, points: [[f32; 3]; 3], contact: &BoxTriangleContact) -> Result<()> {
        // The primitive boundary stores f32 edges. Use f64 thereafter, without
        // production dot/cross helpers, to check its returned triangle-plane point.
        let e = [1, 2]
            .map(|j| std::array::from_fn::<_, 3, _>(|i| f64::from(points[j][i] - points[0][i])));
        let n = [
            e[0][1] * e[1][2] - e[0][2] * e[1][1],
            e[0][2] * e[1][0] - e[0][0] * e[1][2],
            e[0][0] * e[1][1] - e[0][1] * e[1][0],
        ];
        let length = n.into_iter().map(|v| v * v).sum::<f64>().sqrt();
        if length == 0. {
            // Rounded products + fused subtraction can retain a residual native
            // normal even when the independent f64 edge cross is exactly zero.
            self.plane_degenerate += 1;
            return Ok(());
        }
        let distance = n
            .into_iter()
            .enumerate()
            .map(|(i, n)| n * (f64::from(contact.point[i]) - f64::from(points[0][i])))
            .sum::<f64>()
            .abs()
            / length;
        let scale = points
            .into_iter()
            .flatten()
            .chain(contact.point)
            .map(|v| f64::from(v).abs())
            .fold(1f64, f64::max);
        let tolerance = 1e-5 + 8. * f64::from(f32::EPSILON) * scale;
        ensure!(
            distance.is_finite() && distance <= tolerance,
            "contact axis {} leaves triangle plane by {distance} (bound {tolerance})",
            contact.axis
        );
        self.maximum_plane_distance_f64 = self.maximum_plane_distance_f64.max(distance);
        if distance <= 1e-5 {
            self.plane_validated += 1;
        } else {
            self.plane_ambiguous += 1;
        }
        Ok(())
    }
    fn triangle(&mut self, points: [[f32; 3]; 3]) -> Result<()> {
        let mut shape = OrientedBox {
            center: points[0],
            axes: [[1., 0., 0.], [0., 1., 0.]],
            half_extents: [0.25; 3],
        };
        let near = shape.query_triangle(points)?;
        self.outcome(&near);
        if let Some(contact) = near.contact {
            self.plane(points, &contact)?;
        }
        let max_x = points
            .map(|p| p[0])
            .into_iter()
            .fold(f32::NEG_INFINITY, f32::max);
        let margin = (max_x.abs() * (8. * f32::EPSILON)).max(1.);
        shape.center[0] = max_x + margin;
        ensure!(
            shape.center[0].is_finite() && f64::from(shape.center[0]) - 0.25 > f64::from(max_x),
            "cannot construct disjoint far collision probe"
        );
        let far = shape.query_triangle(points)?;
        self.outcome(&far);
        self.far_queries += 1;
        ensure!(
            far.separation_axis.is_some() && far.candidate.is_none() && far.contact.is_none(),
            "disjoint far collision probe returned a candidate/contact"
        );
        self.far_separated += 1;
        self.sampled_triangles += 1;
        Ok(())
    }
}
impl Audit {
    pub fn include(&mut self, mesh: &CollisionMesh) -> Result<()> {
        self.decoded += 1;
        self.batches += mesh.batches.len();
        self.excluded_batches += mesh.skipped_batches;
        self.excluded_declared_triangles += mesh
            .batches
            .iter()
            .filter(|b| !b.included())
            .map(|b| usize::from(b.declared_triangles))
            .sum::<usize>();
        self.triangles += mesh.triangles.len();
        self.disabled_triangles += mesh.disabled_triangles;
        self.enabled_triangles += mesh.triangles.len() - mesh.disabled_triangles;
        self.declared_positions += mesh.declared_positions;
        self.positions_read += mesh.positions_read;
        let mut samples = 0;
        for (index, triangle) in mesh.triangles.iter().enumerate() {
            *self
                .raw_triangle_flags
                .entry(triangle.flags_raw)
                .or_default() += 1;
            if triangle.enabled() {
                let points = mesh
                    .triangle_vertices(index)?
                    .context("enabled collision triangle has no vertices")?;
                let a = std::array::from_fn::<_, 3, _>(|i| {
                    f64::from(points[1][i]) - f64::from(points[0][i])
                });
                let b = std::array::from_fn::<_, 3, _>(|i| {
                    f64::from(points[2][i]) - f64::from(points[0][i])
                });
                let cross = [
                    a[1] * b[2] - a[2] * b[1],
                    a[2] * b[0] - a[0] * b[2],
                    a[0] * b[1] - a[1] * b[0],
                ];
                if cross.into_iter().all(|v| v == 0.) {
                    self.degenerate_enabled_triangles += 1;
                }
                if samples < self.box_triangle.samples_per_mesh_limit {
                    self.box_triangle.triangle(points).with_context(|| {
                        format!("mesh at {:#x} triangle {index} contact probe", mesh.address)
                    })?;
                    samples += 1;
                }
            }
        }
        self.box_triangle.sampled_meshes += usize::from(samples > 0);
        Ok(())
    }
}
