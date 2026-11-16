use super::*;
use std::collections::BTreeMap;

// Authored synthetic layer owner; no original animation or skeletal payload.
struct Owner {
    skeleton: bool,
    count: Read<u8>,
    weights: Vec<u32>,
    tails: Vec<Read<Tail>>,
    pending_prepare: bool,
    pending_node: bool,
    paused_nodes: Vec<NodeId>,
    fail: Option<&'static str>,
    results: BTreeMap<NodeId, u8>,
    links: BTreeMap<NodeId, Option<NodeId>>,
    log: Vec<String>,
}
impl Owner {
    fn read(&mut self, name: String) -> Result<()> {
        ensure!(
            self.fail != name.split(':').next(),
            "authored failure at {name}"
        );
        self.log.push(name);
        Ok(())
    }
}
impl Layers for Owner {
    fn skeleton_present(&mut self) -> Result<bool> {
        self.read("skeleton".into())?;
        Ok(self.skeleton)
    }
    fn active_count(&mut self) -> Result<Read<u8>> {
        self.read("count".into())?;
        Ok(self.count)
    }
    fn weight_bits(&mut self, slot: u8) -> Result<Read<u32>> {
        self.read(format!("weight:{slot}"))?;
        Ok(Read::Known(self.weights[usize::from(slot)]))
    }
    fn tail(&mut self, slot: u8) -> Result<Read<Tail>> {
        self.read(format!("tail:{slot}"))?;
        Ok(self.tails[usize::from(slot)])
    }
    fn prepare(&mut self, slot: u8) -> Result<Step<()>> {
        self.read(format!("prepare:{slot}"))?;
        Ok(if self.pending_prepare {
            Step::Pending
        } else {
            Step::Done(())
        })
    }
    fn invoke(&mut self, node: NodeId, slot: u8, successes: u32) -> Result<Step<u8>> {
        self.read(format!("invoke:{}:{slot}:{successes}", node.get()))?;
        Ok(if self.pending_node || self.paused_nodes.contains(&node) {
            Step::Pending
        } else {
            Step::Done(self.results[&node])
        })
    }
    fn previous(&mut self, node: NodeId) -> Result<Read<Option<NodeId>>> {
        self.read(format!("previous:{}", node.get()))?;
        Ok(self
            .links
            .get(&node)
            .copied()
            .map_or(Read::Unknown, Read::Known))
    }
}
fn id(value: u64) -> NodeId {
    NodeId::new(value).unwrap()
}
fn one_node() -> Owner {
    Owner {
        skeleton: true,
        count: Read::Known(1),
        weights: vec![1f32.to_bits()],
        tails: vec![Read::Known(Tail::Node(id(1)))],
        pending_prepare: false,
        pending_node: false,
        paused_nodes: Vec::new(),
        fail: None,
        results: [(id(1), 0)].into(),
        links: [(id(1), None)].into(),
        log: Vec::new(),
    }
}
fn shared() -> (Context, Arena) {
    (Context::default(), Arena::from_constructor(5).unwrap())
}

#[test]
fn nonempty_no_sample_layer_reserves_clears_masks_visits_node_and_releases() {
    let (mut context, mut arena) = shared();
    let earlier = context.refresh_empty(&mut arena).unwrap();
    assert_eq!(context.masks(), &[[None; 4]; 2]);
    let mut owner = one_node();
    let mut evaluation = Evaluation::begin(&mut context, &mut arena, &mut owner, 8).unwrap();
    assert_eq!(context.holder(), evaluation.ordinal());
    assert_eq!(context.flags(), Some(0));
    assert_eq!(arena.live_reservations(), 1);
    for (region, offset) in context.regions().iter().zip([0, 0x1000, 0x2000, 0x2100]) {
        assert_eq!(region.unwrap().start, offset);
        assert!(arena.region_is_live(region.unwrap()));
    }
    let progress = evaluation
        .advance(&mut context, &mut arena, &mut owner)
        .unwrap();
    assert_eq!(progress, Progress::Returned);
    assert_eq!(
        owner.log,
        [
            "skeleton",
            "count",
            "weight:0",
            "tail:0",
            "prepare:0",
            "invoke:1:0:0",
            "previous:1",
            "count"
        ]
    );
    assert_eq!(context.masks(), &[[Some(0); 4]; 2]);
    assert_eq!(context.flags(), Some(0));
    assert_eq!(context.holder(), None);
    let slot = &evaluation.slots()[0];
    assert_eq!(slot.branch, Some(Branch::Nonempty(id(1))));
    assert!(slot.prepared && slot.masks_cleared);
    assert_eq!(slot.nodes, [(id(1), 0)]);
    assert_eq!(slot.flags_after, Some(0));
    assert_eq!(evaluation.nonempty_layers(), 1);
    assert_eq!(evaluation.successful_samples(), 0);
    // The first visited nonempty layer still advances the local context.
    assert!(evaluation.advanced_context());
    assert!(!evaluation.committed());
    assert!(evaluation.released());
    assert_eq!(evaluation.live_after(), Some(0));
    assert!(evaluation.follows_refresh(&earlier));
    assert!(
        context
            .regions()
            .iter()
            .all(|region| !arena.region_is_live(region.unwrap()))
    );
    assert!(
        evaluation
            .advance(&mut context, &mut arena, &mut owner)
            .is_err()
    );
}

#[test]
fn pending_prepare_retains_scope_and_resumes_without_repeating_reached_reads() {
    let (mut context, mut arena) = shared();
    let mut owner = one_node();
    owner.pending_prepare = true;
    let mut evaluation = Evaluation::begin(&mut context, &mut arena, &mut owner, 8).unwrap();
    let progress = evaluation
        .advance(&mut context, &mut arena, &mut owner)
        .unwrap();
    assert_eq!(progress, Progress::Pending(Boundary::Prepare));
    assert_eq!(context.masks(), &[[None; 4]; 2]);
    assert_eq!(arena.live_reservations(), 1);
    assert!(context.refresh_empty(&mut arena).is_err());
    assert!(Evaluation::begin(&mut context, &mut arena, &mut one_node(), 8).is_err());
    owner.log.clear();
    owner.pending_prepare = false;
    evaluation
        .advance(&mut context, &mut arena, &mut owner)
        .unwrap();
    assert_eq!(
        owner.log,
        ["prepare:0", "invoke:1:0:0", "previous:1", "count"]
    );
    assert_eq!(evaluation.stage(), Stage::Returned);
    assert_eq!(arena.live_reservations(), 0);
}

#[test]
fn explicit_abort_releases_without_return_and_forbids_replay() {
    let (mut context, mut arena) = shared();
    let mut owner = one_node();
    owner.pending_node = true;
    let mut evaluation = Evaluation::begin(&mut context, &mut arena, &mut owner, 8).unwrap();
    evaluation
        .advance(&mut context, &mut arena, &mut owner)
        .unwrap();
    assert_eq!(evaluation.stage(), Stage::Pending(Boundary::Node));
    assert_eq!(context.masks(), &[[Some(0); 4]; 2]);
    evaluation.abort(&mut context, &mut arena).unwrap();
    assert_eq!(evaluation.stage(), Stage::Aborted);
    assert_eq!((arena.live_reservations(), context.holder()), (0, None));
    assert_eq!(evaluation.nonempty_layers(), 0);
    assert!(
        evaluation
            .advance(&mut context, &mut arena, &mut owner)
            .is_err()
    );
    // A repeated abort observes the reaped scope without a second release.
    evaluation.abort(&mut context, &mut arena).unwrap();
    assert_eq!((arena.allocations, arena.releases), (1, 1));
    context.refresh_empty(&mut arena).unwrap();
}

#[test]
fn provider_failure_latches_and_keeps_reservation_until_explicit_abort() {
    let (mut context, mut arena) = shared();
    let mut owner = one_node();
    owner.fail = Some("invoke");
    let mut evaluation = Evaluation::begin(&mut context, &mut arena, &mut owner, 8).unwrap();
    assert!(
        evaluation
            .advance(&mut context, &mut arena, &mut owner)
            .is_err()
    );
    assert_eq!(evaluation.stage(), Stage::Failed);
    assert_eq!(arena.live_reservations(), 1);
    owner.fail = None;
    assert!(
        evaluation
            .advance(&mut context, &mut arena, &mut owner)
            .is_err()
    );
    evaluation.abort(&mut context, &mut arena).unwrap();
    assert_eq!(arena.live_reservations(), 0);
}

#[test]
fn low_weight_skips_flag_work_and_empty_tail_clears_only_current_layer() {
    let (mut context, mut arena) = shared();
    let mut owner = one_node();
    owner.count = Read::Known(2);
    owner.weights = vec![0.0009f32.to_bits(), f32::NAN.to_bits()];
    owner.tails = vec![Read::Unknown, Read::Known(Tail::Null)];
    let mut evaluation = Evaluation::begin(&mut context, &mut arena, &mut owner, 8).unwrap();
    context.flags = Some(0xc000);
    evaluation
        .advance(&mut context, &mut arena, &mut owner)
        .unwrap();
    assert!(!owner.log.iter().any(|call| call.starts_with("prepare")));
    assert!(!owner.log.contains(&"tail:0".to_string()));
    let slots = evaluation.slots();
    assert_eq!(slots[0].branch, Some(Branch::LowWeight));
    assert_eq!(slots[0].flags_after, None);
    // NaN is not ordered-less than epsilon, so the source treats it as qualifying.
    assert_eq!(slots[1].branch, Some(Branch::EmptyTail));
    assert_eq!(slots[1].flags_after, Some(0x4000));
    assert_eq!(context.masks(), &[[None; 4]; 2]);
    assert_eq!(evaluation.nonempty_layers(), 0);
    assert_eq!(evaluation.stage(), Stage::Pending(Boundary::Commit));
    assert_eq!(arena.live_reservations(), 1);
}

#[test]
fn nonzero_return_counts_and_a_flagged_later_layer_pauses_before_blend() {
    let mut owner = one_node();
    owner.count = Read::Known(2);
    owner.weights = vec![1f32.to_bits(); 2];
    owner.tails = vec![
        Read::Known(Tail::Node(id(2))),
        Read::Known(Tail::Node(id(3))),
    ];
    owner.results = [(id(1), 0), (id(2), 0x80), (id(3), 0)].into();
    owner.links = [(id(2), Some(id(1))), (id(1), None), (id(3), None)].into();
    let (mut context, mut arena) = shared();
    let mut evaluation = Evaluation::begin(&mut context, &mut arena, &mut owner, 8).unwrap();
    evaluation
        .advance(&mut context, &mut arena, &mut owner)
        .unwrap();
    // 0x80 sign-extends nonzero; the next node receives the updated count.
    assert!(owner.log.contains(&"invoke:1:0:1".to_string()));
    assert_eq!(evaluation.slots()[0].nodes, [(id(2), 0x80), (id(1), 0)]);
    assert_eq!(evaluation.slots()[0].successes, 1);
    assert_eq!(evaluation.nonempty_layers(), 2);
    assert!(evaluation.advanced_context());
    assert_eq!(evaluation.stage(), Stage::Returned);

    let (mut context, mut arena) = shared();
    owner.paused_nodes = vec![id(3)];
    let mut evaluation = Evaluation::begin(&mut context, &mut arena, &mut owner, 8).unwrap();
    assert_eq!(
        evaluation
            .advance(&mut context, &mut arena, &mut owner)
            .unwrap(),
        Progress::Pending(Boundary::Node)
    );
    // Authored current-layer bit while paused; no original flag producer.
    context.flags = Some(CURRENT_LAYER);
    owner.paused_nodes.clear();
    assert_eq!(
        evaluation
            .advance(&mut context, &mut arena, &mut owner)
            .unwrap(),
        Progress::Pending(Boundary::LayerBlend)
    );
    assert_eq!(evaluation.nonempty_layers(), 1);
    assert_eq!(evaluation.blended_layers(), 0);
    assert_eq!(context.flags(), Some(CURRENT_LAYER));
    evaluation.abort(&mut context, &mut arena).unwrap();
}

#[test]
fn null_skeleton_returns_without_reservation_and_exhaustion_mutates_nothing() {
    let (mut context, mut arena) = shared();
    let mut owner = one_node();
    owner.skeleton = false;
    let mut evaluation = Evaluation::begin(&mut context, &mut arena, &mut owner, 8).unwrap();
    assert_eq!(
        evaluation
            .advance(&mut context, &mut arena, &mut owner)
            .unwrap(),
        Progress::Returned
    );
    assert!(!evaluation.reserved());
    assert_eq!(owner.log, ["skeleton"]);
    assert_eq!((context.refreshes, context.flags()), (0, None));
    let mut prefix = arena.reserve(0x1e00).unwrap().unwrap();
    let mut owner = one_node();
    assert!(Evaluation::begin(&mut context, &mut arena, &mut owner, 8).is_err());
    assert_eq!(
        (context.refreshes, context.flags(), context.holder()),
        (0, None, None)
    );
    assert_eq!(context.regions(), &[None; 4]);
    assert_eq!((arena.allocations, arena.live_reservations()), (1, 1));
    arena.release(&mut prefix).unwrap();
}

#[test]
fn commit_bit_left_by_a_reached_layer_pauses_and_node_budget_is_bounded() {
    let (mut context, mut arena) = shared();
    let mut owner = one_node();
    owner.pending_node = true;
    let mut evaluation = Evaluation::begin(&mut context, &mut arena, &mut owner, 8).unwrap();
    evaluation
        .advance(&mut context, &mut arena, &mut owner)
        .unwrap();
    // Authored shared-flag change while paused; no original flag producer.
    context.flags = Some(COMMIT | CURRENT_LAYER);
    owner.pending_node = false;
    let progress = evaluation
        .advance(&mut context, &mut arena, &mut owner)
        .unwrap();
    assert_eq!(progress, Progress::Pending(Boundary::Commit));
    assert_eq!(evaluation.commit_register(), COMMIT);
    assert!(!evaluation.released());
    evaluation.abort(&mut context, &mut arena).unwrap();
    let mut owner = one_node();
    owner.links = [(id(1), Some(id(1)))].into();
    let mut evaluation = Evaluation::begin(&mut context, &mut arena, &mut owner, 3).unwrap();
    assert!(
        evaluation
            .advance(&mut context, &mut arena, &mut owner)
            .is_err()
    );
    assert_eq!(evaluation.node_visits(), 3);
    evaluation.abort(&mut context, &mut arena).unwrap();
}

fn paused() -> (Context, Arena, Owner, Evaluation) {
    let (mut context, mut arena) = shared();
    let mut owner = one_node();
    owner.pending_node = true;
    let mut evaluation = Evaluation::begin(&mut context, &mut arena, &mut owner, 8).unwrap();
    evaluation
        .advance(&mut context, &mut arena, &mut owner)
        .unwrap();
    (context, arena, owner, evaluation)
}

#[test]
fn lease_is_registered_at_begin_and_shareable_across_threads() {
    fn shareable<T: Send + Sync + Clone>() {}
    shareable::<CancellationLease>();
    let (context, arena, _, evaluation) = paused();
    let lease = evaluation.lease().unwrap();
    assert!(lease.same_scope(&evaluation.lease().unwrap()));
    assert_eq!(Some(lease.ordinal()), context.holder());
    assert_eq!(lease.owner(), 5);
    assert_eq!(
        Some(lease.allocation_ordinal()),
        evaluation.allocation_ordinal()
    );
    assert!(lease.is_live().unwrap() && !lease.is_aborted().unwrap());
    assert_eq!(arena.live_reservations(), 1);
}

#[test]
fn reaping_a_stale_lease_releases_only_its_scope_and_the_old_owner_never_resumes() {
    let (mut context, mut arena, mut owner, mut stale) = paused();
    let lease = stale.lease().unwrap();
    assert!(lease.abort(&mut context, &mut arena).unwrap());
    assert_eq!((arena.live_reservations(), context.holder()), (0, None));
    assert_eq!(stale.stage(), Stage::Aborted);
    assert!(!stale.released());
    // A fresh attempt may reserve immediately after the reap.
    let mut fresh_owner = one_node();
    let mut fresh = Evaluation::begin(&mut context, &mut arena, &mut fresh_owner, 8).unwrap();
    let before = (arena.allocations, arena.releases, context.holder());
    owner.pending_node = false;
    assert!(stale.advance(&mut context, &mut arena, &mut owner).is_err());
    stale.abort(&mut context, &mut arena).unwrap();
    assert!(!lease.abort(&mut context, &mut arena).unwrap());
    assert_eq!(
        (arena.allocations, arena.releases, context.holder()),
        before
    );
    assert_eq!(
        fresh
            .advance(&mut context, &mut arena, &mut fresh_owner)
            .unwrap(),
        Progress::Returned
    );
    assert_eq!(arena.live_reservations(), 0);
    assert!(!fresh.lease().unwrap().is_live().unwrap());
    assert!(
        !fresh
            .lease()
            .unwrap()
            .abort(&mut context, &mut arena)
            .unwrap()
    );
    assert_eq!(fresh.stage(), Stage::Returned);
}

#[test]
fn lease_abort_rejects_foreign_owner_or_non_lifo_scope_without_mutation() {
    let (mut context, mut arena, _, evaluation) = paused();
    let lease = evaluation.lease().unwrap();
    let mut other_arena = Arena::from_constructor(6).unwrap();
    assert!(lease.abort(&mut context, &mut other_arena).is_err());
    let mut other_context = Context::default();
    assert!(lease.abort(&mut other_context, &mut arena).is_err());
    let mut nested = arena.reserve(16).unwrap().unwrap();
    assert!(lease.abort(&mut context, &mut arena).is_err());
    assert!(lease.is_live().unwrap() && !lease.is_aborted().unwrap());
    assert_eq!(context.holder(), Some(lease.ordinal()));
    arena.release(&mut nested).unwrap();
    assert!(lease.abort(&mut context, &mut arena).unwrap());
    assert_eq!(arena.live_reservations(), 0);
}

#[test]
fn identical_semantic_ids_do_not_authorize_another_arena_instance() {
    let (mut first_context, mut first_arena, _, first) = paused();
    let (mut second_context, mut second_arena, _, second) = paused();
    let lease = first.lease().unwrap();
    assert_eq!(lease.owner(), second.lease().unwrap().owner());
    assert_eq!(first_context.holder(), second_context.holder());
    assert!(lease.abort(&mut second_context, &mut first_arena).is_err());
    assert!(lease.abort(&mut second_context, &mut second_arena).is_err());
    assert_eq!(first_arena.live_reservations(), 1);
    assert_eq!(second_arena.live_reservations(), 1);
    assert!(lease.is_live().unwrap());
    assert!(lease.abort(&mut first_context, &mut first_arena).unwrap());
    assert!(
        second
            .lease()
            .unwrap()
            .abort(&mut second_context, &mut second_arena)
            .unwrap()
    );
}
