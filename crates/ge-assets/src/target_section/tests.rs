use super::*;

const OWNER: u32 = 0x0100_0041;
const SKIN: u32 = 0x0d00_0101;
const TEXTURE: u32 = 0x0600_0101;

fn put32(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn put16(data: &mut [u8], at: usize, value: u16) {
    data[at..at + 2].copy_from_slice(&value.to_be_bytes());
}
fn table(name: &'static str, count: u16, hash_size_raw: i16, offset: usize) -> Table {
    Table {
        name,
        count,
        hash_size_raw,
        offset,
    }
}
fn fixture() -> (Vec<u8>, Vec<Table>) {
    let mut data = vec![0; 256];
    put32(&mut data, 4, OWNER);
    put32(&mut data, 64, SKIN);
    put16(&mut data, 68, 0x8001);
    put32(&mut data, 128, TEXTURE);
    put16(&mut data, 132, 1);
    put32(&mut data, 160, BASE_KEY);
    put32(&mut data, 176, 0x0800_0042);
    put32(&mut data, 180, 208);
    put32(&mut data, 184, 212);
    data[208..212].copy_from_slice(b"data");
    let mut tables = (0..16)
        .map(|_| table("unused", 0, 0, 0))
        .collect::<Vec<_>>();
    tables[0] = table("sections", 2, 2, 160);
    tables[4] = table("skins", 1, 1, 64);
    tables[15] = table("textures", 1, 1, 128);
    (data, tables)
}
fn locate<'a>(data: &'a [u8], tables: &[Table], request: u32) -> Result<Option<View<'a>>> {
    read(data, tables, OWNER, request, Budget::default())
}

#[test]
fn skin_hash_lookup_masks_section_and_borrows_only_the_checked_owner_span() {
    let (data, tables) = fixture();
    let view = locate(&data, &tables, SKIN).unwrap().unwrap();
    assert_eq!(view.resource_hash(), OWNER);
    assert_eq!(view.request(), SKIN);
    assert_eq!(view.kind(), Kind::Skin);
    assert_eq!(view.directory_index(), 0);
    assert_eq!(view.stored_hash(), SKIN);
    assert_eq!(view.raw_section(), 0x8001);
    assert_eq!(view.section_index(), 1);
    assert_eq!(view.key(), 0x0800_0042);
    assert!(!view.is_base());
    assert_eq!(view.span(), Some(208..212));
    assert_eq!(view.payload(), Some(&data[208..212]));
    assert_eq!(view.payload().unwrap().as_ptr(), data[208..].as_ptr());
    let mut metadata = view.span().unwrap();
    metadata.start = 0;
    assert_eq!(metadata, 0..212);
    assert_eq!(view.payload(), Some(b"data".as_slice()));
    assert!(!format!("{view:?}").contains("100, 97, 116, 97"));
}

#[test]
fn coded_texture_index_zero_does_not_require_a_hash_match() {
    let (data, mut tables) = fixture();
    tables[15].hash_size_raw = 0;
    assert!(locate(&data, &tables, TEXTURE).unwrap().is_none());
    let view = locate(&data, &tables, 0x8600_0000).unwrap().unwrap();
    assert_eq!(view.kind(), Kind::Texture);
    assert_eq!(view.directory_index(), 0);
    assert_eq!(view.stored_hash(), TEXTURE);
    assert_eq!(view.payload(), Some(b"data".as_slice()));
    assert!(locate(&data, &tables, 0x8600_0001).unwrap().is_none());
}

#[test]
fn literal_prefix_only_target_is_a_lookup_key_not_an_absent_token() {
    let (mut data, tables) = fixture();
    put32(&mut data, 64, 0x0d00_0000);
    assert!(locate(&data, &tables, 0x0d00_0000).unwrap().is_some());
}

#[test]
fn target_miss_does_not_address_sections_or_apply_their_budgets() {
    let (data, mut tables) = fixture();
    tables[0] = table("invalid", u16::MAX, i16::MIN, usize::MAX);
    let budget = Budget {
        targets: 1,
        sections: 0,
        section_bytes: 0,
    };
    assert!(
        read(&data, &tables, OWNER, SKIN + 1, budget)
            .unwrap()
            .is_none()
    );
    assert!(read(&data, &tables, OWNER, SKIN, budget).is_err());
}

#[test]
fn exact_base_key_is_lazy_even_at_a_nonzero_section_index() {
    let (mut data, tables) = fixture();
    put32(&mut data, 176, BASE_KEY);
    data.truncate(180); // Start/end/runtime fields are genuinely absent.
    let view = read(
        &data,
        &tables,
        OWNER,
        SKIN,
        Budget {
            section_bytes: 0,
            ..Budget::default()
        },
    )
    .unwrap()
    .unwrap();
    assert_eq!(view.section_index(), 1);
    assert!(view.is_base());
    assert_eq!(view.span(), None);
    assert_eq!(view.payload(), None);
}

#[test]
fn section_index_zero_is_valid_and_does_not_alone_select_the_base_case() {
    let (mut data, tables) = fixture();
    put16(&mut data, 68, 0);
    let base = locate(&data, &tables, SKIN).unwrap().unwrap();
    assert!(base.is_base());
    put32(&mut data, 160, 0x0800_0043);
    put32(&mut data, 164, 208);
    put32(&mut data, 168, 212);
    let streamed = locate(&data, &tables, SKIN).unwrap().unwrap();
    assert_eq!(streamed.section_index(), 0);
    assert!(!streamed.is_base());
    assert_eq!(streamed.payload(), Some(b"data".as_slice()));
}

#[test]
fn unknown_non_base_key_remains_raw_checked_data_without_stream_success() {
    let (mut data, tables) = fixture();
    put32(&mut data, 176, 0x1200_0042);
    let view = locate(&data, &tables, SKIN).unwrap().unwrap();
    assert_eq!(view.key(), 0x1200_0042);
    assert_eq!(view.payload(), Some(b"data".as_slice()));
    assert!(!view.is_base());
}

#[test]
fn malformed_reached_section_index_and_spans_are_errors_not_misses() {
    let (mut data, tables) = fixture();
    put16(&mut data, 68, 2);
    assert!(locate(&data, &tables, SKIN).is_err());
    put16(&mut data, 68, 1);
    for (start, end) in [(212, 208), (255, 260), (u32::MAX, u32::MAX)] {
        put32(&mut data, 180, start);
        put32(&mut data, 184, end);
        assert!(locate(&data, &tables, SKIN).is_err());
    }
}

#[test]
fn caller_budgets_preflight_each_reached_directory_and_payload() {
    let (data, tables) = fixture();
    for budget in [
        Budget {
            targets: 0,
            ..Budget::default()
        },
        Budget {
            sections: 1,
            ..Budget::default()
        },
        Budget {
            section_bytes: 3,
            ..Budget::default()
        },
    ] {
        assert!(read(&data, &tables, OWNER, SKIN, budget).is_err());
    }
    assert!(read(&data, &tables, OWNER + 1, SKIN, Budget::default()).is_err());
}

#[test]
fn coded_lookup_addresses_only_reached_descriptor_fields_and_checks_overflow() {
    let (mut data, mut tables) = fixture();
    tables[4] = table("skins", 2, 0, 220);
    put32(&mut data, 220, SKIN);
    put16(&mut data, 224, 0);
    data.truncate(226); // No remaining row bytes or second descriptor.
    assert!(
        locate(&data, &tables, 0x8d00_0000)
            .unwrap()
            .unwrap()
            .is_base()
    );
    data.truncate(225);
    assert!(locate(&data, &tables, 0x8d00_0000).is_err());
    tables[4].offset = usize::MAX - 2;
    assert!(locate(&data, &tables, 0x8d00_0001).is_err());
    assert!(locate(&data, &tables, 0x0900_0001).is_err());
}

#[test]
fn edb_entry_point_binds_the_actual_retained_owner_and_rejects_other_versions() {
    let mut data = vec![0; 512];
    for (at, value) in [
        (0, 0x4745_4f4d),
        (4, OWNER),
        (8, 263),
        (20, 512),
        (24, 512),
        (0x40, 0x0002_0002),
        (0x44, 0xbc),
        (0x60, 0x0001_0001),
        (0x64, 0xdc),
        (0x100, BASE_KEY),
        (0x110, 0x0800_0042),
        (0x114, 400),
        (0x118, 404),
        (0x140, SKIN),
    ] {
        put32(&mut data, at, value);
    }
    put16(&mut data, 0x144, 1);
    data[400..404].copy_from_slice(b"held");
    let edb = crate::edb::Edb::parse(data.clone()).unwrap();
    let view = edb
        .target_section(SKIN, Budget::default())
        .unwrap()
        .unwrap();
    assert_eq!(view.resource_hash(), OWNER);
    assert_eq!(view.payload(), Some(b"held".as_slice()));
    put32(&mut data, 8, 262);
    assert!(
        crate::edb::Edb::parse(data)
            .unwrap()
            .target_section(SKIN, Budget::default())
            .is_err()
    );
}
