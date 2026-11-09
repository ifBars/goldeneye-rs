use super::{Binding, Budget, Entity, Placement, PlacementLocation, SourceInstanceKey};
use crate::mission::world::{InstanceId, WrapperId};

#[path = "binding.rs"]
mod binding_tests;
#[path = "owner.rs"]
mod owner_tests;

fn wrapper(id: u64) -> WrapperId {
    WrapperId::new(id).unwrap()
}

fn instance(id: u64) -> InstanceId {
    InstanceId::new(id).unwrap()
}

fn budget() -> Budget {
    Budget {
        maps: 8,
        placements: 32,
        faces: 32,
        cache_edges: 96,
        category_members: 32,
    }
}

fn placement(key: u32, reference: u32, position_bits: [u32; 3]) -> Placement {
    Placement {
        key: SourceInstanceKey::new(key),
        entity_reference_raw: reference,
        position_bits,
    }
}

fn nav(index: usize) -> Entity {
    Entity {
        directory_index: index,
        entity_type: 0x607,
    }
}

fn binding() -> Binding {
    Binding {
        level_wrapper: wrapper(7),
        source: PlacementLocation {
            instance: instance(3),
            index: 0,
        },
        found: PlacementLocation {
            instance: instance(2),
            index: 1,
        },
        key: SourceInstanceKey::new(0x1234_5678),
        source_entity_reference_raw: 0x8200_0017,
        source_entity_directory_index: 23,
        cached_entity_reference_raw: 0x8200_0029,
        entity_directory_index: 41,
        translation_bits: [0x4120_0000, 0xc140_0000, 0x8000_0000, 0],
    }
}
