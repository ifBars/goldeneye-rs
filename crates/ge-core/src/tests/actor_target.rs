use super::*;
struct Fixture {
    triangles: Vec<SurfaceTriangle>,
    neighbors: Vec<[Option<usize>; 3]>,
    flags: Vec<Option<u32>>,
}
impl Fixture {
    fn new() -> Self {
        Self {
            triangles: vec![
                SurfaceTriangle {
                    index: 0,
                    vertices: [[0.; 3], [10., 0., 0.], [0., 0., 10.]],
                },
                SurfaceTriangle {
                    index: 1,
                    vertices: [[10., 0., 0.], [10., 0., 10.], [0., 0., 10.]],
                },
            ],
            neighbors: vec![[Some(1), None, None], [Some(0), None, None]],
            flags: vec![None; 2],
        }
    }
    fn mesh(&self) -> ActorTargetMesh<'_> {
        ActorTargetMesh {
            triangles: &self.triangles,
            neighbors: &self.neighbors,
            flags: &self.flags,
        }
    }
    fn request(&self) -> ActorTargetRequest {
        ActorTargetRequest {
            target: [6., 0., 6.],
            active_position: [1., 0., 1.],
            owner_frame: frame([0.; 3]),
            max_face_tests: 100,
        }
    }
}
fn frame(translation: [f32; 3]) -> NavigationFrame {
    NavigationFrame { translation }
}
#[test]
fn separate_owner_and_later_binding_frames_preserve_unsnapped_raw_policy_positions() {
    let f = Fixture::new();
    let mut request = f.request();
    request.target = [16., 10., 26.];
    request.owner_frame = frame([10., 10., 20.]);
    request.active_position = [31., 40., 51.];
    let mut calls = 0;
    let r = prepare_actor_target(request, Some(f.mesh()), || {
        calls += 1;
        Ok(Some(frame([30., 40., 50.])))
    })
    .unwrap();
    assert_eq!(r.decision, ActorTargetDecision::Ready);
    assert_eq!(calls, 1);
    assert_eq!(r.local_owner_target, [6., 0., 6.]);
    assert_eq!(r.target_hit.unwrap().triangle, 1);
    let p = r.prepared.unwrap();
    assert_eq!(p.start_triangle, 0);
    assert_eq!(p.origin, [1., 0., 1.]);
    assert_eq!(p.point, [-14., -30., -24.]);
    let segment = r.active_segment.unwrap();
    assert_eq!(segment.origin, [1., 1., 1.]);
    assert_eq!(segment.displacement, [0., -2., 0.]);
    assert_eq!(segment.max_depth, 5);
}
#[test]
fn missing_binding_and_failed_initial_ray_do_not_resolve_later_state() {
    let f = Fixture::new();
    let mut request = f.request();
    request.active_position = [f32::NAN; 3];
    let r = prepare_actor_target(request, None, || panic!("unused binding lookup")).unwrap();
    assert_eq!(r.decision, ActorTargetDecision::MissingBinding);
    request.target = [100., 2., 100.];
    let r =
        prepare_actor_target(request, Some(f.mesh()), || panic!("unused binding lookup")).unwrap();
    assert_eq!(r.decision, ActorTargetDecision::NoTargetFace); // Ray miss precedes height rejection.
}
#[test]
fn height_uses_original_local_target_and_rejects_both_equal_limits_before_binding_reload() {
    for height in [-1., 1.] {
        let mut f = Fixture::new();
        for t in &mut f.triangles {
            for p in &mut t.vertices {
                p[1] = height;
            }
        }
        let mut request = f.request();
        request.target[1] = height;
        request.active_position = [f32::NAN; 3];
        let r = prepare_actor_target(request, Some(f.mesh()), || {
            panic!("height gate must precede binding reload")
        })
        .unwrap();
        assert!(r.target_hit.is_some());
        assert_eq!(r.decision, ActorTargetDecision::TargetHeight);
    }
}
#[test]
fn initial_storage_order_and_strict_ray_endpoints_differ_from_nearest_or_ordinary_binding() {
    let mut f = Fixture::new();
    f.triangles[1].vertices = f.triangles[0].vertices;
    for p in &mut f.triangles[0].vertices {
        p[1] = -0.1;
    }
    for p in &mut f.triangles[1].vertices {
        p[1] = 0.2;
    }
    let mut request = f.request();
    request.target = [1., 0.1, 1.];
    request.active_position = [1., -0.1, 1.];
    let r = prepare_actor_target(request, Some(f.mesh()), || Ok(Some(frame([0.; 3])))).unwrap();
    assert_eq!(r.target_hit.unwrap().triangle, 0);
    assert_eq!(r.decision, ActorTargetDecision::Ready);
    for plane_y in [-0.25, 1.] {
        for t in &mut f.triangles {
            for p in &mut t.vertices {
                p[1] = plane_y;
            }
        }
        request.target[1] = 0.;
        let r = prepare_actor_target(request, Some(f.mesh()), || {
            panic!("strict ray endpoint must miss")
        })
        .unwrap();
        assert_eq!(r.decision, ActorTargetDecision::NoTargetFace);
    }
}
#[test]
fn binding_reload_is_lazy_and_connected_lookup_uses_manager_flags() {
    let mut f = Fixture::new();
    let request = f.request();
    let lost = prepare_actor_target(
        ActorTargetRequest {
            active_position: [f32::NAN; 3],
            ..request
        },
        Some(f.mesh()),
        || Ok(None),
    )
    .unwrap();
    assert_eq!(lost.decision, ActorTargetDecision::BindingLost);
    f.flags[0] = Some(2);
    let blocked =
        prepare_actor_target(request, Some(f.mesh()), || Ok(Some(frame([0.; 3])))).unwrap();
    assert_eq!(blocked.decision, ActorTargetDecision::NoActiveFace);
    f.flags[1] = Some(2);
    assert_eq!(
        prepare_actor_target(request, Some(f.mesh()), || Ok(Some(frame([0.; 3]))))
            .unwrap()
            .decision,
        ActorTargetDecision::Ready
    );
    f.flags = vec![];
    let same = ActorTargetRequest {
        active_position: request.target,
        ..request
    };
    assert_eq!(
        prepare_actor_target(same, Some(f.mesh()), || Ok(Some(frame([0.; 3]))))
            .unwrap()
            .decision,
        ActorTargetDecision::Ready
    ); // No policy call on current-face hit.
    assert!(prepare_actor_target(request, Some(f.mesh()), || Ok(Some(frame([0.; 3])))).is_err());
}
#[test]
fn checked_policy_frame_callback_and_host_budgets_propagate_errors() {
    let f = Fixture::new();
    let request = f.request();
    assert!(
        prepare_actor_target(request, Some(f.mesh()), || anyhow::bail!(
            "binding source failed"
        ))
        .is_err()
    );
    let limited = ActorTargetRequest {
        max_face_tests: 1,
        ..request
    };
    let error = prepare_actor_target(limited, Some(f.mesh()), || Ok(Some(frame([0.; 3]))))
        .err()
        .unwrap();
    assert!(error.to_string().contains("budget exhausted"));
    let r = prepare_actor_target(
        ActorTargetRequest {
            max_face_tests: 2,
            ..request
        },
        Some(f.mesh()),
        || Ok(Some(frame([0.; 3]))),
    )
    .unwrap();
    assert_eq!(r.decision, ActorTargetDecision::Ready);
}

#[test]
fn target_instance_subtraction_is_deferred_until_an_active_face_is_found() {
    let f = Fixture::new();
    let request = ActorTargetRequest {
        target: [f32::MAX, 0., 0.],
        active_position: [-f32::MAX, 0., 100.],
        owner_frame: frame([f32::MAX, 0., 0.]),
        max_face_tests: 100,
    };
    let r = prepare_actor_target(request, Some(f.mesh()), || {
        Ok(Some(frame([-f32::MAX, 0., 0.])))
    })
    .unwrap();
    assert_eq!(r.decision, ActorTargetDecision::NoActiveFace);
}
