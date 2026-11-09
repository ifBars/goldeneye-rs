use super::*;

fn header() -> Header {
    Header {
        statement_count_raw: 3,
        variables: 3,
        shared_variables: 2,
        states: 1,
        format: 0x38,
        event_ff: 0xff,
        event_create: 0,
        event_105: 0xff,
        event_10e: 0xff,
    }
}

fn context() -> FreshContext {
    FreshContext {
        trigger_handle: 7,
        map_selector: 2,
        position_bits: [0x8000_0000, 0x3f80_0000, 0, 0],
        manager_present: true,
        camera: CameraContext::KnownMissing,
    }
}

fn budget() -> Budget {
    Budget {
        variables: 8,
        states: 8,
    }
}

#[test]
fn fresh_state_owns_shared_zero_and_unwritten_extra_without_execution() {
    let value = Runtime::fresh_startpoint(header(), context(), budget()).unwrap();
    assert_eq!(value.shared_variables(), [0, 0]);
    assert_eq!(value.extra_scratch(), Some([None, None].as_slice()));
    assert_eq!(value.state_flags(), [0]);
    assert_eq!(value.execution_slots(), &[None; 16]);
    assert_eq!(
        value.source_state().constructor_create,
        EventOutcome::Denied
    );
    assert_eq!(
        value.source_state().registration_event,
        EventOutcome::Unmapped
    );
    assert_eq!(value.source_state().active_executions, 0);
    assert_eq!(value.context().position_bits[0], 0x8000_0000);
}

#[test]
fn denied_create_does_not_require_a_valid_program_or_state() {
    let mut h = header();
    h.states = 0;
    h.statement_count_raw = 0;
    h.event_create = 0xee;
    let value = Runtime::fresh_startpoint(h, context(), budget()).unwrap();
    assert_eq!(
        value.evaluate_create(OwnerBinding::Unbound, 4, 4),
        EventOutcome::Denied
    );
    assert!(value.execution_slots().iter().all(Option::is_none));
}

#[test]
fn later_matching_owner_reports_immediate_execution_without_mutation() {
    let value = Runtime::fresh_startpoint(header(), context(), budget()).unwrap();
    let before = value.source_state();
    let player = OwnerBinding::PlayerBasic(NonZeroU64::new(9).unwrap());
    assert_eq!(
        value.evaluate_create(player, 4, 4),
        EventOutcome::UnsupportedImmediateExecution { state: 0 }
    );
    assert_eq!(value.source_state(), before);
    assert!(value.execution_slots().iter().all(Option::is_none));
    assert_eq!(value.evaluate_create(player, 4, 5), EventOutcome::Denied);
    assert_eq!(
        value.evaluate_create(OwnerBinding::Other(NonZeroU64::new(8).unwrap()), 4, 4),
        EventOutcome::Denied
    );
}

#[test]
fn valid_registration_state_is_an_unsupported_initializer() {
    let mut h = header();
    h.event_ff = 0;
    assert!(Runtime::fresh_startpoint(h, context(), budget()).is_err());
    h.event_ff = 1;
    assert!(Runtime::fresh_startpoint(h, context(), budget()).is_ok());
}

#[test]
fn constructor_requires_actual_manager_identity_format_and_budgets() {
    let mut ctx = context();
    ctx.manager_present = false;
    assert!(Runtime::fresh_startpoint(header(), ctx, budget()).is_err());
    ctx = context();
    ctx.trigger_handle = 0;
    assert!(Runtime::fresh_startpoint(header(), ctx, budget()).is_err());
    let mut h = header();
    h.format = 0x30;
    assert!(Runtime::fresh_startpoint(h, context(), budget()).is_err());
    assert!(
        Runtime::fresh_startpoint(
            header(),
            context(),
            Budget {
                variables: 2,
                states: 8
            }
        )
        .is_err()
    );
    assert!(
        Runtime::fresh_startpoint(
            header(),
            context(),
            Budget {
                variables: 8,
                states: 0
            }
        )
        .is_err()
    );
}

#[test]
fn optional_camera_and_source_callback_flags_are_retained() {
    let camera = NonZeroU64::new(11).unwrap();
    let mut ctx = context();
    ctx.camera = CameraContext::Bound(camera);
    let mut h = header();
    h.event_105 = 0;
    h.event_10e = 0;
    let value = Runtime::fresh_startpoint(h, ctx, budget()).unwrap();
    assert_eq!(value.context().camera, CameraContext::Bound(camera));
    assert_eq!(value.source_state().callback_flags_raw, 3);
    assert_eq!(value.source_state().unique_states_raw, 1);
    assert_eq!(value.source_state().execution_budget_raw, 5);
    assert_eq!(value.source_state().selected_marker_raw, 0xff);
}

#[test]
fn no_extra_scratch_when_shared_count_covers_all_variables() {
    let mut h = header();
    h.variables = h.shared_variables;
    let value = Runtime::fresh_startpoint(h, context(), budget()).unwrap();
    assert!(value.extra_scratch().is_none());
    h.event_create = 0xff;
    let value = Runtime::fresh_startpoint(h, context(), budget()).unwrap();
    assert_eq!(
        value.evaluate_create(OwnerBinding::PlayerBasic(NonZeroU64::new(9).unwrap()), 1, 1),
        EventOutcome::Unmapped
    );
}
