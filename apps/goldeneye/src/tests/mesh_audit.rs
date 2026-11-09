use super::*;
use serde_json::json;

fn fixture() -> Value {
    json!({"reference_compared":false,"request":{"world_from_query":IDENTITY},
      "resident_meshes":1,"inverse_preparations":1,"contacts":[],
      "collection":{"records":[{"geometry_key":100,"flags_raw":0}],"dispatch":[{"full_rejections":2}]},
      "probes":[{"record_key":1,"geometry_key":100,"coarse":null,
        "cache":{"disposition":"generated","triangle_visits":4,"coordinate_reads":12},
        "mesh":{"coarse_tests":0,"coarse_rejections":0,"included_batches":2,"batch_rejections":1,
          "triangle_tests":3,"triangle_rejections":2,"flag_words_consumed":8,"primitive_queries":1,
          "primitive_contacts":0,"appended":0,"capacity_mesh_skips":0,"post_hit_full":0}}]})
}
fn contact() -> Value {
    json!({"collected_record_key":1,"first_corner_key":120,"query_shape_index":0,"flags_raw":0,
      "triangle_flags_raw":4,"material_raw":7,"position_indices":[1,3,2],"normal":[1.,0.,0.,0.],
      "witness_a":[0.,0.,0.,0.],"witness_b":[1.,0.,0.,0.],"depth":0.25})
}
#[test]
fn counters_distinguish_absent_bypassed_and_measured_root_leaf_coarse_queries() {
    let mut audit = Audit::default();
    let mut result = fixture();
    audit
        .observe(&result, 1000, |_, _| panic!("no contacts"))
        .unwrap();
    assert_eq!(
        (
            audit.records_collected,
            audit.cache_generated,
            audit.batch_rejections
        ),
        (1, 1, 1)
    );
    assert_eq!(audit.coarse_absent_records, 1);
    assert_eq!(audit.coarse_maximum_fanout, None);
    result["collection"]["records"][0]["flags_raw"] = json!(2);
    result["probes"][0]["cache"]["disposition"] = json!("reused");
    audit
        .observe(&result, 1000, |_, _| panic!("no contacts"))
        .unwrap();
    assert_eq!(audit.coarse_bypassed_general_records, 1);
    assert_eq!(audit.cache_reused, 1);
    result["collection"]["records"][0]["flags_raw"] = json!(0);
    result["probes"][0]["coarse"] =
        json!({"overlaps":false,"node_reads":1,"sphere_tests":0,"leaf_tests":1,"maximum_fanout":0});
    result["probes"][0]["mesh"]["coarse_tests"] = json!(1);
    result["probes"][0]["mesh"]["coarse_rejections"] = json!(1);
    audit
        .observe(&result, 1000, |_, _| panic!("no contacts"))
        .unwrap();
    assert_eq!(
        (
            audit.coarse_queries,
            audit.coarse_rejections,
            audit.coarse_leaf_tests
        ),
        (1, 1, 1)
    );
    assert_eq!(audit.coarse_maximum_fanout, Some(0));
    assert_eq!(audit.record_capacity_rejections, 6);
}
#[test]
fn contact_counts_require_finite_identity_values_and_provenance_callback_success() {
    let mut result = fixture();
    result["contacts"] = json!([contact()]);
    result["probes"][0]["mesh"]["primitive_contacts"] = json!(1);
    result["probes"][0]["mesh"]["appended"] = json!(1);
    let mut calls = 0;
    let mut audit = Audit::default();
    audit
        .observe(&result, 1000, |_, key| {
            assert_eq!(key, 100);
            calls += 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(calls, 1);
    assert_eq!(
        (
            audit.contacts_appended,
            audit.contacts_validated,
            audit.queries_with_contacts
        ),
        (1, 1, 1)
    );
    assert!(
        Audit::default()
            .observe(&result, 1000, |_, _| anyhow::bail!("source mismatch"))
            .is_err()
    );
    result["contacts"][0]["normal"][3] = json!(1.);
    assert!(
        Audit::default()
            .observe(&result, 1000, |_, _| Ok(()))
            .is_err()
    );
}
#[test]
fn missing_or_inconsistent_mesh_evidence_is_an_error_instead_of_zero_work() {
    let mut result = fixture();
    result["probes"][0]["mesh"]["triangle_tests"] = Value::Null;
    assert!(
        Audit::default()
            .observe(&result, 1000, |_, _| Ok(()))
            .is_err()
    );
    let mut result = fixture();
    result["probes"][0]["cache"]["disposition"] = json!("unknown");
    assert!(
        Audit::default()
            .observe(&result, 1000, |_, _| Ok(()))
            .is_err()
    );
    let mut result = fixture();
    result["probes"][0]["mesh"]["appended"] = json!(1);
    assert!(
        Audit::default()
            .observe(&result, 1000, |_, _| Ok(()))
            .is_err()
    );
}
#[test]
fn source_provenance_checks_odd_strip_swap_flags_and_batch_material() {
    use ge_assets::collision::{CollisionBatch, CollisionTriangle};
    let mut mesh = CollisionMesh {
        entity: None,
        address: 100,
        index_format_raw: 0,
        declared_positions: 4,
        positions_read: 0,
        disabled_triangles: 0,
        skipped_batches: 0,
        positions: vec![],
        batches: vec![CollisionBatch {
            flags_raw: 0,
            material_raw: 7,
            declared_triangles: 1,
            first_triangle: 0,
            first_flag_index: 0,
            triangle_count: 1,
            unconsumed_stream_bytes: 0,
        }],
        triangles: vec![CollisionTriangle {
            position_indices: [1, 2, 3],
            first_corner_offset: 120,
            flag_index: 0,
            flags_raw: 4,
            command_raw: 0x98,
            batch_index: 0,
            strip_in_batch: 0,
            triangle_in_strip: 1,
        }],
    };
    let mut value = contact();
    validate_provenance(&mesh, &value).unwrap();
    value["position_indices"] = json!([1, 2, 3]);
    assert!(validate_provenance(&mesh, &value).is_err());
    value = contact();
    value["material_raw"] = json!(8);
    assert!(validate_provenance(&mesh, &value).is_err());
    mesh.triangles[0].flags_raw |= 0x200;
    assert!(validate_provenance(&mesh, &contact()).is_err());
}
