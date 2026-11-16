use super::*;
use crate::{
    animation::scratch::evaluation::{Branch, Stage as EvaluationStage},
    mission::player::components::{
        Registry,
        temporary::{
            SkinLookup,
            fixtures::{Skin, bind_returned},
        },
    },
};

// Authored synthetic call/skin/scalar owners; no original payload.
struct Fixture {
    context: scratch::Context,
    arena: scratch::Arena,
    registry: Registry,
    temporary: temporary::Pending,
    call: Call,
    scalar: ProcessScalar,
}
fn fixture(reference: u32) -> Fixture {
    let mut context = scratch::Context::default();
    let mut arena = scratch::Arena::from_constructor(21).unwrap();
    let mut registry = Registry::from_constructor(8);
    let (fp, temporary) = bind_returned(&mut registry, reference, &mut context, &mut arena);
    let mut call = Call::begin(Request {
        player: fp.player(),
        receiver: fp.identity(),
        ordinal: 0,
        source_ordinal: 0,
        weapon: 0x55,
        alternate: false,
        variant: 0,
    })
    .unwrap();
    // Authored returned extraction matching the captured temporary reference.
    call.extraction_returned = true;
    call.reference = (reference != 0).then_some(reference);
    call.stage = CallStage::AwaitingTemporary;
    Fixture {
        context,
        arena,
        registry,
        temporary,
        call,
        scalar: ProcessScalar::from_startup(3).unwrap(),
    }
}
impl Fixture {
    fn advance(&mut self, post: &mut PostBind, skin: &mut Skin) -> Result<()> {
        post.advance(
            &mut self.temporary,
            &mut self.scalar,
            &mut self.context,
            &mut self.arena,
            skin,
        )
    }
    fn first_words(&self) -> Vec<u32> {
        let mut words = Vec::new();
        if let Some(evaluation) = self.temporary.evaluation() {
            evaluation.visit_state_words(|w| words.push(w));
        }
        for prepare in self.temporary.skin_prepares() {
            prepare.visit_state_words(&mut |w| words.push(w));
        }
        for guard in self.temporary.node_guards() {
            guard.visit_state_words(|w| words.push(w));
        }
        if let Some(forward) = self.temporary.forward_tick() {
            forward.visit_state_words(|w| words.push(w));
        }
        words
    }
}

#[test]
fn class_four_head_patches_parameter_defers_phase_and_runs_a_second_evaluation() {
    let mut f = fixture(0x8400_0007);
    let first = f.first_words();
    let first_lease = f.temporary.evaluation_lease().unwrap();
    let mut post = PostBind::begin(&f.call, &f.temporary).unwrap();
    let mut skin = Skin::resident();
    f.advance(&mut post, &mut skin).unwrap();
    assert_eq!(post.stage(), Stage::AwaitingBoneOutputs);
    assert_eq!(post.dependency(), Some(Dependency::BoneOutputs));
    assert_eq!(
        post.dependency().unwrap().as_str(),
        "preload_temporary_bone_outputs"
    );
    let head = f.temporary.head().unwrap();
    assert_eq!(post.head(), Some(PostBindHead::ClassFour(head)));
    // 0.5 went through the actual +60 role into the same header's +64.
    let parameter = post.parameter().unwrap();
    assert_eq!((parameter.offset, parameter.bits), (0x64, PARAMETER_BITS));
    assert_eq!(parameter.previous, Some(0));
    let header = f.temporary.factory_header().unwrap();
    assert_eq!(header.word(0x64), Some(PARAMETER_BITS));
    // One actual read with UnknownCurrent; KnownInitial is not current.
    let capture = post.capture().unwrap();
    assert_eq!((capture.read_sequence(), capture.current_bits()), (1, None));
    assert_eq!(f.scalar.reads(), 1);
    assert_eq!(f.scalar.initial_bits(), STARTUP_BITS);
    assert_eq!(f.scalar.current_bits(), None);
    assert_eq!(post.assignment(), Some(PhaseAssignment::Deferred));
    assert_eq!(header.word(0x80), None);
    assert!(header.phase_bits().is_err());
    assert_eq!(header.phase_expression(), post.phase());
    // Second scope: new ordinal/allocation in the same arena, released.
    let second = post.evaluation().unwrap();
    let first_evaluation = f.temporary.evaluation().unwrap();
    assert!(second.ordinal() > first_evaluation.ordinal());
    assert!(second.allocation_ordinal() > first_evaluation.allocation_ordinal());
    assert!(!post.evaluation_lease().unwrap().same_scope(&first_lease));
    assert!(second.released());
    assert_eq!(second.slots()[0].branch, Some(Branch::Nonempty(head)));
    assert_eq!(second.slots()[0].nodes, [(head, 0)]);
    assert_eq!(
        (second.nonempty_layers(), second.successful_samples()),
        (1, 0)
    );
    assert!(!second.committed());
    assert_eq!(f.context.masks(), &[[Some(0); 4]; 2]);
    assert_eq!((f.arena.live_reservations(), f.context.holder()), (0, None));
    // Fresh mandatory skin work and the MAX guard; +60/+64/+80 left unread.
    assert_eq!(skin.calls, ["lookup", "body"]);
    assert_eq!(post.skin_prepares().len(), 1);
    let guard = &post.node_guards()[0];
    assert_eq!(guard.clip_reference, Some(u32::MAX));
    assert!(!guard.animator_reference_read);
    // The first bind's history is bit-for-bit unchanged and still returned1.
    assert_eq!(f.first_words(), first);
    assert_eq!(f.temporary.bind_return(), Some(1));
    // Stopped at the bone-output provider; advancing again does nothing.
    f.advance(&mut post, &mut Skin::resident()).unwrap();
    assert_eq!(post.stage(), Stage::AwaitingBoneOutputs);
    assert_eq!(f.scalar.reads(), 1);
}

#[test]
fn later_reads_cannot_retarget_the_retained_time_local_capture() {
    let mut f = fixture(0x8400_0007);
    let mut post = PostBind::begin(&f.call, &f.temporary).unwrap();
    f.advance(&mut post, &mut Skin::resident()).unwrap();
    let later = f.scalar.read().unwrap();
    assert_eq!(later.read_sequence(), 2);
    assert_eq!(post.capture().unwrap().read_sequence(), 1);
    let header = f.temporary.factory_header().unwrap();
    assert_eq!(header.phase_expression().unwrap().read().read_sequence(), 1);
    assert!(f.scalar.issued(post.capture().unwrap()));
    assert!(!ProcessScalar::from_startup(3).unwrap().issued(&later));
}

#[test]
fn null_head_skips_both_stores_and_read_but_still_runs_its_own_evaluation() {
    let mut f = fixture(0);
    assert!(f.temporary.evaluation().is_none());
    // Known-zero extraction matches only the temporary that captured zero.
    assert_eq!(f.call.reference(), None);
    let mut nonzero = Call::begin(f.call.request()).unwrap();
    nonzero.extraction_returned = true;
    nonzero.reference = Some(0x8400_0007);
    nonzero.stage = CallStage::AwaitingTemporary;
    assert!(PostBind::begin(&nonzero, &f.temporary).is_err());
    let mut post = PostBind::begin(&f.call, &f.temporary).unwrap();
    let mut skin = Skin::resident();
    f.advance(&mut post, &mut skin).unwrap();
    assert_eq!(post.head(), Some(PostBindHead::Null));
    assert!(post.parameter().is_none() && post.capture().is_none());
    assert_eq!(f.scalar.reads(), 0);
    let second = post.evaluation().unwrap();
    assert!(second.follows_refresh(f.temporary.refresh().unwrap()));
    assert_eq!(second.slots()[0].branch, Some(Branch::EmptyTail));
    assert!(skin.calls.is_empty());
    assert!(second.released());
    assert_eq!(post.stage(), Stage::AwaitingBoneOutputs);
}

#[test]
fn second_skin_pause_retains_prefix_and_resumes_without_rereading_scalar() {
    let mut f = fixture(0x8400_0007);
    let mut post = PostBind::begin(&f.call, &f.temporary).unwrap();
    let mut skin = Skin::resident();
    skin.lookups.insert(0, SkinLookup::Pending);
    f.advance(&mut post, &mut skin).unwrap();
    assert_eq!(post.stage(), Stage::Evaluating);
    assert_eq!(post.dependency(), Some(Dependency::EvaluationSkin));
    let lease = post.evaluation_lease().unwrap();
    assert!(lease.is_live().unwrap());
    assert_eq!(f.context.holder(), Some(lease.ordinal()));
    assert_eq!(f.scalar.reads(), 1);
    let parameter = post.parameter();
    f.advance(&mut post, &mut skin).unwrap();
    assert_eq!(post.stage(), Stage::AwaitingBoneOutputs);
    assert_eq!(f.scalar.reads(), 1);
    assert_eq!(post.parameter(), parameter);
    assert_eq!(post.head_reads(), 1);
    assert_eq!(skin.calls, ["lookup", "lookup", "body"]);
}

#[test]
fn reaping_the_second_lease_aborts_only_that_scope() {
    let mut f = fixture(0x8400_0007);
    let first = f.first_words();
    let mut post = PostBind::begin(&f.call, &f.temporary).unwrap();
    let mut skin = Skin::resident();
    skin.lookups.insert(0, SkinLookup::Pending);
    f.advance(&mut post, &mut skin).unwrap();
    let lease = post.evaluation_lease().unwrap();
    assert!(lease.abort(&mut f.context, &mut f.arena).unwrap());
    assert_eq!(post.stage(), Stage::Aborted);
    assert_eq!(post.dependency(), None);
    assert_eq!(post.evaluation().unwrap().stage(), EvaluationStage::Aborted);
    assert_eq!((f.arena.live_reservations(), f.context.holder()), (0, None));
    let releases = (f.arena.cursor(), f.arena.live_reservations());
    assert!(f.advance(&mut post, &mut skin).is_err());
    post.abort(&mut f.context, &mut f.arena).unwrap();
    assert_eq!((f.arena.cursor(), f.arena.live_reservations()), releases);
    // The prefix stays as evidence; the first bind is untouched.
    assert!(post.parameter().is_some() && post.capture().is_some());
    assert_eq!(f.first_words(), first);
    assert_eq!(f.temporary.bind_return(), Some(1));
}

#[test]
fn provider_failure_latches_with_prefix_and_live_scope_until_abort() {
    let mut f = fixture(0x8400_0007);
    let mut post = PostBind::begin(&f.call, &f.temporary).unwrap();
    let mut skin = Skin::resident();
    skin.lookups.clear();
    assert!(f.advance(&mut post, &mut skin).is_err());
    assert_eq!(post.stage(), Stage::Failed);
    assert_eq!(f.arena.live_reservations(), 1);
    assert!(post.parameter().is_some() && post.capture().is_some());
    assert!(f.advance(&mut post, &mut Skin::resident()).is_err());
    assert_eq!(f.scalar.reads(), 1);
    post.abort(&mut f.context, &mut f.arena).unwrap();
    assert_eq!(post.stage(), Stage::Aborted);
    assert_eq!(f.arena.live_reservations(), 0);
}

#[test]
fn begin_and_resume_require_this_calls_actual_returned_temporary() {
    let mut f = fixture(0x8400_0007);
    let mut early = Call::begin(f.call.request()).unwrap();
    assert!(PostBind::begin(&early, &f.temporary).is_err());
    // AwaitingTemporary without an actual returned extraction is rejected.
    early.stage = CallStage::AwaitingTemporary;
    early.reference = f.call.reference();
    assert!(PostBind::begin(&early, &f.temporary).is_err());
    early.extraction_returned = true;
    assert!(PostBind::begin(&early, &f.temporary).is_ok());
    // Same FP receiver, different extracted reference: not this temporary.
    early.reference = Some(0x8400_0008);
    assert!(PostBind::begin(&early, &f.temporary).is_err());
    // A returned known-zero reference is not the captured class-four word.
    early.reference = None;
    assert!(PostBind::begin(&early, &f.temporary).is_err());
    early.reference = f.call.reference();
    early.request.receiver = f.temporary.animator().identity();
    assert!(PostBind::begin(&early, &f.temporary).is_err());
    let mut post = PostBind::begin(&f.call, &f.temporary).unwrap();
    let mut context = scratch::Context::default();
    let mut arena = scratch::Arena::from_constructor(22).unwrap();
    let (_, mut other) = bind_returned(&mut f.registry, 0x8400_0007, &mut context, &mut arena);
    assert_ne!(
        other.animator().identity(),
        f.temporary.animator().identity()
    );
    assert!(
        post.advance(
            &mut other,
            &mut f.scalar,
            &mut f.context,
            &mut f.arena,
            &mut Skin::resident(),
        )
        .is_err()
    );
    assert_eq!(post.stage(), Stage::AwaitingHead);
    assert_eq!(f.scalar.reads(), 0);
}
