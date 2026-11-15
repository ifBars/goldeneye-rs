use super::super::{
    Owned, files,
    tests::{FILE, SKIN, TEXTURE, bytes, operation},
};
use super::*;
use ge_assets::edb::Edb;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn new_world_attempt_reuses_wrappers_sections_and_failed_request_history() {
    let registry = Registry::from_constructor();
    let loads = AtomicUsize::new(0);
    let first = Owned::build_with_registry(
        operation(SKIN, TEXTURE),
        |_| {
            loads.fetch_add(1, Ordering::Relaxed);
            Edb::parse(bytes())
        },
        registry.clone(),
    )
    .unwrap();
    let source = registry
        .with_files(|files| files::retain(files, FILE, &mut |_| panic!("cached file reloaded")))
        .unwrap();
    let before = first.evidence().clone();
    let original = registry
        .with_files(|files| Ok(Arc::clone(&files[&FILE].edb)))
        .unwrap();
    // An unrecognized real request leaves a retained unknown prefix.
    assert!(
        registry
            .with_files(|files| super::super::resolve(
                Request {
                    ordinal: 0,
                    file: FILE,
                    target: 0x0400_0001
                },
                files,
                &mut |_| panic!("cached file reloaded")
            ))
            .is_err()
    );
    drop(first);
    let second = Owned::build_with_registry(
        operation(SKIN, TEXTURE),
        |_| panic!("retry reloaded file"),
        registry.clone(),
    )
    .unwrap();
    let retained = registry
        .with_files(|files| Ok(Arc::clone(&files[&FILE].edb)))
        .unwrap();
    assert!(Arc::ptr_eq(&original, &retained));
    assert_eq!(loads.load(Ordering::Relaxed), 1);
    assert_eq!(second.evidence(), &before);
    assert!(retained.publication.has_unknown_history().unwrap());
    drop(second);
    assert_eq!(source.file(), FILE);
}

#[test]
fn operation_snapshot_excludes_unrelated_prior_files_and_sections() {
    let registry = Registry::from_constructor();
    let first = Owned::build_with_registry(
        operation(SKIN, TEXTURE),
        |_| Edb::parse(bytes()),
        registry.clone(),
    )
    .unwrap();
    let snapshot = first.evidence().clone();
    registry
        .with_files(|files| {
            files::retain(files, FILE + 1, &mut |_| {
                let mut data = bytes();
                data[4..8].copy_from_slice(&(FILE + 1).to_be_bytes());
                Edb::parse(data)
            })?;
            Ok(())
        })
        .unwrap();
    let second = Owned::build_with_registry(
        operation(SKIN, TEXTURE),
        |_| panic!("same request reloaded file"),
        registry,
    )
    .unwrap();
    assert_eq!(first.evidence(), &snapshot);
    assert_eq!(second.evidence(), &snapshot);
    assert_eq!(second.evidence().retained_files, 1);
}
