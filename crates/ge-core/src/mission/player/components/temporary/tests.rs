use super::*;
use crate::{animation::skeleton::BoneInput, mission::player::identity::PlayerId};
mod evaluation;
mod skin_prepare;
struct MissingBody {
    fail_generation: bool,
}
impl script_child::Provider for MissingBody {
    fn file(&self) -> u32 {
        0x0102_0016
    }
    fn generation(&mut self) -> Result<u32> {
        ensure!(!self.fail_generation, "synthetic provider unavailable");
        Ok(4)
    }
    fn resolve_script(&mut self, _: u32) -> Result<Option<script_child::Binding>> {
        Ok(None)
    }
    fn bind_body(&mut self, _: script_child::Binding) -> Result<bool> {
        panic!("unresolved body must not bind")
    }
}
impl script_child::forward_tick::Source for MissingBody {
    fn duration(&mut self, _: script_child::Binding) -> Result<u32> {
        panic!("absent body duration")
    }
    fn fps_bits(&mut self, _: script_child::Binding) -> Result<u32> {
        panic!("absent body FPS")
    }
}
fn bones() -> [BoneInput; 1] {
    [BoneInput {
        parent: u16::MAX,
        range_end: 1,
        auxiliary_raw: [0; 2],
    }]
}
fn input(bones: &[BoneInput]) -> SkeletonInput<'_> {
    SkeletonInput {
        primary: ResourceBinding {
            file: 0x010a_0037,
            reference: 0x0d00_016f,
        },
        bones,
        auxiliary_words: 0,
        captured_frame_counter: None,
    }
}
fn fixture(reference: u32) -> (Registry, Animator, Pending) {
    let mut registry = Registry::from_constructor(8);
    let identity = registry.allocate(PlayerId::new(1).unwrap()).unwrap();
    let mut fp = Animator::from_constructor(identity);
    fp.bind_temporary_skeleton(
        input(&bones()),
        ResourceBinding {
            file: 0x0102_0016,
            reference: 0x8400_0022,
        },
        SkeletonBudget::default(),
    )
    .unwrap();
    let pending = Pending::begin(&fp, &mut registry, reference).unwrap();
    (registry, fp, pending)
}
fn words(owner: &Animator) -> Vec<u32> {
    let mut words = Vec::new();
    owner.visit_state_words(|w| words.push(w));
    words
}
#[test]
fn zero_reference_still_owns_new_animator_skeleton_and_shared_refresh() {
    let (registry, fp, mut pending) = fixture(0);
    let before = words(&fp);
    assert_eq!(registry.len(), 2);
    assert_ne!(pending.animator().identity(), fp.identity());
    assert_eq!(pending.skin(), fp.skeleton().unwrap().secondary().unwrap());
    pending
        .bind_skeleton(input(&bones()), SkeletonBudget::default())
        .unwrap();
    assert_eq!(pending.animator().item(), None);
    assert!(pending.animator().callback_table().is_none());
    assert!(
        pending
            .animator()
            .skeleton()
            .unwrap()
            .pose_callback()
            .is_none()
    );
    assert!(
        pending
            .animator()
            .skeleton()
            .unwrap()
            .frame_stamp_bits()
            .is_err()
    );
    let mut source = MissingBody {
        fail_generation: true,
    };
    pending.initialize_factory(&mut source).unwrap();
    pending.forward_zero(&mut source).unwrap();
    let mut context = scratch::Context::default();
    let mut arena = scratch::Arena::from_constructor(19).unwrap();
    let earlier = context.refresh_empty(&mut arena).unwrap();
    pending
        .finish_bind_zero_time(&mut context, &mut arena)
        .unwrap();
    assert_eq!(pending.bind_return(), Some(1));
    assert!(pending.refresh().unwrap().follows_in_same_arena(&earlier));
    assert_eq!(arena.live_reservations(), 0);
    assert_eq!(words(&fp), before);
}
#[test]
fn direct_script_factory_and_zero_tick_stop_before_nonempty_sampling() {
    let (_, fp, mut pending) = fixture(0x8400_0007);
    let before = words(&fp);
    pending
        .bind_skeleton(input(&bones()), SkeletonBudget::default())
        .unwrap();
    let mut source = MissingBody {
        fail_generation: false,
    };
    pending.initialize_factory(&mut source).unwrap();
    assert_eq!(pending.stage(), Stage::AwaitingInsertion);
    assert_eq!(pending.head(), None);
    let header = pending.factory_header().unwrap();
    assert_eq!(header.word(0x20), Some(3));
    assert_eq!(header.slot(), Some(0));
    assert!(header.initializer_returned());
    pending.finish_factory().unwrap();
    let head = pending.head().unwrap();
    let links = pending.animator().node_ids().links(head).unwrap();
    assert_eq!(links.slot, 0);
    assert_eq!(links.previous, None);
    assert_eq!(links.next, None);
    assert_eq!(pending.animator().word(0x138), None);
    assert_eq!(pending.animator().word(0x13c), None);
    assert_eq!(pending.animator().byte(0x130), Some(1));
    pending.forward_zero(&mut source).unwrap();
    assert!(pending.maintenance_skipped());
    assert!(pending.reverse_skipped());
    assert_eq!(pending.factory_header().unwrap().word(0x84), Some(0));
    let mut context = scratch::Context::default();
    let mut arena = scratch::Arena::from_constructor(20).unwrap();
    pending
        .finish_bind_zero_time(&mut context, &mut arena)
        .unwrap();
    assert_eq!(pending.stage(), Stage::AwaitingEvaluation);
    assert_eq!(pending.bind_return(), None);
    assert_eq!(pending.dependency(), Some(Dependency::Evaluation));
    assert!(pending.refresh().is_none());
    assert_eq!(words(&fp), before);
}
#[test]
fn forward_provider_failure_preserves_owned_child_and_reached_zero_writes() {
    let (_, _, mut pending) = fixture(0x0400_0007);
    pending
        .bind_skeleton(input(&bones()), SkeletonBudget::default())
        .unwrap();
    let mut source = MissingBody {
        fail_generation: false,
    };
    pending.initialize_factory(&mut source).unwrap();
    pending.finish_factory().unwrap();
    source.fail_generation = true;
    assert!(pending.forward_zero(&mut source).is_err());
    assert_eq!(pending.stage(), Stage::Failed);
    assert!(pending.head().is_some());
    assert_eq!(pending.factory_header().unwrap().word(0x84), Some(0));
    assert!(pending.animator().skeleton().is_some());
    assert_eq!(pending.bind_return(), None);
}
#[test]
fn wrong_skin_or_registry_cannot_substitute_another_owner() {
    let (_, fp, mut pending) = fixture(0);
    assert!(Pending::begin(&fp, &mut Registry::from_constructor(8), 0).is_err());
    let bone_data = bones();
    let mut wrong = input(&bone_data);
    wrong.primary.reference += 1;
    assert!(
        pending
            .bind_skeleton(wrong, SkeletonBudget::default())
            .is_err()
    );
    assert_eq!(pending.stage(), Stage::AwaitingSkeleton);
    assert!(pending.animator().skeleton().is_none());
}

struct PendingBody;
impl script_child::Provider for PendingBody {
    fn file(&self) -> u32 {
        0x0102_0016
    }
    fn generation(&mut self) -> Result<u32> {
        Ok(4)
    }
    fn resolve_script(&mut self, _: u32) -> Result<Option<script_child::Binding>> {
        Ok(Some(script_child::Binding {
            directory_index: 7,
            hash: 0x0400_00ab,
        }))
    }
    fn bind_body(&mut self, _: script_child::Binding) -> Result<bool> {
        panic!("unresolved publication must not become a NULL body")
    }
    fn body_state(&mut self, _: script_child::Binding) -> Result<script_child::BodyState> {
        Ok(script_child::BodyState::Pending)
    }
}
#[test]
fn unknown_section_body_keeps_temporary_skeleton_and_cached_descriptor_pending() {
    let (_, fp, mut pending) = fixture(0x8400_0007);
    let before = words(&fp);
    pending
        .bind_skeleton(input(&bones()), SkeletonBudget::default())
        .unwrap();
    pending.initialize_factory(&mut PendingBody).unwrap();
    assert_eq!(pending.stage(), Stage::AwaitingInitializer);
    assert_eq!(pending.dependency(), Some(Dependency::ScriptBodyResidency));
    assert_eq!(
        pending
            .factory_header()
            .unwrap()
            .cached()
            .unwrap()
            .directory_index,
        7
    );
    assert_eq!(pending.factory_header().unwrap().body_bound(), None);
    assert!(pending.animator().skeleton().is_some());
    assert!(!pending.factory_returned());
    assert!(pending.head().is_none());
    assert!(pending.forward_tick().is_none());
    assert_eq!(words(&fp), before);
}

struct NoPlay;
impl script_child::Provider for NoPlay {
    fn file(&self) -> u32 {
        0x0102_0016
    }
    fn generation(&mut self) -> Result<u32> {
        Ok(2)
    }
    fn resolve_script(&mut self, _: u32) -> Result<Option<script_child::Binding>> {
        Ok(Some(script_child::Binding {
            directory_index: 3,
            hash: 7,
        }))
    }
    fn bind_body(&mut self, _: script_child::Binding) -> Result<bool> {
        Ok(true)
    }
}
impl script_child::initialization::Source for NoPlay {
    fn file(&self) -> u32 {
        0x0102_0016
    }
    fn duration(&mut self) -> Result<u32> {
        Ok(30)
    }
    fn entry(&mut self, _: u32) -> Result<script_child::initialization::Entry> {
        Ok(script_child::initialization::Entry {
            start: 0,
            opcode: 0x12,
            next: 1,
        })
    }
    fn counts(&mut self, _: u32) -> Result<[u8; 2]> {
        panic!("END has no clip counts")
    }
    fn file_token(&mut self, _: u32, _: bool, _: usize, _: u8) -> Result<u32> {
        panic!("END has no file")
    }
    fn lookup_file(&mut self, _: u32) -> Result<script_child::initialization::FileLookup> {
        panic!("END has no lookup")
    }
    fn reference(&mut self, _: u32, _: bool, _: usize, _: u8) -> Result<u32> {
        panic!("END has no reference")
    }
    fn blend(&mut self, _: u32, _: usize) -> Result<u32> {
        panic!("END has no blend")
    }
    fn start(&mut self, _: u32, _: bool, _: usize, _: u8) -> Result<i16> {
        panic!("END has no start")
    }
    fn end(&mut self, _: u32, _: bool, _: usize, _: u8) -> Result<i16> {
        panic!("END has no range")
    }
    fn initial_index(&mut self, _: u32) -> Result<u8> {
        panic!("END has no index")
    }
    fn flags(&mut self, _: u32) -> Result<u8> {
        panic!("END has no flags")
    }
}
impl script_child::forward_tick::Source for NoPlay {
    fn duration(&mut self, _: script_child::Binding) -> Result<u32> {
        Ok(30)
    }
    fn fps_bits(&mut self, _: script_child::Binding) -> Result<u32> {
        Ok(30f32.to_bits())
    }
}
#[test]
fn actual_phase_zero_no_play_return_precedes_direct_insertion() {
    let (_, _, mut pending) = fixture(0x0400_0007);
    pending
        .bind_skeleton(input(&bones()), SkeletonBudget::default())
        .unwrap();
    pending.initialize_factory(&mut NoPlay).unwrap();
    assert_eq!(pending.stage(), Stage::AwaitingInitializer);
    assert!(pending.finish_factory().is_err());
    pending.initialize_phase_zero(&mut NoPlay, 0, 32).unwrap();
    assert_eq!(
        pending.factory_header().unwrap().stage(),
        script_child::Stage::ReturnedWithoutPlay
    );
    pending.finish_factory().unwrap();
    pending.forward_zero(&mut NoPlay).unwrap();
    assert_eq!(pending.stage(), Stage::AwaitingEvaluation);
    assert_eq!(pending.forward_tick().unwrap().duration_raw(), Some(30));
    assert_eq!(pending.bind_return(), None);
}
