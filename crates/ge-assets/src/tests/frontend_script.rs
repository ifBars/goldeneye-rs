use super::*;

#[test]
fn live_selection_reads_only_reached_common_fields_and_each_requested_clip_scalar() {
    let mut data = fixture();
    word(&mut data, 0x84, u32::MAX);
    let script = read_at(&data[..0xca], 0x80).unwrap();
    assert_eq!(script.duration_raw().unwrap(), u32::MAX);
    // Truncated instance payload is irrelevant to selection. Only +0,+1,+4..6.
    let entry = script.selection_entry(0).unwrap();
    assert_eq!((entry.opcode, entry.next, entry.start), (4, 24, -2));
    assert!(script.command_at(0xc4).is_err());
    assert!(script.selection_entry(6).is_err());
    // A first normal reference is readable before counts/record payloads.
    word(&mut data, 0xe4, 0x8300_0021);
    data[0xd4..0xd8].copy_from_slice(&[1, 0xff, 255, 2]);
    let script = read_at(&data[..0xe8], 0x80).unwrap();
    assert_eq!(script.clip_counts(0).unwrap(), [1, 2]);
    assert_eq!(script.clip_reference(0, false, 0, 1).unwrap(), 0x8300_0021);
    assert!(script.clip_blend(0, 0).is_err());
    assert!(script.clip_file(0, true, 0, 1).is_err());
}

fn u16_at(data: &mut [u8], at: usize, value: u16) {
    data[at..at + 2].copy_from_slice(&value.to_be_bytes());
}
fn word(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn relative(data: &mut [u8], field: usize, target: usize) {
    word(data, field, (target as i32 - field as i32) as u32);
}
fn fixture() -> Vec<u8> {
    let mut data = vec![0; 0x300];
    word(&mut data, 0x80, 0x300);
    data[0x88] = 3;
    word(&mut data, 0x8c, 30f32.to_bits());
    // Deliberately opaque unused bound, not eagerly read as a finite float.
    word(&mut data, 0x90, f32::NAN.to_bits());
    relative(&mut data, 0xb4, 0x120);
    // Instance, unknown record, terminal. Signed timing and parent sentinel.
    data[0xc4..0xcc].copy_from_slice(&[4, 24, 0, 0xfe, 0xff, 0xfe, 0, 37]);
    u16_at(&mut data, 0xcc, 2);
    data[0xce] = 1;
    data[0xcf] = 0xff;
    word(&mut data, 0xd0, 0x1500_0007);
    word(&mut data, 0xd4, u32::MAX);
    word(&mut data, 0xd8, 0x8400_0002);
    data[0xdc..0xe4].copy_from_slice(&[0x7e, 12, 24, 0xff, 0, 4, 0, 1]);
    word(&mut data, 0xe4, 0xdead_beef);
    data[0xe8..0xf0].copy_from_slice(&[0x12, 0, 12, 0xff, 0, 56, 0, 0]);
    // Only selected pointer #2 is valid; #0 is malformed and remains unread.
    word(&mut data, 0x120, 0x7fff_ffff);
    relative(&mut data, 0x128, 0x160);
    word(&mut data, 0x160, 0x1234);
    word(&mut data, 0x164, 0x94); // scale, translation, alpha, packed in bit order.
    word(&mut data, 0x168, 0x94);
    u16_at(&mut data, 0x16c, 2);
    u16_at(&mut data, 0x16e, 0xfffe);
    relative(&mut data, 0x170, 0x200);
    u16_at(&mut data, 0x174, 1);
    relative(&mut data, 0x178, 0x1e0); // backwards pointer.
    u16_at(&mut data, 0x17c, 2);
    relative(&mut data, 0x180, 0x240);
    for (index, value) in [0., 1., 2., 3., 10., 4., 5., 6.].into_iter().enumerate() {
        word(&mut data, 0x200 + index * 4, f32::to_bits(value));
    }
    for (index, value) in [3., -2., -4., -6.].into_iter().enumerate() {
        word(&mut data, 0x1e0 + index * 4, f32::to_bits(value));
    }
    for (index, value) in [0., 0.25, 10., 1.].into_iter().enumerate() {
        word(&mut data, 0x240 + index * 4, f32::to_bits(value));
    }
    data
}

#[test]
fn command_chain_preserves_unknowns_signed_times_and_coded_nested_reference() {
    let data = fixture();
    let script = read_at(&data, 0x80).unwrap();
    assert!(f32::from_bits(script.header().unwrap().bounds_words_raw[0]).is_nan());
    let commands = script.commands(3).unwrap();
    assert_eq!(
        commands.iter().map(|c| c.address).collect::<Vec<_>>(),
        [0xc4, 0xdc, 0xe8]
    );
    assert_eq!((commands[0].start, commands[0].duration), (-2, 37));
    let instance = commands[0].instance().unwrap().unwrap();
    assert_eq!(instance.kind, InstanceKind::Script);
    assert_eq!(
        (
            instance.track_selector,
            instance.slot,
            instance.parent_slot_raw
        ),
        (2, 1, 255)
    );
    assert_eq!(
        (
            instance.word_0c_raw,
            instance.file_reference_raw,
            instance.reference_raw
        ),
        (0x1500_0007, u32::MAX, 0x8400_0002)
    );
    assert_eq!(&commands[1].bytes_raw[8..], &0xdead_beefu32.to_be_bytes());
    assert!(commands[1].instance().unwrap().is_none());
    assert!(script.commands(2).is_err());
}

#[test]
fn selected_tracks_use_packed_mask_order_and_signed_field_relative_pointers() {
    let data = fixture();
    let script = read_at(&data, 0x80).unwrap();
    let bundle = script.track_bundle(2).unwrap();
    assert_eq!(bundle.descriptor(4).unwrap().unwrap().auxiliary_raw, -2);
    assert_eq!(
        bundle.linear_keys(4, 2).unwrap(),
        Some(LinearKeys::Vector(vec![
            Key {
                time: 0.,
                value: [1., 2., 3.]
            },
            Key {
                time: 10.,
                value: [4., 5., 6.]
            },
        ]))
    );
    assert_eq!(
        bundle.linear_keys(0x10, 1).unwrap(),
        Some(LinearKeys::Vector(vec![Key {
            time: 3.,
            value: [-2., -4., -6.]
        },]))
    );
    assert_eq!(
        bundle.linear_keys(0x80, 2).unwrap(),
        Some(LinearKeys::Scalar(vec![
            Key {
                time: 0.,
                value: [0.25]
            },
            Key {
                time: 10.,
                value: [1.]
            },
        ]))
    );
    assert!(bundle.linear_keys(8, 0).unwrap().is_none());
    assert!(bundle.descriptor(3).is_err());
    assert!(bundle.linear_keys(4, 1).is_err());
    assert!(script.track_bundle(0).is_err());
}

#[test]
fn unsupported_and_corrupt_unused_tracks_are_lazy_but_addressed_errors_propagate() {
    let mut data = fixture();
    u16_at(&mut data, 0x17c, (-2i16) as u16);
    word(&mut data, 0x180, 0x7fff_ffff);
    let bundle = read_at(&data, 0x80).unwrap().track_bundle(2).unwrap();
    assert!(bundle.linear_keys(4, 2).is_ok());
    assert!(bundle.linear_keys(0x80, 2).is_err());
    // Zero count remains raw metadata, but cannot silently evaluate empty.
    u16_at(&mut data, 0x17c, 0);
    let bundle = read_at(&data, 0x80).unwrap().track_bundle(2).unwrap();
    assert_eq!(bundle.descriptor(0x80).unwrap().unwrap().count, 0);
    assert!(bundle.linear_keys(0x80, 0).is_err());
    // Unknown channel descriptors remain available; decoding never guesses width.
    word(&mut data, 0x164, 1);
    let bundle = read_at(&data, 0x80).unwrap().track_bundle(2).unwrap();
    assert_eq!(bundle.descriptor(1).unwrap().unwrap().count, 2);
    assert!(bundle.linear_keys(1, 2).is_err());
}

#[test]
fn quaternion_stride_preserves_all_four_lanes_and_key_times() {
    let mut data = fixture();
    word(&mut data, 0x164, 8);
    u16_at(&mut data, 0x16c, 2);
    for (index, value) in [0., 0., 0., 0., 1., 37., 0., 0., 1., 0.]
        .into_iter()
        .enumerate()
    {
        word(&mut data, 0x200 + index * 4, f32::to_bits(value));
    }
    let bundle = read_at(&data, 0x80).unwrap().track_bundle(2).unwrap();
    assert_eq!(
        bundle.linear_keys(8, 2).unwrap(),
        Some(LinearKeys::Quaternion(vec![
            Key {
                time: 0.,
                value: [0., 0., 0., 1.]
            },
            Key {
                time: 37.,
                value: [0., 0., 1., 0.]
            },
        ]))
    );
    word(&mut data, 0x224, f32::INFINITY.to_bits());
    assert!(
        read_at(&data, 0x80)
            .unwrap()
            .track_bundle(2)
            .unwrap()
            .linear_keys(8, 2)
            .is_err()
    );
}

#[test]
fn unknown_lower_channel_occupies_a_descriptor_without_addressing_its_keys() {
    let mut data = fixture();
    word(&mut data, 0x164, 0x81);
    u16_at(&mut data, 0x16c, (-7i16) as u16);
    word(&mut data, 0x170, 0x7fff_ffff);
    // Alpha is descriptor one, even though descriptor zero is unsupported.
    u16_at(&mut data, 0x174, 2);
    relative(&mut data, 0x178, 0x240);
    let bundle = read_at(&data, 0x80).unwrap().track_bundle(2).unwrap();
    assert_eq!(bundle.descriptor(ALPHA).unwrap().unwrap().address, 0x174);
    let Some(LinearKeys::Scalar(keys)) = bundle.linear_keys(ALPHA, 2).unwrap() else {
        panic!("expected alpha keys");
    };
    assert_eq!(
        keys[1],
        Key {
            time: 10.,
            value: [1.]
        }
    );
    assert!(bundle.linear_keys(1, 10).is_err());
}

#[test]
fn frame_selection_uses_signed_halfword_relative_extension_and_signed_indices() {
    let mut data = fixture();
    word(&mut data, 0x164, FRAME_SELECTOR);
    u16_at(&mut data, 0x16c, 2);
    // Extension is before its descriptor; key data retains its separate pointer.
    u16_at(&mut data, 0x16e, (0x140i16 - 0x16ei16) as u16);
    relative(&mut data, 0x170, 0x240);
    u16_at(&mut data, 0x140, 32);
    data[0x142] = 3;
    data[0x143] = 2;
    for (i, value) in [17i16, 0, -3].into_iter().enumerate() {
        u16_at(&mut data, 0x144 + 2 * i, value as u16);
    }
    let bundle = read_at(&data, 0x80).unwrap().track_bundle(2).unwrap();
    let extension = bundle.frame_selector_extension().unwrap().unwrap();
    assert_eq!(
        (
            extension.address,
            extension.period_raw,
            extension.mode_raw,
            extension.playback_mode_raw
        ),
        (0x140, 32, 3, 2)
    );
    assert_eq!(
        bundle.frame_selector_indices(3).unwrap(),
        Some(vec![17, 0, -3])
    );
    assert!(bundle.frame_selector_indices(2).is_err());
    assert!(matches!(
        bundle.linear_keys(FRAME_SELECTOR, 2).unwrap(),
        Some(LinearKeys::Scalar(_))
    ));
    data[0x142] = (-1i8) as u8;
    let bundle = read_at(&data, 0x80).unwrap().track_bundle(2).unwrap();
    assert!(bundle.frame_selector_indices(10).is_err());
    assert_eq!(
        bundle.frame_selector_extension().unwrap().unwrap().mode_raw,
        -1
    );
}

#[test]
fn malformed_steps_truncation_and_nonterminal_zero_links_fail_explicitly() {
    let mut data = fixture();
    data[0xc5] = 7;
    assert!(read_at(&data, 0x80).unwrap().commands(10).is_err());
    data[0xc5] = 8; // too short for its known instance payload.
    assert!(read_at(&data, 0x80).unwrap().commands(10).is_err());
    data[0xc5] = 24;
    data[0xdd] = 0;
    assert!(read_at(&data, 0x80).unwrap().commands(10).is_err());
    assert!(read_at(&data[..0xc0], 0x80).is_err());
}

#[test]
fn directory_index_lookup_does_not_read_other_descriptors_or_nested_resources() {
    let mut data = fixture();
    let table = Table {
        name: "animation_scripts",
        count: 2,
        hash_size_raw: 0,
        offset: 0x20,
    };
    word(&mut data, 0x28, u32::MAX); // unselected descriptor invalid.
    word(&mut data, 0x38, 0x80);
    assert_eq!(
        read(&data, &table, 0x8400_0001)
            .unwrap()
            .unwrap()
            .header()
            .unwrap()
            .slots,
        3
    );
    assert!(read(&data, &table, 0x8400_0002).unwrap().is_none());
    assert!(read(&data, &table, 0x0400_0001).unwrap().is_none());
    assert!(read(&data, &table, 0x8400_0000).is_err());
}

fn play_command() -> Command {
    let mut data = fixture();
    let at = 0xc4;
    data[at..at + 72].fill(0);
    data[at..at + 8].copy_from_slice(&[2, 72, 0, 0xff, 0xff, 0xfe, 0, 19]);
    u16_at(&mut data, at + 8, 7);
    word(&mut data, at + 12, 0x7fc0_0001);
    data[at + 16..at + 20].copy_from_slice(&[2, 0xa5, 1, 1]);
    word(&mut data, at + 20, u32::MAX);
    word(&mut data, at + 24, 0x0d00_00c1);
    for index in 0..2 {
        let clip = at + 28 + index * 16;
        word(&mut data, clip, u32::MAX);
        word(&mut data, clip + 4, 0x8300_0001);
        word(
            &mut data,
            clip + 8,
            if index == 0 { 0x7fc0_0002 } else { 0x8000_0000 },
        );
        u16_at(&mut data, clip + 12, (-4i16) as u16);
        u16_at(&mut data, clip + 14, 0);
    }
    let secondary = at + 60;
    word(&mut data, secondary, 0x0102_00c5);
    word(&mut data, secondary + 4, 0); // Preserve native coded index zero.
    u16_at(&mut data, secondary + 8, 3);
    u16_at(&mut data, secondary + 10, (-2i16) as u16);
    read_at(&data, 0x80).unwrap().command_at(at).unwrap()
}

#[test]
fn play_animation_preserves_order_repeated_references_raw_bits_and_signed_ranges() {
    let command = play_command();
    let play = command.play_animation(3).unwrap().unwrap();
    assert_eq!(
        (play.address, play.track_selector, play.word_0c_raw),
        (0xd4, 7, 0x7fc0_0001)
    );
    assert_eq!(
        (
            play.normal_count_raw,
            play.flags_raw,
            play.initial_index_raw,
            play.secondary_count_raw
        ),
        (2, 0xa5, 1, 1)
    );
    assert_eq!((play.word_4_raw, play.word_8_raw), (u32::MAX, 0x0d00_00c1));
    assert_eq!(
        play.normal_clips
            .iter()
            .map(|c| c.source_address)
            .collect::<Vec<_>>(),
        [0xe0, 0xf0]
    );
    assert_eq!(
        play.normal_clips[0].animation_reference,
        play.normal_clips[1].animation_reference
    );
    assert_eq!(play.normal_clips[0].file_reference_raw, u32::MAX);
    assert_eq!(play.normal_clips[0].blend_key_bits, 0x7fc0_0002);
    assert_eq!(play.normal_clips[1].blend_key_bits, 0x8000_0000);
    assert_eq!(
        (play.normal_clips[0].start_raw, play.normal_clips[0].end_raw),
        (-4, 0)
    );
    assert_eq!(
        play.secondary_clips[0],
        SecondaryAnimationClip {
            source_address: 0x100,
            file_reference_raw: 0x0102_00c5,
            animation_reference: 0,
            start_raw: 3,
            end_raw: -2,
        }
    );
    assert!(command.instance().unwrap().is_none());
}

#[test]
fn play_animation_empty_lists_do_not_invent_a_clip_or_validate_unused_suffix() {
    let mut command = play_command();
    command.bytes_raw[16] = 0;
    command.bytes_raw[19] = 0;
    command.bytes_raw.truncate(28);
    let play = command.play_animation(0).unwrap().unwrap();
    assert!(play.normal_clips.is_empty());
    assert!(play.secondary_clips.is_empty());
    assert_eq!(play.initial_index_raw, 1); // Source metadata, not a selected live index.
}

#[test]
fn play_animation_budget_precedes_clip_reads_and_other_opcodes_stay_lazy() {
    let mut command = play_command();
    command.bytes_raw.truncate(28);
    assert!(
        command
            .play_animation(2)
            .unwrap_err()
            .to_string()
            .contains("budget")
    );
    assert!(
        command
            .play_animation(3)
            .unwrap_err()
            .to_string()
            .contains("clip list")
    );
    command.opcode = 0x7e;
    command.bytes_raw.clear();
    assert!(command.play_animation(0).unwrap().is_none());
}

#[test]
fn reached_play_animation_headers_and_each_array_are_bounded() {
    for length in [8, 27, 28, 59, 71] {
        let mut command = play_command();
        command.bytes_raw.truncate(length);
        assert!(command.play_animation(3).is_err());
    }
    let mut command = play_command();
    command.address = usize::MAX;
    assert!(command.play_animation(3).is_err());
}
