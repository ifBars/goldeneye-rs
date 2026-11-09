use super::*;

fn put(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn fixture(channels: u16, blocks: usize) -> Vec<u8> {
    let mut bytes = vec![0; 0x800 + usize::from(channels) * 32 * blocks];
    bytes[..4].copy_from_slice(b"MUSX");
    put(&mut bytes, 4, 0x2d400123);
    put(&mut bytes, 8, 10);
    let len = bytes.len() as u32;
    put(&mut bytes, 0xc, len);
    bytes[0x10..0x14].copy_from_slice(b"WII_");
    bytes[0x40..0x44].copy_from_slice(b"DAT8");
    put(&mut bytes, 0x44, len - 0x800);
    put(&mut bytes, 0x48, u32::from(channels));
    put(&mut bytes, 0x4c, 31999);
    put(&mut bytes, 0x50, 1);
    put(&mut bytes, 0x54, 3);
    put(&mut bytes, 0x60, (blocks * 56) as u32);
    put(&mut bytes, 0x64, u32::MAX);
    bytes
}
fn info(bytes: &[u8]) -> StreamInfo {
    read_header(bytes.len() as u64, |a, n| {
        Ok(bytes[a as usize..a as usize + n].to_vec())
    })
    .unwrap()
}
fn decode(bytes: &[u8], first: u64, count: usize) -> PcmChunk {
    info(bytes)
        .decode_blocks(first, count, |a, n| {
            Ok(bytes[a as usize..a as usize + n].to_vec())
        })
        .unwrap()
}

#[test]
fn header_preserves_exact_rate_and_loop_sample_interval() {
    let mut bytes = fixture(2, 2);
    put(&mut bytes, 0x64, 56);
    let h = info(&bytes);
    assert_eq!(
        (
            h.resource_hash,
            h.sample_rate,
            h.channels,
            h.sample_frames,
            h.block_count()
        ),
        (0x2d400123, 31999, 2, 112, 2)
    );
    assert_eq!(
        (h.loop_start_frame, h.loop_end_frame),
        (Some(56), Some(112))
    );
}

#[test]
fn high_nibble_first_shift_add_ima_does_not_emit_predictor_header() {
    let mut bytes = fixture(1, 1);
    bytes[0x800..0x802].copy_from_slice(&1000i16.to_le_bytes());
    bytes[0x803] = 0x41;
    bytes[0x804] = 0x12;
    bytes[0x805] = 0x34;
    let pcm = decode(&bytes, 0, 1);
    assert_eq!(&pcm.samples[..4], &[1001, 1004, 1008, 1015]);
    assert_eq!(pcm.frame_count, 56);
}

#[test]
fn stereo_channels_interleave_and_each_block_restarts_its_own_history() {
    let mut bytes = fixture(2, 2);
    for (block, predictors) in [(0, [1000i16, -1000]), (1, [2000, -2000])] {
        for (channel, predictor) in predictors.into_iter().enumerate() {
            let at = 0x800 + block * 64 + channel * 32;
            bytes[at..at + 2].copy_from_slice(&predictor.to_le_bytes());
            bytes[at + 4] = 0x11;
        }
    }
    let all = decode(&bytes, 0, 2);
    assert_eq!(&all.samples[..4], &[1001, -999, 1002, -998]);
    let second = decode(&bytes, 1, 1);
    assert_eq!(second.first_frame, 56);
    assert_eq!(&second.samples[..4], &[2001, -1999, 2002, -1998]);
    assert_eq!(&all.samples[112..], second.samples);
}

#[test]
fn predictor_and_step_clamp_at_signed_pcm_limits() {
    let mut bytes = fixture(1, 1);
    bytes[0x802] = 88;
    bytes[0x804] = 0x7f;
    bytes[0x805] = 0xff;
    assert_eq!(
        &decode(&bytes, 0, 1).samples[..4],
        &[32767, -28669, -32768, -32768]
    );
    bytes[0x802] = 89;
    assert!(
        info(&bytes)
            .decode_blocks(0, 1, |a, n| Ok(bytes[a as usize..a as usize + n].to_vec()))
            .is_err()
    );
}

#[test]
fn final_sample_count_trims_padding_and_empty_range_performs_no_read() {
    let mut bytes = fixture(2, 2);
    put(&mut bytes, 0x60, 57);
    assert_eq!(decode(&bytes, 1, 1).samples.len(), 2);
    assert_eq!(decode(&bytes, 0, 2).frame_count, 57);
    assert!(
        info(&bytes)
            .decode_blocks(2, 0, |_, _| panic!("empty range"))
            .unwrap()
            .samples
            .is_empty()
    );
    assert!(info(&bytes).decode_blocks(1, 2, |_, _| panic!()).is_err());
}

#[test]
fn malformed_containers_counts_loops_and_short_reads_are_explicit_errors() {
    for (at, value) in [
        (8, 11),
        (0x48, 0),
        (0x48, 9),
        (0x4c, 0),
        (0x50, 2),
        (0x60, 57),
        (0x64, 56),
        (0x64, u32::MAX - 1),
        (0x44, 31),
    ] {
        let mut bytes = fixture(1, 1);
        put(&mut bytes, at, value);
        assert!(
            read_header(bytes.len() as u64, |a, n| Ok(bytes
                [a as usize..a as usize + n]
                .to_vec()))
            .is_err(),
            "{at:x}"
        );
    }
    let bytes = fixture(1, 1);
    assert!(read_header(bytes.len() as u64, |_, _| Ok(vec![0; 5])).is_err());
    assert!(
        info(&bytes)
            .decode_blocks(0, 1, |_, _| Ok(vec![0; 1]))
            .is_err()
    );
    assert!(
        info(&bytes)
            .decode_blocks(0, 4097, |_, _| panic!())
            .is_err()
    );
}

fn beput(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn bank_fixture(channels: usize, frames: u32) -> Vec<u8> {
    let stride = 0x40 + (frames as usize).div_ceil(14) * 8;
    let mut bytes = vec![0; 0x1000 + stride * channels];
    bytes[..4].copy_from_slice(b"MUSX");
    put(&mut bytes, 4, 0x2d500017);
    put(&mut bytes, 8, 10);
    let length = bytes.len() as u32;
    put(&mut bytes, 12, length);
    bytes[0x10..0x14].copy_from_slice(b"WII_");
    bytes[0x800..0x804].copy_from_slice(b"SBNK");
    beput(&mut bytes, 0x804, 0x1e);
    beput(&mut bytes, 0x80c, 0x2d500017);
    beput(&mut bytes, 0x814, 1);
    beput(&mut bytes, 0x818, 0x3c); // descriptor854
    beput(&mut bytes, 0x834, 1);
    beput(&mut bytes, 0x838, 0x13c); // wave974
    beput(&mut bytes, 0x84c, (stride * channels) as u32);
    beput(&mut bytes, 0x850, 0x1000);
    beput(&mut bytes, 0x854, 0x2d700109);
    beput(&mut bytes, 0x85c, 0xa4); // pair900
    bytes[0x860] = 1;
    beput(&mut bytes, 0x900, 0x20); // params920
    beput(&mut bytes, 0x904, 0x70); // wave974
    beput(&mut bytes, 0x974, 0x2da021bf);
    beput(&mut bytes, 0x978, frames);
    bytes[0x98c..0x98e].copy_from_slice(&31999u16.to_be_bytes());
    bytes[0x98e] = 0x1b;
    bytes[0x98f] = channels as u8;
    beput(&mut bytes, 0x994, (stride * channels) as u32);
    for channel in 0..channels {
        let at = 0x1000 + stride * channel;
        bytes[at..at + 4].copy_from_slice(b"NGCA");
        beput(&mut bytes, at + 8, channels as u32);
    }
    bytes
}
fn bank_info(bytes: &[u8]) -> NgcaInfo {
    read_bank_sound(bytes.len() as u64, 0x2d700109, |a, n| {
        Ok(bytes[a as usize..a as usize + n].to_vec())
    })
    .unwrap()
}
fn bank_decode(decoder: &mut NgcaDecoder, info: &NgcaInfo, count: usize, bytes: &[u8]) -> PcmChunk {
    decoder
        .decode_blocks(info, count, |a, n| {
            Ok(bytes[a as usize..a as usize + n].to_vec())
        })
        .unwrap()
}

#[test]
fn bank_sound_follows_both_field_relative_pointers_and_preserves_loop_metadata() {
    let bytes = bank_fixture(2, 28);
    let info = bank_info(&bytes);
    assert_eq!(
        (info.resource_hash, info.sound_hash, info.wave_hash),
        (0x2d500017, 0x2d700109, 0x2da021bf)
    );
    assert_eq!(
        (
            info.channel_stride,
            info.data_offset,
            info.sample_frames,
            info.sample_rate
        ),
        (0x50, 0x1000, 28, 31999)
    );
    assert_eq!(
        (info.channels, info.codec_raw, info.loop_start_frame),
        (2, 0x1b, Some(0))
    );
}

#[test]
fn ngca_uses_separate_channel_coefficients_and_combined_prediction_rounding() {
    let mut bytes = bank_fixture(2, 14);
    bytes[0x100c..0x100e].copy_from_slice(&1024i16.to_be_bytes());
    // Left: residual1 then zero -> rounded prediction .5 becomes1.
    bytes[0x1041] = 0x10;
    // Right: distinct coefficients/payload, negative residual.
    bytes[0x1048 + 0xc..0x1048 + 0xe].copy_from_slice(&2048i16.to_be_bytes());
    bytes[0x1089] = 0xf0;
    let info = bank_info(&bytes);
    let pcm = bank_decode(&mut info.decoder(), &info, 1, &bytes);
    assert_eq!(&pcm.samples[..8], &[1, -1, 1, -1, 1, -1, 1, -1]);
}

#[test]
fn ngca_chunk_history_matches_continuous_and_restart_is_explicit() {
    let mut bytes = bank_fixture(1, 29);
    bytes[0x100c..0x100e].copy_from_slice(&2048i16.to_be_bytes());
    bytes[0x1041] = 0x10;
    let info = bank_info(&bytes);
    let all = bank_decode(&mut info.decoder(), &info, 3, &bytes);
    let mut decoder = info.decoder();
    let first = bank_decode(&mut decoder, &info, 1, &bytes);
    let rest = bank_decode(&mut decoder, &info, 2, &bytes);
    assert_eq!(rest.first_frame, 14);
    assert_eq!([first.samples, rest.samples].concat(), all.samples);
    assert_eq!(decoder.next_frame(), 29);
    assert_eq!(decoder.remaining_blocks(&info), 0);
    assert_eq!(all.samples, vec![1; 29]);
    assert_eq!(
        bank_decode(&mut info.decoder(), &info, 1, &bytes).samples,
        vec![1; 14]
    );
}

#[test]
fn ngca_clips_fixed_point_samples_and_trims_the_final_block() {
    let mut bytes = bank_fixture(1, 2);
    bytes[0x1040] = 15;
    bytes[0x1041] = 0x7f;
    let info = bank_info(&bytes);
    assert_eq!(
        bank_decode(&mut info.decoder(), &info, 1, &bytes).samples,
        [32767, -32768]
    );
}

#[test]
fn ngca_failures_do_not_publish_partial_channel_history_or_progress() {
    let mut bytes = bank_fixture(2, 28);
    bytes[0x1041] = 0x10;
    bytes[0x1090] = 0x80; // invalid right-channel predictor in block0
    let info = bank_info(&bytes);
    let mut decoder = info.decoder();
    assert!(
        decoder
            .decode_blocks(&info, 1, |a, n| Ok(
                bytes[a as usize..a as usize + n].to_vec()
            ))
            .is_err()
    );
    assert_eq!(decoder.next_frame(), 0);
    assert_eq!(decoder.history, vec![[0, 0], [0, 0]]);
    assert!(
        decoder
            .decode_blocks(&info, 4097, |_, _| panic!("budget"))
            .is_err()
    );
    assert!(
        decoder
            .decode_blocks(&info, 0, |_, _| panic!("empty"))
            .unwrap()
            .samples
            .is_empty()
    );
}

#[test]
fn bank_rejects_external_layer_bad_wave_reference_and_unconfirmed_history() {
    for (at, value) in [
        (0x804, 0x12),
        (0x85c, 0x7fffffff),
        (0x904, 0x71),
        (0x994, 1),
        (0x978, 999999),
    ] {
        let mut bytes = bank_fixture(1, 14);
        beput(&mut bytes, at, value);
        assert!(
            read_bank_sound(bytes.len() as u64, 0x2d700109, |a, n| Ok(bytes
                [a as usize..a as usize + n]
                .to_vec()))
            .is_err(),
            "{at:x}"
        );
    }
    for (at, value) in [(0x860, 2), (0x92b, 2), (0x1030, 1), (0x98e, 0x17)] {
        let mut bytes = bank_fixture(1, 14);
        bytes[at] = value;
        assert!(
            read_bank_sound(bytes.len() as u64, 0x2d700109, |a, n| Ok(bytes
                [a as usize..a as usize + n]
                .to_vec()))
            .is_err(),
            "{at:x}"
        );
    }
}
