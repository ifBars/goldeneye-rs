//! Observed actor goal/edge search with explicit raw policy inputs; no actor simulation.
use super::*;
use ge_core::navigation::{
    ActorGoalContext, ActorGoalDecision, ActorGoalMesh, ActorGoalResult, actor_target_goal,
};

#[derive(Default, Serialize)]
pub struct GoalTests {
    pub too_far: usize,
    pub normal_rejections: usize,
    pub neighbor_rejections: usize,
    pub accepted: usize,
    pub walked_faces: usize,
    pub edge_tests: usize,
}
#[derive(Serialize)]
pub struct ActorGoalPath {
    pub result: GoalPathResult,
    pub goal_tests: GoalTests,
    pub last_center: Option<[f32; 3]>,
    pub last_goal_test: Option<ActorGoalResult>,
}
impl PathData {
    pub(super) fn evaluate_actor_goal(
        &self,
        context: &mut ActorGoalContext,
        candidate: usize,
        max_face_steps: usize,
    ) -> Result<ActorGoalResult> {
        actor_target_goal(
            ActorGoalMesh {
                vertices: &self.vertices,
                triangles: &self.vertex_ids,
                centers: &self.centers,
                neighbors: &self.neighbors,
                flags: &self.flags,
                normals: &self.normals,
            },
            context,
            candidate,
            max_face_steps,
        )
    }
    pub fn query_actor_path(
        &self,
        request: GoalPathSearch,
        origin: [f32; 3],
        point: [f32; 3],
        max_face_steps: usize,
    ) -> Result<ActorGoalPath> {
        ensure!(
            max_face_steps > 0,
            "actor goal face budget must be positive"
        );
        ensure!(
            origin.into_iter().chain(point).all(f32::is_finite),
            "actor goal origin/point must be finite"
        );
        let mut context = ActorGoalContext {
            origin,
            point,
            start_triangle: request.start_triangle,
            last_center: None,
        };
        let mut tests = GoalTests::default();
        let mut last = None;
        let result = select_triangle_path_to_goal_with_point_and_edge_policy(
            request,
            PathGraph {
                neighbors: &self.neighbors,
                centers: &self.centers,
                weights: &self.weights,
            },
            point,
            |face| {
                let tested = self.evaluate_actor_goal(&mut context, face, max_face_steps)?;
                match tested.decision {
                    ActorGoalDecision::TooFar { .. } => tests.too_far += 1,
                    ActorGoalDecision::NormalY { .. } => tests.normal_rejections += 1,
                    ActorGoalDecision::NoNeighbor { .. } => tests.neighbor_rejections += 1,
                    ActorGoalDecision::Accepted => tests.accepted += 1,
                }
                tests.walked_faces += tested.walked_faces.len();
                tests.edge_tests += tested.edge_tests;
                let accepted = tested.accepted();
                last = Some(tested);
                Ok(accepted)
            },
            |edge| self.goal_edge_allowed(edge, point, true),
        )?;
        Ok(ActorGoalPath {
            result,
            goal_tests: tests,
            last_center: context.last_center,
            last_goal_test: last,
        })
    }
}
