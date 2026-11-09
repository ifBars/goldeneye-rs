//! Stateful ordinary-route probes using authored requests, not captured actor movement.
use super::*;
#[derive(Default, Serialize)]
pub(super) struct Audit {
    samples: usize,
    calls: usize,
    successes: usize,
    retained_misses: usize,
    miss_positions_checked: usize,
    final_pair_replacements: usize,
}
impl Audit {
    pub(super) fn include(&mut self, other: Self) {
        self.samples += other.samples;
        self.calls += other.calls;
        self.successes += other.successes;
        self.retained_misses += other.retained_misses;
        self.miss_positions_checked += other.miss_positions_checked;
        self.final_pair_replacements += other.final_pair_replacements;
    }
}
impl PathData {
    pub(super) fn probe_regeneration(
        &self,
        request: WorldPathRequest,
        chain: &TrianglePath,
        points: &WorldWaypoints,
    ) -> Result<Audit> {
        let mut state = NavigationRouteState::default();
        let first = self.regenerate_route(&mut state, request)?;
        ensure!(
            first.success
                && state.status_raw() == 0
                && state.triangles() == chain.triangles
                && state.points() == points.world_points,
            "initial persistent route disagrees with checked world route"
        );
        let old_triangles = state.triangles().to_vec();
        let old_points = state.points().to_vec();
        let outside = std::array::from_fn(|i| {
            self.vertices
                .iter()
                .map(|v| v[i])
                .fold(f32::NEG_INFINITY, f32::max)
                + 1000.
        });
        let outside = request.frame.position_to_world(outside)?;
        // Independent all-face miss check; do not classify a speculative position as a miss.
        self.validate_endpoint_miss(request.frame, outside)?;
        for (start, goal, status) in [
            (outside, request.goal, 6),
            (request.start, outside, 10),
            (outside, outside, 6),
        ] {
            let miss = self.regenerate_route(
                &mut state,
                WorldPathRequest {
                    start,
                    goal,
                    ..request
                },
            )?;
            ensure!(
                !miss.success && miss.waypoints.is_none() && state.status_raw() == status,
                "persistent route miss classification changed"
            );
            ensure!(
                state.triangles() == old_triangles && state.points() == old_points,
                "ordinary miss discarded/replaced retained route"
            );
        }
        let restored = self.regenerate_route(&mut state, request)?;
        ensure!(
            restored.success
                && state.status_raw() == 0
                && state.triangles() == old_triangles
                && state.points() == old_points,
            "restored request did not replace its retained route"
        );
        let final_pair = self.regenerate_route(
            &mut state,
            WorldPathRequest {
                start: request.goal,
                ..request
            },
        )?;
        ensure!(
            final_pair.success
                && state.status_raw() == 0
                && state.triangles().len() == 1
                && state.points() == [request.goal, request.goal],
            "same-endpoint request did not rebuild a duplicate final pair"
        );
        Ok(Audit {
            samples: 1,
            calls: 6,
            successes: 3,
            retained_misses: 3,
            miss_positions_checked: 1,
            final_pair_replacements: 1,
        })
    }
}
