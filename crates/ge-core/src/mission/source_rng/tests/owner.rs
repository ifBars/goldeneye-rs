use super::*;

#[test]
fn authored_unsigned_vectors_check_packed_halves_and_stored_second_step() {
    for (seed, output, stored) in [
        (0, 0x0026_1e27, 0x1e27_8e7a),
        (1, 0x0029_c823, 0xc823_f683),
        (u32::MAX, 0x0023_742b, 0x742b_2671),
        (0x8000_0000, 0x8026_9e27, 0x9e27_8e7a),
    ] {
        let mut rng = SourceRngOwner::from_snapshot(seed);
        assert_eq!(rng.origin(), Origin::SuppliedSnapshot);
        assert_eq!(rng.draw_u32(Budget { draws: 1 }).unwrap(), output);
        assert_eq!(rng.state_raw(), stored);
        assert_eq!(rng.draws_since_snapshot(), 1);
    }
}

#[test]
fn constructor_snapshot_sequence_is_explicit_and_resumes_from_stored_state() {
    let mut rng = SourceRngOwner::from_constructor();
    assert_eq!(rng.origin(), Origin::FreshConstructor);
    assert_eq!(rng.state_raw(), 0x955);
    assert_eq!(rng.draw_u32(Budget { draws: 3 }).unwrap(), 0x1ea0_6e9e);
    let mut resumed = SourceRngOwner::from_snapshot(rng.state_raw());
    assert_eq!(rng.draw_u32(Budget { draws: 3 }).unwrap(), 0x07f7_b91d);
    assert_eq!(resumed.draw_u32(Budget { draws: 2 }).unwrap(), 0x07f7_b91d);
    assert_eq!(rng.draw_u32(Budget { draws: 3 }).unwrap(), 0xa285_5273);
    assert_eq!(resumed.draw_u32(Budget { draws: 2 }).unwrap(), 0xa285_5273);
    assert_eq!(rng.state_raw(), resumed.state_raw());
    assert_eq!(rng.draws_since_snapshot(), 3);
    assert_eq!(resumed.draws_since_snapshot(), 2);
}

#[test]
fn failed_budget_draw_does_not_modify_state_or_counter() {
    let mut rng = SourceRngOwner::from_snapshot(0);
    assert!(rng.draw_u32(Budget { draws: 0 }).is_err());
    assert_eq!((rng.state_raw(), rng.draws_since_snapshot()), (0, 0));
    rng.draw_u32(Budget { draws: 1 }).unwrap();
    let before = rng.state_raw();
    assert!(rng.draw_u32(Budget { draws: 1 }).is_err());
    assert_eq!((rng.state_raw(), rng.draws_since_snapshot()), (before, 1));
}

#[test]
fn counter_limit_is_checked_before_state_mutation() {
    let mut rng = SourceRngOwner {
        state: 7,
        draws: u64::MAX - 1,
        origin: Origin::SuppliedSnapshot,
    };
    rng.draw_u32(Budget { draws: u64::MAX }).unwrap();
    let before = rng.state_raw();
    assert!(rng.draw_u32(Budget { draws: u64::MAX }).is_err());
    assert_eq!(
        (rng.state_raw(), rng.draws_since_snapshot()),
        (before, u64::MAX)
    );
}
