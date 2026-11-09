//! Persistent route replacement from the world-query wrapper's two branches, not point popping.
use super::{TrianglePath, WorldGoalPathResult, WorldPathResult};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

#[derive(Debug)]
pub enum NavigationRouteSearch {
    MissingStart,
    MissingGoal,
    NoRoute,
    Found(TrianglePath),
}
impl NavigationRouteSearch {
    /// Alternate exhaustion has source wrapper status 0xa, unlike ordinary no-route 0x2.
    pub fn from_world_goal(result: &WorldGoalPathResult) -> Result<Self> {
        if result.start_endpoint.is_none() {
            ensure!(
                result.path_search.is_none() && result.selected_goal.is_none(),
                "missing predicate start unexpectedly searched/selected a goal"
            );
            return Ok(Self::MissingStart);
        }
        let search = &result
            .path_search
            .as_ref()
            .context("grounded predicate request has no search result")?
            .search;
        match (&search.path, result.selected_goal) {
            (Some(path), Some(goal)) => {
                ensure!(
                    path.triangles.last() == Some(&goal.triangle),
                    "predicate selected goal disagrees with returned path"
                );
                Ok(Self::Found(path.clone()))
            }
            (None, None) => Ok(Self::MissingGoal),
            _ => anyhow::bail!("inconsistent predicate route/selected goal"),
        }
    }
    /// Missing-start takes priority when both endpoint lookups miss.
    pub fn from_world(result: &WorldPathResult) -> Result<Self> {
        if result.start_endpoint.is_none() || result.goal_endpoint.is_none() {
            ensure!(
                result.path_search.is_none(),
                "missing endpoints unexpectedly ran route search"
            );
            return Ok(if result.start_endpoint.is_none() {
                Self::MissingStart
            } else {
                Self::MissingGoal
            });
        }
        let search = result
            .path_search
            .as_ref()
            .context("grounded route request has no search result")?;
        Ok(match &search.path {
            Some(path) => Self::Found(path.clone()),
            None => Self::NoRoute,
        })
    }
}

#[derive(Debug, Default, Serialize)]
pub struct NavigationRouteState {
    status_raw: u32,
    triangles: Vec<usize>,
    world_points: Vec<[f32; 3]>,
}
impl NavigationRouteState {
    pub fn status_raw(&self) -> u32 {
        self.status_raw
    }
    pub fn triangles(&self) -> &[usize] {
        &self.triangles
    }
    pub fn points(&self) -> &[[f32; 3]] {
        &self.world_points
    }

    /// Reset status for each wrapper call. Ordinary misses retain BOTH old lists.
    /// On success replace triangles, then clear/rebuild points. Host build errors
    /// leave the new chain and cleared points, rather than restoring an invented route.
    /// An inactive/acquisition-only actor dispatch must not invoke this operation.
    pub fn regenerate(
        &mut self,
        mesh_present: bool,
        mut search: impl FnMut() -> Result<NavigationRouteSearch>,
        mut build: impl FnMut(&TrianglePath) -> Result<Vec<[f32; 3]>>,
    ) -> Result<bool> {
        self.status_raw = 0;
        if !mesh_present {
            self.status_raw = 1;
            return Ok(false);
        }
        let path = match search()? {
            NavigationRouteSearch::MissingStart => {
                self.status_raw = 6;
                return Ok(false);
            }
            NavigationRouteSearch::MissingGoal => {
                self.status_raw = 10;
                return Ok(false);
            }
            NavigationRouteSearch::NoRoute => {
                self.status_raw = 2;
                return Ok(false);
            }
            NavigationRouteSearch::Found(path) => path,
        };
        ensure!(
            !path.triangles.is_empty(),
            "empty successful navigation chain"
        );
        self.triangles.clone_from(&path.triangles);
        self.world_points.clear();
        let points = build(&path)?;
        ensure!(
            points.len() >= 2 && points.iter().flatten().all(|p| p.is_finite()),
            "invalid rebuilt navigation points"
        );
        self.world_points = points;
        Ok(true)
    }
}

#[cfg(test)]
#[path = "../tests/route_state.rs"]
mod tests;
