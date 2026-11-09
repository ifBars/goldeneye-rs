use super::*;

#[test]
fn source_enable_gate_is_lazy_and_preserves_retained_fields() {
    let fields = common_fields();
    let mut owner = BankOwner::from_snapshot(fields);
    let mut provider = Provider::new(Some(row(0x2d50_0381)));
    provider.fail_level = true;
    let receipt = owner.run(0, &mut provider).unwrap();
    assert!(receipt.disabled);
    assert_eq!(receipt.source_calls, 0);
    assert!(provider.events.is_empty());
    assert_eq!(owner.fields(), fields);
    assert_eq!(owner.stage(), BankStage::Disabled);
}

#[test]
fn fresh_calls_common_then_primary_and_reentry_preserves_common_slots() {
    let mut owner = BankOwner::from_constructor();
    let mut provider = Provider::new(Some(row(0x2d50_0381)));
    let receipt = owner.run(0x80, &mut provider).unwrap();
    assert!(!receipt.disabled);
    assert_eq!(calls(&owner), [0x2d50_0001, 0x2d50_0014, 0x2d50_0381]);
    assert_eq!(
        provider.events,
        [
            Event::Level,
            Event::Job,
            Event::Dispatch(0x2d50_0001, 0),
            Event::Job,
            Event::Dispatch(0x2d50_0014, 0),
            Event::Job,
            Event::Dispatch(0x2d50_0381, 0),
            Event::Session
        ]
    );
    assert_eq!(
        owner.fields(),
        BankFields {
            word_4c: 0x2d50_0381,
            word_50: 0x2d50_0014,
            word_54: 0x2d50_0001,
            byte_58: 1
        }
    );
    assert_eq!(receipt.source_calls, 3);
    assert!(owner.calls().iter().all(|call| call.returned));
    let mut provider = Provider::new(Some(row(0x2d50_0381)));
    owner.run(1, &mut provider).unwrap();
    assert_eq!(calls(&owner), [0x2d50_0381]);
    assert_eq!(provider.jobs, 1);
    assert_eq!(owner.stage(), BankStage::Complete);
}

#[test]
fn optional_max_only_skip_and_multiplayer_call_follow_primary_and_optional_order() {
    let mut owner = BankOwner::from_constructor();
    let mut provider = Provider::new(Some(BankRow {
        primary: 0x2d50_0381,
        optional: [0, 0x2d50_0382],
    }));
    provider.multiplayer = true;
    owner.run(1, &mut provider).unwrap();
    assert_eq!(
        calls(&owner),
        [
            0x2d50_0001,
            0x2d50_0014,
            0x2d50_0381,
            0,
            0x2d50_0382,
            0x2d50_0019
        ]
    );
    assert_eq!(owner.calls().len(), 6);
    assert_eq!(owner.calls()[3].valid_class, Some(false));
    assert_eq!(owner.calls()[3].dispatch, None);
    let session = provider
        .events
        .iter()
        .position(|event| *event == Event::Session)
        .unwrap();
    assert!(provider.events[..session].contains(&Event::Dispatch(0x2d50_0382, 0)));
    assert_eq!(
        &provider.events[session + 1..],
        &[Event::Job, Event::Dispatch(0x2d50_0019, 0)]
    );
}

#[test]
fn invalid_tokens_still_ensure_work_before_rejection_and_do_not_dispatch_bank_commands() {
    let mut owner = BankOwner::from_snapshot(common_fields());
    let mut provider = Provider::new(Some(BankRow {
        primary: 0x2d70_0001,
        optional: [0, u32::MAX],
    }));
    owner.run(1, &mut provider).unwrap();
    assert_eq!(
        provider.events,
        [Event::Level, Event::Job, Event::Job, Event::Session]
    );
    assert_eq!(calls(&owner), [0x2d70_0001, 0]);
    assert!(
        owner
            .calls()
            .iter()
            .all(|call| call.job == Some(JobOutcome::Existing)
                && call.valid_class == Some(false)
                && call.returned)
    );
    assert_eq!(owner.fields().word_4c, 0x2d70_0001);
    assert_eq!(owner.last_rejected(), Some(0));
    for token in [0x2d50_0000, 0xff50_0001, 0x0050_0000, 0x815a_bcde] {
        assert!(bank_class_valid(token));
    }
    for token in [0x2d70_0001, 0x2de5_0001, 0x2d90_0000, 0] {
        assert!(!bank_class_valid(token));
    }
}

#[test]
fn job_failure_retains_prior_returned_slot_and_does_not_write_the_failed_slot() {
    let mut owner = BankOwner::from_constructor();
    let mut provider = Provider::new(Some(row(0x2d50_0381)));
    provider.fail_job = Some(2);
    assert!(owner.run(1, &mut provider).is_err());
    assert_eq!(
        owner.stage(),
        BankStage::Failed(BankStep::EnsureJob { ordinal: 1 })
    );
    assert_eq!(
        owner.fields(),
        BankFields {
            word_4c: 0,
            word_50: 0,
            word_54: 0x2d50_0001,
            byte_58: 1
        }
    );
    assert!(owner.calls()[0].returned);
    assert_eq!(owner.calls()[1].job, None);
    assert_eq!(owner.calls()[1].valid_class, None);
    assert!(!owner.calls()[1].returned);
    let mut retry = Provider::new(Some(row(0x2d50_0381)));
    owner.run(1, &mut retry).unwrap();
    assert_eq!(calls(&owner), [0x2d50_0014, 0x2d50_0381]);
}

#[test]
fn dispatch_failure_preserves_job_result_and_source_slot_write_waits_for_return() {
    let mut owner = BankOwner::from_snapshot(common_fields());
    let mut provider = Provider::new(Some(row(0x2d50_0381)));
    provider.fail_dispatch = Some(1);
    assert!(owner.run(1, &mut provider).is_err());
    assert_eq!(
        owner.stage(),
        BankStage::Failed(BankStep::Dispatch { ordinal: 0 })
    );
    assert_eq!(owner.fields().word_4c, 0);
    let call = owner.calls()[0];
    assert_eq!(call.job, Some(JobOutcome::Existing));
    assert_eq!(call.valid_class, Some(true));
    assert_eq!(call.dispatch, None);
    assert!(!call.returned);
    assert!(!provider.events.contains(&Event::Session));
}

#[test]
fn level_failure_precedes_outer_slot_clear_and_absent_row_does_not_invent_a_primary() {
    let mut owner = BankOwner::from_snapshot(common_fields());
    let previous = owner.fields();
    let mut provider = Provider::new(None);
    provider.fail_level = true;
    assert!(owner.run(1, &mut provider).is_err());
    assert_eq!(owner.fields(), previous);
    assert!(owner.calls().is_empty());
    let mut provider = Provider::new(None);
    owner.run(1, &mut provider).unwrap();
    assert!(owner.calls().is_empty());
    assert_eq!(owner.fields().word_4c, 0);
    assert_eq!(provider.events, [Event::Level, Event::Session]);
}

#[test]
fn source_helper_return_does_not_claim_work_completion_or_asset_residency() {
    let mut owner = BankOwner::from_constructor();
    let mut provider = Provider::new(Some(row(0x2d50_0381)));
    provider.job = JobOutcome::Unavailable;
    provider.dispatch = DispatchOutcome::BackendAbsent;
    let receipt = owner.run(1, &mut provider).unwrap();
    assert_eq!(receipt.source_calls, 3);
    assert_eq!(receipt.jobs_unavailable, 3);
    assert_eq!(receipt.backends_absent, 3);
    assert_eq!(receipt.job_submissions, 0);
    assert_eq!(receipt.backend_submissions, 0);
    assert_eq!(owner.stage(), BankStage::Complete);
    assert_eq!(owner.fields().word_4c, 0x2d50_0381);
}
