use super::super::tests::{FILE, SKIN, TEXTURE, bytes, operation};
use super::*;

#[test]
fn later_player_requests_share_actual_payload_and_section_owners_after_world_drop() {
    let mut world = Owned::build_with(operation(SKIN, TEXTURE), |_| Edb::parse(bytes())).unwrap();
    let evidence = world.evidence().clone();
    let first = world
        .request_target_with(FILE, SKIN, |_| panic!("duplicate file load"))
        .unwrap();
    let second = world
        .request_target_with(FILE, TEXTURE, |_| panic!("duplicate file load"))
        .unwrap();
    assert!(Arc::ptr_eq(&first.source, &second.source));
    assert!(Arc::ptr_eq(
        first.section.as_ref().unwrap(),
        second.section.as_ref().unwrap()
    ));
    assert_eq!(world.evidence(), &evidence);
    assert_eq!(first.file_bytes(), 0x300);
    assert_eq!(first.section_bytes(), 16);
    assert_ne!(first.fingerprint(), second.fingerprint());
    drop(world);
    assert_eq!(first.file(), FILE);
    assert_eq!(first.target(), SKIN);
    let expected: [u8; 32] = Sha256::digest(b"authored payload").into();
    assert_eq!(first.section.as_ref().unwrap().sha256, expected);
}

#[test]
fn missing_targets_keep_world_hook_receipt_without_creating_a_ready_target() {
    let mut world = Owned::build_with(operation(SKIN, TEXTURE), |_| Edb::parse(bytes())).unwrap();
    assert!(
        world
            .request_target_with(FILE, 0x0d00_ffff, |_| panic!("duplicate file load"))
            .is_err()
    );
    assert!(world.is_complete());
    assert_eq!(world.evidence().resolved_requests, 2);
}

#[test]
fn postbind_file_lookup_keeps_the_same_initial_visual_target_wrapper() {
    let mut world = Owned::build_with(operation(SKIN, TEXTURE), |_| Edb::parse(bytes())).unwrap();
    let source = super::super::files::retain(&mut world.files, FILE, &mut |_| {
        panic!("duplicate wrapper load")
    })
    .unwrap();
    let target = world
        .request_target_with(FILE, SKIN, |_| panic!("duplicate wrapper load"))
        .unwrap();
    assert!(source.same_target(&target));
    assert!(source.skin_body(SKIN).is_err());
}
