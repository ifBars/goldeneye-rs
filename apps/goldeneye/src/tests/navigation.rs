use super::*;

#[test]
fn adapter_preserves_group_face_ids_endpoint_rules_and_checked_segment_selection() {
    let data = SurfaceData {
        triangles: [0., 2.]
            .into_iter()
            .enumerate()
            .map(|(index, y)| SurfaceTriangle {
                index,
                vertices: [[0., y, 0.], [2., y, 0.], [0., y, 2.]],
            })
            .collect(),
        groups: vec![0..1, 1..2],
        neighbors: vec![[Some(1), None, None], [Some(0), None, None]],
    };
    let ray = SurfaceRay {
        origin: [0.5, 3., 0.5],
        direction: [0., -1., 0.],
        max_parameter: 5.,
    };
    for selection in [Selection::First, Selection::Nearest] {
        assert_eq!(data.query(ray, 1, selection).unwrap().unwrap().triangle, 1);
        assert_eq!(
            data.query(ray, i32::MAX, selection).unwrap(),
            data.query(ray, -1, selection).unwrap()
        );
    }
    assert_eq!(
        data.query(ray, -1, Selection::First)
            .unwrap()
            .unwrap()
            .triangle,
        0
    );
    assert_eq!(
        data.query(ray, -1, Selection::Nearest)
            .unwrap()
            .unwrap()
            .triangle,
        1
    );
    assert!(
        data.segment(ray.origin, ray.direction, 0)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        data.segment(ray.origin, ray.direction, 1)
            .unwrap()
            .unwrap()
            .parameter,
        1.
    );
    for missing in [2, usize::MAX] {
        assert!(data.segment(ray.origin, ray.direction, missing).is_err());
    }
    let found = data
        .connected_segment(SegmentSearch {
            origin: ray.origin,
            displacement: ray.direction,
            start_triangle: 0,
            previous_triangle: None,
            max_depth: 1,
            max_face_tests: 2,
        })
        .unwrap();
    assert_eq!(found.hit.unwrap().triangle, 1);
    assert_eq!((found.face_tests, found.policy_checks), (2, 1));
}

#[test]
fn preparing_runtime_surface_data_rejects_bad_neighbor_and_group_indices() {
    use ge_assets::{
        edb::Entity,
        graph::{Diagnostics, Group, Triangle, TriangleGraph},
    };
    let mut graph = TriangleGraph {
        bounds: [[0.; 3], [2., 0., 2.]],
        entity: Entity {
            hash: 1,
            section: 0,
            debug: 0,
            address: 0,
            runtime_pointer_raw: 0,
            unknown_word: 0,
            entity_type: 0x607,
        },
        header_words: Vec::new(),
        vertices: vec![[0., 0., 0.], [2., 0., 0.], [0., 0., 2.]],
        triangles: vec![Triangle {
            normal: None,
            vertices: [0, 1, 2],
            vertex_high_bits_raw: [0; 3],
            attribute_raw: 0,
            neighbors: [None; 3],
            neighbor_high_bits_raw: [0; 3],
            center: None,
            weights: None,
            flags_raw: None,
        }],
        groups: vec![Group {
            tag_raw: 0,
            triangle_count: 1,
            first_triangle: 0,
            start_high_bits_raw: 0,
        }],
        diagnostics: Diagnostics::default(),
    };
    assert!(SurfaceData::new(&graph).is_ok());
    graph.triangles[0].neighbors[0] = Some(1);
    assert!(SurfaceData::new(&graph).is_err());
    graph.triangles[0].neighbors[0] = None;
    graph.groups[0].first_triangle = u32::MAX;
    assert!(SurfaceData::new(&graph).is_err());
}
