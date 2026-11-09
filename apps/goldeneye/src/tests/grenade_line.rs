use super::*;
use crate::world_query::{BoundsInput, World};
use std::cell::Cell;

fn snapshot() -> Snapshot {
    Snapshot {
        worlds: Vec::new(),
        cached_apex: [2., 3., 4.],
        target: [8., 7., -2.],
        small_lift_raw: 0,
    }
}

#[test]
fn empty_worlds_never_load_and_retain_constructor_bounds_before_large_lift() {
    let result = query_loaded(&snapshot(), |_| panic!("empty worlds must not load")).unwrap();
    assert_eq!(result["blocked"], false);
    assert_eq!(result["world_query"]["world_count"], 0);
    assert!(result["world_query"]["selected_world"].is_null());
    assert!(result["world_query"]["query"].is_null());
    let request = &result["typed_request"];
    assert_eq!(request["origin"], json!([2., 3., 4.]));
    assert_eq!(request["constructor_delta"], json!([6., 4., -6.]));
    assert_eq!(request["delta"], json!([6., 5., -6.]));
    assert_eq!(
        request["initial_world_bounds"]["minimum"],
        json!([2., 3., -2.])
    );
    assert_eq!(
        request["initial_world_bounds"]["maximum"],
        json!([8., 7., 4.])
    );
    assert_eq!(request["maximum_parameter"], 1.);
    for (field, expected) in [
        ("filter_slot", 1),
        ("filter_flags", 0),
        ("required_any", 0),
        ("excluded_any", 0),
        ("stop_first_raw", 0),
    ] {
        assert_eq!(request[field], expected, "{field}");
    }
    assert_eq!(result["reference_compared"], false);
    let scope = result["scope"].as_str().unwrap();
    assert!(scope.contains("world miss still requires the separate actor pass"));
    assert!(scope.contains("No full clearance"));
    assert!(result.get("ready").is_none());
    assert!(result.get("action_state").is_none());
}

#[test]
fn raw_255_uses_small_lift_and_preserves_source_constructor_displacement() {
    let mut snapshot = snapshot();
    snapshot.small_lift_raw = 255;
    let result = query_loaded(&snapshot, |_| panic!("empty worlds must not load")).unwrap();
    let request = &result["typed_request"];
    assert_eq!(request["constructor_delta"], json!([6., 4., -6.]));
    let delta: [f32; 3] = serde_json::from_value(request["delta"].clone()).unwrap();
    assert_eq!(delta, [6., 4.1, -6.]);
    assert_eq!(
        request["initial_world_bounds"]["maximum"],
        json!([8., 7., 4.])
    );
}

#[test]
fn selected_resource_error_propagates_and_invalid_request_fails_before_loading() {
    let mut snapshot = snapshot();
    snapshot.worlds.push(World {
        resource: "missing-grenade-line.edb".into(),
        map: 0x85000000,
        // Single-world selection must not read these stored bounds.
        bounds: BoundsInput {
            minimum: [f32::NAN; 3],
            maximum: [f32::NAN; 3],
        },
    });
    let calls = Cell::new(0);
    let error = query_loaded(&snapshot, |resource| {
        calls.set(calls.get() + 1);
        assert_eq!(resource, "missing-grenade-line.edb");
        anyhow::bail!("missing selected grenade line resource")
    })
    .unwrap_err();
    assert_eq!(calls.get(), 1);
    assert!(format!("{error:#}").contains("missing selected grenade line resource"));
    snapshot.cached_apex[0] = f32::INFINITY;
    assert!(
        query_loaded(&snapshot, |_| panic!(
            "invalid line input must precede load"
        ))
        .is_err()
    );
}

#[test]
fn hit_presence_controls_only_world_blocking_and_missing_payloads_are_errors() {
    assert!(
        world_blocked(
            1,
            &json!({"selected_world":0,"query":{"line":{"hit":{"parameter":0.5}}}})
        )
        .unwrap()
    );
    assert!(
        !world_blocked(
            1,
            &json!({"selected_world":0,"query":{"line":{"hit":null}}})
        )
        .unwrap()
    );
    for report in [
        json!({}),
        json!({"selected_world":null,"query":null}),
        json!({"selected_world":0,"query":null}),
        json!({"selected_world":0,"query":{"line":{}}}),
        json!({"selected_world":0,"query":{"line":{"hit":false}}}),
        json!({"selected_world":1,"query":{"line":{"hit":null}}}),
    ] {
        assert!(world_blocked(1, &report).is_err(), "{report}");
    }
}

#[test]
fn snapshot_schema_rejects_unknown_fields_and_out_of_range_lift_bytes() {
    let mut value =
        json!({"worlds":[],"cached_apex":[0.,1.,2.],"target":[3.,4.,5.],"small_lift_raw":1});
    value["assume_no_actors"] = json!(true);
    assert!(serde_json::from_value::<Snapshot>(value.clone()).is_err());
    value.as_object_mut().unwrap().remove("assume_no_actors");
    value["small_lift_raw"] = json!(256);
    assert!(serde_json::from_value::<Snapshot>(value).is_err());
}
