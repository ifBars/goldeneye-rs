use super::*;

#[test]
fn clear_releases_owned_allocation_and_preserves_growth_even_with_zero_count() {
    let mut bytes = Vec::with_capacity(16);
    bytes.extend_from_slice(b"opaque");
    let allocation = bytes.capacity();
    let mut buffer = Buffer::from_owned(8, 37, 0, Some(bytes)).unwrap();
    assert_eq!(buffer.backing_len(), 6);
    assert_eq!(buffer.backing_capacity(), allocation);
    let stats = buffer.clear_backing();
    assert!(stats.had_backing);
    assert_eq!(stats.bytes_released, allocation);
    assert_eq!((stats.previous_capacity, stats.previous_count), (8, 0));
    assert_eq!(
        (buffer.capacity(), buffer.growth(), buffer.count()),
        (0, 37, 0)
    );
    assert_eq!(buffer.backing(), None);
    assert_eq!(buffer.backing_len(), 0);
    assert_eq!(buffer.backing_capacity(), 0);
    assert!(!buffer.has_backing());
    let repeat = buffer.clear_backing();
    assert!(!repeat.had_backing);
    assert_eq!(repeat.bytes_released, 0);
    assert_eq!(buffer.growth(), 37);
}

#[test]
fn logical_headers_do_not_cast_or_own_objects_named_by_opaque_bytes() {
    let outside_object = std::rc::Rc::new(41u32);
    let held_object = outside_object.clone();
    let mut buffer = Buffer::from_owned(100, 0xffff_ffff, 2, Some(vec![1, 2, 3])).unwrap();
    assert_eq!(buffer.backing(), Some([1, 2, 3].as_slice()));
    buffer.clear_backing();
    assert_eq!(std::rc::Rc::strong_count(&outside_object), 2);
    assert_eq!(*held_object, 41);
    assert_eq!(buffer.growth(), u32::MAX);
    assert!(Buffer::from_owned(1, 4, 2, None).is_err());
    let mut null_header = Buffer::from_owned(3, 9, 2, None).unwrap();
    assert!(!null_header.clear_backing().had_backing);
    assert_eq!(
        (
            null_header.capacity(),
            null_header.count(),
            null_header.growth()
        ),
        (0, 0, 9)
    );
}

#[test]
fn buffer_debug_does_not_dump_owned_payload_bytes() {
    let buffer = Buffer::from_owned(1, 4, 1, Some(vec![91, 92, 93, 94])).unwrap();
    let debug = format!("{buffer:?}");
    assert!(debug.contains("backing_len: 4"));
    assert!(!debug.contains("91, 92"));
}
