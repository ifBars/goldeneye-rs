use super::*;

fn request(file: u32) -> Request {
    Request {
        file,
        map_reference: MAP_REFERENCE,
    }
}

fn discovery(root: u32, files: usize) -> Discovery {
    Discovery::new(root, Budget { files }).unwrap()
}

#[test]
fn retained_resources_append_before_follow_and_keep_source_discovery_order() {
    let mut state = discovery(5, 3);
    assert_eq!(state.state(), State::WaitingForResource(request(5)));
    assert!(state.entries().is_empty());
    state.resource_ready(request(5)).unwrap();
    assert_eq!(state.entries(), &[request(5)]);
    assert_eq!(state.state(), State::WaitingForLink(request(5)));
    assert_eq!(
        state
            .linked(request(5), Some(Link { file: 2, map: 123 }))
            .unwrap(),
        State::WaitingForResource(request(2))
    );
    assert_eq!(state.entries(), &[request(5)]);
    state.resource_ready(request(2)).unwrap();
    state
        .linked(request(2), Some(Link { file: 9, map: 456 }))
        .unwrap();
    state.resource_ready(request(9)).unwrap();
    assert_eq!(
        state.linked(request(9), None).unwrap(),
        State::Complete(Completion::NoLink)
    );
    assert_eq!(state.entries(), &[request(5), request(2), request(9)]);
}

#[test]
fn file_only_repeat_stops_a_loop_even_when_the_returned_map_differs() {
    let mut state = discovery(1, 2);
    state.resource_ready(request(1)).unwrap();
    state
        .linked(request(1), Some(Link { file: 2, map: 0 }))
        .unwrap();
    state.resource_ready(request(2)).unwrap();
    // At the file budget, a repeat must still complete, rather than being
    // mistaken for another load or a budget failure.
    assert_eq!(
        state
            .linked(
                request(2),
                Some(Link {
                    file: 1,
                    map: 0x0500_1234
                })
            )
            .unwrap(),
        State::Complete(Completion::RepeatedFile { file: 1 })
    );
    assert_eq!(state.entries(), &[request(1), request(2)]);
}

#[test]
fn self_link_and_no_match_are_distinct_stops_and_never_append_twice() {
    for (link, completion) in [
        (None, Completion::NoLink),
        (
            Some(Link {
                file: 7,
                map: u32::MAX,
            }),
            Completion::RepeatedFile { file: 7 },
        ),
    ] {
        let mut state = discovery(7, 1);
        state.resource_ready(request(7)).unwrap();
        assert_eq!(
            state.linked(request(7), link).unwrap(),
            State::Complete(completion)
        );
        assert_eq!(state.entries(), &[request(7)]);
        let completed = state.clone();
        assert!(state.resource_ready(request(7)).is_err());
        assert!(state.linked(request(7), None).is_err());
        assert_eq!(state, completed);
    }
}

#[test]
fn missing_file_property_sentinel_is_requested_and_can_fail_loading() {
    let mut state = discovery(7, 2);
    state.resource_ready(request(7)).unwrap();
    assert_eq!(
        state
            .linked(
                request(7),
                Some(Link {
                    file: u32::MAX,
                    map: u32::MAX
                })
            )
            .unwrap(),
        State::WaitingForResource(request(u32::MAX))
    );
    assert_eq!(state.entries(), &[request(7)]);
    state.resource_failed(request(u32::MAX)).unwrap();
    assert_eq!(
        state.state(),
        State::Failed(Failure::ResourceUnavailable { file: u32::MAX })
    );
    assert_eq!(state.entries(), &[request(7)]);
    assert_eq!(
        discovery(u32::MAX, 1).state(),
        State::WaitingForResource(request(u32::MAX))
    );
}

#[test]
fn zero_and_exhausted_budgets_fail_instead_of_completing_discovery() {
    assert!(Discovery::new(1, Budget { files: 0 }).is_err());
    let mut state = discovery(1, 1);
    state.resource_ready(request(1)).unwrap();
    assert!(
        state
            .linked(request(1), Some(Link { file: 2, map: 0 }))
            .is_err()
    );
    assert_eq!(
        state.state(),
        State::Failed(Failure::FileBudgetExhausted { limit: 1 })
    );
    assert_eq!(state.entries(), &[request(1)]);
    let failed = state.clone();
    assert!(state.linked(request(1), None).is_err());
    assert!(state.resource_ready(request(2)).is_err());
    assert_eq!(state, failed);
}

#[test]
fn wrong_resource_phase_and_map_callbacks_do_not_mutate_state() {
    let mut state = discovery(1, 2);
    let waiting = state.clone();
    assert!(state.linked(request(1), None).is_err());
    assert!(state.resource_ready(request(2)).is_err());
    assert!(
        state
            .resource_ready(Request {
                file: 1,
                map_reference: 0x0500_0070
            })
            .is_err()
    );
    assert!(state.link_failed(request(1)).is_err());
    assert_eq!(state, waiting);
    state.resource_ready(request(1)).unwrap();
    let retained = state.clone();
    assert!(state.resource_ready(request(1)).is_err());
    assert!(state.resource_failed(request(1)).is_err());
    assert!(state.linked(request(2), None).is_err());
    assert!(
        state
            .linked(
                Request {
                    file: 1,
                    map_reference: 0
                },
                None
            )
            .is_err()
    );
    assert_eq!(state, retained);
    state
        .linked(
            request(1),
            Some(Link {
                file: 2,
                map: 0x0500_0070,
            }),
        )
        .unwrap();
    let next = state.clone();
    assert!(state.linked(request(1), None).is_err());
    assert!(state.resource_ready(request(1)).is_err());
    assert_eq!(state, next);
}

#[test]
fn lookup_failure_is_latched_after_retention_without_becoming_no_link() {
    let mut state = discovery(8, 1);
    state.resource_ready(request(8)).unwrap();
    state.link_failed(request(8)).unwrap();
    assert_eq!(
        state.state(),
        State::Failed(Failure::LinkLookupFailed { file: 8 })
    );
    assert_eq!(state.entries(), &[request(8)]);
    let failed = state.clone();
    assert!(state.linked(request(8), None).is_err());
    assert!(state.resource_failed(request(8)).is_err());
    assert_eq!(state, failed);
}
