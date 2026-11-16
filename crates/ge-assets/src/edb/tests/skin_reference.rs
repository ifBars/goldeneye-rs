use super::*;
fn word(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn input() -> Vec<u8> {
    let mut data = vec![0; 0x300];
    for (at, value) in [
        (0, 0x4745_4f4d),
        (4, 0x0102_00ab),
        (8, 263),
        (0x14, 0x300),
        (0x18, 0x200),
        (0x60, 0x0001_0001),
        (0x64, 0x100 - 0x64),
        (0x100, 0x0d00_00ab),
        (0x108, 0x180),
        (0x180, 0x200),
    ] {
        word(&mut data, at, value);
    }
    data
}
#[test]
fn metadata_skips_poisoned_body_and_unused_topology() {
    let mut data = input();
    word(&mut data, 0x108, u32::MAX);
    let edb = Edb::parse(data).unwrap();
    let binding = edb.base_skin_reference(0x8d00_0000).unwrap().unwrap();
    assert_eq!(binding.directory_index(), 0);
    assert_eq!(binding.hash(), 0x0d00_00ab);
    assert_eq!(binding.authored_address(), u32::MAX);
    assert!(edb.base_skin_body(0x8d00_0000).is_err());
}
#[test]
fn missing_descriptor_and_nonbase_body_are_distinct() {
    let mut data = input();
    word(&mut data, 0x104, 0x8001_0000);
    word(&mut data, 0x108, 0x240);
    word(&mut data, 0x240, 0x200);
    let edb = Edb::parse(data).unwrap();
    assert!(edb.base_skin_reference(0x0d00_00ac).unwrap().is_none());
    assert!(edb.base_skin_body(0x0d00_00ac).unwrap().is_none());
    assert_eq!(
        edb.base_skin_reference(0x0d00_00ab)
            .unwrap()
            .unwrap()
            .raw_section(),
        0x8001
    );
    assert!(edb.base_skin_body(0x0d00_00ab).is_err());
}
#[test]
fn base_header_read_skips_topology_but_never_uses_tail_bytes() {
    let mut data = input();
    word(&mut data, 0x184, u32::MAX); // bone count is not consumed by body binding
    let edb = Edb::parse(data.clone()).unwrap();
    assert_eq!(
        edb.base_skin_body(0x0d00_00ab).unwrap().unwrap().address(),
        0x180
    );
    word(&mut data, 0x108, 0x240);
    word(&mut data, 0x240, 0x200);
    assert!(
        Edb::parse(data)
            .unwrap()
            .base_skin_body(0x0d00_00ab)
            .is_err()
    );
}
#[test]
fn metadata_is_bounded_to_base_and_checks_retained_identity() {
    let mut data = input();
    word(&mut data, 0x18, 0x108);
    assert!(
        Edb::parse(data)
            .unwrap()
            .base_skin_reference(0x8d00_0000)
            .is_err()
    );
    let mut edb = Edb::parse(input()).unwrap();
    edb.header.hash ^= 1;
    assert!(edb.base_skin_reference(0x8d00_0000).is_err());
}

fn half(data: &mut [u8], at: usize, value: u16) {
    data[at..at + 2].copy_from_slice(&value.to_be_bytes());
}
/// Tag table at 0x1e0 inside the 0x200-byte base; entries are (bone, tag).
fn tagged(entries: &[(u16, u16)], count: u32) -> Vec<u8> {
    let mut data = input();
    word(&mut data, 0x184, count);
    word(&mut data, 0x1dc, 0x1e0 - 0x1dc);
    for (i, (bone, tag)) in entries.iter().enumerate() {
        half(&mut data, 0x1e0 + i * 4, *bone);
        half(&mut data, 0x1e0 + i * 4 + 2, *tag);
    }
    data
}
fn body(edb: &Edb) -> crate::skin::Body {
    edb.base_skin_body(0x0d00_00ab).unwrap().unwrap()
}
#[test]
fn tag_walk_compares_only_the_low_half_and_stops_at_the_first_match() {
    // The later bone is outside the count and the later entry is outside the
    // budget: neither may be read once the first match is found.
    let edb = Edb::parse(tagged(&[(1, 0x41), (2, 0x42), (0xffff, 0x42)], 4)).unwrap();
    let found = edb
        .base_skin_tag(&body(&edb), 0x7e00_0042, 2)
        .unwrap()
        .unwrap();
    assert_eq!(
        (found.bone_index, found.tag_raw, found.source_index),
        (2, 0x42, 1)
    );
    assert_eq!(found.source_address, 0x1e4);
}
#[test]
fn tag_walk_stops_at_the_sentinel_before_its_bone_and_later_entries() {
    // Sentinel bone is garbage and a matching entry follows: neither is read.
    let edb = Edb::parse(tagged(&[(1, 0x41), (0xdead, 0xffff), (2, 0x42)], 4)).unwrap();
    assert!(edb.base_skin_tag(&body(&edb), 0x42, 3).unwrap().is_none());
    // The sentinel value itself never matches a request.
    assert!(
        edb.base_skin_tag(&body(&edb), 0x0e00_ffff, 3)
            .unwrap()
            .is_none()
    );
}
#[test]
fn tag_walk_errors_are_not_missing_results() {
    let edb = Edb::parse(tagged(&[(1, 0x41), (2, 0x43)], 4)).unwrap();
    // Neither a match nor a sentinel within the budget.
    assert!(edb.base_skin_tag(&body(&edb), 0x42, 2).is_err());
    assert!(edb.base_skin_tag(&body(&edb), 0x42, 0).is_err());
    // A third entry (zero bone, zero tag) is not a sentinel: still no result.
    assert!(edb.base_skin_tag(&body(&edb), 0x42, 3).is_err());
    // NULL table pointer is not an empty table.
    let mut data = tagged(&[(1, 0x42)], 4);
    word(&mut data, 0x1dc, 0);
    let null = Edb::parse(data).unwrap();
    assert!(null.base_skin_tag(&body(&null), 0x42, 1).is_err());
    // A table pointer outside the retained base.
    let mut data = tagged(&[(1, 0x42)], 4);
    word(&mut data, 0x1dc, 0x400);
    let far = Edb::parse(data).unwrap();
    assert!(far.base_skin_tag(&body(&far), 0x42, 1).is_err());
}
#[test]
fn tag_walk_never_reads_past_the_base_into_the_cached_tail() {
    // The table runs to the end of the 0x200 base; a match lies beyond it.
    let mut data = input();
    word(&mut data, 0x184, 4);
    word(&mut data, 0x1dc, 0x1fc - 0x1dc);
    half(&mut data, 0x1fc, 1);
    half(&mut data, 0x1fe, 0x41);
    half(&mut data, 0x200, 2);
    half(&mut data, 0x202, 0x42);
    let edb = Edb::parse(data).unwrap();
    assert!(edb.base_skin_tag(&body(&edb), 0x42, 2).is_err());
    assert!(edb.base_skin_tag(&body(&edb), 0x41, 1).is_ok());
}
#[test]
fn tag_walk_checks_the_matched_bone_against_the_body_count() {
    let edb = Edb::parse(tagged(&[(2, 0x42), (1, 0x43)], 2)).unwrap();
    assert!(edb.base_skin_tag(&body(&edb), 0x42, 1).is_err());
    assert_eq!(
        edb.base_skin_tag(&body(&edb), 0x43, 2)
            .unwrap()
            .unwrap()
            .bone_index,
        1
    );
}
#[test]
fn tag_walk_rejects_a_foreign_or_stale_body() {
    let edb = Edb::parse(tagged(&[(1, 0x42)], 4)).unwrap();
    let held = body(&edb);
    // Same resource, directory now points at another body address.
    let mut moved = tagged(&[(1, 0x42)], 4);
    word(&mut moved, 0x108, 0x190);
    word(&mut moved, 0x190, 0x200);
    let moved = Edb::parse(moved).unwrap();
    assert!(moved.base_skin_tag(&held, 0x42, 1).is_err());
    // A body of another resource.
    let mut other = Edb::parse(tagged(&[(1, 0x42)], 4)).unwrap();
    other.header.hash ^= 1;
    assert!(other.base_skin_tag(&held, 0x42, 1).is_err());
    // The same bytes with a nonbase directory entry no longer bind a body.
    let mut nonbase = tagged(&[(1, 0x42)], 4);
    word(&mut nonbase, 0x104, 0x8001_0000);
    let nonbase = Edb::parse(nonbase).unwrap();
    assert!(nonbase.base_skin_tag(&held, 0x42, 1).is_err());
    // The unchanged owner still resolves it.
    assert!(edb.base_skin_tag(&held, 0x42, 1).unwrap().is_some());
}
