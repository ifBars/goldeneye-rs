//! Authored predicates + independent topology/cost/heuristic checks, not actual actor goals.
use super::*;
#[derive(Default, Serialize)]
pub(super) struct Audit {
    queries: usize,
    found: usize,
    multiple_goal_queries: usize,
    goal_tests: usize,
    expanded: usize,
    replacements: usize,
    reopened: usize,
    heuristic_centers: usize,
    max_cost_error: f64,
    max_heuristic_error: f64,
}
impl Audit {
    pub(super) fn include(&mut self, s: Self) {
        self.queries += s.queries;
        self.found += s.found;
        self.multiple_goal_queries += s.multiple_goal_queries;
        self.goal_tests += s.goal_tests;
        self.expanded += s.expanded;
        self.replacements += s.replacements;
        self.reopened += s.reopened;
        self.heuristic_centers += s.heuristic_centers;
        self.max_cost_error = self.max_cost_error.max(s.max_cost_error);
        self.max_heuristic_error = self.max_heuristic_error.max(s.max_heuristic_error);
    }
}
impl PathData {
    pub(super) fn probe_goal_heuristic(&self) -> Result<Audit> {
        let mut audit = Audit::default();
        for &center in &self.centers {
            let expected = center
                .into_iter()
                .map(|v| f64::from(v).powi(2))
                .sum::<f64>();
            let actual = ge_core::navigation::squared_goal_heuristic(center, [0.; 3])?;
            let error = (f64::from(actual) - expected).abs();
            ensure!(
                error <= 5e-6 * (1. + expected),
                "squared point heuristic disagrees with independent f64 check"
            );
            audit.heuristic_centers += 1;
            audit.max_heuristic_error = audit.max_heuristic_error.max(error);
        }
        Ok(audit)
    }
    pub(super) fn probe_goal_paths(&self, start: usize, goal: usize) -> Result<Audit> {
        let mut audit = Audit::default();
        let neighbor = self.neighbors[start]
            .iter()
            .take_while(|n| n.is_some())
            .flatten()
            .copied()
            .find(|&n| n != start)
            .unwrap_or(goal);
        let mut multi = vec![neighbor];
        if neighbor != goal {
            multi.push(goal);
        }
        for accepted in [vec![goal], multi] {
            let result = self.query_goal(
                GoalPathSearch {
                    start_triangle: start,
                    max_expansions: 100_000,
                },
                &accepted,
                self.centers[goal],
                false,
            )?;
            ensure!(
                result.goal_tests == result.search.expanded,
                "alternate goal callback count differs from removed records"
            );
            let route =
                result.search.path.as_ref().context(
                    "predicate search missed an independently reachable acceptable face",
                )?;
            let end = *route.triangles.last().context("predicate path is empty")?;
            ensure!(
                accepted.contains(&end),
                "predicate search returned an unaccepted face"
            );
            let error = self.validate_route(route, start, end)?;
            audit.queries += 1;
            audit.found += 1;
            audit.multiple_goal_queries += usize::from(accepted.len() > 1);
            audit.goal_tests += result.goal_tests;
            audit.expanded += result.search.expanded;
            audit.replacements += result.search.replacements;
            audit.reopened += result.search.reopened;
            audit.max_cost_error = audit.max_cost_error.max(error);
        }
        Ok(audit)
    }
}
