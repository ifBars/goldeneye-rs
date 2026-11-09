use super::super::metadata::map_metadata;
use super::*;

const LEVEL: u32 = 0x0181_00a5;

fn resource() -> Edb {
    resource_with_link(false)
}

fn resource_with_link(linked: bool) -> Edb {
    super::super::fixtures::resource(
        LEVEL,
        0x0500_00a5,
        linked.then_some((0x018b_00a5, 0x0500_00a5, 3)),
    )
}

fn registry(selected: bool) -> LevelRegistry {
    let mut primary = [0; 45];
    primary[0] = LEVEL;
    primary[4] = 0x0801_00a5;
    primary[11] = 0x4210_00a5;
    primary[20..23].copy_from_slice(&[0x2d50_00a5, u32::MAX, 0]);
    primary[44] = 0x7fc0_00a5;
    primary[0x78 / 4] = super::super::player::fixtures::CHARACTER;
    let mut secondary = [0; 29];
    secondary[0] = LEVEL;
    let mut registry = LevelRegistry::new(
        vec![primary],
        vec![secondary],
        ROW_BUDGET,
        InitialSelection {
            current_hash: u32::MAX,
            previous_hash: u32::MAX,
            primary_hash: None,
            secondary_hash: None,
        },
    )
    .unwrap();
    if selected {
        registry.select(LEVEL);
    }
    registry
}

#[test]
fn retains_actual_catalog_resource_and_coded_reference_metadata_without_world_claims() {
    let context = MissionContext::from_resources(
        registry(true),
        resource(),
        Difficulty::Agent,
        0x480,
        (1, 1),
        0x204,
    )
    .unwrap();
    assert_eq!(
        context.registry.selected_primary().unwrap().words_raw()[44],
        0x7fc0_00a5
    );
    assert_eq!(context.retained_resources()[0].header.hash, LEVEL);
    let json = serde_json::to_value(context.evidence()).unwrap();
    assert_eq!(json["state"], "resources_retained");
    assert_eq!(json["selection"]["difficulty"], 1);
    assert_eq!(json["selection"]["source_row_address"], 0x204);
    assert_eq!(json["selection"]["tuning_hash"], LEVEL);
    assert_eq!(json["resource"]["requested_map_reference"], 0x8500_0000u32);
    assert_eq!(json["resource"]["resolved_map_hash"], 0x0500_00a5);
    assert_eq!(json["resource"]["maps"][0]["placements"], 1);
    assert_eq!(
        json["resource"]["maps"][0]["first_placement_reference"],
        0x8200_0000u32
    );
    assert_eq!(
        json["resource"]["maps"][0]["first_placement_entity"]["directory_index"],
        0
    );
    assert_eq!(
        json["resource"]["maps"][0]["first_placement_entity"]["hash"],
        0x0200_00a5
    );
    assert_eq!(
        json["resource"]["maps"][0]["first_placement_entity"]["entity_type"],
        0x607
    );
    assert_eq!(json["resource"]["script_descriptors"], 1);
    assert!(json["resource"]["next_link"].is_null());
    assert_eq!(json["resources_retained"], true);
    for field in [
        "world_registered",
        "player_initialized",
        "reference_compared",
    ] {
        assert_eq!(json[field], false);
    }
    let encoded = serde_json::to_string(&json).unwrap();
    assert!(!encoded.contains("words_raw"));
    assert!(!encoded.contains("vertices"));
    assert!(json.get("registry").is_none() && json.get("data").is_none());
}

#[test]
fn absent_selection_mismatched_resource_and_unsupported_payload_cannot_produce_context() {
    assert!(
        MissionContext::from_resources(
            registry(false),
            resource(),
            Difficulty::Classic,
            0x480,
            (1, 1),
            0x204
        )
        .is_err()
    );
    let mut wrong = resource();
    wrong.header.hash ^= 1;
    assert!(
        MissionContext::from_resources(
            registry(true),
            wrong,
            Difficulty::Classic,
            0x480,
            (1, 1),
            0x204
        )
        .is_err()
    );
    for (version, payload) in [(262, 0x480), (263, 64 * 1024 * 1024 + 1)] {
        let mut wrong = resource();
        wrong.header.version = version;
        wrong.header.payload_bytes = payload;
        assert!(
            MissionContext::from_resources(
                registry(true),
                wrong,
                Difficulty::Classic,
                0x480,
                (1, 1),
                0x204
            )
            .is_err()
        );
    }
}

#[test]
fn malformed_map_metadata_is_an_error_and_missing_entity_reference_stays_explicit() {
    let mut duplicate = resource();
    duplicate.tables[6].count = 2;
    // The second descriptor is zero, so it is a missing map body, not another
    // valid map or permission to silently return the first metadata record.
    assert!(map_metadata(&duplicate).is_err());
    let mut missing = resource();
    missing.tables[2].count = 0;
    let maps = map_metadata(&missing).unwrap();
    assert_eq!(maps[0].first_placement_reference, Some(0x8200_0000));
    assert!(maps[0].first_placement_entity.is_none());
    assert!(Difficulty::try_from(4).is_err());
}

#[test]
fn linked_map_identity_and_actual_residency_are_recorded_without_world_activation() {
    let context = MissionContext::from_resources(
        registry(true),
        resource_with_link(true),
        Difficulty::Operative,
        0x480,
        (1, 1),
        0x204,
    )
    .unwrap();
    let json = serde_json::to_value(context.evidence()).unwrap();
    assert_eq!(json["resource"]["next_link"]["file"], 0x018b_00a5);
    assert_eq!(json["resource"]["next_link"]["map"], 0x0500_00a5);
    assert_eq!(
        json["resource"]["next_link"]["locator"]["map_reference"],
        0x8500_0000u32
    );
    assert_eq!(
        json["resource"]["next_link"]["locator"]["map_hash"],
        0x0500_00a5
    );
    assert_eq!(
        json["resource"]["next_link"]["locator"]["trigger_ordinal"],
        0
    );
    assert_eq!(json["resource"]["resource_hash"], LEVEL);
    assert_eq!(context.retained_resources().len(), 2);
    assert_eq!(context.retained_resources()[1].header.hash, 0x018b_00a5);
    assert_eq!(json["residency"]["complete"], true);
    assert_eq!(json["residency"]["retained_files"], 2);
    assert_eq!(json["map_instances"]["bound"], true);
    assert_eq!(json["map_instances"]["loaded_instances"], 2);
    assert_eq!(context.map_instances().len(), 2);
    assert_eq!(json["spawn"]["catalog_membership_verified"], false);
    assert_eq!(json["spawn"]["catalog_created"], false);
    assert_eq!(json["spawn"]["camera_frame_ready"], false);
    assert_eq!(json["registration"]["prepared"], true);
    assert_eq!(
        json["registration"]["preference_snapshot"],
        "source_initializer_once"
    );
    assert_eq!(json["registration"]["native_init_timing_verified"], false);
    assert_eq!(json["registration"]["rows"].as_array().unwrap().len(), 2);
    assert_eq!(
        context.registration_plan().rows().len(),
        context.retained_resources().len()
    );
    assert_eq!(json["world_registered"], false);
    assert_eq!(json["player_initialized"], false);
}

impl MissionContext {
    fn from_resources(
        registry: LevelRegistry,
        mission: Edb,
        difficulty: Difficulty,
        catalog_bytes: u32,
        read_counts: (usize, usize),
        first_row: usize,
    ) -> Result<Self> {
        let root_file = mission.header.hash;
        let mut root = Some(mission);
        let residency = Residency::from_loader(
            root_file,
            super::super::residency::Budget::default(),
            |file, _maximum| {
                if file == root_file {
                    root.take().context("authored root was loaded twice")
                } else {
                    Ok(super::super::fixtures::resource(file, 0x0500_00a6, None))
                }
            },
        )?;
        let player_components = super::super::player::fixtures::binding()?;
        Self::from_residency(ContextInput {
            registry,
            residency,
            player_components,
            difficulty,
            catalog_bytes,
            read_counts,
            first_row,
            requested_id: 1,
        })
    }
}
