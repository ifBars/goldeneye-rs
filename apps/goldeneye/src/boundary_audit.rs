//! Corpus checks against independent topology and f64 XZ diagnostics, not original execution.
use super::*;

#[derive(Default, Serialize)]
pub(super) struct Audit {
    caches: usize,
    edges: usize,
    topology_faces: usize,
    flagged_faces: usize,
    isolated_faces: usize,
    missing_exclusions: usize,
    queries: usize,
    validated: usize,
    ambiguous: usize,
    blocked: usize,
    actor_samples: usize,
    actor_validated: usize,
    actor_ambiguous: usize,
    actor_accepted: usize,
}
impl Audit {
    pub(super) fn include(&mut self, other: Self) {
        self.caches += other.caches;
        self.edges += other.edges;
        self.topology_faces += other.topology_faces;
        self.flagged_faces += other.flagged_faces;
        self.isolated_faces += other.isolated_faces;
        self.missing_exclusions += other.missing_exclusions;
        self.queries += other.queries;
        self.validated += other.validated;
        self.ambiguous += other.ambiguous;
        self.blocked += other.blocked;
        self.actor_samples += other.actor_samples;
        self.actor_validated += other.actor_validated;
        self.actor_ambiguous += other.actor_ambiguous;
        self.actor_accepted += other.actor_accepted;
    }
}
impl PathData {
    pub(super) fn probe_boundary(&self) -> Result<Audit> {
        let cache = self.boundary.stats();
        let mut audit = Audit {
            caches: 1,
            edges: cache.edges,
            flagged_faces: cache.flagged_faces,
            isolated_faces: cache.isolated_faces,
            missing_exclusions: cache.missing_exclusions,
            ..Audit::default()
        };
        let mut per_face = vec![Vec::new(); self.vertex_ids.len()];
        for edge in self.boundary.edges() {
            let mut pair = edge.vertices;
            pair.sort_unstable();
            per_face[edge.source_face].push(pair);
        }
        for (face, ids) in self.vertex_ids.iter().enumerate() {
            let neighbors = self.neighbors[face]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
            let flagged = self.flags[face].unwrap_or(0) & 1 != 0;
            let mut expected = Vec::new();
            // Independent undirected edge incidence on well-formed source topology.
            // Eligibility is source-specific: unflagged isolated/interior faces emit none.
            if flagged || (1..=2).contains(&neighbors.len()) {
                for i in 0..3 {
                    let mut pair = [ids[i], ids[(i + 1) % 3]];
                    if flagged
                        || !neighbors
                            .iter()
                            .any(|&n| pair.iter().all(|v| self.vertex_ids[n].contains(v)))
                    {
                        pair.sort_unstable();
                        expected.push(pair);
                    }
                }
            }
            expected.sort_unstable();
            per_face[face].sort_unstable();
            ensure!(
                expected == per_face[face],
                "boundary topology disagreement at face {face}: expected {expected:?}, cached {:?}",
                per_face[face]
            );
            audit.topology_faces += 1;
        }
        let stride = self.boundary.edges().len().div_ceil(8).max(1);
        for edge in self.boundary.edges().iter().step_by(stride).take(8) {
            let [a, b] = edge.vertices.map(|v| self.vertices[v]);
            let dx = b[0] - a[0];
            let dz = b[2] - a[2];
            let scale = dx.abs().max(dz.abs());
            if scale == 0. {
                continue;
            }
            let midpoint = [(a[0] + b[0]) * 0.5, 0., (a[2] + b[2]) * 0.5];
            let perpendicular = [-dz / scale * 0.3, 0., dx / scale * 0.3];
            let s = std::array::from_fn(|i| midpoint[i] - perpendicular[i]);
            let t = std::array::from_fn(|i| midpoint[i] + perpendicular[i]);
            for (s, t) in [
                (s, t),
                ([s[0], 1000., s[2]], [t[0], -1000., t[2]]),
                (a, b),
                (
                    [a[0] + dx * 2., 0., a[2] + dz * 2.],
                    [a[0] + dx * 3., 0., a[2] + dz * 3.],
                ),
            ] {
                let found = self.boundary_check(s, t)?;
                audit.queries += 1;
                audit.blocked += usize::from(found.is_some());
                if let Some(expected) = first_edge_oracle(self, s, t) {
                    ensure!(
                        found.as_ref().map(|h| h.edge) == expected,
                        "boundary first-hit disagreement: expected {expected:?}, actual {:?}",
                        found.as_ref().map(|h| h.edge)
                    );
                    audit.validated += 1;
                } else {
                    audit.ambiguous += 1;
                }
            }
            use ge_core::navigation::{
                ActorBoundaryBand, ActorBoundaryResult, ActorBoundarySample, actor_boundary_gate,
            };
            for band in [
                ActorBoundaryBand::Radius45,
                ActorBoundaryBand::Radius34,
                ActorBoundaryBand::Radius345,
                ActorBoundaryBand::Radius36,
            ] {
                // Offset the target from the sampled edge to avoid deliberately
                // placing every endpoint exactly on a closed intersection boundary.
                let target = [midpoint[0] + 0.137, 0., midpoint[2] + 0.193];
                let actor = [target[0] + band.radius(), 0., target[2]];
                let mut calls = 0;
                let mut first = None;
                let result = actor_boundary_gate(
                    band,
                    0,
                    true,
                    || Ok(ActorBoundarySample { actor, target }),
                    |a, b| {
                        calls += 1;
                        first = self.boundary_check(a, b)?;
                        Ok(first.is_none())
                    },
                )?;
                ensure!(
                    calls == 1 && matches!(result, ActorBoundaryResult::Boundary { .. }),
                    "eligible authored actor sample did not reach boundary predicate"
                );
                audit.actor_samples += 1;
                audit.actor_accepted += usize::from(result.accepted());
                if let Some(expected) = first_edge_oracle(self, actor, target) {
                    ensure!(
                        first.as_ref().map(|h| h.edge) == expected
                            && result.accepted() == expected.is_none(),
                        "actor boundary sample disagrees with independent first-hit diagnostic"
                    );
                    audit.actor_validated += 1;
                } else {
                    audit.actor_ambiguous += 1;
                }
            }
        }
        Ok(audit)
    }
}

fn first_edge_oracle(data: &PathData, s: [f32; 3], t: [f32; 3]) -> Option<Option<usize>> {
    for (i, edge) in data.boundary.edges().iter().enumerate() {
        let [a, b] = edge.vertices.map(|v| data.vertices[v]);
        if intersection_oracle(s, t, a, b)? {
            return Some(Some(i));
        }
    }
    Some(None)
}

/// None means numeric ambiguity, including f32 cancellation near parallel/collinear
/// lines or closed parameter endpoints. Never count those samples as validated.
fn intersection_oracle(s: [f32; 3], t: [f32; 3], a: [f32; 3], b: [f32; 3]) -> Option<bool> {
    let xz = |p: [f32; 3]| [f64::from(p[0]), f64::from(p[2])];
    let sub = |p: [f64; 2], q: [f64; 2]| [p[0] - q[0], p[1] - q[1]];
    let cross = |p: [f64; 2], q: [f64; 2]| p[0] * q[1] - p[1] * q[0];
    let (s, t, a, b) = (xz(s), xz(t), xz(a), xz(b));
    let (d, e, r) = (sub(t, s), sub(b, a), sub(a, s));
    let det = cross(d, e);
    let scale = (d[0].abs() + d[1].abs()) * (e[0].abs() + e[1].abs());
    if det.abs() <= scale.max(1.) * 1e-5 {
        return None;
    }
    let u = cross(r, e) / det;
    let v = cross(r, d) / det;
    let margin = 1e-4;
    if [u, v]
        .into_iter()
        .any(|p| p.abs() < margin || (p - 1.).abs() < margin)
    {
        return None;
    }
    Some((0. ..=1.).contains(&u) && (0. ..=1.).contains(&v))
}

#[cfg(test)]
#[path = "tests/boundary_audit.rs"]
mod tests;
