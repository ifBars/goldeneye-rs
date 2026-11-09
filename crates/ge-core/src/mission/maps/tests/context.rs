use super::*;
#[test]
fn bindings_use_instance_identity_and_first_absent_secondary_slot_preserve_second_masks() {
    let mut owner = Owner::from_constructor();
    let mut contexts = Contexts::from_snapshot([[9; 8]; 4], [[0xa5; 8]; 4]);
    assert!(
        contexts
            .bind_secondary(
                &mut owner,
                InstanceId::new(77).unwrap(),
                &SectionKeys {
                    count_raw: 0,
                    keys: vec![]
                }
            )
            .is_err()
    );
    contexts.clear_leading(&mut owner);
    let keys = SectionKeys {
        count_raw: 4,
        keys: vec![0x0800_0000, 0x0800_0021, 0x0100_0007, 0x8800_00ff],
    };
    assert_eq!(
        contexts
            .bind_secondary(&mut owner, InstanceId::new(77).unwrap(), &keys)
            .unwrap(),
        1
    );
    assert_eq!(
        contexts
            .bind_secondary(&mut owner, InstanceId::new(88).unwrap(), &keys)
            .unwrap(),
        2
    );
    contexts
        .bind_primary(&mut owner, InstanceId::new(99).unwrap(), &keys)
        .unwrap();
    assert_eq!(
        owner.contexts()[0],
        ContextSlot::Bound(InstanceId::new(99).unwrap())
    );
    assert_eq!(
        contexts.first_mask(0).unwrap(),
        &[1, 2, 0, 0, 0, 0, 0, 0x8000_0000]
    );
    contexts.clear_leading(&mut owner);
    assert_eq!(contexts.second_mask(0).unwrap(), &[0xa5; 8]);
    assert_eq!(contexts.first_mask(0).unwrap()[1], 2);
}
#[test]
fn failure_preserves_reached_reference_store_and_prior_complete_mask() {
    let mut owner = Owner::from_constructor();
    let mut c = Contexts::from_snapshot([[7; 8]; 4], [[8; 8]; 4]);
    let id = InstanceId::new(3).unwrap();
    let bad = SectionKeys {
        count_raw: 1,
        keys: vec![0x0800_0100],
    };
    assert!(c.bind_primary(&mut owner, id, &bad).is_err());
    assert_eq!(owner.contexts()[0], ContextSlot::Bound(id));
    assert_eq!(c.first_mask(0).unwrap(), &[7; 8]);
    assert_eq!(c.second_mask(0).unwrap(), &[8; 8]);
}
#[test]
fn signed_nonpositive_count_is_lazy_and_secondary_exhaustion_does_not_overwrite() {
    let mut owner = Owner::from_constructor();
    let mut c = Contexts::from_constructor();
    c.clear_leading(&mut owner);
    let keys = SectionKeys {
        count_raw: -1,
        keys: vec![0x0800_ffff],
    };
    for i in 1..4 {
        assert_eq!(
            c.bind_secondary(&mut owner, InstanceId::new(i as u64).unwrap(), &keys)
                .unwrap(),
            i
        );
    }
    let prior = *owner.contexts();
    assert!(
        c.bind_secondary(&mut owner, InstanceId::new(9).unwrap(), &keys)
            .is_err()
    );
    assert_eq!(*owner.contexts(), prior);
    assert_eq!(c.first_mask(1).unwrap(), &[0; 8]);
}
