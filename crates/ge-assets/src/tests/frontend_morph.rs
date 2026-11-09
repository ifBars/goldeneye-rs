use super::*;

fn word(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn half(data: &mut [u8], at: usize, value: u16) {
    data[at..at + 2].copy_from_slice(&value.to_be_bytes());
}
fn relative(data: &mut [u8], field: usize, target: usize) {
    word(data, field, (target as i32 - field as i32) as u32);
}
fn tagged(data: &mut [u8], field: usize, target: usize, tag: u8) {
    word(
        data,
        field,
        ((target as i32 - field as i32) << 8) as u32 | u32::from(tag),
    );
}
fn fixture() -> Vec<u8> {
    let mut data = vec![0; 0x300];
    word(&mut data, 0x100, 0x601);
    half(&mut data, 0x1b4, 0x20);
    // Unused unknown component has an invalid pointer; search reads only its tag.
    word(&mut data, 0x1b8, 0x7fff_ff02);
    tagged(&mut data, 0x1bc, 0x80, 5);
    word(&mut data, 0x80, 3);
    relative(&mut data, 0x84, 0x240);
    relative(&mut data, 0x88, 0x60);
    word(&mut data, 0x8c, 0x7fff_ffff);
    data[0x240..0x24c].copy_from_slice(&[1, 2, 3, 4, 11, 22, 33, 44, 255, 0, 128, 7]);
    data[0x60..0x6c].copy_from_slice(&[5, 6, 7, 8, 55, 66, 77, 88, 0, 255, 32, 9]);
    data
}

#[test]
fn scans_tagged_words_then_decodes_selected_independent_color_index() {
    let data = fixture();
    let frames = read_at(&data, 0x100, 2).unwrap().unwrap();
    assert_eq!((frames.address, frames.frame_count()), (0x80, 3));
    assert_eq!(frames.frame_address(0).unwrap(), Some(0x240));
    assert_eq!(frames.frame_address(1).unwrap(), Some(0x60));
    assert_eq!(frames.color(0, 1).unwrap(), [11, 22, 33, 44]);
    assert_eq!(frames.color(1, 2).unwrap(), [0, 255, 32, 9]);
    assert!(frames.color(2, 0).is_err());
    assert!(frames.color(3, 0).is_err());
}

#[test]
fn absent_component_reads_no_extension_words_even_when_truncated() {
    let mut data = fixture();
    half(&mut data, 0x1b4, 0x10);
    data.truncate(0x1b6);
    assert!(read_at(&data, 0x100, 0).unwrap().is_none());
    half(&mut data, 0x1b4, 0x20);
    assert!(read_at(&data, 0x100, 1).is_err());
}

#[test]
fn host_budget_and_addressed_nulls_are_errors_without_fallback_colors() {
    let mut data = fixture();
    assert!(read_at(&data, 0x100, 0).is_err());
    assert!(read_at(&data, 0x100, 1).is_err());
    word(&mut data, 0x88, 0);
    let frames = read_at(&data, 0x100, 2).unwrap().unwrap();
    assert_eq!(frames.frame_address(1).unwrap(), None);
    assert!(frames.color(1, 0).is_err());
    assert_eq!(frames.color(0, 2).unwrap(), [255, 0, 128, 7]);
    assert!(frames.color(0, u16::MAX).is_err());
}

#[test]
fn validates_mesh_type_component_pointer_and_nonempty_frame_table() {
    let mut data = fixture();
    word(&mut data, 0x100, 0x603);
    assert!(read_at(&data, 0x100, 2).is_err());
    word(&mut data, 0x100, 0x601);
    tagged(&mut data, 0x1bc, 0, 5);
    assert!(read_at(&data, 0x100, 2).is_err()); // zero count at addressed component.
    word(&mut data, 0x1bc, 0x8000_0005);
    assert!(read_at(&data, 0x100, 2).is_err()); // signed 24-bit displacement underflow.
}

#[test]
fn unselected_frame_table_entries_need_not_exist_and_shared_frames_are_preserved() {
    let mut data = fixture();
    // Huge declared count cannot trigger allocation or eager table validation.
    word(&mut data, 0x80, u32::MAX);
    relative(&mut data, 0x88, 0x240);
    let frames = read_at(&data, 0x100, 2).unwrap().unwrap();
    assert_eq!(frames.color(0, 1).unwrap(), frames.color(1, 1).unwrap());
    assert!(frames.frame_address(u32::MAX - 1).is_err());
}
