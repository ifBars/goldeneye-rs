use crate::mission::loader::checkpoint::{
    Budget, Origin, Owner, PAYLOAD_BYTES, UnsupportedRestore,
};

#[test]
fn real_unwritten_constructor_payload_supports_both_fresh_skips_without_zero_data() {
    let owner = Owner::from_constructor(Budget {
        payload_bytes: PAYLOAD_BYTES,
    })
    .unwrap();
    assert_eq!(owner.origin(), Origin::FreshConstructor);
    assert_eq!(owner.payload_len(), PAYLOAD_BYTES);
    assert!(!owner.payload_initialized());
    assert_eq!((owner.valid_raw(), owner.restore_active_raw()), (0, 0));
    owner.require_not_restoring().unwrap(); // case7
    owner.require_not_restoring().unwrap(); // case10: same retained owner
    assert!(!owner.payload_initialized());
    assert!(!owner.valid());
    assert!(!owner.restore_active());
    assert!(
        Owner::from_constructor(Budget {
            payload_bytes: PAYLOAD_BYTES - 1
        })
        .is_err()
    );
}

#[test]
fn explicit_retained_active_state_is_not_silently_replaced_with_the_fresh_skip() {
    for active in [1, 0x80, 0xff] {
        let owner = Owner::from_snapshot(
            1,
            active,
            vec![0xa7; PAYLOAD_BYTES],
            Budget {
                payload_bytes: PAYLOAD_BYTES,
            },
        )
        .unwrap();
        assert_eq!(owner.origin(), Origin::SuppliedSnapshot);
        assert!(owner.payload_initialized());
        assert!(owner.valid());
        assert!(owner.restore_active());
        let error = owner.require_not_restoring().unwrap_err();
        assert_eq!(
            error.downcast_ref::<UnsupportedRestore>(),
            Some(&UnsupportedRestore)
        );
        assert_eq!(owner.restore_active_raw(), active);
    }
    let inactive = Owner::from_snapshot(
        1,
        0,
        vec![0xff; PAYLOAD_BYTES],
        Budget {
            payload_bytes: PAYLOAD_BYTES,
        },
    )
    .unwrap();
    inactive.require_not_restoring().unwrap();
    assert!(inactive.valid());
    assert!(
        Owner::from_snapshot(
            1,
            0,
            vec![1; 5],
            Budget {
                payload_bytes: PAYLOAD_BYTES
            }
        )
        .is_err()
    );
}
