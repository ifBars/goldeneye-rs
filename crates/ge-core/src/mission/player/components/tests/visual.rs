use super::*;
use crate::mission::actor_definitions::{self, Registry};

fn visual(reference: u32) -> Visual {
    let mut words = [u32::MAX; 17];
    words[0] = 0x4d00_0123;
    words[6] = 0x010a_0012;
    words[7] = reference;
    let registry = Registry::build(
        vec![actor_definitions::Primary {
            source_index: 3,
            words,
        }],
        actor_definitions::Budget {
            rows: 1,
            groups: 1,
            members: 1,
        },
    )
    .unwrap();
    Visual::from_record(
        CharacterBinding {
            registry_resource: 0x0109_0013,
            key: words[0],
            source_index: 1,
        },
        &registry.records()[0],
    )
    .unwrap()
}
#[test]
fn visual_callback_uses_full_class_byte_and_low16_without_changing_raw_dependency_request() {
    for (input, expected) in [
        (0x0d00_0123, 0x0d00_0123),
        (0x8d00_0123, 0x0d00_0001),
        (0x0d01_0000, 0x0d00_0001),
        (0x0900_0123, 0x0d00_0001),
        (0, 0x0d00_0001),
        (u32::MAX, 0x0d00_0001),
    ] {
        let visual = visual(input);
        assert_eq!(visual.callback_skin(), expected);
        assert_eq!(visual.skin(), input);
        assert_eq!(visual.extra_skins(), [u32::MAX; 2]);
        assert_eq!(visual.override_textures(), [u32::MAX; 3]);
    }
}
