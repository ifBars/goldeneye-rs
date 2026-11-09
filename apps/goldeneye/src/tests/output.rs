use super::*;

#[test]
fn probe_outputs_never_overwrite_existing_user_files() {
    let root = std::env::temp_dir().join(format!(
        "ge-output-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let path = root.join("probe.bin");
    write_new(&path, b"original").unwrap();
    assert!(write_new(&path, b"replacement").is_err());
    assert_eq!(fs::read(&path).unwrap(), b"original");
    fs::remove_file(path).unwrap();
    fs::remove_dir(root).unwrap();
}
