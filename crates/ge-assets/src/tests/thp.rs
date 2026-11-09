use super::*;
fn put(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn fixture() -> Vec<u8> {
    let mut bytes = vec![0; 176];
    bytes[..4].copy_from_slice(b"THP\0");
    for (at, value) in [
        (4, 0x11000),
        (8, 32),
        (16, 30f32.to_bits()),
        (20, 3),
        (24, 32),
        (28, 96),
        (32, 48),
        (40, 80),
        (44, 144),
        (48, 1),
        (68, 1),
        (72, 1),
    ] {
        put(&mut bytes, at, value);
    }
    for offset in [80, 112, 144] {
        put(&mut bytes, offset, 32);
        put(&mut bytes, offset + 4, 32);
        put(&mut bytes, offset + 8, 4);
        bytes[offset + 12..offset + 16].copy_from_slice(&[255, 216, 255, 217]);
    }
    bytes
}
fn header(bytes: &[u8]) -> Header {
    read_header(bytes.len() as u64, |offset, count| {
        Ok(bytes[offset as usize..offset as usize + count].to_vec())
    })
    .unwrap()
}
fn marker(jpeg: &mut Vec<u8>, kind: u8, data: &[u8]) {
    jpeg.extend_from_slice(&[255, kind]);
    jpeg.extend_from_slice(&((data.len() + 2) as u16).to_be_bytes());
    jpeg.extend_from_slice(data);
}
/// Independently authored one-pixel baseline JPEG: quantization ones, single
/// zero-category DC and EOB Huffman codes, constant level128 image.
fn gray_jpeg() -> Vec<u8> {
    let mut jpeg = vec![255, 216];
    let mut quantization = vec![0];
    quantization.extend([1; 64]);
    marker(&mut jpeg, 0xdb, &quantization);
    marker(&mut jpeg, 0xc0, &[8, 0, 1, 0, 1, 1, 1, 0x11, 0]);
    let mut tables = Vec::new();
    for class in [0, 0x10] {
        tables.push(class);
        tables.push(1);
        tables.extend([0; 15]);
        tables.push(0);
    }
    marker(&mut jpeg, 0xc4, &tables);
    marker(&mut jpeg, 0xda, &[1, 1, 0, 0, 63, 0]);
    jpeg.extend_from_slice(&[0x3f, 255, 217, 0, 0]);
    jpeg
}
#[test]
fn header_and_selected_frame_only_read_component_metadata_and_frame_prefix_headers() {
    let bytes = fixture();
    let metadata = header(&bytes);
    let mut reads = Vec::new();
    let frame = locate_frame(&metadata, 2, |offset, count| {
        reads.push((offset, count));
        Ok(bytes[offset as usize..offset as usize + count].to_vec())
    })
    .unwrap();
    assert_eq!(reads, [(80, 12), (112, 12), (144, 12)]);
    assert_eq!((frame.video_offset, frame.video_bytes), (156, 4));
    assert_eq!(
        (metadata.width, metadata.height, metadata.frame_count),
        (1, 1, 3)
    );
}
#[test]
fn malformed_chain_truncated_reads_and_out_of_range_frames_are_errors() {
    let mut bytes = fixture();
    let metadata = header(&bytes);
    put(&mut bytes, 116, 99);
    assert!(
        locate_frame(&metadata, 1, |offset, count| Ok(bytes
            [offset as usize..offset as usize + count]
            .to_vec()))
        .unwrap_err()
        .to_string()
        .contains("previous")
    );
    assert!(locate_frame(&metadata, 3, |_, _| panic!("out of range must not read")).is_err());
    assert!(locate_frame(&metadata, 0, |_, _| Ok(vec![0; 11])).is_err());
    bytes = fixture();
    put(&mut bytes, 88, 21);
    assert!(
        locate_frame(&metadata, 0, |offset, count| Ok(bytes
            [offset as usize..offset as usize + count]
            .to_vec()))
        .is_err()
    );
}
#[test]
fn unsupported_component_order_and_bad_dimensions_or_data_spans_fail_explicitly() {
    let mut bytes = fixture();
    put(&mut bytes, 48, 2);
    assert!(
        read_header(bytes.len() as u64, |offset, count| Ok(bytes
            [offset as usize..offset as usize + count]
            .to_vec()))
        .is_err()
    );
    bytes = fixture();
    put(&mut bytes, 68, 8192);
    assert!(
        read_header(bytes.len() as u64, |offset, count| Ok(bytes
            [offset as usize..offset as usize + count]
            .to_vec()))
        .is_err()
    );
    bytes = fixture();
    put(&mut bytes, 28, 1000);
    assert!(
        read_header(bytes.len() as u64, |offset, count| Ok(bytes
            [offset as usize..offset as usize + count]
            .to_vec()))
        .is_err()
    );
}
#[test]
fn entropy_marker_lookalikes_are_stuffed_and_only_final_eoi_is_kept() {
    let mut payload = gray_jpeg();
    payload.truncate(payload.len() - 5);
    payload.extend_from_slice(&[255, 217, 255, 0, 255, 217, 0, 0]);
    let jpeg = jpeg_payload(&payload).unwrap();
    assert!(jpeg.ends_with(&[255, 0, 217, 255, 0, 0, 255, 217]));
    assert!(jpeg_payload(&[255, 216, 255, 219, 0, 1]).is_err());
    let mut missing = gray_jpeg();
    missing.truncate(missing.len() - 4);
    assert!(jpeg_payload(&missing).is_err());
}
#[test]
fn authored_constant_jpeg_decodes_to_rgba_and_checks_component_dimensions() {
    let bytes = fixture();
    let mut metadata = header(&bytes);
    let payload = gray_jpeg();
    let frame = Frame {
        index: 0,
        offset: 80,
        total_bytes: payload.len() as u32 + 12,
        next_frame_bytes: 0,
        previous_frame_bytes: 0,
        video_offset: 92,
        video_bytes: payload.len() as u32,
        audio_offset: None,
        audio_bytes: 0,
    };
    let image = decode_frame(&metadata, &frame, &payload).unwrap();
    assert_eq!((image.width, image.height), (1, 1));
    assert_eq!(image.pixels, [128, 128, 128, 255]);
    metadata.width = 2;
    assert!(decode_frame(&metadata, &frame, &payload).is_err());
}

fn audio_fixture() -> Vec<u8> {
    let mut bytes = fixture();
    bytes.resize(352, 0);
    for (at, value) in [
        (8, 128),
        (12, 14),
        (20, 2),
        (24, 128),
        (28, 256),
        (40, 96),
        (44, 224),
        (48, 2),
        (80, 2),
        (84, 32000),
        (88, 28),
        (92, 1),
    ] {
        put(&mut bytes, at, value);
    }
    bytes[53] = 1;
    for offset in [96, 224] {
        put(&mut bytes, offset, 128);
        put(&mut bytes, offset + 4, 128);
        put(&mut bytes, offset + 8, 4);
        put(&mut bytes, offset + 12, 96);
        bytes[offset + 16..offset + 20].copy_from_slice(&[255, 216, 255, 217]);
    }
    bytes
}
fn audio_packet(
    count: u32,
    histories: [[i16; 2]; 2],
    coefficients: [[i16; 16]; 2],
    data: [[u8; 8]; 2],
) -> Vec<u8> {
    let mut packet = vec![0; 96];
    put(&mut packet, 0, 8);
    put(&mut packet, 4, count);
    for channel in 0..2 {
        for (index, value) in coefficients[channel].iter().enumerate() {
            packet[8 + channel * 32 + index * 2..10 + channel * 32 + index * 2]
                .copy_from_slice(&value.to_be_bytes());
        }
        for (index, value) in histories[channel].iter().enumerate() {
            packet[72 + channel * 4 + index * 2..74 + channel * 4 + index * 2]
                .copy_from_slice(&value.to_be_bytes());
        }
        packet[80 + channel * 8..88 + channel * 8].copy_from_slice(&data[channel]);
    }
    packet
}
#[test]
fn audio_header_index_and_cursor_have_exact_offsets_single_reads_and_retryable_errors() {
    let bytes = audio_fixture();
    let metadata = header(&bytes);
    let audio = metadata.audio.as_ref().unwrap();
    assert_eq!(
        (
            audio.channels,
            audio.sample_rate,
            audio.total_samples,
            audio.tracks
        ),
        (2, 32000, 28, 1)
    );
    let mut reads = Vec::new();
    let frames = frame_index(&metadata, |offset, count| {
        reads.push((offset, count));
        Ok(bytes[offset as usize..offset as usize + count].to_vec())
    })
    .unwrap();
    assert_eq!(reads, [(96, 16), (224, 16)]);
    assert_eq!(
        (
            frames[0].video_offset,
            frames[0].audio_offset,
            frames[0].audio_bytes
        ),
        (112, Some(116), 96)
    );
    let mut cursor = FrameCursor::new(&metadata).unwrap();
    assert!(cursor.next_frame(|_, _| Ok(vec![])).is_err());
    assert_eq!(
        cursor
            .next_frame(
                |offset, count| Ok(bytes[offset as usize..offset as usize + count].to_vec())
            )
            .unwrap()
            .unwrap()
            .index,
        0
    );
    cursor
        .next_frame(|offset, count| Ok(bytes[offset as usize..offset as usize + count].to_vec()))
        .unwrap();
    assert!(
        cursor
            .next_frame(|_, _| panic!("EOF must not read"))
            .unwrap()
            .is_none()
    );
}
#[test]
fn dsp_signed_high_first_nibbles_and_exponents_are_independently_authored() {
    let data = [1, 0x17, 0x8f, 0x20, 0, 0, 0, 0];
    let (samples, _) = decode_dsp_channel(&data, 14, [0; 16], [0, 0]).unwrap();
    assert_eq!(&samples[..6], &[2, 14, -16, -2, 4, 0]);
    assert!(samples[6..].iter().all(|&v| v == 0));
    let mut coefficients = [0; 16];
    coefficients[2] = 2048;
    let (samples, _) = decode_dsp_channel(&[0x91, 0x30], 1, coefficients, [5, 0]).unwrap();
    assert_eq!(samples, [11]);
}
#[test]
fn dsp_prediction_uses_arithmetic_shift_and_clamped_history() {
    let mut coefficients = [0; 16];
    coefficients[0] = 1024;
    coefficients[1] = 1024;
    let (samples, _) = decode_dsp_channel(&[0, 0], 2, coefficients, [-1, 0]).unwrap();
    assert_eq!(samples, [-1, -1]);
    coefficients[0] = 2048;
    coefficients[1] = 0;
    let (samples, history) = decode_dsp_channel(&[4, 0x78], 2, coefficients, [32760, 0]).unwrap();
    assert_eq!(samples, [32767, 32639]);
    assert_eq!(history, [32639, 32767]);
    assert!(decode_dsp_channel(&[0], 1, coefficients, [0, 0]).is_err());
}
#[test]
fn sequential_stereo_audio_keeps_state_while_standalone_decode_uses_packet_histories() {
    let bytes = audio_fixture();
    let metadata = header(&bytes);
    let frames = frame_index(&metadata, |offset, count| {
        Ok(bytes[offset as usize..offset as usize + count].to_vec())
    })
    .unwrap();
    let mut coefficients = [[0; 16]; 2];
    coefficients[0][0] = 2048;
    coefficients[1][0] = 2048;
    let packet = audio_packet(
        14,
        [[0; 2]; 2],
        coefficients,
        [
            [0, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11],
            [0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
        ],
    );
    let mut decoder = AudioDecoder::default();
    let first = decoder.decode(&metadata, &frames[0], &packet).unwrap();
    assert_eq!(&first.samples[..4], &[1, -1, 2, -2]);
    assert_eq!(&first.samples[24..], &[13, -13, 14, -14]);
    let packet = audio_packet(
        14,
        [[100, 0], [-100, 0]],
        coefficients,
        [
            [0, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11],
            [0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
        ],
    );
    let second = decoder.decode(&metadata, &frames[1], &packet).unwrap();
    assert_eq!(&second.samples[..2], &[15, -15]);
    let standalone = decode_audio(&metadata, &frames[1], &packet).unwrap();
    assert_eq!(&standalone.samples[..2], &[101, -101]);
    assert!(decoder.decode(&metadata, &frames[0], &packet).is_err());
    decoder.reset();
    assert!(decoder.decode(&metadata, &frames[0], &packet).is_ok());
}
#[test]
fn partial_final_block_uses_declared_channel_stride_and_rejects_invalid_packet_counts() {
    let bytes = audio_fixture();
    let metadata = header(&bytes);
    let frame = locate_frame(&metadata, 0, |offset, count| {
        Ok(bytes[offset as usize..offset as usize + count].to_vec())
    })
    .unwrap();
    let mut packet = audio_packet(
        9,
        [[0; 2]; 2],
        [[0; 16]; 2],
        [
            [0, 0x11, 0x11, 0x11, 0x11, 0x1f, 0xff, 0xff],
            [0, 0x22, 0x22, 0x22, 0x22, 0x2f, 0xff, 0xff],
        ],
    );
    let result = decode_audio(&metadata, &frame, &packet).unwrap();
    assert_eq!(result.samples_per_channel, 9);
    assert_eq!(result.samples, [1, 2].repeat(9));
    put(&mut packet, 4, 15);
    assert!(decode_audio(&metadata, &frame, &packet).is_err());
    put(&mut packet, 4, 14);
    put(&mut packet, 0, 1);
    assert!(decode_audio(&metadata, &frame, &packet).is_err());
}
