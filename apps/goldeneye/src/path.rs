//! Checked path data preparation and sampled diagnostics. Original code is never executed.
use anyhow::{Context, Result, ensure};
use ge_assets::{graph::TriangleGraph, resource::GameAssets};
use ge_core::navigation::{ActorEdgeDecision, ActorEdgeMesh, TraversalEdge, actor_target_edge};
use ge_core::navigation::{
    BoundaryCache, BoundaryHit, BoundaryMesh, BoundaryStats, NavigationFrame, PathGraph,
    PathSearch, PathSearchResult, SurfaceTriangle, TrianglePath, WaypointLimits, WaypointMesh,
    WaypointResult, WaypointRoute, WorldEndpoint, WorldPathRequest, WorldPathResult,
    build_boundary_cache, build_waypoints, select_triangle_path, select_world_triangle_path,
    surface_centroid,
};
use ge_core::navigation::{
    GoalPathResult, GoalPathSearch, select_triangle_path_to_goal_with_point_and_edge_policy,
};
use ge_core::navigation::{NavigationRouteSearch, NavigationRouteState};
use ge_core::navigation::{
    WorldGoalPathRequest, WorldGoalPathResult,
    select_world_triangle_path_to_goal_with_point_and_edge_policy,
};
use serde::Serialize;
use std::{collections::VecDeque, path::Path};

pub struct PathData {
    bounds: [[f32; 3]; 2],
    neighbors: Vec<[Option<usize>; 3]>,
    centers: Vec<[f32; 3]>,
    weights: Vec<Option<[f32; 3]>>,
    triangles: Vec<SurfaceTriangle>,
    vertices: Vec<[f32; 3]>,
    vertex_ids: Vec<[usize; 3]>,
    flags: Vec<Option<u32>>,
    normals: Vec<Option<[f32; 3]>>,
    boundary: BoundaryCache,
}

#[derive(Serialize)]
pub struct WorldWaypoints {
    pub local: WaypointResult,
    pub world_points: Vec<[f32; 3]>,
    pub next_world_target: Option<[f32; 3]>,
    pub length_3d: f32,
}
pub struct RegeneratedRoute {
    pub result: WorldPathResult,
    pub waypoints: Option<WorldWaypoints>,
    pub success: bool,
}
pub struct RegeneratedGoalRoute {
    pub result: WorldGoalPathResult,
    pub waypoints: Option<WorldWaypoints>,
    pub success: bool,
}
impl PathData {
    pub fn world_goal_query(
        &self,
        request: WorldGoalPathRequest,
        accepted: &[usize],
        point: [f32; 3],
        actor_edges: bool,
    ) -> Result<WorldGoalPathResult> {
        ensure!(
            !accepted.is_empty() && accepted.iter().all(|&f| f < self.neighbors.len()),
            "acceptable world goal face set is empty or out of bounds"
        );
        select_world_triangle_path_to_goal_with_point_and_edge_policy(
            request,
            &self.triangles,
            PathGraph {
                neighbors: &self.neighbors,
                centers: &self.centers,
                weights: &self.weights,
            },
            point,
            |face| Ok(accepted.contains(&face)),
            |edge| self.goal_edge_allowed(edge, point, actor_edges),
        )
    }
    pub fn regenerate_goal_route(
        &self,
        state: &mut NavigationRouteState,
        request: WorldGoalPathRequest,
        accepted: &[usize],
        point: [f32; 3],
        actor_edges: bool,
    ) -> Result<RegeneratedGoalRoute> {
        let (mut result, mut waypoints) = (None, None);
        let success = state.regenerate(
            true,
            || {
                let queried = self.world_goal_query(request, accepted, point, actor_edges)?;
                let outcome = NavigationRouteSearch::from_world_goal(&queried)?;
                result = Some(queried);
                Ok(outcome)
            },
            |route| {
                let face = *route
                    .triangles
                    .last()
                    .context("empty predicate chain for waypoints")?;
                let raw_center = *self
                    .centers
                    .get(face)
                    .context("predicate waypoint center out of bounds")?;
                let generated = self.waypoints(
                    WorldPathRequest {
                        frame: request.frame,
                        start: request.start,
                        goal: raw_center,
                        max_expansions: request.max_expansions,
                    },
                    route,
                    WaypointLimits {
                        max_shortcut_tests: 100_000,
                        max_fan_steps: 1_000_000,
                    },
                )?;
                let points = generated.world_points.clone();
                waypoints = Some(generated);
                Ok(points)
            },
        )?;
        Ok(RegeneratedGoalRoute {
            result: result.context("prepared predicate route did not query")?,
            waypoints,
            success,
        })
    }
    pub fn query_goal(
        &self,
        request: GoalPathSearch,
        accepted: &[usize],
        point: [f32; 3],
        actor_edges: bool,
    ) -> Result<GoalPathResult> {
        ensure!(
            !accepted.is_empty() && accepted.iter().all(|&f| f < self.neighbors.len()),
            "acceptable goal face set is empty or out of bounds"
        );
        select_triangle_path_to_goal_with_point_and_edge_policy(
            request,
            PathGraph {
                neighbors: &self.neighbors,
                centers: &self.centers,
                weights: &self.weights,
            },
            point,
            |face| Ok(accepted.contains(&face)),
            |edge| self.goal_edge_allowed(edge, point, actor_edges),
        )
    }
    fn actor_edge_decision(
        &self,
        edge: TraversalEdge,
        point: [f32; 3],
    ) -> Result<ActorEdgeDecision> {
        actor_target_edge(
            ActorEdgeMesh {
                neighbors: &self.neighbors,
                centers: &self.centers,
                flags: &self.flags,
                normals: &self.normals,
            },
            edge,
            point,
        )
    }
    fn goal_edge_allowed(
        &self,
        edge: TraversalEdge,
        point: [f32; 3],
        actor_edges: bool,
    ) -> Result<bool> {
        if actor_edges {
            Ok(self.actor_edge_decision(edge, point)?.allowed())
        } else {
            Ok(Some(edge.to_triangle) != edge.previous_triangle)
        }
    }
    pub fn regenerate_route(
        &self,
        state: &mut NavigationRouteState,
        request: WorldPathRequest,
    ) -> Result<RegeneratedRoute> {
        let (mut result, mut waypoints) = (None, None);
        let success = state.regenerate(
            true,
            || {
                let queried = self.world_query(request)?;
                let outcome = NavigationRouteSearch::from_world(&queried)?;
                result = Some(queried);
                Ok(outcome)
            },
            |route| {
                let generated = self.waypoints(
                    request,
                    route,
                    WaypointLimits {
                        max_shortcut_tests: 100_000,
                        max_fan_steps: 1_000_000,
                    },
                )?;
                let points = generated.world_points.clone();
                waypoints = Some(generated);
                Ok(points)
            },
        )?;
        Ok(RegeneratedRoute {
            result: result.context("prepared route did not query")?,
            waypoints,
            success,
        })
    }
    pub fn new(graph: &TriangleGraph) -> Result<Self> {
        let mut centers = Vec::new();
        centers.try_reserve_exact(graph.triangles.len())?;
        let mut triangles = Vec::new();
        triangles.try_reserve_exact(graph.triangles.len())?;
        for (index, triangle) in graph.triangles.iter().enumerate() {
            let mut vertices = [[0.; 3]; 3];
            for (i, &v) in triangle.vertices.iter().enumerate() {
                vertices[i] = *graph
                    .vertices
                    .get(v as usize)
                    .context("path vertex out of bounds")?;
            }
            let surface = SurfaceTriangle { index, vertices };
            let center = if let Some(center) = triangle.center {
                center
            } else {
                surface_centroid(surface)?
            };
            centers.push(center);
            triangles.push(surface);
        }
        let neighbors: Vec<_> = graph
            .triangles
            .iter()
            .map(|t| t.neighbors.map(|n| n.map(|n| n as usize)))
            .collect();
        ensure!(
            neighbors
                .iter()
                .flatten()
                .all(|n| n.is_none_or(|n| n < centers.len())),
            "path neighbor out of bounds"
        );
        let weights = graph.triangles.iter().map(|t| t.weights).collect();
        let vertices = graph.vertices.clone();
        let vertex_ids: Vec<_> = graph
            .triangles
            .iter()
            .map(|t| t.vertices.map(|v| v as usize))
            .collect();
        let flags: Vec<_> = graph.triangles.iter().map(|t| t.flags_raw).collect();
        let boundary = build_boundary_cache(BoundaryMesh {
            vertex_count: vertices.len(),
            faces: &vertex_ids,
            neighbors: &neighbors,
            flags: &flags,
        })?;
        Ok(Self {
            bounds: graph.bounds,
            neighbors,
            centers,
            weights,
            triangles,
            vertices,
            vertex_ids,
            flags,
            normals: graph.triangles.iter().map(|t| t.normal).collect(),
            boundary,
        })
    }
    pub fn boundary_stats(&self) -> &BoundaryStats {
        self.boundary.stats()
    }
    pub fn boundary_check(&self, start: [f32; 3], end: [f32; 3]) -> Result<Option<BoundaryHit>> {
        self.boundary.first_blocker(&self.vertices, start, end)
    }
    pub fn instance(&self, frame: NavigationFrame) -> ge_core::navigation::NavigationInstance<'_> {
        ge_core::navigation::NavigationInstance {
            frame,
            bounds: self.bounds,
            triangles: &self.triangles,
            centers: &self.centers,
        }
    }
    pub fn waypoints(
        &self,
        request: WorldPathRequest,
        route: &TrianglePath,
        limits: WaypointLimits,
    ) -> Result<WorldWaypoints> {
        let start = request.frame.position_to_local(request.start)?;
        let goal = request.frame.position_to_local(request.goal)?;
        let local = build_waypoints(
            WaypointMesh {
                vertices: &self.vertices,
                triangles: &self.vertex_ids,
                centers: &self.centers,
                neighbors: &self.neighbors,
                flags: &self.flags,
            },
            &route.triangles,
            start,
            goal,
            limits,
        )?;
        let world_points = local
            .points
            .iter()
            .map(|&p| request.frame.position_to_world(p))
            .collect::<Result<Vec<_>>>()?;
        let consumer = WaypointRoute::new(&world_points)?;
        let next_world_target = consumer.target();
        let length_3d = consumer.length()?;
        Ok(WorldWaypoints {
            local,
            world_points,
            next_world_target,
            length_3d,
        })
    }
    pub fn world_query(&self, request: WorldPathRequest) -> Result<WorldPathResult> {
        select_world_triangle_path(
            request,
            &self.triangles,
            PathGraph {
                neighbors: &self.neighbors,
                centers: &self.centers,
                weights: &self.weights,
            },
        )
    }
    fn reachable_faces(&self, start: usize) -> (Vec<bool>, usize) {
        let mut seen = vec![false; self.neighbors.len()];
        let mut queue = VecDeque::from([start]);
        seen[start] = true;
        let mut goal = start;
        while let Some(face) = queue.pop_front() {
            goal = face;
            for next in self.neighbors[face]
                .iter()
                .take_while(|n| n.is_some())
                .flatten()
                .copied()
            {
                if !seen[next] {
                    seen[next] = true;
                    queue.push_back(next);
                }
            }
        }
        (seen, goal)
    }
    pub fn query(&self, request: PathSearch) -> Result<PathSearchResult> {
        select_triangle_path(
            request,
            PathGraph {
                neighbors: &self.neighbors,
                centers: &self.centers,
                weights: &self.weights,
            },
        )
    }
    fn probe(&self) -> Result<Stats> {
        let mut stats = Stats {
            actor_edges: self.probe_actor_edges()?,
            actor_goals: self.probe_actor_goals()?,
            actor_targets: self.probe_actor_targets()?,
            goals: self.probe_goal_heuristic()?,
            boundary: self.probe_boundary()?,
            ..Stats::default()
        };
        let stride = self.neighbors.len().div_ceil(8).max(1);
        for start in (0..self.neighbors.len()).step_by(stride).take(8) {
            // Independent unweighted reachability chooses a distant reachable goal.
            // It is a sample generator, not a shortest-cost/reference comparator.
            let (_, goal) = self.reachable_faces(start);
            let found = self.query(PathSearch {
                start_triangle: start,
                goal_triangle: goal,
                max_expansions: 100_000,
            })?;
            stats.queries += 1;
            stats.expanded += found.expanded;
            stats.replacements += found.replacements;
            stats.reopened += found.reopened;
            stats.peak_open = stats.peak_open.max(found.peak_open);
            let route = found
                .path
                .context("path search missed an independently reachable goal")?;
            let error = self.validate_route(&route, start, goal)?;
            stats.max_cost_error = stats.max_cost_error.max(error);
            stats.longest_chain = stats.longest_chain.max(route.triangles.len());
            stats.same_face_queries += usize::from(start == goal);
            stats.found += 1;
            stats.goals.include(self.probe_goal_paths(start, goal)?);
            // Authored translation exercises the coordinate binding; it is not a
            // recovered placement, spawn, or actual actor request.
            let frame = NavigationFrame {
                translation: [11., -7., 5.],
            };
            let request = WorldPathRequest {
                frame,
                start: std::array::from_fn(|i| self.centers[start][i] + frame.translation[i]),
                goal: std::array::from_fn(|i| self.centers[goal][i] + frame.translation[i]),
                max_expansions: 100_000,
            };
            let world = self.world_query(request)?;
            stats.world_goals.include(self.probe_world_goal(
                request,
                goal,
                world.start_endpoint,
            )?);
            let dispatch = self.probe_dispatch(request, &world).with_context(||format!("actor request sample start face {start}, goal {goal}, position {:?}, bounds {:?}",request.start,self.bounds))?;
            stats.actor_dispatch_calls += 2;
            stats.actor_acquisition_calls += dispatch.acquisitions;
            stats.actor_query_calls += dispatch.queries;
            stats.actor_query_successes += dispatch.successes;
            stats.actor_acquisition_misses += dispatch.misses;
            stats.actor_bounds_rejections += dispatch.bounds_rejections;
            stats.actor_near_surface_excluded += dispatch.near_surface_excluded;
            stats.world_queries += 1;
            for (endpoint, position) in [
                (world.start_endpoint, request.start),
                (world.goal_endpoint, request.goal),
            ] {
                if let Some(endpoint) = endpoint {
                    if let Some(error) = self.validate_endpoint(endpoint)? {
                        stats.world_max_plane_error = stats.world_max_plane_error.max(error);
                    } else {
                        stats.world_small_plane_samples += 1;
                    }
                } else {
                    self.validate_endpoint_miss(frame, position)?;
                }
            }
            match (world.start_endpoint, world.goal_endpoint) {
                (Some(start), Some(goal)) => {
                    stats.world_grounded_pairs += 1;
                    let (start, goal) = (start.local_hit.triangle, goal.local_hit.triangle);
                    let (reachable, _) = self.reachable_faces(start);
                    let search = world
                        .path_search
                        .context("grounded world request did not run search")?;
                    ensure!(
                        search.path.is_some() == reachable[goal],
                        "world route disagrees with independent reachability"
                    );
                    if let Some(route) = search.path {
                        let error = self.validate_route(&route, start, goal)?;
                        stats.world_max_cost_error = stats.world_max_cost_error.max(error);
                        stats.world_paths_found += 1;
                        let waypoints = self.waypoints(
                            request,
                            &route,
                            WaypointLimits {
                                max_shortcut_tests: 100_000,
                                max_fan_steps: 1_000_000,
                            },
                        )?;
                        stats
                            .regeneration
                            .include(self.probe_regeneration(request, &route, &waypoints)?);
                        let checked = self.validate_waypoints(request, &route, &waypoints)?;
                        stats.waypoint_queries += 1;
                        stats.waypoint_seed_points += waypoints.local.seed_count;
                        stats.waypoint_retained_points += waypoints.local.points.len();
                        stats.waypoint_shortcut_tests += waypoints.local.shortcut_tests;
                        stats.waypoint_fan_steps += waypoints.local.fan_steps;
                        stats.waypoint_flag_rejections += waypoints.local.flag_rejections;
                        stats.waypoint_portals_checked += checked;
                        let consumers = validate_route_consumers(&waypoints)?;
                        stats.route_consumer_paths += 1;
                        stats.route_corridor_queries += consumers.corridor_queries;
                        stats.route_corridor_validated += consumers.corridor_validated;
                        stats.route_corridor_ambiguous += consumers.corridor_ambiguous;
                        stats.route_arrival_validated += consumers.arrival_validated;
                        stats.route_max_length_error =
                            stats.route_max_length_error.max(consumers.max_length_error);
                    } else {
                        stats.world_search_misses += 1;
                    }
                }
                _ => {
                    ensure!(
                        world.path_search.is_none(),
                        "world search ran after an endpoint miss"
                    );
                    stats.world_endpoint_misses += 1;
                }
            }
            stats.world_validated += 1;
        }
        ensure!(stats.queries > 0, "no path samples");
        Ok(stats)
    }
    fn probe_dispatch(
        &self,
        request: WorldPathRequest,
        expected: &WorldPathResult,
    ) -> Result<DispatchProbe> {
        use ge_core::navigation::{NavigationDispatch, NavigationRequestState, acquire_navigation};
        let instances = [self.instance(request.frame)];
        let mut state = NavigationRequestState {
            flags_raw: 0xb00,
            binding: None,
        };
        let mut probe = DispatchProbe::default();
        for step in 0..2 {
            let event = state.dispatch(
                || {
                    probe.acquisitions += 1;
                    let result = acquire_navigation(&instances, request.start, 30.)?;
                    if let Some(selected) = &result.selected {
                        ensure!(
                            selected.instance == 0,
                            "acquisition returned an invalid candidate"
                        );
                        self.validate_endpoint(WorldEndpoint {
                            local_hit: selected.hit,
                            world_surface_position: request
                                .frame
                                .position_to_world(selected.hit.position)?,
                        })?;
                    }
                    if result.selected.is_none() {
                        probe.misses += 1;
                        let mut origin=request.frame.position_to_local(request.start)?;
                        origin[1]+=1.;
                        if result.bounds_passed == 0 {
                            let o=origin.map(f64::from);
                            let lower=self.bounds[0].map(f64::from);
                            let upper=self.bounds[1].map(f64::from);
                            ensure!(o[0]<lower[0] || o[0]>upper[0] || o[2]<lower[2] || o[2]>upper[2]
                                || o[1]<lower[1] || o[1]-30.>upper[1], "acquisition bounds rejection disagrees with independent vertical interval check");
                            probe.bounds_rejections += 1;
                            probe.near_surface_excluded += usize::from(expected.start_endpoint.is_some());
                        } else {
                            self.validate_downward_miss(origin.map(f64::from),30.)?;
                        }
                    }
                    Ok(result.selected.map(|s| s.instance))
                },
                |binding| {
                    ensure!(
                        step == 1 && binding == 0,
                        "actor query was not deferred to cached binding"
                    );
                    probe.queries += 1;
                    let replay = self.world_query(request)?;
                    ensure!(
                        serde_json::to_value(&replay)? == serde_json::to_value(expected)?,
                        "actor dispatch changed ordinary world-query behavior"
                    );
                    let success = replay
                        .path_search
                        .as_ref()
                        .is_some_and(|s| s.path.is_some());
                    probe.successes += usize::from(success);
                    Ok(success)
                },
            )?;
            ensure!(event.processed(), "pending actor request was not processed");
            if step == 0 {
                ensure!(
                    matches!(event, NavigationDispatch::Acquire { .. })
                        && state.flags_raw & 0x800 == 0,
                    "first actor phase did not only acquire/clear success"
                );
            }
        }
        ensure!(
            (state.flags_raw & 0x800 != 0) == (probe.successes > 0),
            "actor success flag disagrees with its route result"
        );
        Ok(probe)
    }
    fn validate_route(&self, route: &TrianglePath, start: usize, goal: usize) -> Result<f64> {
        ensure!(
            route.triangles.first() == Some(&start) && route.triangles.last() == Some(&goal),
            "path endpoints changed"
        );
        let mut sum = 0f64;
        for edge in route.triangles.windows(2) {
            let (from, to) = (edge[0], edge[1]);
            let slot = self.neighbors[from]
                .iter()
                .take_while(|n| n.is_some())
                .position(|&n| n == Some(to))
                .context("path contains a nonedge/boundary crossing")?;
            sum += if let Some(weights) = self.weights[from] {
                f64::from(weights[slot])
            } else {
                let squared: f64 = self.centers[from]
                    .iter()
                    .zip(self.centers[to])
                    .map(|(&a, b)| (f64::from(b) - f64::from(a)).powi(2))
                    .sum();
                if squared <= f64::from(f32::EPSILON) {
                    0.
                } else {
                    squared.sqrt()
                }
            };
        }
        let error = (sum - f64::from(route.search_cost)).abs();
        ensure!(
            error <= 1e-4 * (1. + sum.abs()),
            "path record cost disagrees with independent f64 edge sum"
        );
        Ok(error)
    }
    fn validate_endpoint(&self, endpoint: WorldEndpoint) -> Result<Option<f64>> {
        let points = self.triangles[endpoint.local_hit.triangle]
            .vertices
            .map(|p| p.map(f64::from));
        let ab: [f64; 3] = std::array::from_fn(|i| points[1][i] - points[0][i]);
        let ac: [f64; 3] = std::array::from_fn(|i| points[2][i] - points[0][i]);
        let n = [
            ab[1] * ac[2] - ab[2] * ac[1],
            ab[2] * ac[0] - ab[0] * ac[2],
            ab[0] * ac[1] - ab[1] * ac[0],
        ];
        let squared: f64 = n.iter().map(|v| v * v).sum();
        if squared <= f64::from(f32::EPSILON) {
            return Ok(None);
        }
        let error = n
            .iter()
            .enumerate()
            .map(|(i, n)| n * (f64::from(endpoint.local_hit.position[i]) - points[0][i]))
            .sum::<f64>()
            .abs()
            / squared.sqrt();
        ensure!(
            error < 1e-3,
            "world endpoint failed independent f64 plane check"
        );
        Ok(Some(error))
    }
    fn validate_endpoint_miss(&self, frame: NavigationFrame, world: [f32; 3]) -> Result<()> {
        let ray = frame.endpoint_ray(world)?;
        let origin = ray.origin.map(f64::from);
        self.validate_downward_miss(origin, 3.)
    }
    fn validate_downward_miss(&self, origin: [f64; 3], max_parameter: f64) -> Result<()> {
        for triangle in &self.triangles {
            let p = triangle.vertices.map(|p| p.map(f64::from));
            let ab = sub(p[1], p[0]);
            let ac = sub(p[2], p[0]);
            let n = cross(ab, ac);
            let squared = dot(n, n);
            // The observed small-plane fallback is parallel to this vertical ray.
            if squared <= f64::from(f32::EPSILON) {
                continue;
            }
            let normal = n.map(|v| v / squared.sqrt());
            let denominator = -normal[1];
            if denominator.abs() < 1e-10 {
                continue;
            }
            let t = -dot(normal, sub(origin, p[0])) / denominator;
            if !(0. ..max_parameter).contains(&t) {
                continue;
            }
            let hit = [origin[0], origin[1] - t, origin[2]];
            let relative = sub(hit, p[0]);
            let aa = dot(ab, ab);
            let cc = dot(ac, ac);
            let bc = dot(ab, ac);
            let determinant = aa * cc - bc * bc;
            ensure!(
                determinant > 0.,
                "world endpoint miss oracle lost geometric precision"
            );
            let u = (cc * dot(ab, relative) - bc * dot(ac, relative)) / determinant;
            let v = (aa * dot(ac, relative) - bc * dot(ab, relative)) / determinant;
            ensure!(
                !(u >= -1e-9 && v >= -1e-9 && u + v <= 1. + 1e-9),
                "world endpoint missed an independent f64 surface candidate"
            );
        }
        Ok(())
    }
    fn validate_waypoints(
        &self,
        request: WorldPathRequest,
        route: &TrianglePath,
        output: &WorldWaypoints,
    ) -> Result<usize> {
        let result = &output.local;
        let n = route.triangles.len().max(2);
        ensure!(
            result.seed_count == n
                && result.points.len() == result.retained_indices.len()
                && result.points.len() == output.world_points.len()
                && result.retained_indices.first() == Some(&0)
                && result.retained_indices.last() == Some(&(n - 1)),
            "waypoint endpoint/count invariant failed"
        );
        ensure!(
            result.retained_indices.windows(2).all(|p| p[0] < p[1]),
            "waypoint indices are not increasing"
        );
        let start = request.frame.position_to_local(request.start)?;
        let goal = request.frame.position_to_local(request.goal)?;
        for (i, &source) in result.retained_indices.iter().enumerate() {
            let expected = if source == 0 {
                start
            } else if source == n - 1 {
                goal
            } else {
                self.centers[route.triangles[source]]
            };
            ensure!(
                result.points[i] == expected
                    && output.world_points[i] == request.frame.position_to_world(expected)?,
                "waypoint is not an unchanged seed/frame position"
            );
        }
        let mut checked = 0;
        // Independent f64 XZ portal checks; this does not claim original FP/clearance parity.
        for (span, positions) in result
            .retained_indices
            .windows(2)
            .zip(result.points.windows(2))
        {
            let (s, t) = (positions[0].map(f64::from), positions[1].map(f64::from));
            for i in span[0] + 1..span[1] {
                let (from, to) = (route.triangles[i], route.triangles[i + 1]);
                ensure!(
                    self.flags[from].unwrap_or(0) & 1 == 0,
                    "shortcut crossed a blocked interior face"
                );
                let common = self.vertex_ids[from]
                    .iter()
                    .filter(|v| self.vertex_ids[to].contains(v))
                    .copied()
                    .collect::<Vec<_>>();
                if common.len() < 2 {
                    continue;
                }
                let a = self.vertices[common[0]].map(f64::from);
                let b = self.vertices[common[1]].map(f64::from);
                let d = sub(t, s);
                let e = sub(b, a);
                let p = sub(s, a);
                let determinant = d[0] * e[2] - d[2] * e[0];
                ensure!(
                    determinant.abs() > 1e-6,
                    "shortcut failed independent portal determinant check"
                );
                let x = (p[2] * e[0] - p[0] * e[2]) / determinant;
                let y = (p[2] * d[0] - p[0] * d[2]) / determinant;
                ensure!(
                    (-1e-4..=1.0001).contains(&x) && (-1e-4..=1.0001).contains(&y),
                    "shortcut failed independent f64 portal parameters"
                );
                checked += 1;
            }
        }
        Ok(checked)
    }
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[derive(Default)]
struct ConsumerChecks {
    corridor_queries: usize,
    corridor_validated: usize,
    corridor_ambiguous: usize,
    arrival_validated: usize,
    max_length_error: f64,
}

fn validate_route_consumers(points: &WorldWaypoints) -> Result<ConsumerChecks> {
    let route = WaypointRoute::new(&points.world_points)?;
    ensure!(
        points.next_world_target == points.world_points.get(1).copied(),
        "route target is not point one"
    );
    let length: f64 = points
        .world_points
        .windows(2)
        .map(|p| {
            let d = sub(p[1].map(f64::from), p[0].map(f64::from));
            dot(d, d).sqrt()
        })
        .sum();
    let error = (length - f64::from(points.length_3d)).abs();
    ensure!(
        error <= 1e-5 * (1. + length),
        "route length disagrees with independent f64 sum"
    );
    let mut checks = ConsumerChecks {
        max_length_error: error,
        ..Default::default()
    };
    let positions =
        points
            .world_points
            .iter()
            .copied()
            .chain(points.world_points.windows(2).flat_map(|p| {
                let mid: [f32; 3] = std::array::from_fn(|i| {
                    ((f64::from(p[0][i]) + f64::from(p[1][i])) * 0.5) as f32
                });
                [mid, [mid[0], mid[1] + 0.75, mid[2]]]
            }));
    for position in positions {
        let arrived = route.arrived(position)?;
        let expected = if points.world_points.len() == 2 {
            let d = sub(
                position.map(f64::from),
                points.world_points[1].map(f64::from),
            );
            d[0] * d[0] + d[2] * d[2] < 0.0625
        } else {
            false
        };
        ensure!(
            arrived == expected,
            "route arrival disagrees with independent horizontal check"
        );
        checks.arrival_validated += 1;
        for limit in [Some(1), None] {
            let actual = route.corridor_segment(position, 0.5, limit)?;
            checks.corridor_queries += 1;
            if let Some(expected) = corridor_oracle(&points.world_points, position, 0.5, limit) {
                ensure!(
                    actual == expected,
                    "route corridor disagrees with independent f64 segment-distance check"
                );
                checks.corridor_validated += 1;
            } else {
                checks.corridor_ambiguous += 1;
            }
        }
    }
    Ok(checks)
}

// Independent f64 projection/clamp oracle. Near-boundary samples are counted
// separately rather than promoted to numeric agreement with Wii or this host.
fn corridor_oracle(
    points: &[[f32; 3]],
    position: [f32; 3],
    radius: f64,
    limit: Option<usize>,
) -> Option<Option<usize>> {
    for (index, pair) in points
        .windows(2)
        .take(limit.unwrap_or(usize::MAX))
        .enumerate()
    {
        let a = pair[0].map(f64::from);
        let d = sub(pair[1].map(f64::from), a);
        let relative = sub(position.map(f64::from), a);
        let denominator = dot(d, d);
        let projection = dot(relative, d);
        let t = if denominator == 0. {
            0.
        } else {
            (projection / denominator).clamp(0., 1.)
        };
        let delta = std::array::from_fn(|i| relative[i] - t * d[i]);
        let distance = dot(delta, delta);
        if (distance - radius * radius).abs() <= 1e-4 {
            return None;
        }
        if distance >= radius * radius {
            continue;
        }
        if index == 0 {
            let magnitude: f64 = relative
                .into_iter()
                .zip(d)
                .map(|(a, b)| (a * b).abs())
                .sum();
            if projection != 0. && projection.abs() <= 1e-6 * (1. + magnitude) {
                return None;
            }
            if projection < 0. {
                continue;
            }
        }
        return Some(Some(index));
    }
    Some(None)
}

#[derive(Default)]
struct DispatchProbe {
    acquisitions: usize,
    queries: usize,
    successes: usize,
    misses: usize,
    bounds_rejections: usize,
    near_surface_excluded: usize,
}

#[derive(Default, Serialize)]
struct Stats {
    actor_targets: actor_target_audit::Audit,
    actor_goals: actor_goal_audit::Audit,
    actor_edges: actor_edge_audit::Audit,
    world_goals: world_goal_audit::Audit,
    goals: goal_path_audit::Audit,
    regeneration: regeneration_audit::Audit,
    boundary: boundary_audit::Audit,
    actor_dispatch_calls: usize,
    actor_acquisition_calls: usize,
    actor_query_calls: usize,
    actor_query_successes: usize,
    actor_acquisition_misses: usize,
    actor_bounds_rejections: usize,
    actor_near_surface_excluded: usize,
    queries: usize,
    found: usize,
    expanded: usize,
    replacements: usize,
    reopened: usize,
    peak_open: usize,
    longest_chain: usize,
    same_face_queries: usize,
    max_cost_error: f64,
    world_queries: usize,
    world_validated: usize,
    world_grounded_pairs: usize,
    world_paths_found: usize,
    world_endpoint_misses: usize,
    world_search_misses: usize,
    world_small_plane_samples: usize,
    world_max_plane_error: f64,
    world_max_cost_error: f64,
    waypoint_queries: usize,
    waypoint_seed_points: usize,
    waypoint_retained_points: usize,
    waypoint_shortcut_tests: usize,
    waypoint_fan_steps: usize,
    waypoint_flag_rejections: usize,
    waypoint_portals_checked: usize,
    route_consumer_paths: usize,
    route_corridor_queries: usize,
    route_corridor_validated: usize,
    route_corridor_ambiguous: usize,
    route_arrival_validated: usize,
    route_max_length_error: f64,
}
impl Stats {
    fn include(&mut self, s: Self) {
        self.actor_targets.include(s.actor_targets);
        self.actor_goals.include(s.actor_goals);
        self.actor_edges.include(s.actor_edges);
        self.world_goals.include(s.world_goals);
        self.goals.include(s.goals);
        self.regeneration.include(s.regeneration);
        self.boundary.include(s.boundary);
        self.actor_dispatch_calls += s.actor_dispatch_calls;
        self.actor_acquisition_calls += s.actor_acquisition_calls;
        self.actor_query_calls += s.actor_query_calls;
        self.actor_query_successes += s.actor_query_successes;
        self.actor_acquisition_misses += s.actor_acquisition_misses;
        self.actor_bounds_rejections += s.actor_bounds_rejections;
        self.actor_near_surface_excluded += s.actor_near_surface_excluded;
        self.queries += s.queries;
        self.found += s.found;
        self.expanded += s.expanded;
        self.replacements += s.replacements;
        self.reopened += s.reopened;
        self.peak_open = self.peak_open.max(s.peak_open);
        self.longest_chain = self.longest_chain.max(s.longest_chain);
        self.same_face_queries += s.same_face_queries;
        self.max_cost_error = self.max_cost_error.max(s.max_cost_error);
        self.world_queries += s.world_queries;
        self.world_validated += s.world_validated;
        self.world_grounded_pairs += s.world_grounded_pairs;
        self.world_paths_found += s.world_paths_found;
        self.world_endpoint_misses += s.world_endpoint_misses;
        self.world_search_misses += s.world_search_misses;
        self.world_small_plane_samples += s.world_small_plane_samples;
        self.world_max_plane_error = self.world_max_plane_error.max(s.world_max_plane_error);
        self.world_max_cost_error = self.world_max_cost_error.max(s.world_max_cost_error);
        self.waypoint_queries += s.waypoint_queries;
        self.waypoint_seed_points += s.waypoint_seed_points;
        self.waypoint_retained_points += s.waypoint_retained_points;
        self.waypoint_shortcut_tests += s.waypoint_shortcut_tests;
        self.waypoint_fan_steps += s.waypoint_fan_steps;
        self.waypoint_flag_rejections += s.waypoint_flag_rejections;
        self.waypoint_portals_checked += s.waypoint_portals_checked;
        self.route_consumer_paths += s.route_consumer_paths;
        self.route_corridor_queries += s.route_corridor_queries;
        self.route_corridor_validated += s.route_corridor_validated;
        self.route_corridor_ambiguous += s.route_corridor_ambiguous;
        self.route_arrival_validated += s.route_arrival_validated;
        self.route_max_length_error = self.route_max_length_error.max(s.route_max_length_error);
    }
}

#[path = "actor_action_audit.rs"]
mod actor_action_audit;
#[path = "actor_edge_audit.rs"]
mod actor_edge_audit;
#[path = "actor_goal_audit.rs"]
mod actor_goal_audit;
#[path = "actor_goal_path.rs"]
mod actor_goal_path;
#[path = "actor_target_audit.rs"]
mod actor_target_audit;
#[path = "actor_target_path.rs"]
mod actor_target_path;
#[path = "boundary_audit.rs"]
mod boundary_audit;
#[path = "goal_path_audit.rs"]
mod goal_path_audit;
#[path = "regeneration_audit.rs"]
mod regeneration_audit;
#[path = "world_goal_audit.rs"]
mod world_goal_audit;

pub fn scan(disc: &Path) -> Result<serde_json::Value> {
    let mut assets = GameAssets::open(disc, "Filelist.bin")?;
    let resources = assets
        .index
        .entries
        .iter()
        .filter(|e| e.path.to_ascii_lowercase().ends_with(".edb"))
        .map(|e| e.path.clone())
        .collect::<Vec<_>>();
    let (mut graphs, mut probed) = (0, 0);
    let mut stats = Stats::default();
    let mut errors = Vec::new();
    for (i, name) in resources.iter().enumerate() {
        if i % 100 == 0 {
            eprintln!("Path audit {i}/{}", resources.len());
        }
        let result = (|| -> Result<()> {
            let edb = assets.load_edb(name)?;
            for (index, entity) in edb.entities()?.iter().enumerate() {
                if entity.entity_type != 0x607 {
                    continue;
                }
                graphs += 1;
                let graph = edb.entity_graph_at(index)?;
                stats.include(PathData::new(&graph)?.probe()?);
                probed += 1;
            }
            Ok(())
        })();
        if let Err(error) = result {
            errors.push(serde_json::json!({"resource":name,"error":format!("{error:#}")}));
        }
    }
    Ok(
        serde_json::json!({"disc":assets.disc.identity,"resources":resources.len(),"graphs":graphs,"probed":probed,"samples_per_graph_limit":8,"stats":stats,"errors":errors,"reference_compared":false,
        "scope":"Sampled explicit/predicate mesh and world paths, authored frames, waypoints, consumers, dispatch, persistent routes, boundaries, actor bands and optional target-directed edges. Predicate goals/points are authored. Actor edge f64 checks classify near-threshold ambiguity separately and preserve absent stored normals. Alternate world checks preserve start-only lookup, raw selected-center subtraction/addition, round-trip changes, retained start misses and recovery. Independent topology/cost/portal/reachability checks are host diagnostics, not original actor inputs or execution. Full actor goals, scheduling, mutable state, Wii precision, heap reuse, clearance and movement remain unverified."}),
    )
}

#[cfg(test)]
#[path = "tests/path.rs"]
mod tests;
