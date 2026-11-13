use super::*;
impl FirstPerson {
    pub(crate) fn pose_fixture(animator: Animator, character: CharacterBinding) -> Self {
        Self {
            animator,
            character,
            skin_file: 0x010a_0002,
            visual: None,
            stage: FirstPersonStage::Returned,
            refresh: None,
        }
    }
}
