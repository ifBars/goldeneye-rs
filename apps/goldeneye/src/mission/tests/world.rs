use super::*;
use crate::mission::{fixtures, registration::PreparedRegistration};
use ge_core::mission::residency::Request;

#[test]
fn owned_map_bindings_keep_null_slot_separate_from_loaded_order_and_resolve_coded_navmesh() {
    let resources = [
        fixtures::resource(0x0181_00a5, 0x8500_0000, None),
        fixtures::resource(0x018b_00a5, 0x0500_00a6, None),
    ];
    let requests = resources
        .iter()
        .map(|edb| Request {
            file: edb.header.hash,
            map_reference: 0x8500_0000,
        })
        .collect::<Vec<_>>();
    let plan = PreparedRegistration::from_initialization_snapshot(&requests).unwrap();
    let bound = WorldBindings::bind(&resources, plan.plan()).unwrap();
    assert_eq!(bound.registry.slots().len(), 3);
    assert_eq!(bound.registry.slots()[0].wrapper, None);
    assert_eq!(
        bound
            .evidence
            .instances
            .iter()
            .map(|row| (row.loaded_ordinal, row.preregistration_index))
            .collect::<Vec<_>>(),
        [(0, 1), (1, 2)]
    );
    let first = &bound.instances()[0];
    assert_eq!(first.source_ordinal, 0);
    assert_eq!(first.descriptor.canonical_reference, 0x8500_0000);
    assert_eq!(first.spatial.zones.len(), 1);
    assert_eq!(first.placements[0].entity_directory_index, 0);
    assert_eq!(first.placements[0].entity_type, 0x607);
    assert!(!bound.evidence.trigger_factories_run);
    assert!(!bound.evidence.navigation_initialized);
}

#[test]
fn binding_missing_or_wrong_resource_owners_cannot_publish_a_completed_snapshot() {
    let requests = [Request {
        file: 0x0181_00a5,
        map_reference: 0x8500_0000,
    }];
    let plan = PreparedRegistration::from_initialization_snapshot(&requests).unwrap();
    assert!(WorldBindings::bind(&[], plan.plan()).is_err());
    let wrong = [fixtures::resource(0x0181_00a6, 0x8500_0000, None)];
    assert!(WorldBindings::bind(&wrong, plan.plan()).is_err());
}
