use super::*;
use serde_json::json;

fn fixture() -> Value {
    json!({"reference_compared":false,"request":{"stop_first_raw":0},"resident_meshes":1,
        "zone_binding":{"zones":[0],"capacity_reached":false,"node_tests":1},
        "line":{"maximum_parameter":1.,"hit":null,"binding":null,"world_normal":null,"parameter_point":null},
        "roots":[{"root_key":100,"working_origin":[0.,3.,0.,1.],"working_delta":[0.,-2.,0.,0.],"child_coarse":[],
            "meshes":[{"coarse":null,"mesh":{"geometry_key":100,"hit":null,
                "cache_preparation":{"disposition":"generated"},"planes_generated":true,"planes_count":4,
                "bounds_rejected":false,"coarse_rejected":false,"batch_bounds_tests":2,"batches_rejected":1,
                "triangles_tested":3,"triangles_rejected":3,"flag_words_advanced":5,"plane_records_advanced":5}}]}]})
}
fn hit() -> Value {
    json!({"geometry_key":100,"parameter":0.5,"local_plane":[0.,1.,0.,-2.],"first_corner_key":120,
        "flag_index":0,"batch_index":0,"triangle_in_strip":1,"triangle_flags_raw":4,"material_raw":7})
}
fn with_hit() -> Value {
    let mut value = fixture();
    let hit = hit();
    value["line"] = json!({"maximum_parameter":0.5,"hit":hit,"binding":{"root_key":100,"zone":0},
        "world_normal":[0.,1.,0.,-2.],"parameter_point":[0.,2.,0.]});
    value["roots"][0]["meshes"][0]["mesh"]["hit"] = hit;
    value
}
#[test]
fn counters_distinguish_absent_coarse_from_measured_zero_fanout_and_plane_reuse() {
    let mut audit = Audit::default();
    let mut value = fixture();
    audit
        .observe(&value, false, |_| panic!("miss has no source hits"))
        .unwrap();
    assert_eq!(audit.coarse_maximum_fanout, None);
    assert_eq!(
        (
            audit.queries,
            audit.nearest_queries,
            audit.misses,
            audit.planes_generated,
            audit.planes_prepared
        ),
        (1, 1, 1, 1, 4)
    );
    value["roots"][0]["meshes"][0]["mesh"]["planes_generated"] = json!(false);
    value["roots"][0]["meshes"][0]["mesh"]["cache_preparation"]["disposition"] = json!("reused");
    value["roots"][0]["meshes"][0]["coarse"] =
        json!({"node_reads":1,"sphere_tests":0,"leaf_tests":1,"maximum_fanout":0});
    audit.observe(&value, false, |_| panic!("miss")).unwrap();
    assert_eq!(audit.coarse_maximum_fanout, Some(0));
    assert_eq!(
        (
            audit.coarse_queries,
            audit.coarse_leaf_tests,
            audit.planes_reused,
            audit.cache_reused
        ),
        (1, 1, 1, 1)
    );
    assert_eq!(
        (audit.flag_words_advanced, audit.plane_records_advanced),
        (10, 10)
    );
}
#[test]
fn final_binding_and_world_normal_w_are_checked_but_any_hit_leaves_normal_absent() {
    let mut value = with_hit();
    let mut calls = 0;
    let mut audit = Audit::default();
    audit
        .observe(&value, false, |_| {
            calls += 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(calls, 2);
    assert_eq!(
        (audit.mesh_hits_validated, audit.final_hits_validated),
        (1, 1)
    );
    value["line"]["world_normal"][3] = json!(0.);
    assert!(Audit::default().observe(&value, false, |_| Ok(())).is_err());
    value["line"]["world_normal"] = Value::Null;
    value["request"]["stop_first_raw"] = json!(1);
    Audit::default().observe(&value, true, |_| Ok(())).unwrap();
    value["line"]["binding"]["root_key"] = json!(999);
    assert!(Audit::default().observe(&value, true, |_| Ok(())).is_err());
}
#[test]
fn raw_source_validation_requires_corner_flags_material_and_strip_ordinals() {
    use ge_assets::collision::{CollisionBatch, CollisionTriangle};
    let mesh = CollisionMesh {
        entity: None,
        address: 100,
        index_format_raw: 0,
        declared_positions: 0,
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
            position_indices: [0, 1, 2],
            first_corner_offset: 120,
            flag_index: 0,
            flags_raw: 4,
            command_raw: 0x98,
            batch_index: 0,
            strip_in_batch: 0,
            triangle_in_strip: 1,
        }],
    };
    let mut value = hit();
    assert_eq!(validate_hit(&mesh, &value).unwrap(), 0);
    value["triangle_in_strip"] = json!(0);
    assert!(validate_hit(&mesh, &value).is_err());
    value = hit();
    value["material_raw"] = json!(8);
    assert!(validate_hit(&mesh, &value).is_err());
    value = hit();
    value["flag_index"] = json!(1);
    assert!(validate_hit(&mesh, &value).is_err());
}
#[test]
fn mode_comparison_requires_same_existence_and_nearest_parameter_no_later_than_first() {
    let nearest = with_hit();
    let mut first = with_hit();
    first["line"]["hit"]["parameter"] = json!(0.75);
    compare_modes(&nearest, &first).unwrap();
    first["line"]["hit"]["parameter"] = json!(0.25);
    assert!(compare_modes(&nearest, &first).is_err());
    first["line"]["hit"] = Value::Null;
    assert!(compare_modes(&nearest, &first).is_err());
}
#[test]
fn independent_plane_diagnostic_separates_regular_short_and_degenerate_geometry() {
    let vertices = [[0., 0., 0.], [1., 0., 0.], [0., 0., 1.]];
    match plane_distance(vertices, [0.25, 2., 0.25]).unwrap() {
        PlaneDistance::Distance(distance) => assert_eq!(distance, 2.),
        _ => panic!("regular plane"),
    }
    assert!(matches!(
        plane_distance([[0.; 3]; 3], [0.; 3]).unwrap(),
        PlaneDistance::Degenerate
    ));
    assert!(matches!(
        plane_distance([[0., 0., 0.], [0.001, 0., 0.], [0., 0., 0.001]], [0.; 3]).unwrap(),
        PlaneDistance::Short
    ));
}
