use super::super::tests::{FILE, SKIN, TEXTURE, bytes, operation};
use super::*;

#[test]
fn file_only_lookup_reuses_world_wrapper_and_survives_registry_drop_without_new_target() {
    let mut world = Owned::build_with(operation(SKIN, TEXTURE), |_| Edb::parse(bytes())).unwrap();
    let before = world.evidence().clone();
    let first = retain(&mut world.files, FILE, &mut |_| {
        panic!("duplicate file read")
    })
    .unwrap();
    let second = retain(&mut world.files, FILE, &mut |_| {
        panic!("duplicate file read")
    })
    .unwrap();
    assert!(Arc::ptr_eq(&first.source, &second.source));
    assert!(Arc::ptr_eq(&first.source, &world.files[&FILE].edb));
    assert_eq!(world.files[&FILE].sections.len(), 1);
    assert_eq!(world.evidence(), &before);
    drop(world);
    assert_eq!(first.file(), FILE);
    assert_eq!(first.file_bytes(), 0x300);
    assert!(first.skin(SKIN).is_err()); // non-base remains a residency dependency
}

#[test]
fn absent_wrong_or_failed_file_is_not_retained_as_a_successful_lookup() {
    let mut files = BTreeMap::new();
    assert!(retain(&mut files, FILE + 1, &mut |_| Edb::parse(bytes())).is_err());
    assert!(files.is_empty());
    assert!(
        retain(&mut files, 0x0200_0001, &mut |_| panic!(
            "wrong prefix read"
        ))
        .is_err()
    );
    assert!(
        retain(&mut files, FILE, &mut |_| anyhow::bail!(
            "authored I/O failure"
        ))
        .is_err()
    );
    assert!(files.is_empty());
    let source = retain(&mut files, FILE, &mut |_| Edb::parse(bytes())).unwrap();
    assert_eq!(source.file(), FILE);
}

#[test]
fn base_skin_lookup_owns_checked_topology_and_reads_auxiliary_count_lazily() {
    let mut data = bytes();
    let word = |data: &mut [u8], at: usize, value: u32| {
        data[at..at + 4].copy_from_slice(&value.to_be_bytes())
    };
    word(&mut data, 0x18, 0x300);
    data[0x144..0x146].copy_from_slice(&0u16.to_be_bytes());
    word(&mut data, 0x148, 0x200);
    word(&mut data, 0x200, 0x200);
    word(&mut data, 0x204, 1);
    word(&mut data, 0x248, 0x48); // relative hierarchy, at290
    word(&mut data, 0x280, 16);
    data[0x290..0x298].copy_from_slice(&[0xff, 0xff, 0, 0, 0, 7, 0, 9]);
    let mut files = BTreeMap::new();
    let source = retain(&mut files, FILE, &mut |_| Edb::parse(data.clone())).unwrap();
    let skin = source.skin(SKIN).unwrap();
    let body = source.skin_body(SKIN).unwrap();
    assert_eq!(body.file(), FILE);
    assert_eq!(body.reference(), SKIN);
    assert_eq!(skin.bone_count(), 1);
    assert_eq!(skin.bones()[0].parent_raw, u16::MAX);
    assert_eq!(skin.bones()[0].word_6_raw, 9);
    assert_eq!(source.auxiliary_words(&skin).unwrap(), 16);
    assert!(source.skin(SKIN + 1).is_err());
    assert!(files[&FILE].sections.is_empty());
    let mut nonresident = data.clone();
    word(&mut nonresident, 0x18, 0x200);
    let mut other_files = BTreeMap::new();
    let other = retain(&mut other_files, FILE, &mut |_| {
        Edb::parse(nonresident.clone())
    })
    .unwrap();
    assert!(other.skin_body(SKIN).is_err());
}
