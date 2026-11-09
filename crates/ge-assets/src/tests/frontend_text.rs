use super::{Budget, read};
use crate::edb::Table;

fn word(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_be_bytes());
}

fn fixture() -> Vec<u8> {
    let mut data = vec![0; 512];
    word(&mut data, 16, 0x0801_0001);
    word(&mut data, 20, 160);
    word(&mut data, 24, 512);
    word(&mut data, 48 + 8, 176);
    word(&mut data, 64, 0x1400_0018);
    word(&mut data, 64 + 8, 128);
    word(&mut data, 128, 1);
    word(&mut data, 132, 0x0801_0001);
    word(&mut data, 136, 0);
    word(&mut data, 180, 1);
    word(&mut data, 184, 0x4300_0001);
    word(&mut data, 188, 210 - 188);
    word(&mut data, 196, u32::MAX);
    for (i, unit) in "Authored text".encode_utf16().chain([0]).enumerate() {
        data[210 + i * 2..212 + i * 2].copy_from_slice(&unit.to_be_bytes());
    }
    data
}

fn table(name: &'static str, offset: usize) -> Table {
    Table {
        name,
        count: 1,
        hash_size_raw: 1,
        offset,
    }
}

fn text(data: &[u8], hash: u32, budget: Budget) -> anyhow::Result<Option<super::Text>> {
    read(
        data,
        &table("sections", 16),
        &table("references", 48),
        &table("spreadsheets", 64),
        hash,
        budget,
    )
}

fn budget() -> Budget {
    Budget {
        pages: 4,
        rows_per_page: 8,
        code_units: 64,
    }
}

#[test]
fn mapped_english_lookup_retains_provenance_and_sparse_misses() {
    let data = fixture();
    let value = text(&data, 0x4300_0001, budget()).unwrap().unwrap();
    assert_eq!(value.text, "Authored text");
    assert_eq!(value.hash, 0x4300_0001);
    assert_eq!(value.section_hash, 0x0801_0001);
    assert_eq!(value.entry_offset, 184);
    assert_eq!(value.flags_raw, u32::MAX);
    assert!(text(&data, 0x4300_0002, budget()).unwrap().is_none());
    let mut data = data;
    word(&mut data, 188, 0);
    assert!(text(&data, 0x4300_0001, budget()).unwrap().is_none());
}

#[test]
fn page_reference_and_section_corruption_are_errors() {
    for (at, value) in [(136, 1), (52, 0x0001_0000), (56, 178), (24, 520)] {
        let mut data = fixture();
        word(&mut data, at, value);
        assert!(text(&data, 0x4300_0001, budget()).is_err());
    }
    let mut data = fixture();
    word(&mut data, 56, 0);
    assert!(text(&data, 0x4300_0001, budget()).unwrap().is_none());
}

#[test]
fn addressed_strings_cannot_escape_page_or_overlap_rows() {
    for target in [190, 209, 512, 600] {
        let mut data = fixture();
        word(&mut data, 188, target - 188);
        assert!(text(&data, 0x4300_0001, budget()).is_err());
    }
    let mut data = fixture();
    word(&mut data, 188, u32::MAX);
    assert!(text(&data, 0x4300_0001, budget()).is_err());
}

#[test]
fn budgets_bound_mapping_rows_and_text_without_silent_truncation() {
    for limit in [
        Budget {
            pages: 0,
            ..budget()
        },
        Budget {
            rows_per_page: 0,
            ..budget()
        },
        Budget {
            code_units: 3,
            ..budget()
        },
    ] {
        assert!(text(&fixture(), 0x4300_0001, limit).is_err());
    }
    let mut data = fixture();
    word(&mut data, 180, u32::MAX);
    assert!(text(&data, 0x4300_0001, budget()).is_err());
}

#[test]
fn utf16_accepts_pairs_and_rejects_unpaired_surrogates() {
    let mut data = fixture();
    data[210..216].copy_from_slice(&[0xd8, 0x3d, 0xde, 0x80, 0, 0]);
    assert_eq!(
        text(&data, 0x4300_0001, budget()).unwrap().unwrap().text,
        "🚀"
    );
    data[212..214].copy_from_slice(&[0, 0]);
    assert!(text(&data, 0x4300_0001, budget()).is_err());
}

#[test]
fn native_binary_lookup_keeps_midpoint_duplicate_and_unsorted_miss() {
    let mut data = fixture();
    word(&mut data, 180, 3);
    word(&mut data, 184, 0x4300_0001);
    word(&mut data, 200, 0x4300_0001);
    word(&mut data, 204, 270 - 204);
    word(&mut data, 216, 0x4300_0002);
    data[270..274].copy_from_slice(&[0, b'M', 0, 0]);
    assert_eq!(
        text(&data, 0x4300_0001, budget()).unwrap().unwrap().text,
        "M"
    );
    word(&mut data, 184, 0x4300_0009);
    word(&mut data, 200, 0x4300_0001);
    word(&mut data, 216, 0x4300_0002);
    assert!(text(&data, 0x4300_0009, budget()).unwrap().is_none());
}
