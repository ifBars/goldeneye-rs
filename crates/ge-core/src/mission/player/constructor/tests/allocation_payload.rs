use super::*;

fn allocation(kind: Kind) -> Allocation {
    Allocation::allocate(kind, kind.bytes()).unwrap()
}

fn words(allocation: &Allocation) -> Vec<u32> {
    let mut words = Vec::new();
    allocation.visit(&mut |word| words.push(word));
    words
}

#[test]
fn payload_offsets_are_separate_from_known_array_headers() {
    for (kind, payload_start) in [(Kind::Raw, 0), (Kind::Array34, 16), (Kind::Array64, 16)] {
        let mut owner = allocation(kind);
        assert_eq!(
            owner.payload_range(0, 4).unwrap(),
            payload_start..payload_start + 4
        );
        assert_eq!(owner.known_payload_bytes(), 0);
        assert!(owner.payload_written.is_none());
        assert_eq!(owner.payload_word(0).unwrap(), None);
        if kind != Kind::Raw {
            assert_eq!(owner.header_word(0), kind.stride());
            assert_eq!(owner.header_word(4), Some(kind.entries()));
            assert_eq!((owner.header_word(8), owner.header_word(12)), (None, None));
        }
        owner.put_payload_word(0, 0x12ab_34cd).unwrap();
        assert_eq!(owner.payload_word(0).unwrap(), Some(0x12ab_34cd));
        assert_eq!(
            (0..4)
                .map(|i| owner.payload_byte(i).unwrap())
                .collect::<Vec<_>>(),
            vec![Some(0x12), Some(0xab), Some(0x34), Some(0xcd)]
        );
        assert_eq!(owner.known_payload_bytes(), 4);
        assert_eq!((owner.header_word(8), owner.header_word(12)), (None, None));
    }
}

#[test]
fn partial_written_word_is_unknown_and_checked_store_preserves_its_prefix() {
    let mut owner = allocation(Kind::Raw);
    owner.put_payload_byte(0, 0).unwrap();
    owner.put_payload_byte(1, 2).unwrap();
    owner.put_payload_byte(2, 3).unwrap();
    assert_eq!(owner.payload_word(0).unwrap(), None);
    assert_eq!(owner.payload_byte(0).unwrap(), Some(0));
    let before = words(&owner);
    for offset in [1, 510, 512, usize::MAX] {
        assert!(owner.put_payload_word(offset, u32::MAX).is_err());
        assert!(owner.payload_word(offset).is_err());
        assert_eq!(words(&owner), before);
    }
    assert!(owner.put_payload_byte(512, 1).is_err());
    assert!(owner.payload_byte(512).is_err());
    assert_eq!(words(&owner), before);
    owner.put_payload_byte(3, 4).unwrap();
    assert_eq!(owner.payload_word(0).unwrap(), Some(0x0002_0304));
    owner.put_payload_word(0, 0x89ab_cdef).unwrap();
    assert_eq!(owner.payload_word(0).unwrap(), Some(0x89ab_cdef));
    assert_eq!(owner.known_payload_bytes(), 4);
}

#[test]
fn invalid_store_does_not_allocate_proof_metadata_or_mark_unknown_bytes() {
    let mut owner = allocation(Kind::Array34);
    let before = words(&owner);
    assert!(owner.put_payload_word(owner.payload_bytes(), 1).is_err());
    assert!(owner.put_payload_byte(usize::MAX, 1).is_err());
    assert!(owner.payload_written.is_none());
    assert_eq!(words(&owner), before);
}

#[test]
fn canonical_payload_proof_is_ordered_and_ignores_empty_metadata_allocation() {
    let mut first = allocation(Kind::Raw);
    let mut second = allocation(Kind::Raw);
    let empty = words(&first);
    first.payload_written = Some(vec![None; first.payload_bytes()]);
    assert_eq!(words(&first), empty);
    assert!(first.host_capacity_bytes() > second.host_capacity_bytes());
    first.put_payload_byte(500, 0).unwrap();
    first.put_payload_word(0, 0x1234_5678).unwrap();
    second.put_payload_word(0, 0x1234_5678).unwrap();
    second.put_payload_byte(500, 0).unwrap();
    assert_eq!(words(&first), words(&second));
    assert_eq!(first.known_payload_bytes(), 5);
    assert_eq!(first.payload_byte(499).unwrap(), None);
}

#[test]
fn last_array_key_translation_and_last_payload_byte_are_bounded() {
    let mut owner = allocation(Kind::Array34);
    let last_key = 33 * 208;
    assert_eq!(owner.payload_range(last_key, 4).unwrap(), 6880..6884);
    owner.put_payload_word(last_key, 7).unwrap();
    assert_eq!(owner.payload_word(last_key).unwrap(), Some(7));
    assert_eq!(owner.payload_byte(last_key - 1).unwrap(), None);
    owner.put_payload_byte(7071, 9).unwrap();
    assert_eq!(owner.payload_range(7071, 1).unwrap(), 7087..7088);
    assert_eq!(owner.payload_byte(7071).unwrap(), Some(9));
    assert!(owner.payload_byte(7072).is_err());
}
