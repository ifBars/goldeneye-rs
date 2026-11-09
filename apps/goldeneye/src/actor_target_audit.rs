//! Authored caller snapshots, independent scalar geometry and manager flag checks.
use super::*;
use ge_core::navigation::{ActorTargetDecision, ActorTargetRequest, manager_navigation_edge};
#[derive(Default, Serialize)]
pub(super) struct Audit {
    actions: super::actor_action_audit::Audit,
    samples: usize,
    initial_hits: usize,
    initial_misses: usize,
    height_rejections: usize,
    connected_misses: usize,
    prepared: usize,
    validated_initial: usize,
    ambiguous_initial: usize,
    validated_connected: usize,
    ambiguous_connected: usize,
    paths: usize,
    misses: usize,
    unsupported_searches: usize,
    budget_searches: usize,
    manager_edges: usize,
    manager_allowed: usize,
    validated_paths: usize,
    ambiguous_paths: usize,
}
impl Audit {
    pub(super) fn include(&mut self, s: Self) {
        self.actions.include(s.actions);
        macro_rules! sum {($($f:ident),*)=>{$(self.$f+=s.$f;)*};}
        sum!(
            samples,
            initial_hits,
            initial_misses,
            height_rejections,
            connected_misses,
            prepared,
            validated_initial,
            ambiguous_initial,
            validated_connected,
            ambiguous_connected,
            paths,
            misses,
            unsupported_searches,
            budget_searches,
            manager_edges,
            manager_allowed,
            validated_paths,
            ambiguous_paths
        );
    }
}
impl PathData {
    pub(super) fn probe_actor_targets(&self) -> Result<Audit> {
        let mut a = Audit::default();
        for (from, row) in self.neighbors.iter().enumerate() {
            for (slot, &to) in row.iter().take_while(|n| n.is_some()).enumerate() {
                let to = to.context("manager audit boundary")?;
                let source = self.flags[from].unwrap_or(0);
                let destination = self.flags[to].unwrap_or(0);
                let expected = (destination & 2 == 0 || source & 2 != 0)
                    && (source & 16 == 0 || destination & 8 == 0);
                let actual = manager_navigation_edge(
                    &self.neighbors,
                    &self.flags,
                    TraversalEdge {
                        from_triangle: from,
                        to_triangle: to,
                        slot,
                        previous_triangle: None,
                    },
                )?;
                ensure!(
                    actual == expected,
                    "manager edge disagrees with independent flag conditions"
                );
                a.manager_edges += 1;
                a.manager_allowed += usize::from(actual);
            }
        }
        let frame = NavigationFrame {
            translation: [11., -7., 5.],
        };
        let goal = self.reachable_faces(0).1;
        a.include(self.actor_target_sample(frame, 0, goal)?);
        let near = (0..self.centers.len())
            .min_by(|&x, &y| {
                self.centers[x][1]
                    .abs()
                    .total_cmp(&self.centers[y][1].abs())
            })
            .context("empty actor target sample mesh")?;
        let distance = |face: usize| {
            let v = sub(
                self.centers[face].map(f64::from),
                self.centers[near].map(f64::from),
            );
            dot(v, v)
        };
        let active = self.neighbors[near]
            .iter()
            .take_while(|n| n.is_some())
            .flatten()
            .copied()
            .max_by(|&x, &y| distance(x).total_cmp(&distance(y)))
            .unwrap_or(near);
        a.include(self.actor_target_sample(frame, active, near)?);
        Ok(a)
    }
    fn actor_target_sample(
        &self,
        frame: NavigationFrame,
        start: usize,
        goal: usize,
    ) -> Result<Audit> {
        let mut a = Audit::default();
        let request = ActorTargetRequest {
            target: frame.position_to_world(self.centers[goal])?,
            active_position: frame.position_to_world(self.centers[start])?,
            owner_frame: frame,
            max_face_tests: 100_000,
        };
        let binding = self.prepare_actor_target(request, || Ok(Some(frame)))?;
        a.samples = 1;
        let ray = binding
            .target_ray
            .context("bound actor target did not form initial ray")?;
        ensure!(
            ray.max_parameter == 1.25 && ray.direction == [0., -1., 0.],
            "actor target ray contract changed"
        );
        let initial = first_oracle(&self.triangles, ray, false);
        if initial == Oracle::Ambiguous {
            a.ambiguous_initial = 1;
        } else {
            ensure!(
                initial == observed(binding.target_hit),
                "actor initial ray differs from independent first-hit geometry"
            );
            a.validated_initial = 1;
        }
        a.initial_hits = usize::from(binding.target_hit.is_some());
        match binding.decision {
            ActorTargetDecision::NoTargetFace => a.initial_misses = 1,
            ActorTargetDecision::TargetHeight => {
                ensure!(
                    binding.local_owner_target[1].abs() >= 1.,
                    "actor target height rejection changed"
                );
                a.height_rejections = 1;
            }
            ActorTargetDecision::Ready | ActorTargetDecision::NoActiveFace => {
                let segment = binding
                    .active_segment
                    .context("actor active lookup missing segment")?;
                let actual = binding
                    .active_lookup
                    .context("actor active lookup missing result")?;
                ensure!(
                    segment.max_depth == 5 && segment.displacement == [0., -2., 0.],
                    "actor connected segment contract changed"
                );
                let expected = self.actor_connected_oracle(segment);
                if expected == Oracle::Ambiguous {
                    a.ambiguous_connected = 1;
                } else {
                    ensure!(
                        expected == observed(actual.hit),
                        "actor active lookup differs from independent depth-first geometry/manager flags"
                    );
                    a.validated_connected = 1;
                }
                if let Some(p) = binding.prepared {
                    a.prepared = 1;
                    ensure!(
                        p.origin == frame.position_to_local(request.active_position)?
                            && p.point == frame.position_to_local(request.target)?,
                        "actor policy inputs were snapped or frame-converted incorrectly"
                    );
                    match self.query_actor_path(
                        GoalPathSearch {
                            start_triangle: p.start_triangle,
                            max_expansions: 100_000,
                        },
                        p.origin,
                        p.point,
                        1024,
                    ) {
                        Ok(r) => {
                            a.paths = usize::from(r.result.search.path.is_some());
                            a.misses = usize::from(r.result.search.path.is_none());
                            if let Some(path) = &r.result.search.path {
                                let last =
                                    *path.triangles.last().context("empty bound actor path")?;
                                self.validate_route(path, p.start_triangle, last)?;
                                let expected = self.actor_goal_oracle(
                                    p.start_triangle,
                                    p.origin,
                                    p.point,
                                    last,
                                );
                                use super::actor_goal_audit::Outcome;
                                ensure!(
                                    matches!(expected, Outcome::Accepted | Outcome::Ambiguous),
                                    "bound actor destination disagrees with independent goal geometry"
                                );
                                let mut ambiguous = expected == Outcome::Ambiguous;
                                for pair in path.triangles.windows(2) {
                                    let slot = self.neighbors[pair[0]]
                                        .iter()
                                        .position(|&n| n == Some(pair[1]))
                                        .context("bound actor nonedge")?;
                                    let edge = self.actor_edge_oracle(
                                        TraversalEdge {
                                            from_triangle: pair[0],
                                            to_triangle: pair[1],
                                            previous_triangle: None,
                                            slot,
                                        },
                                        p.point,
                                    );
                                    ensure!(
                                        edge != Some(false),
                                        "bound actor path has independently rejected edge"
                                    );
                                    ambiguous |= edge.is_none();
                                }
                                a.ambiguous_paths = usize::from(ambiguous);
                                a.validated_paths = usize::from(!ambiguous);
                            }
                        }
                        Err(e)
                            if e.to_string()
                                .contains("navigation path expansion budget exhausted") =>
                        {
                            a.budget_searches = 1
                        }
                        Err(e)
                            if e.to_string().contains("no supported forward crossing")
                                || e.to_string().contains("actor goal face budget exhausted") =>
                        {
                            a.unsupported_searches = 1
                        }
                        Err(e) => return Err(e),
                    }
                } else {
                    a.connected_misses = 1;
                }
            }
            _ => anyhow::bail!("stable bound snapshot unexpectedly lost its binding"),
        }
        a.actions = self.probe_actor_action(request, frame)?;
        Ok(a)
    }
    fn actor_connected_oracle(&self, r: ge_core::navigation::SegmentSearch) -> Oracle {
        let mut stack = vec![(r.start_triangle, None, r.max_depth)];
        let ray = ge_core::navigation::SurfaceRay {
            origin: r.origin,
            direction: r.displacement,
            max_parameter: 1.,
        };
        while let Some((face, previous, depth)) = stack.pop() {
            let tested = first_oracle(&self.triangles[face..=face], ray, true);
            if tested != Oracle::Miss {
                return tested;
            }
            if depth <= 0 {
                continue;
            }
            let source = self.flags[face].unwrap_or(0);
            let prefix = self.neighbors[face]
                .iter()
                .take_while(|n| n.is_some())
                .flatten()
                .copied()
                .collect::<Vec<_>>();
            for n in prefix.into_iter().rev() {
                let destination = self.flags[n].unwrap_or(0);
                if Some(n) != previous
                    && (destination & 2 == 0 || source & 2 != 0)
                    && (source & 16 == 0 || destination & 8 == 0)
                {
                    stack.push((n, Some(face), depth - 1));
                }
            }
        }
        Oracle::Miss
    }
}
#[derive(Debug, PartialEq)]
enum Oracle {
    Miss,
    Hit(usize),
    Ambiguous,
}
fn observed(hit: Option<ge_core::navigation::SurfaceHit>) -> Oracle {
    hit.map_or(Oracle::Miss, |h| Oracle::Hit(h.triangle))
}
fn first_oracle(
    triangles: &[SurfaceTriangle],
    ray: ge_core::navigation::SurfaceRay,
    closed: bool,
) -> Oracle {
    let origin = ray.origin.map(f64::from);
    let direction = ray.direction.map(f64::from);
    for t in triangles {
        let p = t.vertices.map(|v| v.map(f64::from));
        let a = sub(p[1], p[0]);
        let b = sub(p[2], p[0]);
        let n = [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        let norm = dot(n, n).sqrt();
        if norm * norm <= f64::from(f32::EPSILON) {
            continue;
        } // Native small-plane fallback X is parallel to these vertical probes.
        let denom = dot(n, direction);
        if denom.abs() / norm < 1e-10 {
            continue;
        }
        let parameter = dot(n, sub(p[0], origin)) / denom;
        if parameter < -1e-6 || parameter > f64::from(ray.max_parameter) + 1e-6 {
            continue;
        }
        let relative = sub(
            std::array::from_fn(|i| origin[i] + parameter * direction[i]),
            p[0],
        );
        let aa = dot(a, a);
        let bb = dot(b, b);
        let ab = dot(a, b);
        let ar = dot(a, relative);
        let br = dot(b, relative);
        let det = aa * bb - ab * ab;
        let u = (bb * ar - ab * br) / det;
        let v = (aa * br - ab * ar) / det;
        if u < -1e-6 || v < -1e-6 || u + v > 1. + 1e-6 {
            continue;
        }
        if parameter.abs() < 1e-6
            || (parameter - f64::from(ray.max_parameter)).abs() < 1e-6
            || u.abs() < 1e-6
            || v.abs() < 1e-6
            || (u + v - 1.).abs() < 1e-6
        {
            return Oracle::Ambiguous;
        }
        if parameter > 0.
            && (parameter < f64::from(ray.max_parameter)
                || closed && parameter == f64::from(ray.max_parameter))
        {
            return Oracle::Hit(t.index);
        }
    }
    Oracle::Miss
}
