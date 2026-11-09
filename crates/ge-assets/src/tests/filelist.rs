use super::*;
// Authored fixture: one synthetic entry and two duplicate storage locations.
fn fixture() -> Vec<u8> {
    let mut data = Vec::new();
    for n in [7u32, 0, 1, 0x10000, 40, 4, 0x1234, 1, 0, 2, 0, 0, 16, 0, 4] {
        data.extend(n.to_be_bytes());
    }
    for (i, b) in b"x:\\test.bin\0".iter().enumerate() {
        data.push(b.wrapping_sub(0x16).wrapping_add(i as u8));
    }
    let size = (data.len() as u32).to_be_bytes();
    data[4..8].copy_from_slice(&size);
    data
}
#[test]
fn decodes_name_metadata_and_duplicate_locations() {
    let f = Filelist::parse(&fixture()).unwrap();
    assert_eq!(f.entries[0].path, "x:\\test.bin");
    assert_eq!(f.entries[0].hash, 0x1234);
    assert_eq!(f.entries[0].base_bytes, 4);
    assert_eq!(f.entries[0].locations.len(), 2);
    assert_eq!(f.entries[0].locations[1].offset, 16);
}
#[test]
fn rejects_every_truncation_without_panicking() {
    let data = fixture();
    for end in 0..data.len() {
        assert!(Filelist::parse(&data[..end]).is_err(), "truncation {end}");
    }
}
#[test]
fn rejects_invalid_pointer_count_version_and_termination() {
    for (offset, value) in [
        (0, 6),
        (8, u32::MAX),
        (12, 0),
        (16, u32::MAX),
        (36, u32::MAX),
        (56, u32::MAX),
    ] {
        let mut data = fixture();
        data[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
        assert!(Filelist::parse(&data).is_err(), "offset {offset}");
    }
    let mut data = fixture();
    *data.last_mut().unwrap() = 0xff;
    assert!(Filelist::parse(&data).is_err());
}

#[test]
fn supports_unnamed_hash_addressed_entries() {
    let mut data = fixture();
    // First name starts at 60; decoded zero is encoded as -0x16 modulo 256.
    data[60] = 0xea;
    let filelist = Filelist::parse(&data).unwrap();
    assert!(filelist.entries[0].path.is_empty());
    assert_eq!(filelist.entries[0].hash, 0x1234);
}

#[test]
fn malformed_index_mutations_never_panic() {
    let original = fixture();
    for i in 0..original.len() {
        for value in [0u8, 1, 127, 128, 255] {
            let mut data = original.clone();
            data[i] = value;
            let _ = Filelist::parse(&data);
        }
    }
}
