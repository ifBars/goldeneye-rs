use super::*;
use crate::mission::player::{
    components::{TextureRequest, Visual},
    constructor::CharacterBinding,
};

fn registry() -> Registry {
    let input = [0x4d00_0311, 0x4d00_0201, 0x4d00_0401]
        .into_iter()
        .enumerate()
        .map(|(source_index, key)| {
            let mut words = [0; 17];
            words[0] = key;
            words[1] = 7;
            words[6] = 0x0100_0027;
            words[7] = 0x0d00_0034;
            words[10] = u32::MAX;
            Primary {
                source_index,
                words,
            }
        })
        .collect();
    Registry::build(
        input,
        Budget {
            rows: 3,
            groups: 1,
            members: 3,
        },
    )
    .unwrap()
}
#[test]
fn visual_direct_lookup_preserves_group_cursor_and_source_provenance() {
    let mut registry = registry();
    registry.selection_mut(0x4d00_0007).unwrap();
    let cursor = registry.groups()[0].cursor();
    let record = registry
        .direct_record(0x4d00_0311, LookupBudget { comparisons: 16 })
        .unwrap()
        .unwrap();
    assert_eq!(record.source_index(), 0);
    let character = CharacterBinding {
        registry_resource: 0x0109_0013,
        key: record.key(),
        source_index: 19,
    };
    let visual = Visual::from_record(character, record).unwrap();
    assert_eq!(visual.character(), character);
    assert_eq!((visual.file(), visual.skin()), (0x0100_0027, 0x0d00_0034));
    assert_eq!(visual.texture_request(), TextureRequest::EnumerateSkin);
    assert_eq!(registry.groups()[0].cursor(), cursor);
    assert!(
        Visual::from_record(
            CharacterBinding {
                key: 0x4d00_0411,
                ..character
            },
            record
        )
        .is_err()
    );
}
#[test]
fn unsupported_branches_missing_keys_and_exhaustion_cannot_consume_group_state() {
    let registry = registry();
    for key in [
        0x4d00_0007,
        0x4d00_0102,
        0x4d00_01ac,
        0x4d00_0000,
        0x8100_0311,
    ] {
        assert!(
            registry
                .direct_record(key, LookupBudget { comparisons: 16 })
                .is_err()
        );
    }
    assert!(
        registry
            .direct_record(0x4d00_0500, LookupBudget { comparisons: 16 })
            .unwrap()
            .is_none()
    );
    assert!(
        registry
            .direct_record(0x4d00_0311, LookupBudget { comparisons: 0 })
            .is_err()
    );
    assert_eq!(registry.groups()[0].cursor(), 0);
}
