use crate::game::world_hooks::Shared;

#[test]
fn process_scalar_owner_and_unknown_current_survive_retry_without_boot_fallback() {
    let shared = Shared::from_constructor();
    let first = shared.begin(1).unwrap();
    let initial = first.preload_scalar_facts(None).unwrap();
    assert_ne!(initial.owner, 0);
    assert_eq!(initial.initial_bits, 1f32.to_bits());
    assert_eq!(initial.current_bits, None);
    assert_eq!(initial.reads, 0);
    assert_eq!(initial.capture_issued, None);
    let capture = shared
        .0
        .state
        .lock()
        .unwrap()
        .animation_preload_scalar
        .read()
        .unwrap();
    let captured = first.preload_scalar_facts(Some(&capture)).unwrap();
    assert_eq!(captured.owner, initial.owner);
    assert_eq!(captured.reads, 1);
    assert_eq!(captured.current_bits, None);
    assert_eq!(captured.capture_issued, Some(true));
    shared.invalidate();
    assert!(first.preload_scalar_facts(Some(&capture)).is_err());
    let retry = shared.begin(2).unwrap();
    let retained = retry.preload_scalar_facts(Some(&capture)).unwrap();
    assert_eq!(retained, captured);
    let next = shared
        .0
        .state
        .lock()
        .unwrap()
        .animation_preload_scalar
        .read()
        .unwrap();
    let current = retry.preload_scalar_facts(Some(&next)).unwrap();
    assert_eq!(current.owner, retained.owner);
    assert_eq!(current.reads, 2);
    assert_eq!(current.current_bits, None);
    assert_eq!(capture.read_sequence(), 1);
    assert_eq!(capture.current_bits(), None);
    assert_eq!(next.read_sequence(), 2);
    assert_eq!(
        retry
            .preload_scalar_facts(Some(&capture))
            .unwrap()
            .capture_issued,
        Some(true)
    );
}

#[test]
fn diagnostic_process_snapshot_never_mints_a_scalar_capture_or_source_read() {
    let shared = Shared::from_constructor();
    let hook = shared.begin(1).unwrap();
    let before = hook.preload_scalar_facts(None).unwrap();
    for _ in 0..3 {
        assert_eq!(hook.preload_scalar_facts(None).unwrap(), before);
    }
    assert_eq!(
        shared
            .0
            .state
            .lock()
            .unwrap()
            .animation_preload_scalar
            .reads(),
        0
    );
}
