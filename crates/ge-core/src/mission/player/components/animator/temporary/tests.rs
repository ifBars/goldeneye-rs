use super::*;
use crate::{
    animation::skeleton::BoneInput,
    mission::player::components::{Registry, temporary::Pending},
};
#[test]
fn captured_secondary_pair_is_not_replaced_by_primary_or_live_reference() {
    let mut registry = Registry::from_constructor(8);
    let mut fp = Animator::from_constructor(registry.allocate(PlayerId::new(1).unwrap()).unwrap());
    let bones = [BoneInput {
        parent: u16::MAX,
        range_end: 1,
        auxiliary_raw: [0; 2],
    }];
    let primary = ResourceBinding {
        file: 0x010a_0001,
        reference: 0x0d00_0001,
    };
    let secondary = ResourceBinding {
        file: 0x010a_0002,
        reference: 0x0d00_0002,
    };
    // Authored differing binding fixture; no original payload or runtime claim.
    fp.bind_temporary_skeleton(
        SkeletonInput {
            primary,
            bones: &bones,
            auxiliary_words: 0,
            captured_frame_counter: None,
        },
        ResourceBinding {
            file: 0x0102_0016,
            reference: 0x8400_0022,
        },
        SkeletonBudget::default(),
    )
    .unwrap();
    fp.skeleton.as_mut().unwrap().bind_secondary(secondary);
    let mut pending = Pending::begin(&fp, &mut registry, 0x8400_0007).unwrap();
    assert_eq!(pending.skin(), secondary);
    assert_eq!(pending.component().reference, 0x8400_0007);
    pending
        .bind_skeleton(
            SkeletonInput {
                primary: secondary,
                bones: &bones,
                auxiliary_words: 0,
                captured_frame_counter: None,
            },
            SkeletonBudget::default(),
        )
        .unwrap();
    assert_eq!(pending.animator().skeleton().unwrap().primary(), secondary);
    assert_eq!(
        pending.animator().skeleton().unwrap().secondary(),
        Some(secondary)
    );
    assert_eq!(fp.skeleton().unwrap().primary(), primary);
    assert_eq!(fp.component().unwrap().reference, 0x8400_0022);
}
