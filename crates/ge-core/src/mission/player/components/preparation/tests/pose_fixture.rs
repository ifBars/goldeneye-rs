use super::*;
use crate::mission::player::{
    components::{FirstPerson, Registry},
    constructor::CharacterBinding,
};
impl Preparation {
    pub(crate) fn pose_fixture(
        player: PlayerId,
        item: &mut Item,
        character: CharacterBinding,
    ) -> Self {
        let mut registry = Registry::from_constructor(4);
        let tp = Animator::pose_fixture(
            registry.allocate(player).unwrap(),
            item.id(),
            ResourceBinding {
                file: 0x0102_0017,
                reference: 0x8400_0021,
            },
        );
        let fp = Animator::pose_fixture(
            registry.allocate(player).unwrap(),
            item.id(),
            ResourceBinding {
                file: 0x0102_0016,
                reference: 0x8400_0022,
            },
        );
        item.attach_component(tp.identity().id(), 2).unwrap();
        item.attach_component(fp.identity().id(), 2).unwrap();
        let mut prep = Self::new();
        prep.stage = Stage::AwaitingInitialModes;
        prep.player = Some(player);
        prep.item = Some(item.id());
        prep.animator = Some(tp);
        prep.first_person = Some(FirstPerson::pose_fixture(fp, character));
        prep
    }
}
