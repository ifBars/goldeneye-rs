use super::*;

fn entry(path: &str, hash: u32) -> Entry {
    Entry {
        path: path.into(),
        hash,
        base_bytes: 0,
        version: 263,
        flags: 0,
        locations: vec![],
    }
}

#[test]
fn selectors_are_case_insensitive_and_reject_ambiguous_basenames() {
    let entries = [entry("x:\\a\\file.edb", 1), entry("x:\\b\\file.edb", 2)];
    assert!(select(&entries, "file.edb").is_err());
    assert_eq!(select(&entries, "X:\\A\\FILE.EDB").unwrap().hash, 1);
    assert_eq!(select(&entries, "0x00000002").unwrap().hash, 2);
    assert!(select(&entries, "missing.edb").is_err());
}

#[test]
fn follows_external_texture_and_rejects_cycles_or_dimension_mismatches() {
    let make = |external: bool| {
        let mut data = crate::fixtures::edb_fixture();
        if external {
            data[0x11c..0x120].copy_from_slice(&0x1000002u32.to_be_bytes());
            data[0x12c..0x130].copy_from_slice(&0x86000001u32.to_be_bytes());
        } else {
            data[4..8].copy_from_slice(&0x1000002u32.to_be_bytes());
        }
        Edb::parse(data)
    };
    let mut names = Vec::new();
    let image = resolve_texture("start.edb", 0x86000001, 0, 0, |name| {
        names.push(name.to_owned());
        make(name == "start.edb")
    })
    .unwrap();
    assert_eq!(image.pixels, [128; 4]);
    assert_eq!(names, ["start.edb", "0x01000002"]);
    let result = resolve_texture("start.edb", 0x86000001, 0, 0, |_| make(true));
    assert!(format!("{:#}", result.unwrap_err()).contains("cyclic"));
    let result = resolve_texture("start.edb", 0x86000001, 0, 0, |name| {
        let mut edb = make(name == "start.edb")?;
        if name != "start.edb" {
            edb.textures[0].width = 2;
        }
        Ok(edb)
    });
    assert!(result.is_err());
}
