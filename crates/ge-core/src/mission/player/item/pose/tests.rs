use super::*;
impl Item {
    pub(crate) fn pose_test_flags(&mut self, value: u8) {
        self.byte_8c = value;
    }
    pub(crate) fn pose_fixture(player: PlayerId) -> Self {
        let world = WorldId::new(2).unwrap();
        let id = ItemId::new(world, 1).unwrap();
        let mut item = Self::from_constructor(world, id, 5).unwrap();
        item.handler = Some(player);
        item.factory = Some(WrapperlessFactory {
            numeric_id: 1,
            word_174: 0,
            word_178: 0,
            word_17c: 0,
            word_184: 0,
            cache_index: u32::MAX,
            wrapper_bound: false,
            key: u32::MAX,
            resource_cache_index: u32::MAX,
            attachment_bound: false,
            byte_8e: 7,
        });
        item
    }
}
#[test]
fn scale_and_dirty_stores_keep_other_item_flags_and_pose() {
    let mut item = Item::pose_fixture(PlayerId::new(4).unwrap());
    assert_eq!(item.byte_8c(), 0x13);
    item.byte_8c = 0xff;
    let pose = (item.position, item.rotation);
    let flags = item.flags;
    item.store_constructor_scale([1f32.to_bits(), 2f32.to_bits(), 3f32.to_bits(), 0]);
    assert_eq!(item.factory().unwrap().byte_8e(), 7);
    item.invalidate_constructor_pose().unwrap();
    assert_eq!(item.factory().unwrap().byte_8e(), 0);
    assert_eq!(item.clear_constructor_scale_dirty(), (0xff, 0xdf));
    assert_eq!((item.position, item.rotation), pose);
    assert_eq!(item.flags, flags);
}
