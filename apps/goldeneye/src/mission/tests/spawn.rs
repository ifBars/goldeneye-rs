use super::super::fixtures::{self, Point};
use super::*;
use ge_core::mission::{
    registration::{Budget as RegistrationBudget, Preferences},
    residency::Request,
};

const ROOT: u32 = 0x0181_00a5;
const GRAPHICS: u32 = 0x018b_00a5;

fn point(id: Option<u32>, flags: u32) -> Point {
    Point {
        id,
        flags,
        position: [0x3fa0_0000, 0x4020_0000, 0xc060_0000],
        rotation: [0x3e00_0000, 0xbf00_0000, 0x8000_0000],
    }
}

fn registration(files: &[u32], graphics_filter: Vec<u32>) -> RegistrationPlan {
    RegistrationPlan::build(
        &files
            .iter()
            .map(|&file| Request {
                file,
                map_reference: 0x8500_0000,
            })
            .collect::<Vec<_>>(),
        Preferences {
            prefixes_98c: vec![0x018b_0000],
            prefixes_9ac: vec![0x0181_0000],
            prefixes_9cc: vec![],
            filter_9ec: graphics_filter,
            filter_a2c: vec![],
        },
        RegistrationBudget {
            rows: 8,
            preference_words: 16,
        },
    )
    .unwrap()
}

#[test]
fn registration_filter_is_applied_before_excluded_data_and_source_selection_retains_pose_privately()
{
    let mut ignored = fixtures::resource(GRAPHICS, 0x0500_00a6, None);
    ignored.tables[6].count = 0; // Excluded StartPoint data must not be addressed.
    let resources = [
        fixtures::startpoint_resource(ROOT, 0x8500_0000, &[point(Some(2), 0), point(Some(1), 0)]),
        ignored,
    ];
    let prepared = PreparedSpawn::prepare(
        &registration(&[ROOT, GRAPHICS], vec![0x1f00_0088]),
        &resources,
        1,
    )
    .unwrap();
    let pose = prepared.plan().pose().unwrap();
    assert_eq!(
        pose.position_bits,
        [0x3fa0_0000, 0x4020_0000, 0xc060_0000, 0]
    );
    assert_eq!(pose.rotation_bits, [0, 0xbf00_0000, 0, 0]);
    assert_eq!(pose.rotation_mode, 3);
    let value = serde_json::to_value(prepared.evidence()).unwrap();
    assert_eq!(value["prepared_entries"], 2);
    assert_eq!(value["eligible_files"], 1);
    assert_eq!(value["filtered_files"], 1);
    assert_eq!(value["selected_catalog_index"], 1);
    assert_eq!(value["selected"]["file_hash"], ROOT);
    assert_eq!(value["selected"]["locator"]["trigger_ordinal"], 1);
    assert_eq!(
        value["pose_sha256"],
        "aa29b4999c9bf5153f878943d7bf28dee63ecaf916b7b72c634c521416c23238"
    );
    assert_eq!(value["pose_sha256_encoding"], ENCODING);
    for field in [
        "catalog_membership_verified",
        "catalog_created",
        "creation_inputs_verified",
        "camera_frame_ready",
    ] {
        assert_eq!(value[field], false);
    }
    let encoded = serde_json::to_string(&value).unwrap();
    assert!(!encoded.contains("\"position_bits\":") && !encoded.contains("\"rotation_bits\":"));
    assert!(!encoded.contains("3fa00000"));
}

#[test]
fn missing_requested_id_falls_back_to_the_source_default_and_records_request_reset() {
    let resources = [fixtures::startpoint_resource(
        ROOT,
        0x8500_0000,
        &[point(Some(2), 0), point(None, 0)],
    )];
    let prepared = PreparedSpawn::prepare(&registration(&[ROOT], vec![]), &resources, 99).unwrap();
    let value = serde_json::to_value(prepared.evidence()).unwrap();
    assert_eq!(value["requested_id_before"], 99);
    assert_eq!(value["requested_id_after"], 0);
    assert_eq!(value["used_fallback"], true);
    assert_eq!(value["selected_catalog_index"], 1);
    assert_eq!(value["visits"], 4);
}

#[test]
fn disabled_or_missing_startpoints_never_invent_a_zero_origin_or_created_catalog() {
    let resources = [fixtures::startpoint_resource(
        ROOT,
        0x8500_0000,
        &[point(Some(1), 1)],
    )];
    let prepared = PreparedSpawn::prepare(&registration(&[ROOT], vec![]), &resources, 1).unwrap();
    assert!(prepared.plan().pose().is_none());
    let value = serde_json::to_value(prepared.evidence()).unwrap();
    assert_eq!(value["state"], "missing_startpoint");
    assert_eq!(value["selection_found"], false);
    assert_eq!(value["requested_id_after"], 1);
    assert!(value["pose_sha256"].is_null() && value["selected"].is_null());
    assert_eq!(value["catalog_created"], false);
}

#[test]
fn null_and_empty_allowlists_remain_distinct_from_a_filter_admitting_startpoints() {
    let resources = [
        fixtures::startpoint_resource(ROOT, 0x8500_0000, &[]),
        fixtures::startpoint_resource(GRAPHICS, 0x0500_00a6, &[point(Some(1), 0)]),
    ];
    let rejected =
        PreparedSpawn::prepare(&registration(&[ROOT, GRAPHICS], vec![]), &resources, 1).unwrap();
    assert!(rejected.plan().pose().is_none());
    let admitted = PreparedSpawn::prepare(
        &registration(&[ROOT, GRAPHICS], vec![STARTPOINT_SUBTYPE]),
        &resources,
        1,
    )
    .unwrap();
    let value = serde_json::to_value(admitted.evidence()).unwrap();
    assert_eq!(value["selection_found"], true);
    assert_eq!(value["eligible_files"], 2);
    assert_eq!(value["selected"]["source_ordinal"], 1);
    assert_eq!(value["selected"]["locator"]["map_hash"], 0x0500_00a6);
}

#[test]
fn reached_corrupt_or_mismatched_data_fails_without_a_partial_spawn_preparation() {
    let plan = registration(&[ROOT, GRAPHICS], vec![STARTPOINT_SUBTYPE]);
    let root = || fixtures::startpoint_resource(ROOT, 0x8500_0000, &[point(Some(1), 0)]);
    let mut corrupt = fixtures::resource(GRAPHICS, 0x0500_00a6, None);
    corrupt.tables[6].count = 0;
    assert!(PreparedSpawn::prepare(&plan, &[root(), corrupt], 1).is_err());
    assert!(PreparedSpawn::prepare(&plan, &[root()], 1).is_err());
    let wrong = fixtures::resource(GRAPHICS ^ 1, 0x0500_00a6, None);
    assert!(PreparedSpawn::prepare(&plan, &[root(), wrong], 1).is_err());
}
