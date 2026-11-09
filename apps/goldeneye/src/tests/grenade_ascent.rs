use super::*;
use crate::world_query::{BoundsInput, World};
use std::cell::Cell;
fn snapshot() -> Snapshot {
    Snapshot {
        worlds: Vec::new(),
        start: [10., 2., -7.],
        target: [30., 4., 3.],
        angle_index: 0,
        small_lift_raw: 1,
    }
}
fn unavailable_world() -> World {
    World {
        resource: "unavailable-grenade-world.edb".to_owned(),
        map: 0x85000000,
        bounds: BoundsInput {
            minimum: [f32::NAN; 3],
            maximum: [f32::NAN; 3],
        },
    }
}
#[test]
fn ordinary_arc_rejection_never_loads_or_reports_a_clearance_result() {
    let mut snapshot = snapshot();
    snapshot.start = [0.; 3];
    snapshot.target = [1., 100., 0.];
    snapshot.worlds.push(unavailable_world());
    let result = query_loaded(&snapshot, |_| {
        panic!("rejected arc must not load a resource")
    })
    .unwrap();
    assert_eq!(result["candidate"]["rejection"], "elevation");
    for field in ["volume", "typed_request", "world_query", "blocked"] {
        assert!(result[field].is_null(), "{field}");
    }
    assert_eq!(result["reference_compared"], false);
}
#[test]
fn explicit_empty_worlds_preserve_native_request_settings_without_loading() {
    let result = query_loaded(&snapshot(), |_| {
        panic!("explicit empty world list must not load")
    })
    .unwrap();
    assert_eq!(result["blocked"], false);
    assert_eq!(result["world_query"]["world_count"], 0);
    assert!(result["world_query"]["selected_world"].is_null());
    assert!(result["world_query"]["query"].is_null());
    let request = &result["typed_request"];
    assert_eq!(request["world_origin"], json!([10., 2., -7.]));
    assert_ne!(request["world_origin"], request["boxes"][0]["center"]);
    assert_eq!(request["radius"], result["volume"]["selection_radius"]);
    assert_eq!(request["boxes"].as_array().unwrap().len(), 1);
    assert_eq!(request["boxes"][0], result["volume"]["shape"]);
    assert_eq!(request["shape_type"], 3);
    assert_eq!(
        (
            request["filter_slot"].as_u64(),
            request["filter_flags"].as_u64(),
            request["request_flags_raw"].as_u64()
        ),
        (Some(1), Some(3), Some(2))
    );
    assert_eq!(
        (
            request["record_capacity"].as_u64(),
            request["contact_capacity"].as_u64()
        ),
        (Some(64), Some(512))
    );
    assert_eq!(
        request["world_from_query"],
        json!([
            1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.
        ])
    );
    assert!(!result["scope"].as_str().unwrap().is_empty());
}
#[test]
fn selected_resource_failure_propagates_instead_of_becoming_unblocked() {
    let mut snapshot = snapshot();
    snapshot.worlds.push(unavailable_world());
    let calls = Cell::new(0);
    let error = query_loaded(&snapshot, |resource| {
        assert_eq!(resource, "unavailable-grenade-world.edb");
        calls.set(calls.get() + 1);
        anyhow::bail!("unavailable grenade resource")
    })
    .unwrap_err();
    assert_eq!(calls.get(), 1);
    assert!(format!("{error:#}").contains("unavailable grenade resource"));
    let mut invalid = snapshot;
    invalid.angle_index = 5;
    assert!(query_loaded(&invalid, |_| panic!("invalid angle before resource access")).is_err());
}
