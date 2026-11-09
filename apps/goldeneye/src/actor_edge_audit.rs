//! Independent f64 diagnostics for the actor edge predicate; authored targets, not original AI.
use super::*;
#[derive(Default, Serialize)]
pub(super) struct Audit {
    edges: usize,
    allowed: usize,
    flag_rejections: usize,
    normal_rejections: usize,
    direction_rejections: usize,
    validated_edges: usize,
    ambiguous_edges: usize,
    backtrack_samples: usize,
    normal_samples: usize,
    low_normal_faces: usize,
    queries: usize,
    found: usize,
    misses: usize,
    validated_queries: usize,
    ambiguous_queries: usize,
    path_edges: usize,
    ambiguous_path_edges: usize,
    max_cost_error: f64,
}
impl Audit {
    pub(super) fn include(&mut self, s: Self) {
        self.edges += s.edges;
        self.allowed += s.allowed;
        self.flag_rejections += s.flag_rejections;
        self.normal_rejections += s.normal_rejections;
        self.direction_rejections += s.direction_rejections;
        self.validated_edges += s.validated_edges;
        self.ambiguous_edges += s.ambiguous_edges;
        self.backtrack_samples += s.backtrack_samples;
        self.normal_samples += s.normal_samples;
        self.low_normal_faces += s.low_normal_faces;
        self.queries += s.queries;
        self.found += s.found;
        self.misses += s.misses;
        self.validated_queries += s.validated_queries;
        self.ambiguous_queries += s.ambiguous_queries;
        self.path_edges += s.path_edges;
        self.ambiguous_path_edges += s.ambiguous_path_edges;
        self.max_cost_error = self.max_cost_error.max(s.max_cost_error);
    }
}
impl PathData {
    pub(super) fn probe_actor_edges(&self) -> Result<Audit> {
        let mut audit = Audit::default();
        for normal in self.normals.iter().flatten() {
            audit.normal_samples += 1;
            audit.low_normal_faces += usize::from(normal[1] < 0.9);
        }
        for (from, row) in self.neighbors.iter().enumerate() {
            for (slot, to) in row
                .iter()
                .take_while(|n| n.is_some())
                .flatten()
                .copied()
                .enumerate()
            {
                let edge = TraversalEdge {
                    from_triangle: from,
                    to_triangle: to,
                    previous_triangle: None,
                    slot,
                };
                let decision = self.actor_edge_decision(edge, [0.; 3])?;
                audit.edges += 1;
                audit.allowed += usize::from(decision.allowed());
                match decision {
                    ActorEdgeDecision::FaceFlags { .. } => audit.flag_rejections += 1,
                    ActorEdgeDecision::NormalY { .. } => audit.normal_rejections += 1,
                    ActorEdgeDecision::Direction { allowed: false, .. } => {
                        audit.direction_rejections += 1
                    }
                    ActorEdgeDecision::ImmediateBacktrack => {
                        anyhow::bail!("unexpected backtracking without a parent")
                    }
                    _ => {}
                }
                if let Some(expected) = self.actor_edge_oracle(edge, [0.; 3]) {
                    ensure!(
                        decision.allowed() == expected,
                        "actor edge differs from independent f64 predicate"
                    );
                    audit.validated_edges += 1;
                } else {
                    audit.ambiguous_edges += 1;
                }
                if audit.backtrack_samples == 0 {
                    ensure!(
                        matches!(
                            self.actor_edge_decision(
                                TraversalEdge {
                                    previous_triangle: Some(to),
                                    ..edge
                                },
                                [0.; 3]
                            )?,
                            ActorEdgeDecision::ImmediateBacktrack
                        ),
                        "actor edge backtrack order changed"
                    );
                    audit.backtrack_samples = 1;
                }
            }
        }
        let start = 0;
        let goal = self.reachable_faces(start).1;
        let point = self.centers[goal];
        let result = self.query_goal(
            GoalPathSearch {
                start_triangle: start,
                max_expansions: 100_000,
            },
            &[goal],
            point,
            true,
        )?;
        let sure = self.actor_edge_reachable(point, false)[goal];
        let possible = self.actor_edge_reachable(point, true)[goal];
        let found = result.search.path.is_some();
        if sure || !possible {
            ensure!(
                found == sure,
                "actor edge search differs from independent allowed-edge reachability"
            );
            audit.validated_queries = 1;
        } else {
            audit.ambiguous_queries = 1;
        }
        audit.queries = 1;
        audit.found = usize::from(found);
        audit.misses = usize::from(!found);
        if let Some(path) = result.search.path {
            audit.max_cost_error = self.validate_route(&path, start, goal)?;
            for pair in path.triangles.windows(2) {
                let slot = self.neighbors[pair[0]]
                    .iter()
                    .take_while(|n| n.is_some())
                    .position(|&n| n == Some(pair[1]))
                    .context("actor path edge not in neighbor prefix")?;
                let edge = TraversalEdge {
                    from_triangle: pair[0],
                    to_triangle: pair[1],
                    previous_triangle: None,
                    slot,
                };
                if let Some(ok) = self.actor_edge_oracle(edge, point) {
                    ensure!(ok, "actor path contains an independently rejected edge");
                } else {
                    audit.ambiguous_path_edges += 1;
                }
                audit.path_edges += 1;
            }
        }
        Ok(audit)
    }
    pub(super) fn actor_edge_reachable(&self, point: [f32; 3], allow_ambiguous: bool) -> Vec<bool> {
        let mut seen = vec![false; self.neighbors.len()];
        seen[0] = true;
        let mut queue = VecDeque::from([0]);
        while let Some(from) = queue.pop_front() {
            for (slot, to) in self.neighbors[from]
                .iter()
                .take_while(|n| n.is_some())
                .flatten()
                .copied()
                .enumerate()
            {
                let edge = TraversalEdge {
                    from_triangle: from,
                    to_triangle: to,
                    previous_triangle: None,
                    slot,
                };
                if !seen[to]
                    && self
                        .actor_edge_oracle(edge, point)
                        .unwrap_or(allow_ambiguous)
                {
                    seen[to] = true;
                    queue.push_back(to);
                }
            }
        }
        seen
    }
    pub(super) fn actor_edge_oracle(&self, edge: TraversalEdge, point: [f32; 3]) -> Option<bool> {
        if Some(edge.to_triangle) == edge.previous_triangle
            || self.flags[edge.to_triangle].unwrap_or(0) & 3 != 0
        {
            return Some(false);
        }
        if self.normals[edge.to_triangle].is_some_and(|n| f64::from(n[1]) < f64::from(0.9f32)) {
            return Some(false);
        }
        let from = self.centers[edge.from_triangle].map(f64::from);
        let to = self.centers[edge.to_triangle].map(f64::from);
        let point = point.map(f64::from);
        let mut a = sub(to, from);
        let mut b = sub(point, from);
        for v in [&mut a, &mut b] {
            let len = v.iter().map(|x| x * x).sum::<f64>().sqrt();
            if (len - f64::from(1e-5f32)).abs() < 1e-10 {
                return None;
            }
            if len > f64::from(1e-5f32) {
                for x in v {
                    *x /= len;
                }
            }
        }
        let projection = dot(a, b);
        let threshold = f64::from(f32::from_bits(0xbeaf1d42));
        if (projection - threshold).abs() < 1e-5 {
            None
        } else {
            Some(projection > threshold)
        }
    }
}
