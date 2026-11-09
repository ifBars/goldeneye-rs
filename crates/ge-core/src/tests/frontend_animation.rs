use super::*;
use std::collections::BTreeMap;

fn scalar(frame: f32, value: f32) -> Key<1> {
    Key {
        frame,
        value: [value],
    }
}
fn vector(frame: f32, value: [f32; 3]) -> Key<3> {
    Key { frame, value }
}

#[test]
fn forward_cursor_extrapolates_and_holds_last_key() {
    let keys = [
        vector(2., [2., 4., 6.]),
        vector(4., [4., 8., 12.]),
        vector(5., [9., 7., 3.]),
    ];
    let mut cursor = 0;
    assert_eq!(
        sample_vector(&keys, 1., &mut cursor).unwrap(),
        ([1., 2., 3.], false)
    );
    assert_eq!(
        sample_vector(&keys, 4., &mut cursor).unwrap(),
        ([4., 8., 12.], false)
    );
    assert_eq!(cursor, 1);
    assert_eq!(
        sample_vector(&keys, 100., &mut cursor).unwrap(),
        ([9., 7., 3.], true)
    );
    assert_eq!(cursor, 2);
}

#[test]
fn exact_duplicate_time_uses_last_matching_key() {
    let keys = [scalar(0., 1.), scalar(0., 5.), scalar(2., 9.)];
    let mut cursor = 0;
    assert_eq!(sample_scalar(&keys, 0., &mut cursor).unwrap(), (5., false));
    assert_eq!(cursor, 1);
}

#[test]
fn invalid_tracks_and_nonfinite_quaternions_are_errors() {
    assert!(sample_scalar(&[], 0., &mut 0).is_err());
    assert!(sample_vector(&[vector(0., [f32::NAN, 0., 0.])], 0., &mut 0).is_err());
    assert!(sample_scalar(&[scalar(0., 0.)], f32::INFINITY, &mut 0).is_err());
    assert!(quaternion_slerp([f32::NAN; 4], [0.; 4], 0.5).is_err());
}

#[test]
fn quaternion_shortest_path_and_nonunit_linear_branch_are_retained() {
    let q = [0., 0., 0., 1.];
    let result = quaternion_slerp(q, [0., 0., 0., -1.], 0.25).unwrap();
    assert_eq!(result, [0., 0., 0., -1.]);
    // Native does not normalize this near-identical/nonunit branch.
    assert_eq!(
        quaternion_slerp([0., 0., 0., 2.], [0., 0., 0., 2.], 0.5).unwrap()[3],
        2.
    );
    let result = quaternion_slerp(q, [0., 0., 1., 0.], 0.5).unwrap();
    assert!((result[2] - std::f32::consts::FRAC_1_SQRT_2).abs() < 0.00001);
    assert!((result[3] - std::f32::consts::FRAC_1_SQRT_2).abs() < 0.00001);
}

#[test]
fn pose_uses_rotation_times_scale_then_translation() {
    let pose = Pose {
        rotation: [
            0.,
            0.,
            std::f32::consts::FRAC_1_SQRT_2,
            std::f32::consts::FRAC_1_SQRT_2,
        ],
        scale: [2., 3., 4., 0.],
        translation: [10., 20., 30.],
        ..Pose::default()
    };
    let m = pose.matrix().unwrap();
    assert!(m[0].abs() < 0.000001);
    assert!((m[1] - 2.).abs() < 0.000001);
    assert!((m[4] + 3.).abs() < 0.000001);
    assert_eq!(&m[12..], &[10., 20., 30., 1.]);
    assert_eq!(
        Pose::default().matrix().unwrap(),
        [
            1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.
        ]
    );
}

#[derive(Default)]
struct Fixture {
    scripts: BTreeMap<u32, Script>,
    tracks: BTreeMap<(u32, u16), Tracks>,
    initial: BTreeMap<u32, Pose>,
    reads: Vec<(u32, u16)>,
}
impl Source for Fixture {
    fn script(&mut self, r: u32) -> Result<Script> {
        self.scripts
            .get(&r)
            .cloned()
            .context("missing authored script")
    }
    fn tracks(&mut self, r: u32, s: u16) -> Result<Tracks> {
        self.reads.push((r, s));
        Ok(self.tracks.get(&(r, s)).cloned().unwrap_or_default())
    }
    fn initial_pose(&mut self, _: u32, i: &Instance) -> Result<Pose> {
        Ok(self.initial.get(&i.reference).copied().unwrap_or_default())
    }
}
fn create(
    start: i16,
    slot: u8,
    parent_slot: Option<u8>,
    reference: u32,
    kind: InstanceKind,
) -> Command {
    Command {
        start,
        duration: 1,
        kind: CommandKind::Instance(Instance {
            kind,
            slot,
            parent_slot,
            selector: u16::from(slot),
            reference,
            word_0c_raw: 0x15000000 + u32::from(slot),
            file_reference: u32::MAX,
        }),
    }
}
fn fixture(commands: Vec<Command>) -> Fixture {
    let mut f = Fixture::default();
    f.scripts.insert(
        1,
        Script {
            fps: 60.,
            slots: 8,
            commands,
        },
    );
    f
}
fn budget() -> Budget {
    Budget {
        steps: 1000,
        depth: 10,
    }
}
fn run(runtime: &mut Runtime, f: &mut Fixture, frame: f32) -> Vec<Event> {
    let mut events = Vec::new();
    runtime
        .advance_to(
            frame,
            f,
            &mut |e| {
                events.push(e.clone());
                Ok(Control::Continue)
            },
            budget(),
        )
        .unwrap();
    events
}

#[test]
fn root_removal_and_same_frame_creation_preserve_command_order() {
    let mut f = fixture(vec![
        create(0, 1, None, 10, InstanceKind::Entity),
        Command {
            start: 37,
            duration: 30,
            kind: CommandKind::Remove(vec![1]),
        },
        create(37, 7, None, 11, InstanceKind::Entity),
    ]);
    let mut r = Runtime::new(1, &mut f).unwrap();
    run(&mut r, &mut f, 36.999);
    assert_eq!(r.instances().unwrap()[0].reference, 10); // duration1 did not expire.
    let events = run(&mut r, &mut f, 37.);
    assert!(matches!(
        events[0].kind,
        EventKind::Remove {
            slot: 1,
            reference: 10
        }
    ));
    assert!(matches!(
        events[1].kind,
        EventKind::Create(Instance { reference: 11, .. })
    ));
    assert_eq!(r.instances().unwrap()[0].key, vec![7]);
}

#[test]
fn nested_time_is_seconds_converted_by_child_fps() {
    let mut f = fixture(vec![create(0, 0, None, 2, InstanceKind::Script)]);
    f.scripts.insert(
        2,
        Script {
            fps: 30.,
            slots: 1,
            commands: vec![create(0, 0, None, 20, InstanceKind::Entity)],
        },
    );
    f.tracks.insert(
        (2, 0),
        Tracks {
            translation: Some(vec![vector(0., [0.; 3]), vector(30., [30., 0., 0.])]),
            ..Tracks::default()
        },
    );
    let mut r = Runtime::new(1, &mut f).unwrap();
    run(&mut r, &mut f, 30.);
    let instances = r.instances().unwrap();
    assert_eq!(instances[1].key, vec![0, 0]);
    assert_eq!(instances[1].parent_key, Some(vec![0]));
    assert_eq!(instances[1].pose.translation, [15., 0., 0.]);
}

#[test]
fn composed_tree_matrix_color_and_attachment_order_are_preserved() {
    let mut f = fixture(vec![
        create(0, 2, None, 10, InstanceKind::Entity),
        create(0, 1, None, 11, InstanceKind::Entity),
        create(0, 3, Some(2), 12, InstanceKind::Entity),
    ]);
    f.initial.insert(
        10,
        Pose {
            translation: [10., 0., 0.],
            rotation: [
                0.,
                0.,
                std::f32::consts::FRAC_1_SQRT_2,
                std::f32::consts::FRAC_1_SQRT_2,
            ],
            color: [0.5, 1., 1., 0.5],
            color_flags_raw: 6,
            ..Pose::default()
        },
    );
    f.initial.insert(
        12,
        Pose {
            translation: [2., 0., 0.],
            color: [1., 0.25, 1., 0.5],
            color_flags_raw: 6,
            ..Pose::default()
        },
    );
    let mut r = Runtime::new(1, &mut f).unwrap();
    run(&mut r, &mut f, 0.);
    let v = r.instances().unwrap();
    assert_eq!(
        v.iter().map(|v| v.reference).collect::<Vec<_>>(),
        vec![10, 12, 11]
    );
    assert!((v[1].world_matrix[12] - 10.).abs() < 0.000001);
    assert!((v[1].world_matrix[13] - 2.).abs() < 0.000001);
    assert_eq!(v[1].world_color, [0.5, 0.25, 1., 0.25]);
    assert_eq!(v[1].parent_key, Some(vec![2]));
}

#[test]
fn tracks_write_correct_lanes_and_frame_mapping() {
    let mut f = fixture(vec![create(0, 0, None, 10, InstanceKind::Entity)]);
    f.tracks.insert(
        (1, 0),
        Tracks {
            translation: Some(vec![vector(0., [1., 2., 3.])]),
            scale: Some(vec![vector(0., [4., 5., 6.])]),
            alpha: Some(vec![scalar(0., 0.5)]),
            frame_selector: Some(FrameSelector {
                keys: vec![scalar(0., 0.), scalar(10., 2.)],
                indices: vec![17, 16, 15],
                playback_mode: 0,
            }),
            ..Tracks::default()
        },
    );
    let mut r = Runtime::new(1, &mut f).unwrap();
    run(&mut r, &mut f, 2.5);
    let v = r.instances().unwrap();
    assert_eq!(v[0].pose.translation, [1., 2., 3.]);
    assert_eq!(v[0].pose.scale, [4., 5., 6., 0.]);
    assert_eq!(v[0].world_color, [1., 1., 1., 0.5]);
    assert_eq!(
        v[0].frame_selection,
        Some(FrameSelection {
            current: 17,
            next: 16,
            blend_raw: 128
        })
    );
}

#[test]
fn terminal_holds_previous_pose_and_uncreated_tracks_stay_lazy() {
    let mut f = fixture(vec![
        create(0, 0, None, 10, InstanceKind::Entity),
        Command {
            start: 10,
            duration: 0,
            kind: CommandKind::Terminal,
        },
        create(20, 1, None, 11, InstanceKind::Entity),
    ]);
    f.tracks.insert(
        (1, 0),
        Tracks {
            translation: Some(vec![vector(0., [0.; 3]), vector(16., [16., 0., 0.])]),
            ..Tracks::default()
        },
    );
    let mut r = Runtime::new(1, &mut f).unwrap();
    run(&mut r, &mut f, 5.);
    run(&mut r, &mut f, 10.);
    run(&mut r, &mut f, 100.);
    assert!(r.terminal());
    assert_eq!(r.instances().unwrap()[0].pose.translation, [5., 0., 0.]);
    assert_eq!(f.reads, vec![(1, 0)]);
}

#[test]
fn external_effects_are_callback_owned_and_can_stop_execution() {
    let mut f = fixture(vec![
        Command {
            start: 0,
            duration: 0,
            kind: CommandKind::External {
                opcode: 0x0b,
                payload: vec![4, 5],
            },
        },
        create(0, 0, None, 10, InstanceKind::Entity),
    ]);
    let mut r = Runtime::new(1, &mut f).unwrap();
    r.advance_to(
        0.,
        &mut f,
        &mut |event| {
            assert!(
                matches!(&event.kind,EventKind::External {opcode:0x0b,payload} if payload==&[4,5])
            );
            Ok(Control::Stop)
        },
        budget(),
    )
    .unwrap();
    assert!(r.instances().unwrap().is_empty());
    assert!(f.reads.is_empty());
    run(&mut r, &mut f, 0.);
    assert_eq!(r.instances().unwrap().len(), 1);
}

#[test]
fn budgets_rewind_and_invalid_parents_are_errors() {
    let mut f = fixture(vec![create(0, 0, None, 10, InstanceKind::Entity)]);
    let mut r = Runtime::new(1, &mut f).unwrap();
    assert!(
        r.advance_to(
            0.,
            &mut f,
            &mut |_| Ok(Control::Continue),
            Budget {
                steps: 0,
                depth: 10
            }
        )
        .is_err()
    );
    run(&mut r, &mut f, 5.);
    assert!(
        r.advance_to(4., &mut f, &mut |_| Ok(Control::Continue), budget())
            .is_err()
    );
    let mut f = fixture(vec![create(0, 0, Some(0), 10, InstanceKind::Entity)]);
    let mut r = Runtime::new(1, &mut f).unwrap();
    assert!(
        r.advance_to(0., &mut f, &mut |_| Ok(Control::Continue), budget())
            .is_err()
    );
}

#[test]
fn tagged_child_clock_survives_root_terminal_and_reconstructs_on_wrap() {
    let mut f = fixture(vec![
        create(0, 0, None, 2, InstanceKind::Script),
        Command {
            start: 2,
            duration: 0,
            kind: CommandKind::Terminal,
        },
    ]);
    f.scripts.insert(
        2,
        Script {
            fps: 30.,
            slots: 1,
            commands: vec![create(0, 0, None, 20, InstanceKind::Entity)],
        },
    );
    f.tracks.insert(
        (2, 0),
        Tracks {
            translation: Some(vec![vector(0., [0.; 3]), vector(30., [30., 0., 0.])]),
            ..Tracks::default()
        },
    );
    let mut r = Runtime::new(1, &mut f).unwrap();
    run(&mut r, &mut f, 1.);
    run(&mut r, &mut f, 2.);
    assert!(r.terminal());
    let mut callback = |_: &Event| Ok(Control::Continue);
    assert!(
        r.seek_tag_seconds(0x15000000, 0.5, &mut f, &mut callback, budget())
            .unwrap()
    );
    assert_eq!(r.instances().unwrap()[1].pose.translation, [15., 0., 0.]);
    assert!(
        r.seek_tag_seconds(0x15000000, 0., &mut f, &mut callback, budget())
            .unwrap()
    );
    assert_eq!(r.instances().unwrap()[1].pose.translation, [0.; 3]);
    assert!(
        !r.seek_tag_seconds(123, 0., &mut f, &mut callback, budget())
            .unwrap()
    );
    assert_eq!(r.instances().unwrap()[0].word_0c_raw, 0x15000000);
}

#[test]
fn external_stop_updates_existing_pose_without_consuming_the_command() {
    let mut f = fixture(vec![
        create(0, 0, None, 10, InstanceKind::Entity),
        Command {
            start: 2,
            duration: 0,
            kind: CommandKind::External {
                opcode: 0x0b,
                payload: vec![],
            },
        },
        create(2, 1, None, 11, InstanceKind::Entity),
    ]);
    f.tracks.insert(
        (1, 0),
        Tracks {
            translation: Some(vec![vector(0., [0.; 3]), vector(8., [8., 0., 0.])]),
            ..Tracks::default()
        },
    );
    let mut r = Runtime::new(1, &mut f).unwrap();
    run(&mut r, &mut f, 0.);
    let mut stopped = 0;
    r.advance_to(
        2.,
        &mut f,
        &mut |event| {
            if matches!(event.kind, EventKind::External { .. }) {
                stopped += 1;
                Ok(Control::Stop)
            } else {
                Ok(Control::Continue)
            }
        },
        budget(),
    )
    .unwrap();
    assert_eq!(stopped, 1);
    assert_eq!(r.instances().unwrap().len(), 1);
    assert_eq!(r.instances().unwrap()[0].pose.translation, [2., 0., 0.]);
    let events = run(&mut r, &mut f, 2.);
    assert!(matches!(
        events[0].kind,
        EventKind::External { opcode: 0x0b, .. }
    ));
    assert_eq!(r.instances().unwrap().len(), 2);
}

#[test]
fn color_frame_blend_uses_separate_floors_and_a_256_denominator() {
    assert_eq!(blend_color_frames([255; 4], [255; 4], 128), [254; 4]);
    let first = [255, 0, 64, 200];
    let second = [0, 255, 128, 32];
    assert_eq!(blend_color_frames(first, second, 0), first);
    // Byte255 is255/256, not the exact second-frame endpoint.
    assert_eq!(blend_color_frames(first, second, 255), [0, 254, 127, 31]);
    assert_eq!(blend_color_frames(first, second, 128), [127, 127, 96, 116]);
}

fn remove_at(start: i16, slot: u8) -> Command {
    Command {
        start,
        duration: 578,
        kind: CommandKind::Remove(vec![slot]),
    }
}

fn glint_script() -> Script {
    let mut commands = Vec::new();
    for start in [0, 270, 330] {
        commands.push(create(start, 0, None, 20, InstanceKind::Entity));
        commands.push(create(start + 2, 1, None, 20, InstanceKind::Entity));
        commands.push(remove_at(start + 31, 0));
        commands.push(remove_at(start + 33, 1));
    }
    commands.push(Command {
        start: 570,
        duration: 578,
        kind: CommandKind::Loop {
            variable: 0,
            count: -1,
            target_frame: 0,
        },
    });
    commands.push(Command {
        start: 578,
        duration: 578,
        kind: CommandKind::Terminal,
    });
    Script {
        fps: 30.,
        slots: 2,
        commands,
    }
}

#[test]
fn title_glint_loop_restarts_at570_before578_wrapper_boundary() {
    let mut f = fixture(vec![]);
    f.scripts.insert(2, glint_script());
    f.tracks.insert(
        (2, 0),
        Tracks {
            frame_selector: Some(FrameSelector {
                keys: vec![scalar(0., 0.), scalar(30., 17.)],
                indices: (0..18).rev().collect(),
                playback_mode: 0,
            }),
            ..Tracks::default()
        },
    );
    let mut r = Runtime::new(2, &mut f).unwrap();
    run(&mut r, &mut f, 569.999);
    assert!(r.instances().unwrap().is_empty());
    let events = run(&mut r, &mut f, 570.);
    assert!(matches!(
        events[0].kind,
        EventKind::Loop {
            variable: 0,
            counter: -1,
            target_frame: 0,
            taken: true
        }
    ));
    assert_eq!(r.frame(), 0.); // Overshoot is discarded, never carried into frame0.
    assert!(!r.terminal());
    let live = r.instances().unwrap();
    assert_eq!(live.len(), 1);
    assert_eq!(
        live[0].frame_selection,
        Some(FrameSelection {
            current: 17,
            next: 16,
            blend_raw: 0
        })
    );
    let events = run(&mut r, &mut f, 577.);
    assert!(events.iter().any(|e| matches!(
        e.kind,
        EventKind::Loop {
            counter: -1,
            taken: true,
            ..
        }
    )));
    assert_eq!(r.frame(), 0.);
    assert_eq!(r.loop_counters.get(&0), Some(&-1));
}

#[test]
fn positive_loop_counter_persists_and_zero_decrement_falls_through() {
    let mut f = fixture(vec![
        Command {
            start: 1,
            duration: 2,
            kind: CommandKind::Loop {
                variable: 3,
                count: 2,
                target_frame: 0,
            },
        },
        Command {
            start: 2,
            duration: 0,
            kind: CommandKind::Terminal,
        },
    ]);
    let mut r = Runtime::new(1, &mut f).unwrap();
    for (expected_counter, taken) in [(2, true), (1, true), (0, false)] {
        let events = run(&mut r, &mut f, 1.);
        assert!(
            matches!(events[0].kind,EventKind::Loop {variable:3,counter,target_frame:0,taken:jump} if counter==expected_counter && jump==taken)
        );
        assert_eq!(r.loop_counters.get(&3), Some(&expected_counter));
        assert_eq!(r.frame(), if taken { 0. } else { 1. });
    }
    run(&mut r, &mut f, 2.);
    assert!(r.terminal());
}

#[test]
fn loop_zero_cycle_is_budgeted_and_unimplemented_rebuilds_error() {
    let mut f = fixture(vec![Command {
        start: 0,
        duration: 0,
        kind: CommandKind::Loop {
            variable: 0,
            count: 0,
            target_frame: 0,
        },
    }]);
    let mut r = Runtime::new(1, &mut f).unwrap();
    let error = r
        .advance_to(
            0.,
            &mut f,
            &mut |_| Ok(Control::Continue),
            Budget { steps: 5, depth: 2 },
        )
        .unwrap_err();
    assert!(error.to_string().contains("step budget"));
    let mut f = fixture(vec![Command {
        start: 1,
        duration: 0,
        kind: CommandKind::Loop {
            variable: 0,
            count: -1,
            target_frame: 1,
        },
    }]);
    let mut r = Runtime::new(1, &mut f).unwrap();
    assert!(
        r.advance_to(1., &mut f, &mut |_| Ok(Control::Continue), budget())
            .unwrap_err()
            .to_string()
            .contains("rebuild target")
    );
    let mut f = fixture(vec![
        create(0, 0, None, 20, InstanceKind::Entity),
        Command {
            start: 1,
            duration: 0,
            kind: CommandKind::Loop {
                variable: 0,
                count: -1,
                target_frame: 0,
            },
        },
    ]);
    let mut r = Runtime::new(1, &mut f).unwrap();
    assert!(
        r.advance_to(1., &mut f, &mut |_| Ok(Control::Continue), budget())
            .unwrap_err()
            .to_string()
            .contains("live instances")
    );
}

#[test]
fn tagged_glint_loop_and_header_clock_wrap_are_independent_after_root_terminal() {
    let mut f = fixture(vec![
        create(0, 0, None, 2, InstanceKind::Script),
        Command {
            start: 56,
            duration: 0,
            kind: CommandKind::Terminal,
        },
    ]);
    f.scripts.insert(2, glint_script());
    let mut r = Runtime::new(1, &mut f).unwrap();
    run(&mut r, &mut f, 0.);
    run(&mut r, &mut f, 56.);
    assert!(r.terminal());
    let mut events = Vec::new();
    let mut callback = |event: &Event| {
        events.push(event.clone());
        Ok(Control::Continue)
    };
    r.seek_tag_seconds(0x15000000, 19., &mut f, &mut callback, budget())
        .unwrap();
    assert_eq!(r.instances().unwrap().len(), 2); // Root script instance plus first glint.
    r.seek_tag_seconds(0x15000000, 19.25, &mut f, &mut callback, budget())
        .unwrap();
    // The native UI owns this clock: >=578/30 resets to0, not remainder.
    // The wrapper supplies0 at its578/30 boundary; this test exercises that
    // request independently of the loop's earlier570-frame reset.
    let ui_seconds = 0.;
    r.seek_tag_seconds(0x15000000, ui_seconds, &mut f, &mut callback, budget())
        .unwrap();
    assert_eq!(r.instances().unwrap().len(), 2);
    assert_eq!(r.instances().unwrap()[1].key, vec![0, 0]);
    assert!(events.iter().any(|event| event.path == [0]
        && matches!(
            event.kind,
            EventKind::Loop {
                counter: -1,
                taken: true,
                ..
            }
        )));
    assert!(r.terminal());
}

#[test]
fn already_zero_loop_repeats_current_command_without_replaying_earlier_events() {
    let mut f = fixture(vec![
        Command {
            start: 0,
            duration: 0,
            kind: CommandKind::External {
                opcode: 0x0b,
                payload: vec![7],
            },
        },
        Command {
            start: 0,
            duration: 0,
            kind: CommandKind::Loop {
                variable: 0,
                count: 2,
                target_frame: 0,
            },
        },
        create(1, 0, None, 10, InstanceKind::Entity),
    ]);
    let mut r = Runtime::new(1, &mut f).unwrap();
    let events = run(&mut r, &mut f, 0.);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e.kind, EventKind::External { .. }))
            .count(),
        1
    );
    let loops: Vec<_> = events
        .iter()
        .filter_map(|e| match e.kind {
            EventKind::Loop { counter, taken, .. } => Some((counter, taken)),
            _ => None,
        })
        .collect();
    assert_eq!(loops, vec![(2, true), (1, true), (0, false)]);
    assert_eq!(r.frame(), 0.);
    assert!(r.instances().unwrap().is_empty());
    run(&mut r, &mut f, 1.);
    assert_eq!(r.instances().unwrap().len(), 1);
}

#[test]
fn resumed_parent_reconstructs_child_advanced_by_an_independent_tag_clock() {
    let mut f = fixture(vec![create(0, 0, None, 2, InstanceKind::Script)]);
    f.scripts.insert(2, glint_script());
    f.tracks.insert(
        (1, 0),
        Tracks {
            translation: Some(vec![vector(0., [7., 8., 9.])]),
            ..Tracks::default()
        },
    );
    f.tracks.insert(
        (2, 0),
        Tracks {
            alpha: Some(vec![scalar(0., 0.), scalar(30., 1.)]),
            ..Tracks::default()
        },
    );
    let mut r = Runtime::new(1, &mut f).unwrap();
    run(&mut r, &mut f, 29.); // Parent held at29; child is at14.5frames.
    let parent_pose = r.instances().unwrap()[0].pose;
    r.seek_tag_seconds(
        0x15000000,
        2.,
        &mut f,
        &mut |_| Ok(Control::Continue),
        budget(),
    )
    .unwrap();
    assert_eq!(r.frame(), 29.);
    assert_eq!(r.instances().unwrap().len(), 1); // The child's pulse was removed byframe33.
    let events = run(&mut r, &mut f, 30.); // Parent nowrequests childframe15, behind60.
    let instances = r.instances().unwrap();
    assert_eq!(instances[0].pose, parent_pose);
    assert_eq!(instances.len(), 3);
    assert_eq!(instances[1].key, vec![0, 0]);
    assert_eq!(instances[1].pose.color[3], 0.5);
    assert_eq!(
        events
            .iter()
            .filter(|e| e.path == [0] && matches!(e.kind, EventKind::Create(_)))
            .count(),
        2
    );
    assert_eq!(r.frame(), 30.);
    let events = run(&mut r, &mut f, 30.); // Same target does not reconstruct a second time.
    assert!(
        !events
            .iter()
            .any(|e| matches!(e.kind, EventKind::Create(_)))
    );
    assert_eq!(r.instances().unwrap()[1].pose.color[3], 0.5);
}
