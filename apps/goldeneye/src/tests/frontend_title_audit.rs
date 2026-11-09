use super::*;
use ge_render::frontend::Vertex;

fn mesh() -> Mesh {
    Mesh {
        texture: 7,
        blend: Blend::Additive,
        rgb_scale: RgbScale::One,
        vertices: [[10., 10.], [30., 10.], [10., 30.]]
            .into_iter()
            .map(|position| Vertex {
                position,
                clip_w: 1.,
                uv: [0., 1.],
                color: [1., 0.5, 0.25, 1.],
            })
            .collect(),
    }
}

#[test]
fn logo_diagnostic_rejects_zero_alpha_offscreen_and_degenerate_triangles() {
    let mut value = mesh();
    assert!(visible_triangle(&value.vertices));
    for v in &mut value.vertices {
        v.color[3] = 0.;
    }
    assert!(!visible_triangle(&value.vertices));
    for v in &mut value.vertices {
        v.color[3] = 1.;
        v.position[0] += 700.;
    }
    assert!(!visible_triangle(&value.vertices));
    for v in &mut value.vertices {
        v.position = [10., 10.];
    }
    assert!(!visible_triangle(&value.vertices));
}

#[test]
fn color_hash_is_independent_of_pose_but_tracks_actual_color_changes() {
    let initial = mesh();
    let signature = BTreeSet::from([Signature::of(&initial)]);
    let a = inspect(&[initial.into()], &signature, &signature).unwrap();
    let mut moved = mesh();
    moved.vertices[0].position[0] += 1.;
    let b = inspect(&[moved.into()], &signature, &signature).unwrap();
    assert_ne!(a.layers, b.layers);
    assert_eq!(a.colors, b.colors);
    let mut colored = mesh();
    colored.vertices[0].color[2] = 0.5;
    let c = inspect(&[colored.into()], &signature, &signature).unwrap();
    assert_ne!(a.colors, c.colors);
    let mut material = mesh();
    material.rgb_scale = RgbScale::Two;
    let d = inspect(&[material.into()], &signature, &signature).unwrap();
    assert_ne!(a.layers, d.layers);
    assert_eq!(a.colors, d.colors);
    let mut invalid = mesh();
    invalid.vertices[0].uv[1] = f32::NAN;
    assert!(inspect(&[invalid.into()], &signature, &signature).is_err());
}

#[test]
fn frozen_rgb_cannot_animate_through_alpha_pose_or_mesh_population_changes() {
    let signature = BTreeSet::from([Signature::of(&mesh())]);
    let mut metric = ColorChanges::default();
    for tick in 0..8 {
        let mut first = mesh();
        let mut second = mesh();
        for vertex in &mut first.vertices {
            vertex.color[3] = tick as f32 / 8.;
            vertex.position[0] += tick as f32;
        }
        for vertex in &mut second.vertices {
            vertex.color[3] = 1. - tick as f32 / 8.;
        }
        let layers = if tick % 2 == 0 {
            vec![first.into()]
        } else {
            vec![second.into(), first.into()]
        };
        let sample = inspect(&layers, &signature, &signature).unwrap();
        assert_eq!(sample.colors.len(), 1);
        metric.include(&sample.colors);
        // Gaps between the authored bursts must not count as animation.
        metric.include(&BTreeSet::new());
    }
    assert_eq!(metric.changes, 0);
    assert_eq!(metric.distinct.len(), 1);
    assert!(!metric.animated());

    let mut changed = mesh();
    changed.vertices[0].color[2] = 0.5;
    let sample = inspect(&[changed.into()], &signature, &signature).unwrap();
    metric.include(&sample.colors);
    assert_eq!(metric.changes, 1);
    assert_eq!(metric.distinct.len(), 2);
    assert!(metric.animated());
}

#[test]
fn static_different_patterns_reordered_or_removed_do_not_become_color_changes() {
    let signature = BTreeSet::from([Signature::of(&mesh())]);
    let second = || {
        let mut value = mesh();
        value.vertices[1].color[0] = 0.75;
        value
    };
    let mut metric = ColorChanges::default();
    for layers in [
        vec![mesh().into(), second().into()],
        vec![second().into(), mesh().into()],
        vec![mesh().into()],
        vec![second().into(), second().into(), mesh().into()],
    ] {
        let sample = inspect(&layers, &signature, &signature).unwrap();
        metric.include(&sample.colors);
    }
    assert_eq!(metric.distinct.len(), 2);
    assert_eq!(metric.changes, 0);
    assert!(!metric.animated());
}
