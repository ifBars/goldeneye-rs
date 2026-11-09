use super::*;
use std::collections::BTreeMap;

#[derive(Default)]
struct Provider {
    actors: Vec<Option<u64>>,
    masks: BTreeMap<u64, u32>,
    updates: Vec<(u64, u32)>,
    count_reads: usize,
    fail_at: Option<usize>,
    grow_after_first: bool,
}
impl ActorProvider for Provider {
    type Actor = u64;
    fn actor_count(&mut self) -> Result<i32> {
        self.count_reads += 1;
        Ok(self.actors.len() as i32)
    }
    fn actor_at(&mut self, ordinal: usize) -> Result<Option<u64>> {
        Ok(self.actors[ordinal])
    }
    fn membership(&mut self, actor: u64) -> Result<u32> {
        Ok(self.masks[&actor])
    }
    fn update_membership(&mut self, actor: u64, desired: u32) -> Result<()> {
        ensure!(
            self.fail_at != Some(self.updates.len()),
            "authored callback failure"
        );
        self.masks.insert(actor, desired);
        self.updates.push((actor, desired));
        if self.grow_after_first && self.updates.len() == 1 {
            self.actors.push(Some(actor));
        }
        Ok(())
    }
}
fn budget() -> Budget {
    Budget { actor_visits: 8 }
}

#[test]
fn constructor_owns_source_one_gates_and_records_only_sparse_known_fields() {
    let o = Owner::from_constructor();
    let f = o.current_flags().unwrap();
    assert_eq!(
        f,
        RegistrationFlags {
            config_raw: 1,
            game_raw: 1
        }
    );
    assert_eq!(
        0x21 | if f.config_raw != 0 { 8 } else { 0 } | if f.game_raw != 0 { 4 } else { 0 },
        0x2d
    );
    assert_eq!(o.origin(), Origin::ConstructorSnapshot);
    let mut words = Vec::new();
    o.visit_state_words(|w| words.push(w));
    assert_eq!(words, vec![0, 1, 1, 1, 0, 1, 1, 1, 1, 0]);
}
#[test]
fn field_reset_preserves_master_game_and_unknown_provenance() {
    let mut o = Owner::from_snapshot(Snapshot {
        master_2a_raw: None,
        requested_50_raw: Some(0xff),
        cached_51_raw: None,
        game_175_raw: Some(0x80),
    });
    assert!(o.current_flags().is_err());
    o.reset_registration_fields();
    assert_eq!(
        o.current_flags().unwrap(),
        RegistrationFlags {
            config_raw: 1,
            game_raw: 0x80
        }
    );
    assert_eq!(o.snapshot().master_2a_raw, None);
    assert_eq!(o.origin(), Origin::SuppliedSnapshot);
}
#[test]
fn actual_absent_provider_and_equal_gate_are_lazy() {
    let mut o = Owner::from_snapshot(Snapshot {
        master_2a_raw: None,
        requested_50_raw: None,
        cached_51_raw: None,
        game_175_raw: None,
    });
    let mut p = Provider::default();
    assert_eq!(
        o.apply_registration_stage(ApplyContext::Absent, &mut p, budget())
            .unwrap()
            .stage,
        ApplyStage::ProviderAbsent
    );
    assert_eq!(p.count_reads, 0);
    assert!(o.current_flags().is_err());
    let mut o = Owner::from_snapshot(Snapshot {
        master_2a_raw: Some(0x80),
        requested_50_raw: Some(0xff),
        cached_51_raw: Some(0x80),
        game_175_raw: None,
    });
    assert!(
        !o.apply_registration_stage(ApplyContext::MembershipStageReached, &mut p, budget())
            .unwrap()
            .cached_store
    );
    assert_eq!(p.count_reads, 0);
}
#[test]
fn source_order_duplicates_nulls_and_unrelated_membership_bits_are_preserved() {
    let mut o = Owner::from_constructor();
    let mut p = Provider {
        actors: vec![Some(7), None, Some(2), Some(7)],
        masks: BTreeMap::from([(7, 0x1b), (2, 0x88)]),
        ..Provider::default()
    };
    let receipt = o
        .apply_registration_stage(ApplyContext::MembershipStageReached, &mut p, budget())
        .unwrap();
    assert_eq!(p.updates, vec![(7, 0x13), (2, 0x80), (7, 0x13)]);
    assert_eq!(receipt.user_visits, 4);
    assert_eq!(receipt.membership_updates, 3);
    assert_eq!(p.count_reads, 5);
    assert_eq!(o.current_flags().unwrap().config_raw, 0);
}
#[test]
fn callback_failure_retains_reached_membership_but_does_not_commit_cached_gate() {
    let mut o = Owner::from_constructor();
    let mut p = Provider {
        actors: vec![Some(1), Some(2)],
        masks: BTreeMap::from([(1, 0x18), (2, 0x28)]),
        fail_at: Some(1),
        ..Provider::default()
    };
    assert!(
        o.apply_registration_stage(ApplyContext::MembershipStageReached, &mut p, budget())
            .is_err()
    );
    assert_eq!(p.masks[&1], 0x10);
    assert_eq!(p.masks[&2], 0x28);
    assert_eq!(o.current_flags().unwrap().config_raw, 1);
    assert_eq!(o.stage(), ApplyStage::Failed { ordinal: 1 });
}
#[test]
fn loop_reloads_count_after_callback_and_budget_failure_preserves_prefix() {
    let mut o = Owner::from_constructor();
    let mut p = Provider {
        actors: vec![Some(1)],
        masks: BTreeMap::from([(1, 8)]),
        grow_after_first: true,
        ..Provider::default()
    };
    assert!(
        o.apply_registration_stage(
            ApplyContext::MembershipStageReached,
            &mut p,
            Budget { actor_visits: 1 }
        )
        .is_err()
    );
    assert_eq!(p.updates, vec![(1, 0)]);
    assert_eq!(p.count_reads, 2);
    assert_eq!(o.current_flags().unwrap().config_raw, 1);
}
#[test]
fn raw_nonzero_and_value_is_stored_without_boolean_normalization() {
    let mut o = Owner::from_snapshot(Snapshot {
        master_2a_raw: Some(0x82),
        requested_50_raw: Some(0x83),
        cached_51_raw: Some(0),
        game_175_raw: Some(1),
    });
    let mut p = Provider {
        actors: vec![Some(4)],
        masks: BTreeMap::from([(4, 0x20)]),
        ..Provider::default()
    };
    o.apply_registration_stage(ApplyContext::MembershipStageReached, &mut p, budget())
        .unwrap();
    assert_eq!(p.updates, vec![(4, 0x28)]);
    assert_eq!(o.current_flags().unwrap().config_raw, 0x82);
}
