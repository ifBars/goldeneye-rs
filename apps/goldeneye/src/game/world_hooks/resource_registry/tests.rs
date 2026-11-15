use super::super::Shared;

#[test]
fn cancellation_and_reentry_retain_the_same_resource_registry() {
    let shared = Shared::from_constructor();
    let first = shared.begin(1).unwrap();
    let original = first.resource_registry().unwrap();
    assert!(original.same_owner(&first.resource_registry().unwrap()));

    shared.invalidate();
    assert!(first.resource_registry().is_err());
    let second = shared.begin(2).unwrap();
    let retained = second.resource_registry().unwrap();
    assert!(original.same_owner(&retained));
    assert!(first.resource_registry().is_err());

    // A different controller owns a separate resource lifetime.
    let other = Shared::from_constructor();
    let independent = other.begin(1).unwrap().resource_registry().unwrap();
    assert!(!original.same_owner(&independent));
}

#[test]
fn resource_registry_access_does_not_wait_for_the_manager_state_lock() {
    let shared = Shared::from_constructor();
    let attempt = shared.begin(1).unwrap();
    let guard = shared.0.state.lock().unwrap();
    let (send, receive) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        send.send(attempt.resource_registry().is_ok()).unwrap();
    });
    let result = receive.recv_timeout(std::time::Duration::from_secs(2));
    // Release before joining even on failure so a regression cannot deadlock.
    drop(guard);
    worker.join().unwrap();
    assert!(result.unwrap());
}
