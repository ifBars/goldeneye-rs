use super::*;
use crate::mission::world::{MapReference, WrapperId};
fn key() -> Key {
    Key {
        wrapper: WrapperId::new(21).unwrap(),
        map_reference: MapReference::from_directory_index(0).unwrap(),
    }
}
fn input() -> Initializer {
    Initializer {
        declared_bounds_bits: [[0x8000_0000, 7, 9], [1, 2, 3]],
        zones: vec![ZoneInput {
            initial_values_bits: [0x7fc0_1234, 0x8000_0000, 4, 5],
            flag_word10_raw: 0x8000_0000,
            word2c_raw: 0x1234,
        }],
        placement_count: 33,
        component_slots: 2,
        auxiliary_records: 0,
        bitset_bits: 0,
        section_count: 3,
    }
}
#[test]
fn constructor_retains_raw_defaults_and_actual_null_storage() {
    let m = Instance::from_constructor(
        key(),
        InstanceId::new(9).unwrap(),
        4,
        input(),
        Budget::default(),
    )
    .unwrap();
    let z = &m.zones()[0];
    assert_eq!(m.identity().get(), 9);
    assert_eq!(m.key().wrapper.get(), 21);
    assert_eq!(z.handle, 0x400);
    assert_eq!(z.current_values_bits, z.initial_values_bits);
    assert_eq!(z.initial_values_bits[0], 0x7fc0_1234);
    assert!(z.flag_28 && z.flag_40);
    assert_eq!(z.word_24, z.word_3c);
    assert_eq!(z.self_ordinal, 0);
    assert_eq!(z.links, [None; 4]);
    assert_eq!(m.component_slots(), &[None, None]);
    assert_eq!(m.section_cache(), &[0, 0]);
    assert!(!m.publication().zones()[0].initialized());
    assert_eq!(
        m.sections().status(1).unwrap(),
        sections::Status::Unrequested
    );
}
#[test]
fn invalidation_clears_caches_but_preserves_current_values_components_and_publication_owner() {
    let mut m = Instance::from_constructor(
        key(),
        InstanceId::new(2).unwrap(),
        1,
        input(),
        Budget::default(),
    )
    .unwrap();
    m.zones[0].links[0] = Some(0);
    m.zones[0].cache_flags = [1; 4];
    m.zones[0].current_values_bits[2] = 99;
    m.section_cache[0] = 7;
    m.fixed_cache[1] = 8;
    m.component_slots[0] = NonZeroU64::new(5);
    let publication_address = m.publication() as *const _;
    m.invalidate();
    assert_eq!(m.zones()[0].links, [None; 4]);
    assert_eq!(m.zones()[0].cache_flags, [0; 4]);
    assert_eq!(m.zones()[0].current_values_bits[2], 99);
    assert_eq!(m.component_slots()[0].unwrap().get(), 5);
    assert_eq!(m.section_cache(), &[0, 0]);
    assert_eq!(m.fixed_cache(), &[0; 32]);
    assert_eq!(publication_address, m.publication() as *const _);
}
#[test]
fn host_limits_and_unimplemented_nonempty_auxiliary_branches_are_errors() {
    let id = InstanceId::new(1).unwrap();
    assert!(Instance::from_constructor(key(), id, 0, input(), Budget::default()).is_err());
    assert!(
        Instance::from_constructor(
            key(),
            id,
            1,
            input(),
            Budget {
                placements: 32,
                ..Budget::default()
            }
        )
        .is_err()
    );
    let mut nonempty = input();
    nonempty.auxiliary_records = 1;
    assert!(
        Instance::from_constructor(
            key(),
            id,
            1,
            nonempty,
            Budget {
                auxiliary_records: 2,
                ..Budget::default()
            }
        )
        .is_err()
    );
    let mut absent = input();
    absent.section_count = 0;
    assert!(Instance::from_constructor(key(), id, 1, absent, Budget::default()).is_err());
}
#[test]
fn canonical_state_visitor_observes_actual_mutable_words() {
    let mut m = Instance::from_constructor(
        key(),
        InstanceId::new(1).unwrap(),
        3,
        input(),
        Budget::default(),
    )
    .unwrap();
    let mut before = Vec::new();
    m.visit_state_words(|v| before.push(v));
    m.zones[0].current_values_bits[0] = 0xdead_beef;
    let mut after = Vec::new();
    m.visit_state_words(|v| after.push(v));
    assert_eq!(before.len(), after.len());
    assert_eq!(before.iter().zip(&after).filter(|(a, b)| a != b).count(), 1);
    assert!(after.contains(&0xdead_beef));
}
