//! Authored-frame alternate wrapper checks; no recorded actors or original execution.
use super::*;
#[derive(Default, Serialize)]
pub(super) struct Audit {
    requests: usize,
    wrapper_calls: usize,
    start_misses: usize,
    search_misses: usize,
    found: usize,
    waypoint_paths: usize,
    portals_checked: usize,
    retained_start_misses: usize,
    recovery_replacements: usize,
    round_trip_changes: usize,
    max_cost_error: f64,
}
impl Audit {
    pub(super) fn include(&mut self, s: Self) {
        self.requests += s.requests;
        self.wrapper_calls += s.wrapper_calls;
        self.start_misses += s.start_misses;
        self.search_misses += s.search_misses;
        self.found += s.found;
        self.waypoint_paths += s.waypoint_paths;
        self.portals_checked += s.portals_checked;
        self.retained_start_misses += s.retained_start_misses;
        self.recovery_replacements += s.recovery_replacements;
        self.round_trip_changes += s.round_trip_changes;
        self.max_cost_error = self.max_cost_error.max(s.max_cost_error);
    }
}
impl PathData {
    pub(super) fn probe_world_goal(
        &self,
        ordinary: WorldPathRequest,
        goal: usize,
        expected_start: Option<WorldEndpoint>,
    ) -> Result<Audit> {
        let request = WorldGoalPathRequest {
            frame: ordinary.frame,
            start: ordinary.start,
            max_expansions: ordinary.max_expansions,
        };
        let mut state = NavigationRouteState::default();
        let point = self.centers[goal];
        let query = self.regenerate_goal_route(&mut state, request, &[goal], point, false)?;
        ensure!(
            serde_json::to_value(query.result.start_endpoint)?
                == serde_json::to_value(expected_start)?,
            "predicate wrapper changed the ordinary start lookup"
        );
        let mut audit = Audit {
            requests: 1,
            wrapper_calls: 1,
            ..Audit::default()
        };
        let Some(start) = query.result.start_endpoint else {
            self.validate_endpoint_miss(request.frame, request.start)?;
            ensure!(
                !query.success && query.result.path_search.is_none() && state.status_raw() == 6,
                "predicate start miss searched or changed status"
            );
            audit.start_misses = 1;
            return Ok(audit);
        };
        let reachable = self.reachable_faces(start.local_hit.triangle).0;
        ensure!(
            query.success == reachable[goal],
            "predicate world outcome disagrees with independent reachability"
        );
        if !query.success {
            ensure!(
                state.status_raw() == 10
                    && query.result.selected_goal.is_none()
                    && query.waypoints.is_none(),
                "predicate exhaustion did not preserve alternate missing-goal status"
            );
            audit.search_misses = 1;
            return Ok(audit);
        }
        let selected = query
            .result
            .selected_goal
            .context("predicate success has no chosen goal")?;
        ensure!(
            selected.triangle == goal
                && selected.raw_center == point
                && selected.local_waypoint_goal == request.frame.position_to_local(point)?
                && selected.returned_world_goal
                    == request
                        .frame
                        .position_to_world(selected.local_waypoint_goal)?,
            "predicate goal coordinate rule changed"
        );
        let route = query
            .result
            .path_search
            .as_ref()
            .and_then(|p| p.search.path.as_ref())
            .context("predicate success has no chain")?;
        audit.max_cost_error = self.validate_route(route, start.local_hit.triangle, goal)?;
        let points = query
            .waypoints
            .context("predicate success has no waypoints")?;
        audit.portals_checked = self.validate_waypoints(
            WorldPathRequest {
                frame: request.frame,
                start: request.start,
                goal: point,
                max_expansions: request.max_expansions,
            },
            route,
            &points,
        )?;
        ensure!(
            state.status_raw() == 0
                && state.points() == points.world_points
                && points.local.points.last() == Some(&selected.local_waypoint_goal)
                && points.world_points.last() == Some(&selected.returned_world_goal),
            "predicate retained/returned waypoint goal changed"
        );
        let old_points = state.points().to_vec();
        let old_triangles = state.triangles().to_vec();
        let max_y = self
            .vertices
            .iter()
            .map(|v| v[1])
            .fold(f32::NEG_INFINITY, f32::max);
        let outside = request.frame.position_to_world([0., max_y + 1000., 0.])?;
        let origin = request.frame.endpoint_ray(outside)?.origin;
        ensure!(
            f64::from(origin[1]) - f64::from(max_y) > 3.,
            "authored predicate miss is not independently above the vertical hit window"
        );
        let miss = self.regenerate_goal_route(
            &mut state,
            WorldGoalPathRequest {
                start: outside,
                ..request
            },
            &[goal],
            point,
            false,
        )?;
        ensure!(
            !miss.success
                && miss.result.start_endpoint.is_none()
                && state.status_raw() == 6
                && state.points() == old_points
                && state.triangles() == old_triangles,
            "predicate start miss did not retain its successful route"
        );
        let recovered = self.regenerate_goal_route(&mut state, request, &[goal], point, false)?;
        ensure!(
            recovered.success
                && state.status_raw() == 0
                && state.points() == old_points
                && state.triangles() == old_triangles,
            "predicate route recovery changed the original successful result"
        );
        audit.found = 1;
        audit.wrapper_calls += 2;
        audit.waypoint_paths = 1;
        audit.retained_start_misses = 1;
        audit.recovery_replacements = 1;
        audit.round_trip_changes = usize::from(selected.returned_world_goal != selected.raw_center);
        Ok(audit)
    }
}
