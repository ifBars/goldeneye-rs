//! First/connected surface binding and observed actor policy search, before action mutation.
use super::actor_goal_path::ActorGoalPath;
use super::*;
use ge_core::navigation::{
    ActorTargetBinding, ActorTargetMesh, ActorTargetRequest, ActorTargetSelection,
    prepare_actor_target,
};
#[derive(Serialize)]
pub struct ActorTargetSearch {
    pub binding: ActorTargetBinding,
    pub search: Option<ActorGoalPath>,
}
impl ActorTargetSearch {
    pub fn selection(&self) -> Result<Option<ActorTargetSelection>> {
        let Some(search) = &self.search else {
            return Ok(None);
        };
        if search.result.search.path.is_none() {
            return Ok(None);
        }
        let raw_center = search
            .last_center
            .context("successful actor query missing scratch center")?;
        let frame = self
            .binding
            .prepared
            .context("successful actor query missing prepared frame")?
            .frame;
        Ok(Some(ActorTargetSelection { raw_center, frame }))
    }
}
impl PathData {
    pub(super) fn prepare_actor_target(
        &self,
        request: ActorTargetRequest,
        resolve_frame: impl FnOnce() -> Result<Option<NavigationFrame>>,
    ) -> Result<ActorTargetBinding> {
        prepare_actor_target(
            request,
            Some(ActorTargetMesh {
                triangles: &self.triangles,
                neighbors: &self.neighbors,
                flags: &self.flags,
            }),
            resolve_frame,
        )
    }
    pub fn query_actor_target(
        &self,
        request: ActorTargetRequest,
        resolve_frame: impl FnOnce() -> Result<Option<NavigationFrame>>,
        max_expansions: usize,
        max_goal_steps: usize,
    ) -> Result<ActorTargetSearch> {
        ensure!(
            max_expansions > 0 && max_goal_steps > 0,
            "actor target search budgets must be positive"
        );
        let binding = self.prepare_actor_target(request, resolve_frame)?;
        let search = if let Some(p) = binding.prepared {
            Some(self.query_actor_path(
                GoalPathSearch {
                    start_triangle: p.start_triangle,
                    max_expansions,
                },
                p.origin,
                p.point,
                max_goal_steps,
            )?)
        } else {
            None
        };
        Ok(ActorTargetSearch { binding, search })
    }
}
