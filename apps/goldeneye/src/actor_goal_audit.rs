//! Independent XZ/f64 geometry diagnostics with explicit unsupported/ambiguous counts.
use super::*;
use ge_core::navigation::{ActorGoalContext, ActorGoalDecision, ActorGoalResult};
#[derive(Default, Serialize)]
pub(super) struct Audit {
    pairs: usize,
    validated_pairs: usize,
    ambiguous_pairs: usize,
    unsupported_pairs: usize,
    accepted: usize,
    normal_rejections: usize,
    neighbor_rejections: usize,
    queries: usize,
    paths: usize,
    misses: usize,
    unsupported_queries: usize,
    budget_queries: usize,
    validated_queries: usize,
    ambiguous_queries: usize,
    walked_faces: usize,
    edge_tests: usize,
    max_cost_error: f64,
}
impl Audit {
    pub(super) fn include(&mut self, s: Self) {
        macro_rules! sum {($($f:ident),*)=>{$(self.$f+=s.$f;)*};}
        sum!(
            pairs,
            validated_pairs,
            ambiguous_pairs,
            unsupported_pairs,
            accepted,
            normal_rejections,
            neighbor_rejections,
            queries,
            paths,
            misses,
            unsupported_queries,
            budget_queries,
            validated_queries,
            ambiguous_queries,
            walked_faces,
            edge_tests
        );
        self.max_cost_error = self.max_cost_error.max(s.max_cost_error);
    }
}
#[derive(Debug, PartialEq)]
pub(super) enum Outcome {
    Far,
    Normal(usize),
    Blocked(usize, [usize; 2]),
    Accepted,
    Unsupported,
    Ambiguous,
}
impl PathData {
    pub(super) fn probe_actor_goals(&self) -> Result<Audit> {
        let mut a = Audit::default();
        for (from, row) in self.neighbors.iter().enumerate() {
            for to in row.iter().take_while(|n| n.is_some()).flatten().copied() {
                let mut context = ActorGoalContext {
                    origin: self.centers[from],
                    point: self.centers[to],
                    start_triangle: from,
                    last_center: None,
                };
                let expected = self.actor_goal_oracle(from, context.origin, context.point, to);
                let result = self.evaluate_actor_goal(&mut context, to, 1024);
                a.pairs += 1;
                ensure!(
                    context.last_center == Some(self.centers[to]),
                    "actor goal scratch center changed"
                );
                if let Ok(r) = &result {
                    a.walked_faces += r.walked_faces.len();
                    a.edge_tests += r.edge_tests;
                    match r.decision {
                        ActorGoalDecision::Accepted => a.accepted += 1,
                        ActorGoalDecision::NormalY { .. } => a.normal_rejections += 1,
                        ActorGoalDecision::NoNeighbor { .. } => a.neighbor_rejections += 1,
                        ActorGoalDecision::TooFar { .. } => {
                            anyhow::bail!("zero-distance actor goal was too far")
                        }
                    }
                }
                match expected {
                    Outcome::Ambiguous => a.ambiguous_pairs += 1,
                    Outcome::Unsupported => {
                        let error = result
                            .err()
                            .context("independent unsupported walk returned a result")?;
                        ensure!(
                            unsupported(&error),
                            "unexpected actor goal error: {error:#}"
                        );
                        a.unsupported_pairs += 1;
                    }
                    expected => {
                        let actual = result?;
                        ensure!(
                            observed(&actual) == expected,
                            "actor goal differs from independent f64 face walk: {from}->{to}: {:?} vs {expected:?}",
                            actual.decision
                        );
                        a.validated_pairs += 1;
                    }
                }
            }
        }
        a.queries = 1;
        let goal = self.reachable_faces(0).1;
        let origin = self.centers[0];
        let point = self.centers[goal];
        let queried = self.query_actor_path(
            GoalPathSearch {
                start_triangle: 0,
                max_expansions: 100_000,
            },
            origin,
            point,
            1024,
        );
        let queried = match queried {
            Ok(q) => q,
            Err(e)
                if e.to_string()
                    .contains("navigation path expansion budget exhausted") =>
            {
                a.budget_queries = 1;
                return Ok(a);
            }
            Err(e) if unsupported(&e) => {
                a.unsupported_queries = 1;
                return Ok(a);
            }
            Err(e) => return Err(e),
        };
        if let Some(path) = &queried.result.search.path {
            a.paths = 1;
            let last = *path.triangles.last().context("empty actor route")?;
            a.max_cost_error = self.validate_route(path, 0, last)?;
            let expected = self.actor_goal_oracle(0, origin, point, last);
            let mut ambiguous_edge = false;
            for pair in path.triangles.windows(2) {
                let slot = self.neighbors[pair[0]]
                    .iter()
                    .position(|&n| n == Some(pair[1]))
                    .context("actor route nonedge")?;
                let expected_edge = self.actor_edge_oracle(
                    TraversalEdge {
                        from_triangle: pair[0],
                        to_triangle: pair[1],
                        previous_triangle: None,
                        slot,
                    },
                    point,
                );
                ensure!(
                    expected_edge != Some(false),
                    "actor route contains independently rejected edge"
                );
                ambiguous_edge |= expected_edge.is_none();
            }
            if expected == Outcome::Ambiguous || ambiguous_edge {
                a.ambiguous_queries = 1;
            } else {
                ensure!(
                    expected == Outcome::Accepted,
                    "returned actor destination rejected by independent goal test"
                );
                a.validated_queries = 1;
            }
        } else {
            a.misses = 1;
            let sure = self.actor_edge_reachable(point, false);
            let possible = self.actor_edge_reachable(point, true);
            let mut ambiguous = false;
            for (face, &reachable) in possible.iter().enumerate() {
                if !reachable {
                    continue;
                }
                let expected = self.actor_goal_oracle(0, origin, point, face);
                ensure!(
                    !(sure[face] && expected == Outcome::Accepted),
                    "completed actor miss has an independently reachable accepted face"
                );
                ambiguous |= matches!(
                    expected,
                    Outcome::Accepted | Outcome::Ambiguous | Outcome::Unsupported
                );
            }
            if ambiguous {
                a.ambiguous_queries = 1;
            } else {
                a.validated_queries = 1;
            }
        }
        Ok(a)
    }
    pub(super) fn actor_goal_oracle(
        &self,
        start: usize,
        origin: [f32; 3],
        point: [f32; 3],
        candidate: usize,
    ) -> Outcome {
        let center = self.centers[candidate].map(f64::from);
        let distance = center
            .iter()
            .zip(point)
            .map(|(&a, b)| (a - f64::from(b)).powi(2))
            .sum::<f64>();
        if (distance - 4.).abs() < 1e-5 {
            return Outcome::Ambiguous;
        }
        if distance > 4. {
            return Outcome::Far;
        }
        let origin = origin.map(f64::from);
        let d = [center[0] - origin[0], center[2] - origin[2]];
        let (mut face, mut previous) = (start, None);
        for _ in 0..1024 {
            if self.normals[face].is_some_and(|n| n[1] < 0.9) {
                return Outcome::Normal(face);
            }
            let ids = self.vertex_ids[face];
            let mut selected = None;
            let mut forward = 0.;
            for i in 0..3 {
                let v = self.vertices[ids[i]].map(f64::from);
                let w = self.vertices[ids[(i + 1) % 3]].map(f64::from);
                let e = [w[0] - v[0], w[2] - v[2]];
                let offset = [v[0] - origin[0], v[2] - origin[2]];
                // Solve origin + t*d = v + u*e directly in XZ.
                let det = d[0] * e[1] - d[1] * e[0];
                if (det.abs() - f64::from(0.001f32)).abs() < 1e-7 * (1. + det.abs()) {
                    return Outcome::Ambiguous;
                }
                if det.abs() < f64::from(0.001f32) {
                    continue;
                }
                let u = (offset[0] * d[1] - offset[1] * d[0]) / det;
                if u.abs() < 1e-6 || (u - 1.).abs() < 1e-6 {
                    return Outcome::Ambiguous;
                }
                if !(0. ..=1.).contains(&u) {
                    continue;
                }
                let t = (offset[0] * e[1] - offset[1] * e[0]) / det;
                if (t - forward).abs() < 1e-6 * (1. + t.abs()) {
                    return Outcome::Ambiguous;
                }
                if t <= forward {
                    continue;
                }
                let had = selected.is_some();
                selected = Some(i);
                if face == start || had {
                    break;
                }
                forward = t;
            }
            let Some(slot) = selected else {
                return Outcome::Unsupported;
            };
            let edge = [ids[slot], ids[(slot + 1) % 3]];
            let next = self.neighbors[face].iter().flatten().copied().find(|&n| {
                Some(n) != previous
                    && self.flags[n].unwrap_or(0) & 1 == 0
                    && self.vertex_ids[n]
                        .iter()
                        .filter(|&&v| edge.contains(&v))
                        .count()
                        == 2
            });
            let Some(next) = next else {
                return Outcome::Blocked(face, edge);
            };
            previous = Some(face);
            face = next;
            if face == candidate {
                return Outcome::Accepted;
            }
        }
        Outcome::Unsupported
    }
}
fn unsupported(e: &anyhow::Error) -> bool {
    let text = e.to_string();
    text.contains("no supported forward crossing")
        || text.contains("actor goal face budget exhausted")
}
fn observed(result: &ActorGoalResult) -> Outcome {
    match result.decision {
        ActorGoalDecision::TooFar { .. } => Outcome::Far,
        ActorGoalDecision::NormalY { triangle, .. } => Outcome::Normal(triangle),
        ActorGoalDecision::NoNeighbor { triangle, edge } => Outcome::Blocked(triangle, edge),
        ActorGoalDecision::Accepted => Outcome::Accepted,
    }
}
