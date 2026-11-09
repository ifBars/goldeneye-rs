//! Authored portal probes and independent geometric diagnostics, not runtime parity.
use anyhow::{Context, Result, ensure};
use ge_assets::portals::{MapPortals, PortalShape};
use ge_core::collision::portal::{self, PortalGroup};
use serde::Serialize;

#[derive(Default, Serialize)]
pub struct Audit {
    pub maps: usize,
    pub groups: usize,
    pub rows: usize,
    pub inline_shapes: usize,
    pub linked_shapes: usize,
    pub expanded_queries: usize,
    pub stable_expansions: usize,
    pub ambiguous_expansions: usize,
    pub distance_calls: usize,
    pub triangles_tested: usize,
    pub degenerate_distance_calls: usize,
    pub strict_distance_checks: usize,
    pub rounding_ambiguous_distances: usize,
    pub unstable_distance_calls: usize,
    pub maximum_distance_error_f64: f64,
    pub maximum_zones_returned: usize,
}
impl Audit {
    pub fn include(&mut self, data: &MapPortals) {
        self.maps += 1;
        for groups in &data.zone_groups {
            self.groups += groups.len();
            for group in groups {
                self.rows += group.rows.len();
            }
        }
        for shape in data.shapes.values() {
            match shape {
                PortalShape::InlineQuad { .. } => self.inline_shapes += 1,
                PortalShape::LinkedPolygon { .. } => self.linked_shapes += 1,
            }
        }
    }
    pub fn query(
        &mut self,
        data: &MapPortals,
        initial: usize,
        origin: [f32; 3],
        bsp_ambiguous: bool,
    ) -> Result<()> {
        let source = data
            .zone_groups
            .get(initial)
            .context("portal groups missing for selected BSP zone")?;
        let groups = source
            .iter()
            .map(|g| PortalGroup {
                target_zone: g.target_zone,
                portal_indices: g.rows.iter().map(|r| r.portal_index_raw).collect(),
            })
            .collect::<Vec<_>>();
        for radius in [0.25, 3., 20.] {
            let found = portal::expand(
                initial,
                data.zone_groups.len(),
                &groups,
                radius,
                1_000_000,
                256,
                |index| {
                    let vertices = data
                        .shapes
                        .get(&(index as u16))
                        .context("missing decoded portal shape")?
                        .vertices();
                    let result = portal::distance(vertices, origin, 65536)?;
                    self.distance_calls += 1;
                    self.triangles_tested += result.triangles_tested;
                    if result.degenerate_triangles > 0 {
                        self.degenerate_distance_calls += 1;
                    } else {
                        let check = geometry(vertices, origin)?;
                        let error = (f64::from(result.distance) - check.distance).abs();
                        if !check.unstable {
                            ensure!(
                                error <= check.bound,
                                "portal distance differs from independent geometry by {error} (bound {})",
                                check.bound
                            );
                            self.maximum_distance_error_f64 =
                                self.maximum_distance_error_f64.max(error);
                            if error <= 0.0001 {
                                self.strict_distance_checks += 1;
                            } else {
                                self.rounding_ambiguous_distances += 1;
                            }
                        } else {
                            self.unstable_distance_calls += 1;
                        }
                    }
                    Ok(result.distance)
                },
            )?;
            self.expanded_queries += 1;
            self.maximum_zones_returned = self.maximum_zones_returned.max(found.zones.len());
            let mut expected = vec![initial];
            let mut ambiguous = bsp_ambiguous;
            for group in source {
                if group.target_zone < 0 {
                    continue;
                }
                for row in &group.rows {
                    let vertices = data
                        .shapes
                        .get(&(row.portal_index_raw as u16))
                        .context("independent portal shape missing")?
                        .vertices();
                    let check = geometry(vertices, origin)?;
                    ambiguous |=
                        check.unstable || (check.distance - f64::from(radius)).abs() <= check.bound;
                    if check.distance < f64::from(radius) {
                        expected.push(group.target_zone as usize);
                        break;
                    }
                }
            }
            if ambiguous {
                self.ambiguous_expansions += 1;
            } else {
                ensure!(
                    found.zones == expected,
                    "portal zone order differs from independent f64 geometry"
                );
                self.stable_expansions += 1;
            }
        }
        Ok(())
    }
}
type D = [f64; 3];
fn sub(a: D, b: D) -> D {
    std::array::from_fn(|i| a[i] - b[i])
}
fn dot(a: D, b: D) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}
fn cross(a: D, b: D) -> D {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn segment(a: D, b: D, p: D) -> f64 {
    let e = sub(b, a);
    let q = sub(p, a);
    let len = dot(e, e);
    let t = if len > 0. {
        (dot(q, e) / len).clamp(0., 1.)
    } else {
        0.
    };
    let d = std::array::from_fn(|i| q[i] - e[i] * t);
    dot(d, d)
}
struct Geometry {
    distance: f64,
    bound: f64,
    unstable: bool,
}
fn geometry(vertices: &[[f32; 3]], point: [f32; 3]) -> Result<Geometry> {
    let a = vertices[0];
    // The primitive boundary has rounded f32 edges and origin-point difference.
    // Use f64 plane/segment geometry after that boundary, not its region solver
    // or cancellation-prone quadratic distance expression.
    let p = std::array::from_fn::<_, 3, _>(|i| -f64::from(a[i] - point[i]));
    let mut minimum = f64::INFINITY;
    let mut scale = dot(p, p).max(1.);
    let mut unstable = false;
    for i in 1..vertices.len() - 1 {
        let u = std::array::from_fn(|k| f64::from(vertices[i][k] - a[k]));
        let v = std::array::from_fn(|k| f64::from(vertices[i + 1][k] - a[k]));
        let n = cross(u, v);
        let nn = dot(n, n);
        let aa = dot(u, u);
        let cc = dot(v, v);
        scale = scale.max(aa).max(cc);
        unstable |= nn <= 0.0001 + 8. * f64::from(f32::EPSILON) * (aa * cc);
        let mut squared = [([0.; 3], u), (u, v), (v, [0.; 3])]
            .into_iter()
            .map(|(a, b)| segment(a, b, p))
            .fold(f64::INFINITY, f64::min);
        if nn > 0. {
            let h = dot(n, p) / nn;
            let projected = std::array::from_fn(|k| p[k] - n[k] * h);
            let inside = [([0.; 3], u), (u, v), (v, [0.; 3])]
                .into_iter()
                .all(|(a, b)| dot(cross(sub(b, a), sub(projected, a)), n) >= 0.);
            if inside {
                squared = squared.min(h * h * nn);
            }
        }
        minimum = minimum.min(squared);
    }
    let distance = minimum.sqrt();
    let bound = 0.00001 + (64. * f64::from(f32::EPSILON) * scale).sqrt();
    ensure!(
        distance.is_finite() && bound.is_finite(),
        "independent portal geometry overflow"
    );
    Ok(Geometry {
        distance,
        bound,
        unstable,
    })
}
