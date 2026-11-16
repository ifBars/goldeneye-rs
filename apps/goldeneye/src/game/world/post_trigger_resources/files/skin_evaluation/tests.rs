use super::*;
use crate::game::world::post_trigger_resources::tests::{FILE, SKIN, bytes};
fn input(section: u16) -> Vec<u8> {
    let mut data = bytes();
    data[0x18..0x1c].copy_from_slice(&0x300u32.to_be_bytes());
    data[0x144..0x146].copy_from_slice(&section.to_be_bytes());
    data[0x148..0x14c].copy_from_slice(&0x200u32.to_be_bytes());
    data[0x200..0x204].copy_from_slice(&0x200u32.to_be_bytes());
    data[0x204..0x208].copy_from_slice(&u32::MAX.to_be_bytes()); // unused topology
    data
}
#[test]
fn mandatory_lookup_and_body_are_distinct_and_skip_topology() {
    let mut files = BTreeMap::new();
    let source = retain(&mut files, FILE, &mut |_| Edb::parse(input(0))).unwrap();
    let primary = ResourceBinding {
        file: FILE,
        reference: SKIN,
    };
    let mut adapter = source.evaluation_skin();
    let SkinLookup::Present(descriptor) = adapter.lookup(primary).unwrap() else {
        panic!("missing authored descriptor")
    };
    assert_eq!(adapter.body(descriptor).unwrap(), SkinBody::Resident);
    assert_eq!(adapter.generation(primary).unwrap(), descriptor.generation);
    assert!(
        adapter
            .lookup(ResourceBinding {
                file: FILE + 1,
                reference: SKIN
            })
            .is_err()
    );
    assert!(
        adapter
            .body(SkinDescriptor {
                generation: 1,
                ..descriptor
            })
            .is_err()
    );
}
#[test]
fn absent_descriptor_and_nonbase_pending_never_become_resident() {
    let mut files = BTreeMap::new();
    let source = retain(&mut files, FILE, &mut |_| Edb::parse(input(1))).unwrap();
    let mut adapter = source.evaluation_skin();
    assert_eq!(
        adapter
            .lookup(ResourceBinding {
                file: FILE,
                reference: SKIN + 1
            })
            .unwrap(),
        SkinLookup::Absent
    );
    let SkinLookup::Present(descriptor) = adapter
        .lookup(ResourceBinding {
            file: FILE,
            reference: SKIN,
        })
        .unwrap()
    else {
        panic!("missing authored descriptor")
    };
    assert_eq!(adapter.body(descriptor).unwrap(), SkinBody::Pending);
    assert!(files[&FILE].sections.is_empty());
}
