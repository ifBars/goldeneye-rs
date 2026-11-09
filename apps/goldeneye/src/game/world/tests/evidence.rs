use super::*;
use ge_core::mission::{
    animator::{self, CameraContext, Runtime},
    triggers::{self, AnimatorId, AnimatorRequest, CommonInput, Descriptor, RowInput},
    world::InstanceId,
};

fn identity() -> Identity {
    Identity {
        game_id: "AUTHR0".into(),
        revision: 9,
        main_dol_partition_offset: 0x400,
        dol_bytes: 1024,
        dol_sha256: "authored-code-identity".into(),
        data_section_index: 1,
        data_virtual_address: 0x1000,
        data_bytes: 32,
        data_sha256: "authored-data-identity".into(),
    }
}

struct Factory(super::super::animators::Owned);

impl triggers::Factory for Factory {
    fn initialize_animator(&mut self, request: AnimatorRequest) -> Result<AnimatorId> {
        let id = AnimatorId::new(self.0.values.len() as u64 + 1)?;
        self.0.values.push(Runtime::fresh_startpoint(
            animator::Header {
                statement_count_raw: 2,
                variables: 3,
                shared_variables: 2,
                states: 1,
                format: 0x38,
                event_ff: 255,
                event_create: 0,
                event_105: 255,
                event_10e: 255,
            },
            animator::FreshContext {
                trigger_handle: request.trigger.get(),
                map_selector: request.map.map_selector,
                position_bits: request.position_bits,
                manager_present: true,
                camera: CameraContext::KnownMissing,
            },
            animator::Budget {
                variables: 4,
                states: 2,
            },
        )?);
        Ok(id)
    }
}

fn fixture(position: u32) -> (Registry, super::super::animators::Owned, MapContext) {
    let mut registry = Registry::new(triggers::Budget {
        maps: 1,
        rows: 2,
        objects: 2,
        filter_words: 4,
        fixups: 16,
        catalog_entries: 2,
    });
    let map = MapContext {
        instance: InstanceId::new(1).unwrap(),
        map_selector: 1,
    };
    registry.begin_map(map, 2, None).unwrap();
    let mut factory = Factory(super::super::animators::Owned::default());
    registry
        .visit(
            RowInput {
                ordinal: 0,
                zone: 0x102,
                descriptor: Descriptor {
                    native_type_index: 28,
                    class: 0x1e00_0003,
                    subtype: 0x1f00_0081,
                    enable_byte_raw: 1,
                },
            },
            || {
                Ok(CommonInput {
                    header_flags_raw: 0,
                    property_mask_raw: (1 << 16) | (1 << 26),
                    position_bits: [position, 0, 0, 0],
                    rotation_bits: [0; 4],
                    numeric: [0; 16],
                    reference_ordinals: [
                        1,
                        u32::MAX,
                        u32::MAX,
                        u32::MAX,
                        u32::MAX,
                        u32::MAX,
                        u32::MAX,
                        u32::MAX,
                    ],
                    word_74_raw: 0,
                    word_78_raw: 0,
                    animator_present: true,
                    animator_index_raw: 0,
                    shape_index_raw: None,
                })
            },
            &mut factory,
        )
        .unwrap();
    registry
        .mark_unsupported(
            1,
            Unsupported::Unclassified {
                native_type_index: 99,
            },
        )
        .unwrap();
    registry.fix_references().unwrap();
    registry.link_parents().unwrap();
    registry.initialize_map().unwrap();
    (registry, factory.0, map)
}

fn audit(registry: &Registry, map: MapContext) -> Audit {
    let mut audit = Audit::new(&identity());
    audit.map(map, None, 2);
    for (ordinal, index) in [(0, 28), (1, 99)] {
        audit.query(
            ordinal,
            index,
            zone::Context {
                loaded_ordinal: Some(0),
                zone_leaf: Some(2),
                callback_zone: 0x102,
                packed: 0x0102_0000,
                nodes_visited: 1,
            },
        );
    }
    audit.slots(registry.slots(0).unwrap());
    audit
}

#[test]
fn partial_world_counts_owned_state_and_serializes_only_closed_summary() {
    let (registry, animators, map) = fixture(0x3f80_0000);
    let evidence = audit(&registry, map)
        .finish(&registry, &animators, 1)
        .unwrap();
    assert_eq!(
        (
            evidence.original_slots,
            evidence.owned_triggers,
            evidence.unclassified_slots
        ),
        (2, 1, 1)
    );
    assert_eq!(
        (evidence.blocked_references, evidence.animators_initialized),
        (1, 1)
    );
    assert_eq!(
        (
            evidence.shared_variables,
            evidence.unwritten_scratch_words,
            evidence.state_flags
        ),
        (2, 2, 1)
    );
    assert_eq!(
        (evidence.execution_slots, evidence.active_executions),
        (16, 0)
    );
    assert_eq!(
        (
            evidence.create_events_denied,
            evidence.registration_events_unmapped
        ),
        (1, 1)
    );
    assert!(!evidence.catalog_complete && !evidence.native_zone_precision_verified);
    let json = serde_json::to_value(&evidence).unwrap();
    assert_eq!(json.as_object().unwrap().len(), 61);
    for forbidden in [
        "position_bits",
        "numeric",
        "reference_ordinals",
        "data_sha256",
        "trigger_handle",
    ] {
        assert!(!json.as_object().unwrap().contains_key(forbidden));
    }
    assert_eq!(evidence.state_sha256.len(), 64);
}

#[test]
fn fingerprint_detects_private_pose_change_without_changing_public_counts() {
    let (first, first_animators, map) = fixture(0x3f80_0000);
    let (second, second_animators, _) = fixture(0x4000_0000);
    let a = audit(&first, map)
        .finish(&first, &first_animators, 1)
        .unwrap();
    let b = audit(&second, map)
        .finish(&second, &second_animators, 1)
        .unwrap();
    assert_ne!(a.state_sha256, b.state_sha256);
    let mut a = serde_json::to_value(a).unwrap();
    let mut b = serde_json::to_value(b).unwrap();
    a.as_object_mut().unwrap().remove("state_sha256");
    b.as_object_mut().unwrap().remove("state_sha256");
    assert_eq!(a, b);
}

#[test]
fn inconsistent_callback_counts_or_missing_animator_ownership_are_errors() {
    let (registry, mut animators, map) = fixture(0);
    let mut missing_query = Audit::new(&identity());
    missing_query.map(map, None, 2);
    missing_query.slots(registry.slots(0).unwrap());
    assert!(missing_query.finish(&registry, &animators, 1).is_err());
    assert!(
        audit(&registry, map)
            .finish(&registry, &animators, 2)
            .is_err()
    );
    animators.values.clear();
    assert!(
        audit(&registry, map)
            .finish(&registry, &animators, 1)
            .is_err()
    );
}

#[test]
fn additional_inactive_kinds_keep_real_references_and_fingerprint_private_state() {
    let observe = |radius: u32, provider: u32| {
        let map = MapContext {
            instance: InstanceId::new(1).unwrap(),
            map_selector: 1,
        };
        let mut registry = Registry::new(triggers::Budget {
            maps: 1,
            rows: 4,
            objects: 4,
            filter_words: 0,
            fixups: 32,
            catalog_entries: 4,
        });
        registry.begin_map(map, 4, None).unwrap();
        let mut factory = Factory(super::super::animators::Owned::default());
        let mut audit = Audit::new(&identity());
        audit.map(map, None, 4);
        for (ordinal, (index, class, subtype)) in [
            (63, 0x1e00_0004, 0x1f00_00a3),
            (17, 0x1e00_0005, 0x1f00_0027),
            (6, 0x1e00_0004, 0x1f00_0009),
            (73, 0x1e00_0009, 0x1f00_00ad),
        ]
        .into_iter()
        .enumerate()
        {
            let mut numeric = [0; 16];
            numeric[0] = radius;
            numeric[13] = provider;
            if index == 73 {
                numeric[2] = 0x4200_00ab;
                numeric[15] = 0x4108_0023;
            }
            let mut references = [u32::MAX; 8];
            references[0] = (ordinal as u32 + 1) % 4;
            registry
                .visit(
                    RowInput {
                        ordinal,
                        zone: 0x102,
                        descriptor: Descriptor {
                            native_type_index: index,
                            class,
                            subtype,
                            enable_byte_raw: 1,
                        },
                    },
                    || {
                        Ok(CommonInput {
                            header_flags_raw: 1,
                            property_mask_raw: 1
                                | (1 << 2)
                                | (1 << 13)
                                | (1 << 15)
                                | (1 << 16)
                                | (1 << 26),
                            position_bits: [0; 4],
                            rotation_bits: [0; 4],
                            numeric,
                            reference_ordinals: references,
                            word_74_raw: 0,
                            word_78_raw: 0,
                            animator_present: true,
                            animator_index_raw: 7,
                            shape_index_raw: None,
                        })
                    },
                    &mut factory,
                )
                .unwrap();
            audit.query(
                ordinal,
                index,
                zone::Context {
                    loaded_ordinal: Some(0),
                    zone_leaf: Some(2),
                    callback_zone: 0x102,
                    packed: 0x0102_0000,
                    nodes_visited: 1,
                },
            );
        }
        registry.fix_references().unwrap();
        registry.link_parents().unwrap();
        registry.initialize_map().unwrap();
        audit.slots(registry.slots(0).unwrap());
        audit.finish(&registry, &factory.0, 1).unwrap()
    };
    let first = observe(1, 0);
    let second = observe(2, 0);
    assert_eq!((first.a3_triggers, first.range27_triggers), (1, 1));
    assert_eq!((first.common09_triggers, first.ad_triggers), (1, 1));
    assert_eq!(
        (
            first.membership_entries,
            first.active_triggers,
            first.stopped_animators
        ),
        (4, 0, 4)
    );
    assert_eq!(
        (first.resolved_references, first.blocked_references),
        (4, 0)
    );
    assert_eq!(first.animators_initialized, 0);
    assert_ne!(first.state_sha256, second.state_sha256);
    let staged = observe(1, 0x4200_0023);
    assert_eq!((staged.a3_triggers, staged.range27_triggers), (1, 1));
    assert_eq!(
        (
            staged.owned_triggers,
            staged.membership_entries,
            staged.staged_triggers
        ),
        (4, 3, 1)
    );
    assert_eq!(staged.unsupported_factory_slots, 1);
    assert_eq!(
        (staged.resolved_references, staged.blocked_references),
        (2, 1)
    );
    assert!(!staged.catalog_complete);
}

#[test]
fn mixed_inactive_and_staged_objects_preserve_distinct_ownership_and_membership() {
    let mut registry = Registry::new(triggers::Budget {
        maps: 1,
        rows: 4,
        objects: 4,
        filter_words: 4,
        fixups: 32,
        catalog_entries: 4,
    });
    let map = MapContext {
        instance: InstanceId::new(1).unwrap(),
        map_selector: 1,
    };
    registry.begin_map(map, 4, None).unwrap();
    let mut factory = Factory(super::super::animators::Owned::default());
    let classes = [
        (28, 0x1e00_0003, 0x1f00_0081),
        (0, 0x1e00_0001, 0x1f00_0001),
        (89, 0x1e00_0004, 0x1f00_00be),
        (0, 0x1e00_0001, 0x1f00_0001),
    ];
    for (ordinal, (native_type_index, class, subtype)) in classes.into_iter().enumerate() {
        let mut numeric = [0; 16];
        if ordinal == 2 {
            numeric[0] = 0x4700_00a5;
        }
        if ordinal == 3 {
            numeric[1] = 0x4200_0002;
        }
        let mut references = [u32::MAX; 8];
        if ordinal < 2 {
            references[0] = ordinal as u32 + 1;
        }
        registry
            .visit(
                RowInput {
                    ordinal,
                    zone: 0x102,
                    descriptor: Descriptor {
                        native_type_index,
                        class,
                        subtype,
                        enable_byte_raw: 1,
                    },
                },
                || {
                    Ok(CommonInput {
                        header_flags_raw: 0x200,
                        property_mask_raw: 3 | (1 << 16) | (1 << 26),
                        position_bits: [0; 4],
                        rotation_bits: [0; 4],
                        numeric,
                        reference_ordinals: references,
                        word_74_raw: 0,
                        word_78_raw: 0,
                        animator_present: true,
                        animator_index_raw: 0,
                        shape_index_raw: None,
                    })
                },
                &mut factory,
            )
            .unwrap();
    }
    registry.fix_references().unwrap();
    registry.link_parents().unwrap();
    registry.initialize_map().unwrap();
    let mut audit = Audit::new(&identity());
    audit.map(map, None, 4);
    for (ordinal, index) in [28, 0, 89, 0].into_iter().enumerate() {
        audit.query(
            ordinal,
            index,
            zone::Context {
                loaded_ordinal: Some(0),
                zone_leaf: Some(2),
                callback_zone: 0x102,
                packed: 0x0102_0000,
                nodes_visited: 1,
            },
        );
    }
    audit.slots(registry.slots(0).unwrap());
    let evidence = audit.finish(&registry, &factory.0, 1).unwrap();
    assert_eq!(
        (
            evidence.original_slots,
            evidence.owned_triggers,
            evidence.common_scratch
        ),
        (4, 4, 4)
    );
    assert_eq!(
        (
            evidence.membership_entries,
            evidence.active_triggers,
            evidence.staged_triggers
        ),
        (3, 1, 1)
    );
    assert_eq!(
        (
            evidence.start_points,
            evidence.generic_triggers,
            evidence.common_be_triggers
        ),
        (1, 2, 1)
    );
    assert_eq!(
        (evidence.stopped_animators, evidence.animators_initialized),
        (2, 1)
    );
    assert_eq!(
        (
            evidence.unsupported_factory_slots,
            evidence.resolved_references
        ),
        (1, 2)
    );
    assert!(!evidence.catalog_complete);
    assert_eq!(
        serde_json::to_value(evidence)
            .unwrap()
            .as_object()
            .unwrap()
            .len(),
        61
    );
}
