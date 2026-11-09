//! Native portal distance and one-zone, ordered portal-group expansion.
use super::math::{V, add, finite, scale, sub};
use anyhow::{Result, ensure};
use serde::Serialize;

const SENTINEL: f32 = f32::from_bits(0x5f7fffff);
fn dot(a: V, b: V) -> f32 {
    a[2].mul_add(b[2], a[0] * b[0] + a[1] * b[1])
}
fn scalars(values: &[f32]) -> Result<()> {
    ensure!(
        values.iter().all(|v| v.is_finite()),
        "portal distance intermediate overflow"
    );
    Ok(())
}
fn unit(v: f32) -> f32 {
    v.clamp(0., 1.)
}
struct Projection {
    squared: f32,
    s: f32,
    t: f32,
    degenerate: bool,
}
/// Squared distance over origin/edge storage. Determinant rejection and the
/// region decisions are separate from the later polygon wrapper's sqrt/cutoff.
fn triangle(origin: V, edges: [V; 2], point: V) -> Result<Projection> {
    let delta = sub(origin, point);
    finite(delta)?;
    let [u, v] = edges;
    let a = dot(u, u);
    let b = dot(u, v);
    let c = dot(v, v);
    let determinant = a * c - b * b;
    scalars(&[a, b, c, determinant])?;
    if determinant <= 0.0001 {
        return Ok(Projection {
            squared: SENTINEL,
            s: 0.,
            t: 0.,
            degenerate: true,
        });
    }
    let d = dot(u, delta);
    let e = dot(v, delta);
    let f = dot(delta, delta);
    let sn = b * e - c * d;
    let tn = b * d - a * e;
    let mut s = sn / determinant;
    let mut t = tn / determinant;
    scalars(&[d, e, f, sn, tn, s, t, s + t])?;
    if s + t <= 1. {
        if s < 0. {
            if t < 0. {
                if d < 0. {
                    s = (-d / a).min(1.);
                    t = 0.;
                } else if e < 0. {
                    s = 0.;
                    t = (-e / c).min(1.);
                } else {
                    s = 0.;
                    t = 0.;
                }
            } else {
                s = 0.;
                t = unit(-e / c);
            }
        } else if t < 0. {
            s = unit(-d / a);
            t = 0.;
        }
    } else if s < 0. {
        let q = (d + (b - c)) - e;
        if q < 0. {
            s = unit(-q / ((a - 2. * b) + c));
            t = 1. - s;
        } else if c + e > 0. {
            s = 0.;
            t = unit(-e / c);
        } else {
            s = 0.;
            t = 1.;
        }
    } else if t < 0. {
        let q = (d + (a - b)) - e;
        if q > 0. {
            t = unit(q / ((a - 2. * b) + c));
            s = 1. - t;
        } else if a + d > 0. {
            s = unit(-d / a);
            t = 0.;
        } else {
            s = 1.;
            t = 0.;
        }
    } else {
        let q = (d + (b - c)) - e;
        s = unit(-q / ((a - 2. * b) + c));
        t = 1. - s;
    }
    // Retain the quadratic distance expression and its cancellation. Computing
    // length(point - reconstructed_closest) changes native small-distance gates.
    let first = s * ((2. * d) + (a * s + b * t));
    let second = t * ((2. * e) + (b * s + c * t));
    let squared = (f + (first + second)).abs();
    scalars(&[s, t, first, second, squared])?;
    Ok(Projection {
        squared,
        s,
        t,
        degenerate: false,
    })
}

#[derive(Debug, Serialize)]
pub struct PortalDistance {
    pub distance: f32,
    pub point: V,
    pub triangles_tested: usize,
    pub degenerate_triangles: usize,
}
/// Quad and linked polygon inputs share the same alternating origin-anchored fan:
/// [0,1,2], [0,3,2], [0,3,4], ... . No winding repair or fallback segment solver.
/// Host sqrt remains provisional relative to the Wii estimate/refinement.
pub fn distance(vertices: &[V], point: V, triangle_budget: usize) -> Result<PortalDistance> {
    ensure!(
        vertices.len() >= 3,
        "portal requires at least three vertices"
    );
    ensure!(
        triangle_budget > 0 && triangle_budget <= 65536,
        "invalid portal triangle budget"
    );
    finite(point)?;
    let origin = vertices[0];
    finite(origin)?;
    finite(vertices[1])?;
    finite(vertices[2])?;
    let mut edges = [sub(vertices[1], origin), sub(vertices[2], origin)];
    let mut best = SENTINEL;
    let mut closest = origin;
    let mut degenerates = 0;
    for i in 0..vertices.len() - 2 {
        ensure!(i < triangle_budget, "portal triangle budget exhausted");
        finite(vertices[i + 2])?;
        edges[(i + 1) % 2] = sub(vertices[i + 2], origin);
        for e in edges {
            finite(e)?;
        }
        let result = triangle(origin, edges, point)?;
        degenerates += usize::from(result.degenerate);
        let cutoff = best * best;
        scalars(&[cutoff])?;
        let candidate = if result.squared > cutoff {
            best + 1.
        } else {
            // Point writes precede the best-distance comparison, including ties.
            closest = add(
                origin,
                add(scale(edges[0], result.s), scale(edges[1], result.t)),
            );
            finite(closest)?;
            if result.squared > f32::EPSILON {
                result.squared.sqrt()
            } else {
                0.
            }
        };
        scalars(&[candidate])?;
        if best > candidate {
            best = candidate;
        }
    }
    Ok(PortalDistance {
        distance: best,
        point: closest,
        triangles_tested: vertices.len() - 2,
        degenerate_triangles: degenerates,
    })
}

#[derive(Debug, Clone)]
pub struct PortalGroup {
    pub target_zone: i16,
    pub portal_indices: Vec<i16>,
}
#[derive(Debug, Serialize)]
pub struct ExpandedZones {
    pub zones: Vec<usize>,
    pub portal_tests: usize,
}
/// Expand only the initial zone's groups, retaining duplicates and first-hit
/// short circuit. The callback supplies actual portal distance or an error.
pub fn expand(
    initial: usize,
    zone_count: usize,
    groups: &[PortalGroup],
    radius: f32,
    portal_budget: usize,
    output_budget: usize,
    mut portal_distance: impl FnMut(i16) -> Result<f32>,
) -> Result<ExpandedZones> {
    ensure!(
        initial < zone_count,
        "initial portal zone outside directory"
    );
    ensure!(radius.is_finite(), "nonfinite portal radius");
    ensure!(
        portal_budget > 0 && portal_budget <= 1_000_000,
        "invalid portal-test budget"
    );
    ensure!(
        output_budget > 0 && output_budget <= 65536,
        "invalid portal-output budget"
    );
    let mut result = ExpandedZones {
        zones: vec![initial],
        portal_tests: 0,
    };
    for group in groups {
        if group.target_zone < 0 {
            continue;
        }
        for &index in &group.portal_indices {
            ensure!(
                result.portal_tests < portal_budget,
                "portal-test budget exhausted"
            );
            ensure!(index >= 0, "negative addressed portal index");
            result.portal_tests += 1;
            let distance = portal_distance(index)?;
            ensure!(distance.is_finite(), "nonfinite portal distance result");
            if distance < radius {
                let target = group.target_zone as usize;
                ensure!(
                    target < zone_count,
                    "expanded portal zone outside directory"
                );
                ensure!(
                    result.zones.len() < output_budget,
                    "portal-output budget exhausted"
                );
                result.zones.push(target);
                break;
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
#[path = "../tests/collision_portal.rs"]
mod tests;
