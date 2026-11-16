//! Test-only access to a retained skeletal runtime's real commit leaf.
use super::*;
use crate::animation::skeleton::LocalPose;

/// Commits authored locals through the actual skeleton commit operation.
pub(crate) fn commit_pose(runtime: &mut SkeletonRuntime, locals: &[LocalPose]) -> Result<usize> {
    let bones = runtime.skeleton.bones().len();
    runtime.skeleton.commit_local_pose(locals, None, bones)
}
