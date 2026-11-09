use super::*;
fn word(b: &mut [u8], at: usize, value: u32) {
    b[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
#[test]
fn snapshot_retains_order_flags_shared_targets_and_cycles() {
    let mut bytes = vec![0; 0x80];
    bytes[0] = 2;
    bytes[1] = 0x89;
    for p in [4, 0x20] {
        word(&mut bytes, p, (0x40i32 - p as i32) as u32);
        for (axis, value) in [-1f32, -2., -3., 4., 5., 6.].into_iter().enumerate() {
            word(&mut bytes, p + 4 + axis * 4, value.to_bits());
        }
    }
    bytes[0x41] = 8;
    bytes[0x43] = 2;
    word(&mut bytes, 0x44, 0x000708aa);
    word(&mut bytes, 0x48, 0x000700bb);
    let tree = read(&bytes, 0).unwrap();
    assert_eq!(tree.nodes.len(), 2);
    let Entries::Branches(branches) = &tree.nodes[&0].entries else {
        panic!()
    };
    assert_eq!(
        branches.iter().map(|v| v.target).collect::<Vec<_>>(),
        [0x40, 0x40]
    );
    let Entries::Leaves(leaves) = &tree.nodes[&0x40].entries else {
        panic!()
    };
    assert_eq!(
        (
            leaves[0].placement,
            leaves[0].flags_raw,
            leaves[1].unknown_byte
        ),
        (7, 8, 0xbb)
    );
    word(&mut bytes, 4, (-4i32) as u32);
    assert_eq!(read(&bytes, 0).unwrap().nodes.len(), 2);
}
#[test]
fn zero_relative_is_a_real_target_and_addressed_invalid_data_is_error() {
    let mut bytes = vec![0; 0x20];
    bytes[0] = 1;
    bytes[1] = 8;
    let tree = read(&bytes, 0).unwrap();
    assert!(tree.nodes.contains_key(&4));
    word(&mut bytes, 4, u32::MAX / 2);
    assert!(read(&bytes, 0).is_err());
    word(&mut bytes, 4, 0);
    word(&mut bytes, 8, f32::NAN.to_bits());
    assert!(read(&bytes, 0).is_err());
    assert!(read(&bytes[..3], 0).is_err());
}
