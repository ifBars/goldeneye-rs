use super::*;
use crate::animation::{
    scratch::evaluation::{Branch, Stage as EvaluationStage},
    script_child::{
        ClipPointer,
        evaluation::{NO_REFERENCE, Outcome},
    },
};

// Authored synthetic skin publication; no original skin or animation payload.
struct Skin {
    lookups: Vec<SkinLookup>,
    bodies: Vec<SkinBody>,
    generation: u32,
    seen: Vec<ResourceBinding>,
    calls: Vec<&'static str>,
}
impl SkinSource for Skin {
    fn lookup(&mut self, primary: ResourceBinding) -> Result<SkinLookup> {
        self.calls.push("lookup");
        self.seen.push(primary);
        Ok(self.lookups.remove(0))
    }
    fn generation(&mut self, _: ResourceBinding) -> Result<u32> {
        self.calls.push("generation");
        Ok(self.generation)
    }
    fn body(&mut self, descriptor: SkinDescriptor) -> Result<SkinBody> {
        self.calls.push("body");
        ensure!(descriptor.generation == 9, "authored descriptor differs");
        Ok(self.bodies.remove(0))
    }
}
fn descriptor(primary: ResourceBinding) -> SkinDescriptor {
    SkinDescriptor {
        file: primary.file,
        reference: primary.reference,
        generation: 9,
    }
}
fn skin(lookups: Vec<SkinLookup>, bodies: Vec<SkinBody>) -> Skin {
    Skin {
        lookups,
        bodies,
        generation: 9,
        seen: Vec::new(),
        calls: Vec::new(),
    }
}
fn resident() -> Skin {
    let primary = input(&bones()).primary;
    skin(
        vec![SkinLookup::Present(descriptor(primary))],
        vec![SkinBody::Resident],
    )
}
/// Fresh NULL-body factory, actual insertion and zero-time forward return.
fn awaiting(auxiliary_words: usize) -> (Animator, Pending) {
    let (_, fp, mut pending) = fixture(0x8400_0007);
    let bone_data = bones();
    let mut skeleton = input(&bone_data);
    skeleton.auxiliary_words = auxiliary_words;
    pending
        .bind_skeleton(skeleton, SkeletonBudget::default())
        .unwrap();
    let mut source = MissingBody {
        fail_generation: false,
    };
    pending.initialize_factory(&mut source).unwrap();
    pending.finish_factory().unwrap();
    pending.forward_zero(&mut source).unwrap();
    assert_eq!(pending.stage(), Stage::AwaitingEvaluation);
    (fp, pending)
}
fn shared() -> (scratch::Context, scratch::Arena) {
    (
        scratch::Context::default(),
        scratch::Arena::from_constructor(21).unwrap(),
    )
}

#[test]
fn constructor_sentinel_layer_returns_bind_after_actual_reservation_release() {
    let (fp, mut pending) = awaiting(0);
    let before = words(&fp);
    let (mut context, mut arena) = shared();
    let earlier = context.refresh_empty(&mut arena).unwrap();
    let mut source = resident();
    pending
        .evaluate_bind_zero_time(&mut context, &mut arena, &mut source)
        .unwrap();
    assert_eq!(pending.stage(), Stage::BindReturned);
    assert_eq!(pending.bind_return(), Some(1));
    assert_eq!(pending.dependency(), None);
    let head = pending.head().unwrap();
    let evaluation = pending.evaluation().unwrap();
    assert!(evaluation.follows_refresh(&earlier));
    assert!(evaluation.released());
    assert_eq!(evaluation.live_after(), Some(0));
    assert_eq!(evaluation.slots().len(), 1);
    let slot = &evaluation.slots()[0];
    assert_eq!(slot.weight_bits, Some(1f32.to_bits()));
    assert_eq!(slot.branch, Some(Branch::Nonempty(head)));
    assert!(slot.prepared && slot.masks_cleared);
    assert_eq!(slot.nodes, [(head, 0)]);
    assert_eq!(evaluation.nonempty_layers(), 1);
    assert_eq!(evaluation.successful_samples(), 0);
    assert_eq!(
        (evaluation.blended_layers(), evaluation.committed()),
        (0, false)
    );
    assert_eq!(context.flags(), Some(0));
    assert_eq!(context.masks(), &[[Some(0); 4]; 2]);
    assert_eq!(context.holder(), None);
    assert!(!pending.evaluation_lease().unwrap().is_live().unwrap());
    // Guard: constructor inline MAX, so Animator+20 stayed unread.
    let guard = pending.node_guards();
    assert_eq!(guard.len(), 1);
    assert_eq!(guard[0].pointer, ClipPointer::ConstructorInline);
    assert_eq!(guard[0].clip_reference, Some(NO_REFERENCE));
    assert!(!guard[0].animator_reference_read);
    assert_eq!(guard[0].outcome, Outcome::Returned(0));
    // Skin: the actual temporary primary, one lookup, one body read, aux0.
    let prepare = &pending.skin_prepares()[0];
    assert_eq!(
        source.seen,
        [pending.animator().skeleton().unwrap().primary()]
    );
    assert_eq!((prepare.lookups(), prepare.body_reads()), (1, 1));
    assert_eq!(prepare.returned(), Some(prepare::Return::AuxiliaryZero));
    assert_eq!(prepare.auxiliary_words(), Some(0));
    // Forward history and the live FP receiver stay unchanged.
    assert_eq!(pending.forward_tick().unwrap().body_bound(), Some(false));
    assert_eq!(words(&fp), before);
    assert!(
        pending
            .evaluate_bind_zero_time(&mut context, &mut arena, &mut resident())
            .is_err()
    );
}

#[test]
fn pending_lookup_retains_reserved_prefix_and_resumes_at_the_same_point() {
    let (_, mut pending) = awaiting(0);
    let (mut context, mut arena) = shared();
    let primary = input(&bones()).primary;
    let mut source = skin(
        vec![
            SkinLookup::Pending,
            SkinLookup::Present(descriptor(primary)),
        ],
        vec![SkinBody::Resident],
    );
    pending
        .evaluate_bind_zero_time(&mut context, &mut arena, &mut source)
        .unwrap();
    assert_eq!(pending.stage(), Stage::EvaluatingBind);
    assert_eq!(pending.dependency(), Some(Dependency::EvaluationSkin));
    assert_eq!(pending.bind_return(), None);
    let lease = pending.evaluation_lease().unwrap();
    assert!(lease.is_live().unwrap());
    assert_eq!(context.holder(), Some(lease.ordinal()));
    assert_eq!(context.masks(), &[[None; 4]; 2]);
    assert!(pending.node_guards().is_empty());
    assert!(context.refresh_empty(&mut arena).is_err());
    pending
        .evaluate_bind_zero_time(&mut context, &mut arena, &mut source)
        .unwrap();
    assert_eq!(pending.bind_return(), Some(1));
    assert_eq!(source.calls, ["lookup", "lookup", "body"]);
    assert_eq!(pending.skin_prepares().len(), 1);
    assert_eq!(arena.live_reservations(), 0);
}

#[test]
fn changed_generation_after_body_pause_latches_until_explicit_abort() {
    let (_, mut pending) = awaiting(0);
    let (mut context, mut arena) = shared();
    let primary = input(&bones()).primary;
    let mut source = skin(
        vec![SkinLookup::Present(descriptor(primary))],
        vec![SkinBody::Pending],
    );
    pending
        .evaluate_bind_zero_time(&mut context, &mut arena, &mut source)
        .unwrap();
    assert_eq!(pending.dependency(), Some(Dependency::EvaluationSkin));
    source.generation = 10;
    assert!(
        pending
            .evaluate_bind_zero_time(&mut context, &mut arena, &mut source)
            .is_err()
    );
    assert_eq!(pending.stage(), Stage::Failed);
    assert_eq!(source.calls, ["lookup", "body", "generation"]);
    assert_eq!(arena.live_reservations(), 1);
    pending.abort_evaluation(&mut context, &mut arena).unwrap();
    assert_eq!(pending.stage(), Stage::Aborted);
    assert_eq!((arena.live_reservations(), context.holder()), (0, None));
    assert_eq!(pending.bind_return(), None);
    assert!(
        pending
            .evaluate_bind_zero_time(&mut context, &mut arena, &mut resident())
            .is_err()
    );
}

#[test]
fn host_reaped_lease_is_observed_by_the_abandoned_owner_without_second_release() {
    let (_, mut pending) = awaiting(0);
    let (mut context, mut arena) = shared();
    let mut source = skin(vec![SkinLookup::Pending], Vec::new());
    pending
        .evaluate_bind_zero_time(&mut context, &mut arena, &mut source)
        .unwrap();
    let lease = pending.evaluation_lease().unwrap();
    assert!(lease.abort(&mut context, &mut arena).unwrap());
    assert_eq!(pending.stage(), Stage::Aborted);
    assert_eq!(
        pending.evaluation().unwrap().stage(),
        EvaluationStage::Aborted
    );
    assert_eq!(pending.dependency(), None);
    // A fresh attempt may now use the shared context and arena.
    context.refresh_empty(&mut arena).unwrap();
    let state = (arena.cursor(), arena.live_reservations(), context.holder());
    assert!(
        pending
            .evaluate_bind_zero_time(&mut context, &mut arena, &mut resident())
            .is_err()
    );
    pending.abort_evaluation(&mut context, &mut arena).unwrap();
    assert_eq!(
        (arena.cursor(), arena.live_reservations(), context.holder()),
        state
    );
    assert_eq!(pending.bind_return(), None);
}

#[test]
fn absent_descriptor_and_absent_body_are_distinct_known_returns() {
    for (lookup, body, expected) in [
        (
            SkinLookup::Absent,
            Vec::new(),
            prepare::Return::DescriptorAbsent,
        ),
        (
            SkinLookup::Present(descriptor(input(&bones()).primary)),
            vec![SkinBody::Absent],
            prepare::Return::BodyAbsent,
        ),
    ] {
        let (_, mut pending) = awaiting(0);
        let (mut context, mut arena) = shared();
        let mut source = skin(vec![lookup], body);
        pending
            .evaluate_bind_zero_time(&mut context, &mut arena, &mut source)
            .unwrap();
        assert_eq!(pending.bind_return(), Some(1));
        let prepare = &pending.skin_prepares()[0];
        assert_eq!(prepare.returned(), Some(expected));
        assert_eq!(prepare.auxiliary_words(), None);
    }
}

#[test]
fn nonzero_auxiliary_or_foreign_descriptor_never_completes() {
    let (_, mut pending) = awaiting(1);
    let (mut context, mut arena) = shared();
    pending
        .evaluate_bind_zero_time(&mut context, &mut arena, &mut resident())
        .unwrap();
    assert_eq!(pending.dependency(), Some(Dependency::EvaluationAuxiliary));
    assert_eq!(context.masks(), &[[None; 4]; 2]);
    pending.abort_evaluation(&mut context, &mut arena).unwrap();

    let (_, mut pending) = awaiting(0);
    let mut foreign = descriptor(input(&bones()).primary);
    foreign.reference += 1;
    let mut source = skin(vec![SkinLookup::Present(foreign)], Vec::new());
    assert!(
        pending
            .evaluate_bind_zero_time(&mut context, &mut arena, &mut source)
            .is_err()
    );
    assert_eq!(pending.stage(), Stage::Failed);
    assert!(source.calls.iter().all(|call| *call != "body"));
    pending.abort_evaluation(&mut context, &mut arena).unwrap();
    assert_eq!(arena.live_reservations(), 0);
}

#[test]
fn empty_direct_list_keeps_the_existing_refresh_path() {
    let (_, _, mut pending) = fixture(0);
    pending
        .bind_skeleton(input(&bones()), SkeletonBudget::default())
        .unwrap();
    let mut source = MissingBody {
        fail_generation: true,
    };
    pending.initialize_factory(&mut source).unwrap();
    pending.forward_zero(&mut source).unwrap();
    let (mut context, mut arena) = shared();
    assert!(
        pending
            .evaluate_bind_zero_time(&mut context, &mut arena, &mut resident())
            .is_err()
    );
    assert_eq!(pending.stage(), Stage::AwaitingEvaluation);
    assert!(pending.evaluation().is_none());
    pending
        .finish_bind_zero_time(&mut context, &mut arena)
        .unwrap();
    assert_eq!(pending.bind_return(), Some(1));
}
