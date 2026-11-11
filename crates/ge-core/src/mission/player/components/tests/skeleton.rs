use super::*;
use crate::animation::skeleton::{BoneInput, ROOT_PARENT};
use crate::mission::player::components::skeleton::{DEFAULT_SKIN, TP_COMPONENT};

fn pending() -> Preparation {
    // Authored boundary snapshot; this does not stand in for testing the earlier
    // reset/resource providers. Public entry still requires their stage chain.
    let mut owner = Preparation::new();
    owner.stage = Stage::AwaitingSkeleton;
    owner.animator = Some(Animator::from_constructor(PlayerId::new(7).unwrap()));
    owner
}
fn bones() -> [BoneInput; 2] {
    [
        BoneInput {
            parent: ROOT_PARENT,
            range_end: 2,
            auxiliary_raw: [9, 10],
        },
        BoneInput {
            parent: 0,
            range_end: 1,
            auxiliary_raw: [11, 12],
        },
    ]
}
fn bind(
    owner: &mut Preparation,
    bones: &[BoneInput],
    auxiliary_words: usize,
    counter: Option<u32>,
    budget: SkeletonBudget,
) -> Result<()> {
    owner.bind_default_skeleton(
        SkeletonInput {
            primary: DEFAULT_SKIN,
            bones,
            auxiliary_words,
            captured_frame_counter: counter,
        },
        TP_COMPONENT,
        budget,
    )
}
fn state(owner: &Preparation) -> Vec<u32> {
    let mut words = Vec::new();
    owner.visit_state_words(|word| words.push(word));
    words
}

#[test]
fn primary_secondary_and_backlink_are_real_owned_state_before_refresh() {
    let mut owner = pending();
    bind(&mut owner, &bones(), 16, None, SkeletonBudget::default()).unwrap();
    assert_eq!(owner.stage(), Stage::AwaitingAnimationRefresh);
    let animator = owner.animator().unwrap();
    let runtime = animator.skeleton().unwrap();
    assert_eq!(runtime.animator(), Some(animator.player()));
    assert_eq!(runtime.primary(), DEFAULT_SKIN);
    assert_eq!(runtime.secondary(), Some(DEFAULT_SKIN));
    assert_eq!(animator.component(), Some(TP_COMPONENT));
    assert_eq!(animator.word(0x114), None);
    assert_eq!(animator.word(0x1c), None);
    assert_eq!(animator.word(0x20), Some(0));
    assert_eq!(animator.word(0x24), Some(u32::MAX));
    assert_eq!(animator.word(0x138), Some(0));
    assert_eq!(animator.word(0x13c), Some(0));
    assert_eq!(animator.byte(0x130), Some(1));
    assert_eq!(runtime.auxiliary(), &[0; 16]);
    assert_eq!(runtime.native_storage_bytes(), 2 * 0x88 + 16 * 4);
    assert_eq!(runtime.skeleton().roots(), &[0]);
    assert_eq!(runtime.skeleton().bones()[0].children(), &[1]);
    assert_eq!(runtime.skeleton().bones()[1].auxiliary_raw(), [11, 12]);
    assert_eq!(
        runtime.skeleton().bones()[1].local_translation_bits(),
        [0, 0, 0, 1f32.to_bits()]
    );
    assert!(runtime.frame_stamp_bits().is_err());
    // A successful prefix is deliberately not a successful native bind return.
    assert!(
        bind(
            &mut owner,
            &bones(),
            16,
            Some(42),
            SkeletonBudget::default()
        )
        .is_err()
    );
    assert!(
        owner
            .animator()
            .unwrap()
            .skeleton()
            .unwrap()
            .frame_stamp_bits()
            .is_err()
    );
}

#[test]
fn constructor_stamp_captures_wrapping_counter_once_and_affects_private_digest() {
    let mut zero = pending();
    let mut known = pending();
    let mut unknown = pending();
    bind(&mut zero, &bones(), 0, Some(0), SkeletonBudget::default()).unwrap();
    bind(&mut known, &bones(), 0, Some(42), SkeletonBudget::default()).unwrap();
    bind(&mut unknown, &bones(), 0, None, SkeletonBudget::default()).unwrap();
    assert_eq!(
        zero.animator()
            .unwrap()
            .skeleton()
            .unwrap()
            .frame_stamp_bits()
            .unwrap(),
        u32::MAX
    );
    assert_eq!(
        known
            .animator()
            .unwrap()
            .skeleton()
            .unwrap()
            .frame_stamp_bits()
            .unwrap(),
        41
    );
    assert_ne!(state(&zero), state(&known));
    assert_ne!(state(&known), state(&unknown));
}

#[test]
fn bind_requires_finished_texture_requests_and_is_not_replayed_after_failure() {
    let mut owner = pending();
    owner.stage = Stage::AwaitingTextureResidency;
    let before = state(&owner);
    assert!(bind(&mut owner, &bones(), 0, None, SkeletonBudget::default()).is_err());
    assert_eq!(state(&owner), before);
    owner.stage = Stage::AwaitingSkeleton;
    let invalid = [BoneInput {
        parent: 0,
        range_end: 0,
        auxiliary_raw: [0; 2],
    }];
    assert!(bind(&mut owner, &invalid, 0, None, SkeletonBudget::default()).is_err());
    assert_eq!(owner.stage(), Stage::Failed);
    assert!(owner.animator().unwrap().skeleton().is_none());
    assert_eq!(owner.animator().unwrap().word(0x114), Some(0));
    let reached = state(&owner);
    assert!(bind(&mut owner, &bones(), 0, None, SkeletonBudget::default()).is_err());
    assert_eq!(state(&owner), reached);
}

#[test]
fn budget_or_reference_failures_never_install_a_ready_skeleton() {
    for case in 0..5 {
        let mut owner = pending();
        let mut budget = SkeletonBudget::default();
        let mut primary = DEFAULT_SKIN;
        let mut component = TP_COMPONENT;
        match case {
            0 => budget.bones = 1,
            1 => budget.parent_visits = 1,
            2 => budget.auxiliary_words = 15,
            3 => primary.reference = 0x0d00_0002,
            _ => component.file ^= 1,
        }
        assert!(
            owner
                .bind_default_skeleton(
                    SkeletonInput {
                        primary,
                        bones: &bones(),
                        auxiliary_words: 16,
                        captured_frame_counter: None
                    },
                    component,
                    budget
                )
                .is_err()
        );
        assert!(owner.animator().unwrap().skeleton().is_none());
        assert_eq!(owner.stage(), Stage::Failed);
    }
}
