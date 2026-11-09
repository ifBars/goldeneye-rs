use super::*;
use crate::edb::Edb;
fn put(data: &mut [u8], at: usize, v: u32) {
    data[at..at + 4].copy_from_slice(&v.to_be_bytes());
}
fn fixture() -> Vec<u8> {
    let mut d = crate::fixtures::graph_fixture();
    d.resize(0x600, 0);
    for (p, v) in [
        (20, 0x600),
        (24, 0x600),
        (0x70, 0x10000),
        (0x74, 0x280 - 0x74),
        (0x280, 0x85000000),
        (0x288, 0x2a0),
        (0x2a0, 0x500),
        (0x2e8, 2),
        (0x2ec, 0x340 - 0x2ec),
    ] {
        put(&mut d, p, v);
    }
    for (i, x) in [10f32, 20.].into_iter().enumerate() {
        let p = 0x340 + i * 60;
        put(&mut d, p, 0x02000100 + i as u32);
        put(&mut d, p + 48, 0x82000000);
        for (axis, v) in [x, 3., -4.].into_iter().enumerate() {
            put(&mut d, p + 4 + axis * 4, v.to_bits());
        }
    }
    d
}
#[test]
fn ordered_registrations_share_mesh_but_recover_separate_positions_without_render_matrices() {
    let mut d = fixture();
    put(&mut d, 0x354, f32::NAN.to_bits()); // Unused rotation is not a binding dependency.
    let m = Edb::parse(d).unwrap().map_navigation(0x85000000).unwrap();
    assert_eq!(
        (
            m.placements,
            m.registrations.len(),
            m.unresolved_entity_references
        ),
        (2, 2, 0)
    );
    for (i, r) in m.registrations.iter().enumerate() {
        assert_eq!(r.source_placement, i);
        assert!(
            matches!(r.binding,NavigationBinding::Ready {entity_index:0,resolved_placement,translation} if resolved_placement==i && translation==[10.+i as f32*10.,3.,-4.])
        );
    }
}
#[test]
fn duplicate_instance_hashes_resolve_first_match_and_registrations_are_not_deduplicated() {
    let mut d = fixture();
    put(&mut d, 0x37c, 0x02000100);
    let m = Edb::parse(d).unwrap().map_navigation(0x85000000).unwrap();
    assert_eq!(m.registrations.len(), 2);
    for r in m.registrations {
        assert!(matches!(
            r.binding,
            NavigationBinding::Ready {
                resolved_placement: 0,
                translation: [10., 3., -4.],
                ..
            }
        ));
    }
}
#[test]
fn numeric_instance_keys_are_indices_and_missing_bindings_are_visible() {
    let mut d = fixture();
    put(&mut d, 0x340, 1);
    let m = Edb::parse(d.clone())
        .unwrap()
        .map_navigation(0x85000000)
        .unwrap();
    assert!(matches!(
        m.registrations[0].binding,
        NavigationBinding::Ready {
            resolved_placement: 1,
            translation: [20., 3., -4.],
            ..
        }
    ));
    put(&mut d, 0x3ac, 0x82000001); // Second source entity reference is out of range.
    let m = Edb::parse(d.clone())
        .unwrap()
        .map_navigation(0x85000000)
        .unwrap();
    assert_eq!(
        (m.registrations.len(), m.unresolved_entity_references),
        (1, 1)
    );
    assert!(matches!(
        m.registrations[0].binding,
        NavigationBinding::EntityMissing {
            resolved_placement: 1
        }
    ));
    put(&mut d, 0x340, 3);
    let m = Edb::parse(d).unwrap().map_navigation(0x85000000).unwrap();
    assert!(matches!(
        m.registrations[0].binding,
        NavigationBinding::InstanceMissing
    ));
}
#[test]
fn malformed_ranges_positions_and_ambiguous_map_selection_are_errors() {
    for (at, v) in [
        (0x344, f32::NAN.to_bits()),
        (0x2e8, u32::MAX),
        (0x2ec, 0x7fffffff),
    ] {
        let mut d = fixture();
        put(&mut d, at, v);
        assert!(Edb::parse(d).unwrap().map_navigation(0x85000000).is_err());
    }
    let mut d = fixture();
    put(&mut d, 0x70, 0x20000);
    d.copy_within(0x280..0x290, 0x290);
    let mut edb = Edb::parse(d).unwrap();
    assert_eq!(edb.map_hashes().unwrap(), vec![0x85000000, 0x85000000]);
    assert!(edb.map_navigation(0x85000000).is_err());
    edb.tables.clear();
    assert!(edb.map_hashes().is_err());
}

#[test]
fn a_registered_wrapper_can_resolve_to_a_non_navigation_instance_without_fallback() {
    let mut d = fixture();
    put(&mut d, 0x50, 0x20000);
    put(&mut d, 0xe4, 0x12345678);
    put(&mut d, 0xec, 0x500);
    put(&mut d, 0x500, 0x601);
    put(&mut d, 0x340, 1);
    put(&mut d, 0x3ac, 0x82000001);
    let m = Edb::parse(d).unwrap().map_navigation(0x85000000).unwrap();
    assert_eq!(m.registrations.len(), 1);
    assert!(matches!(
        m.registrations[0].binding,
        NavigationBinding::WrongEntityType {
            resolved_placement: 1,
            entity_index: 1,
            entity_type: 0x601
        }
    ));
}
