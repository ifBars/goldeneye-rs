use super::*;
use serde_json::json;
#[test]
fn cohort_concatenation_retains_duplicate_zones_and_caps_once_without_stopping_later_calls() {
    let mut calls = Vec::new();
    let result = concatenate(&[2, 4, 2], 3, |zone| {
        calls.push(zone);
        Ok(vec![json!(zone), json!(zone + 10)])
    })
    .unwrap();
    assert_eq!(result, [json!(2), json!(12), json!(4)]);
    assert_eq!(calls, [2, 4, 2]);
    let mut calls = Vec::new();
    assert!(
        concatenate(&[2, 4], 0, |zone| {
            calls.push(zone);
            Ok(vec![json!(zone)])
        })
        .unwrap()
        .is_empty()
    );
    assert_eq!(calls, [2, 4]);
}
fn fixture() -> Value {
    json!({"reference_compared":false,"resident_meshes":1,"inverse_preparations":2,"contacts":[],
      "zone_binding":{"request_flags_raw":2,"initial_zone":1,"zones":[1,1],"bsp_nodes_tested":3,"portal_tests":2},
      "collection":{"records":[{"geometry_key":100},{"geometry_key":100}],"zones":[
        {"zone":1,"record_start":0,"record_end":1,"dispatch":[{"appended":1,"full_rejections":2}]},
        {"zone":1,"record_start":1,"record_end":2,"dispatch":[{"appended":1,"full_rejections":0}]}]},
      "probes":[{"mesh":{"capacity_mesh_skips":0,"post_hit_full":0}},{"mesh":{"capacity_mesh_skips":1,"post_hit_full":0}}]})
}
#[test]
fn observer_distinguishes_duplicate_zones_and_rejects_per_zone_record_resets() {
    let result = fixture();
    let mut audit = Audit::default();
    audit.observe(&result, false, 0).unwrap();
    assert_eq!(
        (
            audit.expanded_queries,
            audit.zones_selected,
            audit.duplicate_zones
        ),
        (1, 2, 1)
    );
    assert_eq!(
        (
            audit.records_collected,
            audit.record_capacity_rejections,
            audit.contact_capacity_mesh_skips
        ),
        (2, 2, 1)
    );
    let mut bad = fixture();
    bad["collection"]["zones"][1]["record_start"] = json!(0);
    assert!(Audit::default().observe(&bad, false, 0).is_err());
    let mut bad = fixture();
    bad["collection"]["zones"][1]["zone"] = json!(2);
    assert!(Audit::default().observe(&bad, false, 0).is_err());
    assert!(Audit::default().observe(&result, true, 0).is_err());
}
#[test]
fn single_zone_cohort_comparison_requires_both_record_order_and_contact_identity() {
    let value = json!({"collection":{"records":[{"geometry_key":100},{"geometry_key":200}]},"contacts":[{"depth":1.}]});
    compare_single(&value, &value).unwrap();
    let mut bad = value.clone();
    bad["collection"]["records"]
        .as_array_mut()
        .unwrap()
        .reverse();
    assert!(compare_single(&value, &bad).is_err());
    let mut bad = value.clone();
    bad["contacts"][0]["depth"] = json!(2.);
    assert!(compare_single(&value, &bad).is_err());
}
