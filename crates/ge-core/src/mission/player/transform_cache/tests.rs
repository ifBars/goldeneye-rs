use super::*;
use crate::{
    animation::skeleton::{self, BoneInput, Skeleton},
    mission::player::components::{Identity, Registry},
};

fn skeleton(parents: &[u16]) -> Skeleton {
    Skeleton::initialize(
        &parents
            .iter()
            .map(|&parent| BoneInput {
                parent,
                range_end: 0,
                auxiliary_raw: [0, 0],
            })
            .collect::<Vec<_>>(),
        skeleton::Budget {
            bones: 128,
            parent_visits: 128,
        },
    )
    .unwrap()
}
fn identities() -> [Identity; 2] {
    let mut registry = Registry::from_constructor(2);
    let player = PlayerId::new(1).unwrap();
    [
        registry.allocate(player).unwrap(),
        registry.allocate(player).unwrap(),
    ]
}
fn inputs<'a>(ids: [Identity; 2], skeleton: &'a Skeleton) -> [Option<Input<'a>>; 2] {
    ids.map(|receiver| {
        Some(Input {
            receiver,
            skeleton: Some(skeleton),
        })
    })
}
fn build(owner: &mut Owner, skeleton: &Skeleton) {
    let [fp, tp] = inputs(identities(), skeleton);
    owner
        .initialize_if_unselected(fp, tp, Budget::default())
        .unwrap();
}
fn bytes(backing: &Backing) -> Vec<u8> {
    (0..backing.native_bytes())
        .map(|offset| backing.byte(offset).unwrap().unwrap())
        .collect()
}
fn words(owner: &Owner) -> Vec<u32> {
    let mut result = Vec::new();
    owner.visit_state_words(|word| result.push(word));
    result
}

#[test]
fn startup_and_independent_headers_have_explicit_null_state() {
    let owner = Owner::from_boot();
    assert_eq!(owner.selected(), None);
    assert_eq!(owner.native_bytes(), 0);
    assert_eq!(owner.snapshot().counts, [0, 0]);
    assert_eq!(owner.snapshot().associations, [None, None]);
    for slot in [Slot::FirstPerson, Slot::ThirdPerson] {
        let header = owner.header(slot);
        assert!(header.matrix().is_none() && header.validity().is_none());
        assert!(header.offsets().is_none() && header.descendants().is_none());
    }
}

#[test]
fn ordered_preorder_lists_exclude_self_and_keep_independent_backings() {
    // 0 -> [1 -> [3], 2 -> [4]], and an independent root5.
    let skeleton = skeleton(&[u16::MAX, 0, 0, 1, 2, u16::MAX]);
    let mut owner = Owner::from_boot();
    build(&mut owner, &skeleton);
    let expected = [1, 3, 2, 4, 0xff, 3, 0xff, 4, 0xff, 0xff, 0xff, 0xff];
    assert_eq!(owner.selected(), Some(Slot::FirstPerson));
    for slot in [Slot::FirstPerson, Slot::ThirdPerson] {
        let header = owner.header(slot);
        assert_eq!(bytes(header.descendants().unwrap()), expected);
        let offsets = header.offsets().unwrap();
        assert_eq!(
            (0..6)
                .map(|i| offsets.half(i * 2).unwrap().unwrap())
                .collect::<Vec<_>>(),
            [0, 5, 7, 9, 10, 11]
        );
        assert!(header.temporary().is_none());
        assert_eq!(header.count(), 6);
        let matrix = header.matrix().unwrap();
        assert_eq!(matrix.word(0).unwrap(), Some(64));
        assert_eq!(matrix.word(4).unwrap(), Some(6));
        for offset in 8..matrix.native_bytes() {
            assert_eq!(matrix.byte(offset).unwrap(), None);
        }
    }
    assert_ne!(
        owner.header(Slot::FirstPerson).matrix().unwrap().id(),
        owner.header(Slot::ThirdPerson).matrix().unwrap().id()
    );
    let trace = owner.trace().unwrap();
    assert!(trace.returned && trace.selection_returned);
    let fp = trace.builds[0].as_ref().unwrap();
    let tp = trace.builds[1].as_ref().unwrap();
    assert!(fp.allocations.last().unwrap().get() < tp.allocations.first().unwrap().get());
    assert_eq!(fp.roots_completed, 6);
    assert_eq!(fp.descendant_visits, 6);
    assert_eq!(fp.releases.len(), 1); // temp after completed compact copy
}

#[test]
fn empty_and_single_bone_are_source_branches_not_identity_matrices() {
    let mut empty = Owner::from_boot();
    build(&mut empty, &skeleton(&[]));
    assert_eq!(empty.selected(), None);
    assert_eq!(empty.native_bytes(), 0);
    assert!(empty.trace().unwrap().selection_returned);
    let mut one = Owner::from_boot();
    build(&mut one, &skeleton(&[u16::MAX]));
    assert_eq!(
        bytes(one.header(Slot::FirstPerson).descendants().unwrap()),
        [0xff]
    );
    assert_eq!(
        one.header(Slot::FirstPerson)
            .offsets()
            .unwrap()
            .half(0)
            .unwrap(),
        Some(0)
    );
    assert_eq!(
        one.header(Slot::FirstPerson)
            .matrix()
            .unwrap()
            .word(16)
            .unwrap(),
        None
    );
}

#[test]
fn deep_maximum_chain_uses_all_levels_and_fits_native_widths() {
    let mut parents = vec![u16::MAX];
    parents.extend(0..127);
    let skeleton = skeleton(&parents);
    let mut owner = Owner::from_boot();
    build(&mut owner, &skeleton);
    let header = owner.header(Slot::FirstPerson);
    let descendants = bytes(header.descendants().unwrap());
    assert_eq!(descendants.len(), 8256);
    assert_eq!(&descendants[..127], &(1..128).collect::<Vec<u8>>());
    assert_eq!(descendants[127], 0xff);
    assert_eq!(header.offsets().unwrap().half(254).unwrap(), Some(8255));
    assert_eq!(
        owner.trace().unwrap().builds[0]
            .as_ref()
            .unwrap()
            .descendant_visits,
        8128
    );
}

#[test]
fn each_allocation_failure_retains_reached_prefix_and_denies_replay() {
    let skeleton = skeleton(&[u16::MAX, 0, 1]);
    let ids = identities();
    let expected = [
        BuildStage::MatrixAllocation,
        BuildStage::ValidityAllocation,
        BuildStage::OffsetsAllocation,
        BuildStage::TemporaryAllocation,
        BuildStage::CompactAllocation,
    ];
    for (limit, stage) in expected.into_iter().enumerate() {
        let mut owner = Owner::from_boot();
        let [fp, tp] = inputs(ids, &skeleton);
        assert!(
            owner
                .initialize_if_unselected(
                    fp,
                    tp,
                    Budget {
                        allocations: limit,
                        ..Budget::default()
                    }
                )
                .is_err()
        );
        assert!(owner.trace().unwrap().failed);
        let trace = owner.trace().unwrap().builds[0].as_ref().unwrap();
        assert_eq!(trace.stage, stage);
        assert_eq!(trace.allocations.len(), limit);
        let header = owner.header(Slot::FirstPerson);
        assert_eq!(header.count(), if limit >= 2 { 3 } else { 0 });
        if limit >= 2 {
            assert_eq!(bytes(header.validity().unwrap()), [0, 0, 0]);
        }
        if limit == 4 {
            assert!(header.temporary().is_some());
            assert!(header.descendants().is_none());
            assert_eq!(trace.roots_completed, 3);
        }
        let before = words(&owner);
        assert!(
            owner
                .initialize_if_unselected(fp, tp, Budget::default())
                .is_err()
        );
        assert_eq!(words(&owner), before);
        assert!(owner.header(Slot::ThirdPerson).matrix().is_none());
    }
}

#[test]
fn descendant_budget_retains_written_offset_and_partial_temporary_prefix() {
    let skeleton = skeleton(&[u16::MAX, 0, 1]);
    let [fp, tp] = inputs(identities(), &skeleton);
    let mut owner = Owner::from_boot();
    assert!(
        owner
            .initialize_if_unselected(
                fp,
                tp,
                Budget {
                    descendant_visits: 1,
                    ..Budget::default()
                }
            )
            .is_err()
    );
    let header = owner.header(Slot::FirstPerson);
    let trace = owner.trace().unwrap().builds[0].as_ref().unwrap();
    assert_eq!(trace.stage, BuildStage::Descendants);
    assert_eq!(trace.offsets_written, 1);
    assert_eq!(trace.roots_completed, 0);
    assert_eq!(trace.descendant_visits, 1);
    assert_eq!(header.offsets().unwrap().half(0).unwrap(), Some(0));
    assert_eq!(header.offsets().unwrap().half(2).unwrap(), None);
    assert_eq!(header.temporary().unwrap().byte(0).unwrap(), Some(1));
    assert_eq!(header.temporary().unwrap().byte(1).unwrap(), None);
}

#[test]
fn unknown_skeleton_waits_after_cleanup_and_resumes_without_repeating_fp() {
    let skeleton = skeleton(&[u16::MAX, 0]);
    let ids = identities();
    let [fp, _] = inputs(ids, &skeleton);
    let missing = Some(Input {
        receiver: ids[1],
        skeleton: None,
    });
    let mut owner = Owner::from_boot();
    assert!(
        owner
            .initialize_if_unselected(fp, missing, Budget::default())
            .is_err()
    );
    assert!(!owner.trace().unwrap().failed);
    let fp_id = owner.header(Slot::FirstPerson).matrix().unwrap().id();
    assert_eq!(owner.trace().unwrap().stage, Stage::ThirdPerson);
    assert!(
        owner.trace().unwrap().builds[1]
            .as_ref()
            .unwrap()
            .waiting_skeleton
    );
    let [fp, tp] = inputs(ids, &skeleton);
    owner
        .initialize_if_unselected(fp, tp, Budget::default())
        .unwrap();
    assert_eq!(
        owner.header(Slot::FirstPerson).matrix().unwrap().id(),
        fp_id
    );
    assert_eq!(owner.invocation_count(), 1);
    assert_eq!(owner.selected(), Some(Slot::FirstPerson));
}

#[test]
fn missing_receiver_skips_entire_block_and_selected_reuse_does_not_read_skeleton() {
    let skeleton = skeleton(&[u16::MAX]);
    let ids = identities();
    let unknown = Some(Input {
        receiver: ids[0],
        skeleton: None,
    });
    let mut owner = Owner::from_boot();
    let trace = owner
        .initialize_if_unselected(
            unknown,
            None,
            Budget {
                allocations: 0,
                ..Budget::default()
            },
        )
        .unwrap();
    assert!(trace.returned && trace.missing_receiver_skip);
    assert!(trace.builds.iter().all(Option::is_none));
    build(&mut owner, &skeleton);
    let snapshot = owner.snapshot();
    let trace = owner
        .initialize_if_unselected(
            unknown,
            Some(Input {
                receiver: ids[1],
                skeleton: None,
            }),
            Budget {
                allocations: 0,
                ..Budget::default()
            },
        )
        .unwrap();
    assert!(trace.reused_selected && trace.returned);
    assert!(trace.builds.iter().all(Option::is_none));
    assert_eq!(owner.snapshot().matrix_backings, snapshot.matrix_backings);
}

#[test]
fn alias_binding_is_lazy_typed_and_association_survives_rebuild_release() {
    let skeleton = skeleton(&[u16::MAX, 0]);
    let mut owner = Owner::from_boot();
    let player = PlayerId::new(1).unwrap();
    let alias = PlayerId::new(7).unwrap();
    owner
        .bind_selected_with(Some(player), |_| panic!("unselected must not read alias"))
        .unwrap();
    build(&mut owner, &skeleton);
    owner
        .bind_selected_with(None, |_| panic!("NULL player must not read alias"))
        .unwrap();
    assert!(owner.bind_selected(Some(player), Some(alias)).unwrap());
    assert!(owner.selected_matches(Some(player), Some(alias)).unwrap());
    assert!(!owner.selected_matches(Some(player), None).unwrap());
    assert_eq!(owner.header(Slot::ThirdPerson).association(), None);
    owner.select(-1);
    build(&mut owner, &skeleton);
    assert_eq!(owner.header(Slot::FirstPerson).association(), Some(alias));
    let fp = owner.header(Slot::FirstPerson);
    let tp = owner.header(Slot::ThirdPerson);
    let expected = [
        tp.descendants().unwrap().id(),
        tp.offsets().unwrap().id(),
        tp.validity().unwrap().id(),
        tp.matrix().unwrap().id(),
        fp.descendants().unwrap().id(),
        fp.offsets().unwrap().id(),
        fp.validity().unwrap().id(),
        fp.matrix().unwrap().id(),
    ];
    assert_eq!(owner.release_all().unwrap(), expected);
    assert_eq!(owner.selected(), None);
    assert_eq!(owner.native_bytes(), 0);
    assert_eq!(owner.header(Slot::FirstPerson).association(), Some(alias));
}

#[test]
fn selection_only_requires_matrix_presence_and_invalidation_observes_zero_count() {
    let skeleton = skeleton(&[u16::MAX]);
    let [fp, tp] = inputs(identities(), &skeleton);
    let mut owner = Owner::from_boot();
    assert!(
        owner
            .initialize_if_unselected(
                fp,
                tp,
                Budget {
                    allocations: 1,
                    ..Budget::default()
                }
            )
            .is_err()
    );
    assert_eq!(owner.select(0), Some(Slot::FirstPerson));
    owner.invalidate_selected().unwrap(); // count0, missing validity still read
    assert_eq!(owner.header(Slot::FirstPerson).invalidation_reads(), (1, 1));
    assert_eq!(
        owner
            .header(Slot::FirstPerson)
            .matrix()
            .unwrap()
            .word(16)
            .unwrap(),
        None
    );
    assert_eq!(owner.select(1), None);
    assert_eq!(owner.select(7), None);
}

#[test]
fn build_cleanup_matrix_gate_differs_from_independent_destructor() {
    let mut header = Header::from_boot();
    header.association = Some(PlayerId::new(5).unwrap());
    header.offsets = Some(Backing::allocate(BackingId(99), BackingKind::Offsets, 2).unwrap());
    header.count = 1;
    assert!(header.release(true).is_empty());
    assert!(header.offsets().is_some());
    assert_eq!(header.count(), 1);
    assert_eq!(header.release(false), [BackingId(99)]);
    assert_eq!(header.count(), 0);
    assert_eq!(header.association(), Some(PlayerId::new(5).unwrap()));
}

#[test]
fn invalidation_keeps_matrix_unknown_and_state_hash_tracks_written_metadata() {
    let mut owner = Owner::from_boot();
    build(&mut owner, &skeleton(&[u16::MAX, 0]));
    let before = words(&owner);
    let matrix = owner.header(Slot::FirstPerson).matrix().unwrap().id();
    owner.headers[0]
        .validity
        .as_mut()
        .unwrap()
        .put(0, 1)
        .unwrap();
    assert_ne!(words(&owner), before);
    owner.invalidate_selected().unwrap();
    assert_eq!(
        bytes(owner.header(Slot::FirstPerson).validity().unwrap()),
        [0, 0]
    );
    assert_eq!(
        owner.header(Slot::FirstPerson).matrix().unwrap().id(),
        matrix
    );
    assert_eq!(
        owner
            .header(Slot::FirstPerson)
            .matrix()
            .unwrap()
            .word(16)
            .unwrap(),
        None
    );
}

#[test]
fn mismatched_players_or_duplicate_component_cannot_mutate_owner() {
    let skeleton = skeleton(&[u16::MAX]);
    let mut registry = Registry::from_constructor(2);
    let first = registry.allocate(PlayerId::new(1).unwrap()).unwrap();
    let second = registry.allocate(PlayerId::new(2).unwrap()).unwrap();
    let fp = Some(Input {
        receiver: first,
        skeleton: Some(&skeleton),
    });
    let wrong_player = Some(Input {
        receiver: second,
        skeleton: Some(&skeleton),
    });
    let mut owner = Owner::from_boot();
    let before = words(&owner);
    assert!(
        owner
            .initialize_if_unselected(fp, wrong_player, Budget::default())
            .is_err()
    );
    assert_eq!(words(&owner), before);
    assert!(
        owner
            .initialize_if_unselected(fp, fp, Budget::default())
            .is_err()
    );
    assert_eq!(words(&owner), before);
}

#[test]
fn pending_input_change_cannot_replay_or_replace_reached_fp() {
    let skeleton = skeleton(&[u16::MAX]);
    let ids = identities();
    let [fp, _] = inputs(ids, &skeleton);
    let unknown_tp = Some(Input {
        receiver: ids[1],
        skeleton: None,
    });
    let mut owner = Owner::from_boot();
    assert!(
        owner
            .initialize_if_unselected(fp, unknown_tp, Budget::default())
            .is_err()
    );
    let before = words(&owner);
    assert!(
        owner
            .initialize_if_unselected(None, unknown_tp, Budget::default())
            .is_err()
    );
    assert_eq!(words(&owner), before);
    assert!(owner.release_slot(Slot::FirstPerson).is_err());
    assert_eq!(words(&owner), before);
}
