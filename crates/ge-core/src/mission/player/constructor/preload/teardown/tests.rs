use super::*;
use crate::mission::player::{
    components::{
        Registry,
        temporary::fixtures::{
            Skin, bind_from, receiver, store_animator_word, store_head_word, store_script_word,
        },
    },
    constructor::preload::{
        Budget, Call, CallStage, Mapping, Pending, Request, ReturnReason, Source,
        Stage as PreloadStage,
        bone_outputs::tests::{HASHES, MAPPED, REFERENCE, Tags, World, shared, world},
        post_bind::{PostBind, ProcessScalar},
    },
    transform_cache,
};

fn outputs_done(w: &mut World) -> BoneOutputs {
    let mut outputs = BoneOutputs::begin(&w.call, &w.post, &w.temporary).unwrap();
    outputs
        .advance(
            w.lease,
            &mut w.shared,
            &w.temporary,
            &mut Tags::found(HASHES),
        )
        .unwrap();
    outputs
}

#[test]
fn destructor_runs_in_order_then_tombstone_commits_only_its_own_call() {
    let mut w = world(2);
    let head = w.temporary.head().unwrap();
    let outputs = outputs_done(&mut w);
    let mut teardown = Teardown::begin(&outputs, &w.temporary).unwrap();
    let digest = teardown.snapshot().state_digest();
    teardown.advance(&mut w.temporary).unwrap();
    assert_eq!(teardown.stage(), Stage::AwaitingFree);
    assert_eq!(teardown.dependency(), None);
    // One class-four child unlinked and destroyed, inline clips not freed.
    assert_eq!(teardown.children().len(), 1);
    assert_eq!(teardown.children()[0].0, 0);
    assert_eq!(teardown.children()[0].1.node, head);
    assert_eq!(teardown.children()[0].1.heap_records, None);
    assert_eq!(w.temporary.head(), None);
    assert!(w.temporary.animator().script_children().is_empty());
    let skeleton = teardown.skeleton().unwrap();
    assert_eq!((skeleton.bones, skeleton.auxiliary_words), (1, 0));
    assert!(w.temporary.animator().skeleton().is_none());
    assert_eq!(w.temporary.animator().word(0x1c), Some(0));
    assert_eq!(w.temporary.animator().word(0x20), Some(0));
    assert_eq!(w.temporary.animator().word(0x24), Some(u32::MAX));
    // Destroyed owners refuse their earlier live leaves.
    assert!(w.temporary.post_bind_head().is_err());
    let identity = w.temporary.animator().identity();
    let tombstone = teardown.finish(w.temporary).unwrap();
    assert_eq!(tombstone.temporary(), identity);
    assert_eq!(tombstone.bind_return(), Some(1));
    assert_eq!(tombstone.snapshot().head(), Some(head));
    assert_eq!(tombstone.snapshot().state_digest(), digest);
    w.registry.retire(identity).unwrap();
    assert!(w.registry.is_retired(identity));
    assert_eq!(w.registry.resolve(identity.id()), Some(identity));
    assert!(w.registry.retire(identity).is_err());
    // The key is committed only by this call's completion, then return 1.
    assert_eq!(w.shared.preload_entry_key(w.lease, 2).unwrap(), 0);
    let mut foreign = Call::begin(Request {
        ordinal: 1,
        ..w.call.request()
    })
    .unwrap();
    foreign.stage = CallStage::AwaitingTemporary;
    foreign.extraction_returned = true;
    foreign.reference = w.call.reference();
    foreign.mapping = w.call.mapping();
    foreign.entry_index = Some(2);
    assert!(
        foreign
            .complete_temporary(w.lease, &mut w.shared, &tombstone)
            .is_err()
    );
    assert_eq!(w.shared.preload_entry_key(w.lease, 2).unwrap(), 0);
    w.call
        .complete_temporary(w.lease, &mut w.shared, &tombstone)
        .unwrap();
    assert_eq!(w.shared.preload_entry_key(w.lease, 2).unwrap(), MAPPED);
    assert_eq!(w.call.return_word(), Some(1));
    assert_eq!(
        w.call.return_reason(),
        Some(ReturnReason::TemporaryCompleted)
    );
    assert_eq!(w.call.temporary_completed(), Some(identity));
    assert!(
        w.call
            .complete_temporary(w.lease, &mut w.shared, &tombstone)
            .is_err()
    );
    // The six outputs written before the destructor are untouched.
    assert_eq!(
        w.shared.preload_entry_bone_index(w.lease, 2, 5).unwrap(),
        Some(0)
    );
    assert_eq!((w.arena.live_reservations(), w.context.holder()), (0, None));
}

#[test]
fn destructor_requires_completed_outputs_for_this_temporary() {
    let mut w = world(0);
    let mut outputs = BoneOutputs::begin(&w.call, &w.post, &w.temporary).unwrap();
    assert!(Teardown::begin(&outputs, &w.temporary).is_err());
    let mut tags = Tags::found(HASHES);
    tags.lookups = vec![crate::mission::player::components::temporary::SkinLookup::Pending];
    outputs
        .advance(w.lease, &mut w.shared, &w.temporary, &mut tags)
        .unwrap();
    assert!(Teardown::begin(&outputs, &w.temporary).is_err());
    assert!(!w.temporary.teardown_started());
}

#[test]
fn populated_or_unknown_owner_words_pause_before_effects_and_abort_does_nothing_else() {
    let mut w = world(0);
    let outputs = outputs_done(&mut w);
    store_animator_word(&mut w.temporary, 0x124, None);
    let mut teardown = Teardown::begin(&outputs, &w.temporary).unwrap();
    teardown.advance(&mut w.temporary).unwrap();
    assert_eq!(teardown.stage(), Stage::AttachmentList);
    assert_eq!(teardown.dependency(), Some(Dependency::AttachmentList));
    assert!(!w.temporary.teardown_started());

    let mut w = world(0);
    let outputs = outputs_done(&mut w);
    store_animator_word(&mut w.temporary, 0x120, Some(7));
    let mut teardown = Teardown::begin(&outputs, &w.temporary).unwrap();
    teardown.advance(&mut w.temporary).unwrap();
    assert_eq!(teardown.dependency(), Some(Dependency::ExtraOwner));
    assert!(w.temporary.head().is_some());
    assert!(w.temporary.animator().skeleton().is_some());
    teardown.abort();
    assert_eq!(teardown.stage(), Stage::Aborted);
    assert!(teardown.advance(&mut w.temporary).is_err());
    let identity = w.temporary.animator().identity();
    drop(w.temporary);
    // No Drop effect: no key, no retirement, outputs still historical.
    assert_eq!(w.shared.preload_entry_key(w.lease, 0).unwrap(), 0);
    assert!(!w.registry.is_retired(identity));
    assert_eq!(w.call.stage(), CallStage::AwaitingTemporary);

    // A populated base owner word pauses after the reached destructor prefix.
    let mut w = world(0);
    let outputs = outputs_done(&mut w);
    store_animator_word(&mut w.temporary, 0x5c, Some(1));
    let mut teardown = Teardown::begin(&outputs, &w.temporary).unwrap();
    teardown.advance(&mut w.temporary).unwrap();
    assert_eq!(teardown.stage(), Stage::BaseDestructor);
    assert_eq!(teardown.dependency(), Some(Dependency::BaseOwner));
    assert!(w.temporary.animator().skeleton().is_none());
    assert!(teardown.skeleton().is_some());
}

#[test]
fn unlink_is_retained_before_a_paused_child_destructor_and_resume_continues() {
    let mut w = world(0);
    let head = w.temporary.head().unwrap();
    let outputs = outputs_done(&mut w);
    store_head_word(&mut w.temporary, 0x8c, None);
    let mut teardown = Teardown::begin(&outputs, &w.temporary).unwrap();
    teardown.advance(&mut w.temporary).unwrap();
    assert_eq!(teardown.stage(), Stage::SlotChildren);
    assert_eq!(teardown.dependency(), Some(Dependency::ChildState));
    // Source order: next read and unlink already happened; the header and its
    // identity remain until the destructor actually returns.
    let cursor = teardown.child_in_progress().unwrap();
    assert_eq!((cursor.node(), cursor.next()), (head, None));
    assert_eq!(w.temporary.head(), None);
    let animator = w.temporary.animator();
    assert!(animator.node_ids().links(head).is_none());
    assert!(animator.node_ids().kind(head).is_some());
    assert!(animator.script_children().get(head).is_some());
    assert!(teardown.children().is_empty());
    // Nothing after the slot walk ran.
    assert!(w.temporary.animator().skeleton().is_some());
    // Repeated advances keep the same pause without repeating the unlink.
    teardown.advance(&mut w.temporary).unwrap();
    assert_eq!(teardown.child_in_progress(), Some(cursor));
    // A populated listener list pauses after the +8c phase.
    store_script_word(&mut w.temporary, head, 0x8c, Some(0));
    store_script_word(&mut w.temporary, head, 0x78, Some(5));
    teardown.advance(&mut w.temporary).unwrap();
    assert_eq!(teardown.dependency(), Some(Dependency::ChildReferenceList));
    assert_eq!(
        teardown
            .child_in_progress()
            .unwrap()
            .destructor()
            .unwrap()
            .phase(),
        crate::animation::script_child::destroy::Phase::ReferenceList
    );
    store_script_word(&mut w.temporary, head, 0x78, Some(0));
    teardown.advance(&mut w.temporary).unwrap();
    assert_eq!(teardown.stage(), Stage::AwaitingFree);
    assert_eq!(teardown.children().len(), 1);
    assert!(w.temporary.animator().node_ids().kind(head).is_none());
    assert!(w.temporary.animator().script_children().get(head).is_none());
}

/// Synthetic registry: one enabled row, a distinct mapped key per call.
struct Rows {
    weapon_reads: usize,
    count_reads: usize,
}
impl Source for Rows {
    fn registry_count(&mut self) -> Result<usize> {
        self.count_reads += 1;
        Ok(1)
    }
    fn enabled_byte(&mut self, _: usize) -> Result<u8> {
        Ok(1)
    }
    fn weapon(&mut self, _: usize) -> Result<u32> {
        self.weapon_reads += 1;
        Ok(0x55)
    }
    fn remap(&mut self, request: Request) -> Result<Mapping> {
        Ok(Mapping {
            output: MAPPED + request.ordinal as u32,
            accepted: true,
        })
    }
    fn extract_reference(&mut self, _: Identity, _: u32) -> Result<Option<u32>> {
        Ok(Some(REFERENCE))
    }
}

#[test]
fn six_calls_complete_in_source_order_and_only_exhaustion_clears_the_flag() {
    let (mut shared, lease) = shared();
    let mut caches = transform_cache::Owner::from_boot();
    let mut registry = Registry::from_constructor(64);
    let mut context = crate::animation::scratch::Context::default();
    let mut arena = crate::animation::scratch::Arena::from_constructor(41).unwrap();
    let mut scalar = ProcessScalar::from_startup(5).unwrap();
    let fp = receiver(&mut registry);
    let mut preload = Pending::new(lease, fp.identity());
    let mut rows = Rows {
        weapon_reads: 0,
        count_reads: 0,
    };
    preload
        .advance(&mut shared, &mut caches, &mut rows, Budget::default())
        .unwrap();
    let mut identities = Vec::new();
    for ordinal in 0..6u32 {
        assert_eq!(preload.stage(), PreloadStage::AwaitingTemporary);
        assert_eq!(rows.weapon_reads, ordinal as usize + 1);
        assert_ne!(shared.cache_enabled(), 0);
        let call = preload.current_call().unwrap();
        let entry = call.entry_index().unwrap();
        let mut temporary = bind_from(&fp, &mut registry, REFERENCE, &mut context, &mut arena);
        let mut post = PostBind::begin(call, &temporary).unwrap();
        post.advance(
            &mut temporary,
            &mut scalar,
            &mut context,
            &mut arena,
            &mut Skin::resident(),
        )
        .unwrap();
        let mut outputs = BoneOutputs::begin(call, &post, &temporary).unwrap();
        outputs
            .advance(lease, &mut shared, &temporary, &mut Tags::found(HASHES))
            .unwrap();
        let mut teardown = Teardown::begin(&outputs, &temporary).unwrap();
        teardown.advance(&mut temporary).unwrap();
        let tombstone = teardown.finish(temporary).unwrap();
        registry.retire(tombstone.temporary()).unwrap();
        identities.push(tombstone.temporary());
        preload
            .resume_call(
                &mut shared,
                &mut caches,
                &mut rows,
                Budget::default(),
                &tombstone,
            )
            .unwrap();
        assert_eq!(
            shared.preload_entry_key(lease, entry).unwrap(),
            MAPPED + ordinal
        );
        // A completed tombstone cannot complete the next call.
        if let Some(next) = preload.current_call() {
            assert_ne!(next.request(), tombstone.binding().request);
        }
    }
    assert!(preload.helper_returned() && preload.flag_cleared());
    assert_eq!(shared.cache_enabled(), 0);
    assert_eq!((rows.weapon_reads, preload.weapon_reads()), (6, 6));
    assert_eq!(rows.count_reads, 1);
    assert_eq!(scalar.reads(), 6);
    let calls = preload.completed_calls();
    assert_eq!(calls.len(), 6);
    for (ordinal, call) in calls.iter().enumerate() {
        let request = call.request();
        assert_eq!(request.ordinal, ordinal);
        assert_eq!(request.alternate, ordinal % 2 != 0);
        assert_eq!(usize::from(request.variant), ordinal / 2);
        assert_eq!(call.return_reason(), Some(ReturnReason::TemporaryCompleted));
        assert_eq!(call.temporary_completed(), Some(identities[ordinal]));
    }
    identities.dedup();
    assert_eq!(identities.len(), 6);
    assert_eq!((arena.live_reservations(), context.holder()), (0, None));
}
