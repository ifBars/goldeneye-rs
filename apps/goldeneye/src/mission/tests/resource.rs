use super::*;

fn prefix(payload: u32) -> Vec<u8> {
    let mut bytes = vec![0; Edb::HEADER_BYTES];
    for (at, value) in [
        (0, 0x4745_4f4d),
        (4, 0x0181_00a5),
        (8, 263),
        (0x14, payload),
        (0x18, Edb::HEADER_BYTES as u32),
    ] {
        bytes[at..at + 4].copy_from_slice(&value.to_be_bytes());
    }
    bytes
}

#[test]
fn small_header_rejects_oversized_payload_before_any_full_resource_allocation() {
    let bytes = prefix(2 * 1024 * 1024);
    assert_eq!(bytes.len(), Edb::HEADER_BYTES);
    assert!(
        checked_header(
            &bytes,
            0x0181_00a5,
            263,
            Edb::HEADER_BYTES as u32,
            1024 * 1024
        )
        .is_err()
    );
    assert_eq!(
        checked_header(
            &bytes,
            0x0181_00a5,
            263,
            Edb::HEADER_BYTES as u32,
            2 * 1024 * 1024
        )
        .unwrap()
        .payload_bytes,
        2 * 1024 * 1024
    );
}

#[test]
fn preflight_checks_index_identity_base_size_and_truncation() {
    let bytes = prefix(1024);
    for (hash, version, base) in [
        (0x0181_00a6, 263, Edb::HEADER_BYTES as u32),
        (0x0181_00a5, 262, Edb::HEADER_BYTES as u32),
        (0x0181_00a5, 263, 512),
    ] {
        assert!(checked_header(&bytes, hash, version, base, 1024).is_err());
    }
    assert!(
        checked_header(
            &bytes[..100],
            0x0181_00a5,
            263,
            Edb::HEADER_BYTES as u32,
            1024
        )
        .is_err()
    );
}
