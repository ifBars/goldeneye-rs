//! Bounded Wii MUSX v10 DAT8 streams and SBNK v0x1e resident NGCA DSP waves.
//! DAT8 uses independently headed IMA blocks; NGCA preserves channel histories.
//! Format reference: https://github.com/vgmstream/vgmstream/blob/master/src/meta/musx.c
use anyhow::{Context, Result, ensure};
use serde::Serialize;

const DATA_OFFSET: u64 = 0x800;
const BLOCK_BYTES: usize = 32;
const BLOCK_FRAMES: u64 = 56;
const MAX_BYTES: u64 = 256 * 1024 * 1024;
const MAX_BLOCKS_PER_READ: usize = 4096;

#[derive(Debug, Clone, Serialize)]
pub struct StreamInfo {
    pub resource_hash: u32,
    pub file_bytes: u64,
    pub data_offset: u64,
    pub data_bytes: u32,
    pub sample_rate: u32,
    pub channels: u16,
    pub sample_frames: u64,
    pub flags_raw: u32,
    pub loop_start_frame: Option<u64>,
    pub loop_end_frame: Option<u64>,
}
#[derive(Debug, Serialize)]
pub struct PcmChunk {
    pub first_frame: u64,
    pub sample_rate: u32,
    pub channels: u16,
    pub frame_count: usize,
    #[serde(skip)]
    pub samples: Vec<i16>,
}

fn le32(bytes: &[u8], at: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        bytes
            .get(at..at.checked_add(4).context("stream header offset overflow")?)
            .context("truncated music stream header")?
            .try_into()?,
    ))
}
fn read_exact(
    read: &mut impl FnMut(u64, usize) -> Result<Vec<u8>>,
    at: u64,
    n: usize,
) -> Result<Vec<u8>> {
    let bytes = read(at, n)?;
    ensure!(bytes.len() == n, "truncated frontend stream read");
    Ok(bytes)
}

fn be32(bytes: &[u8], at: usize) -> Result<u32> {
    Ok(u32::from_be_bytes(
        bytes
            .get(at..at + 4)
            .context("truncated sound bank word")?
            .try_into()?,
    ))
}
fn bounded_read(
    read: &mut impl FnMut(u64, usize) -> Result<Vec<u8>>,
    size: u64,
    at: u64,
    n: usize,
) -> Result<Vec<u8>> {
    ensure!(
        at.checked_add(n as u64)
            .context("sound bank range overflow")?
            <= size,
        "sound bank read outside resource"
    );
    read_exact(read, at, n)
}
fn relative(field: u64, value: u32) -> Result<u64> {
    let target = i128::from(field) + i128::from(value as i32);
    ensure!(
        target >= 0 && target <= i128::from(u64::MAX),
        "sound bank relative address overflow"
    );
    Ok(target as u64)
}

#[derive(Debug, Clone, Serialize)]
pub struct NgcaInfo {
    pub resource_hash: u32,
    pub sound_hash: u32,
    pub wave_hash: u32,
    pub file_bytes: u64,
    pub data_offset: u64,
    pub data_bytes: u32,
    pub channel_stride: u64,
    pub sample_frames: u64,
    pub sample_rate: u32,
    pub channels: u16,
    pub codec_raw: u8,
    pub loop_start_frame: Option<u64>,
    #[serde(skip)]
    coefficients: Vec<[i16; 16]>,
}

/// Resolve a single-layer internal sound to its actual wave. External streams,
/// composite programs and unconfirmed initial-history variants are errors.
/// The sound's paired relative pointers are field-relative, not bank-relative.
pub fn read_bank_sound(
    file_bytes: u64,
    sound_hash: u32,
    mut read: impl FnMut(u64, usize) -> Result<Vec<u8>>,
) -> Result<NgcaInfo> {
    ensure!(
        (0x854..=MAX_BYTES).contains(&file_bytes),
        "sound bank size outside host bounds"
    );
    let outer = bounded_read(&mut read, file_bytes, 0, 0x14)?;
    ensure!(
        &outer[..4] == b"MUSX" && le32(&outer, 8)? == 10 && &outer[0x10..0x14] == b"WII_",
        "unsupported sound bank container"
    );
    ensure!(
        u64::from(le32(&outer, 0xc)?) == file_bytes,
        "sound bank index size mismatch"
    );
    let h = bounded_read(&mut read, file_bytes, 0x800, 0x54)?;
    ensure!(
        &h[..4] == b"SBNK" && be32(&h, 4)? == 0x1e,
        "unsupported SBNK version"
    );
    let resource_hash = le32(&outer, 4)?;
    ensure!(be32(&h, 0xc)? == resource_hash, "sound bank hash mismatch");
    let count = be32(&h, 0x14)?;
    ensure!(
        (1..=65536).contains(&count),
        "sound directory work budget exceeded"
    );
    let directory = relative(0x818, be32(&h, 0x18)?)?;
    let rows = bounded_read(&mut read, file_bytes, directory, count as usize * 16)?;
    let mut selected = None;
    for (index, row) in rows.chunks_exact(16).enumerate() {
        if be32(row, 0)? == sound_hash {
            ensure!(selected.is_none(), "ambiguous duplicate sound hash");
            selected = Some((directory + index as u64 * 16, row));
        }
    }
    let (descriptor, row) = selected.context("sound hash absent from bank")?;
    ensure!(row[12] == 1, "unsupported multi-layer sound program");
    let pair_at = relative(descriptor + 8, be32(row, 8)?)?;
    let pair = bounded_read(&mut read, file_bytes, pair_at, 8)?;
    let params_at = relative(pair_at, be32(&pair, 0)?)?;
    let params = bounded_read(&mut read, file_bytes, params_at, 12)?;
    ensure!(
        params[11] & 2 == 0,
        "sound uses an external stream, not a resident wave"
    );
    let wave_at = relative(pair_at + 4, be32(&pair, 4)?)?;
    let wave_count = be32(&h, 0x34)?;
    ensure!(
        (1..=65536).contains(&wave_count),
        "wave directory work budget exceeded"
    );
    let wave_directory = relative(0x838, be32(&h, 0x38)?)?;
    let wave_end = wave_directory
        .checked_add(u64::from(wave_count) * 40)
        .context("wave directory overflow")?;
    ensure!(
        wave_end <= file_bytes
            && wave_at >= wave_directory
            && wave_at < wave_end
            && (wave_at - wave_directory).is_multiple_of(40),
        "sound wave reference outside directory"
    );
    let wave = bounded_read(&mut read, file_bytes, wave_at, 40)?;
    let codec_raw = wave[0x1a];
    ensure!(
        matches!(codec_raw, 0x13 | 0x1b),
        "unsupported resident wave codec flags"
    );
    let channels = u16::from(wave[0x1b]);
    ensure!(matches!(channels, 1 | 2), "NGCA supports mono/stereo only");
    let sample_frames = u64::from(be32(&wave, 4)?);
    let sample_rate = u32::from(u16::from_be_bytes([wave[0x18], wave[0x19]]));
    ensure!(
        sample_frames > 0 && (1000..=192000).contains(&sample_rate),
        "invalid NGCA sample count/rate"
    );
    let data_bytes = be32(&wave, 0x20)?;
    ensure!(
        data_bytes > 0 && data_bytes.is_multiple_of(u32::from(channels)),
        "partial NGCA channel payload"
    );
    let channel_stride = u64::from(data_bytes) / u64::from(channels);
    let encoded = sample_frames
        .div_ceil(14)
        .checked_mul(8)
        .context("NGCA encoded size overflow")?;
    ensure!(
        channel_stride >= 0x40 + encoded,
        "NGCA sample count exceeds channel payload"
    );
    let audio_base = u64::from(be32(&h, 0x50)?);
    let audio_bytes = u64::from(be32(&h, 0x4c)?);
    ensure!(
        audio_base
            .checked_add(audio_bytes)
            .context("bank payload overflow")?
            <= file_bytes,
        "bank audio payload outside resource"
    );
    let wave_offset = u64::from(be32(&wave, 0x1c)?);
    ensure!(
        wave_offset + u64::from(data_bytes) <= audio_bytes,
        "wave outside bank audio payload"
    );
    let data_offset = audio_base + wave_offset;
    let mut coefficients = Vec::new();
    for channel in 0..channels {
        let ch = bounded_read(
            &mut read,
            file_bytes,
            data_offset + u64::from(channel) * channel_stride,
            0x40,
        )?;
        ensure!(
            &ch[..4] == b"NGCA" && be32(&ch, 8)? == u32::from(channels),
            "NGCA channel header mismatch"
        );
        // The confirmed title asset starts with zero histories. Their nonzero
        // header interpretation is deliberately unsupported until reconstructed.
        ensure!(
            ch[0x30..0x34] == [0; 4],
            "unconfirmed nonzero NGCA initial histories"
        );
        let mut coefs = [0; 16];
        for (index, coefficient) in coefs.iter_mut().enumerate() {
            *coefficient = i16::from_be_bytes([ch[0xc + index * 2], ch[0xd + index * 2]]);
        }
        coefficients.push(coefs);
    }
    let loop_start_frame = if codec_raw & 8 != 0 {
        let start = be32(&wave, 0x14)? as i32;
        ensure!(
            start >= 0 && (start as u64) < sample_frames,
            "NGCA loop starts outside samples"
        );
        Some(start as u64)
    } else {
        None
    };
    Ok(NgcaInfo {
        resource_hash,
        sound_hash,
        wave_hash: be32(&wave, 0)?,
        file_bytes,
        data_offset,
        data_bytes,
        channel_stride,
        sample_frames,
        sample_rate,
        channels,
        codec_raw,
        loop_start_frame,
        coefficients,
    })
}

impl NgcaInfo {
    pub fn decoder(&self) -> NgcaDecoder {
        NgcaDecoder {
            next_frame: 0,
            history: vec![[0; 2]; usize::from(self.channels)],
        }
    }
}
/// Sequential decoder; restarting at frame zero constructs fresh zero history.
/// Looping elsewhere requires decoding the prefix, not resetting at a byte offset.
pub struct NgcaDecoder {
    next_frame: u64,
    history: Vec<[i16; 2]>,
}
impl NgcaDecoder {
    pub fn next_frame(&self) -> u64 {
        self.next_frame
    }
    pub fn remaining_blocks(&self, info: &NgcaInfo) -> u64 {
        info.sample_frames
            .saturating_sub(self.next_frame)
            .div_ceil(14)
    }
    pub fn decode_blocks(
        &mut self,
        info: &NgcaInfo,
        count: usize,
        mut read: impl FnMut(u64, usize) -> Result<Vec<u8>>,
    ) -> Result<PcmChunk> {
        ensure!(
            count <= MAX_BLOCKS_PER_READ && count as u64 <= self.remaining_blocks(info),
            "NGCA decode range/work budget exceeded"
        );
        let channels = usize::from(info.channels);
        ensure!(
            matches!(channels, 1 | 2)
                && self.history.len() == channels
                && info.coefficients.len() == channels
                && self.next_frame <= info.sample_frames,
            "NGCA decoder/format mismatch"
        );
        let frames = (info.sample_frames - self.next_frame).min(count as u64 * 14) as usize;
        let mut samples = vec![0; frames * channels];
        let mut histories = self.history.clone();
        for (channel, history) in histories.iter_mut().enumerate() {
            let at = info.data_offset
                + channel as u64 * info.channel_stride
                + 0x40
                + self.next_frame / 14 * 8;
            let bytes = if count == 0 {
                Vec::new()
            } else {
                bounded_read(&mut read, info.file_bytes, at, count * 8)?
            };
            for frame in 0..frames {
                let header = bytes[frame / 14 * 8];
                let predictor = usize::from(header >> 4);
                ensure!(predictor < 8, "invalid NGCA DSP coefficient selector");
                let packed = bytes[frame / 14 * 8 + 1 + frame % 14 / 2];
                let code = if frame % 2 == 0 {
                    packed >> 4
                } else {
                    packed & 15
                };
                let residual = i64::from(if code < 8 {
                    i16::from(code)
                } else {
                    i16::from(code) - 16
                });
                let coefs = info.coefficients[channel];
                // DSP rounds the combined fixed-point prediction. THP's
                // truncating variant is intentionally not reused here.
                let expanded = residual * (1i64 << (header & 15)) * 2048
                    + i64::from(history[0]) * i64::from(coefs[predictor * 2])
                    + i64::from(history[1]) * i64::from(coefs[predictor * 2 + 1])
                    + 1024;
                let value = (expanded >> 11).clamp(-32768, 32767) as i16;
                samples[frame * channels + channel] = value;
                *history = [value, history[0]];
            }
        }
        let first_frame = self.next_frame;
        self.next_frame += frames as u64;
        self.history = histories;
        Ok(PcmChunk {
            first_frame,
            sample_rate: info.sample_rate,
            channels: info.channels,
            frame_count: frames,
            samples,
        })
    }
}

/// Parse the addressed resource only. All offsets are relative to its MUSX start.
/// The caller supplies bounded archive reads and the File_COM resource size.
pub fn read_header(
    file_bytes: u64,
    mut read: impl FnMut(u64, usize) -> Result<Vec<u8>>,
) -> Result<StreamInfo> {
    ensure!(
        (DATA_OFFSET..=MAX_BYTES).contains(&file_bytes),
        "frontend stream resource size outside host bounds"
    );
    let h = read_exact(&mut read, 0, 0x80)?;
    ensure!(
        &h[..4] == b"MUSX" && le32(&h, 8)? == 10 && &h[0x10..0x14] == b"WII_",
        "unsupported frontend music container/version/platform"
    );
    ensure!(
        u64::from(le32(&h, 0xc)?) == file_bytes,
        "MUSX size differs from resource index"
    );
    ensure!(
        &h[0x40..0x44] == b"DAT8",
        "unsupported frontend music codec; expected DAT8 IMA stream"
    );
    let data_bytes = le32(&h, 0x44)?;
    let channel_word = le32(&h, 0x48)?;
    ensure!(
        (1..=8).contains(&channel_word),
        "frontend stream channel count outside 1..8"
    );
    let channels = channel_word as u16;
    let sample_rate = le32(&h, 0x4c)?;
    ensure!(
        (1000..=192_000).contains(&sample_rate),
        "frontend stream sample rate outside host bounds"
    );
    ensure!(
        le32(&h, 0x50)? == 1,
        "unsupported DAT8 loop metadata version"
    );
    let flags_raw = le32(&h, 0x54)?;
    let group_bytes = BLOCK_BYTES * u32::from(channels) as usize;
    ensure!(
        data_bytes > 0 && (data_bytes as usize).is_multiple_of(group_bytes),
        "partial DAT8 channel block group"
    );
    ensure!(
        DATA_OFFSET
            .checked_add(u64::from(data_bytes))
            .context("DAT8 data offset overflow")?
            <= file_bytes,
        "DAT8 data outside resource"
    );
    let capacity = u64::from(data_bytes) / group_bytes as u64 * BLOCK_FRAMES;
    let declared_end = le32(&h, 0x60)? as i32;
    ensure!(
        declared_end > 0 && declared_end as u64 <= capacity,
        "DAT8 declared sample count outside encoded capacity"
    );
    let sample_frames = declared_end as u64;
    let start = le32(&h, 0x64)? as i32;
    let (loop_start_frame, loop_end_frame) = if start >= 0 {
        ensure!(
            (start as u64) < sample_frames,
            "DAT8 loop starts outside samples"
        );
        (Some(start as u64), Some(sample_frames))
    } else {
        ensure!(start == -1, "invalid DAT8 absent-loop sentinel");
        (None, None)
    };
    Ok(StreamInfo {
        resource_hash: le32(&h, 4)?,
        file_bytes,
        data_offset: DATA_OFFSET,
        data_bytes,
        sample_rate,
        channels,
        sample_frames,
        flags_raw,
        loop_start_frame,
        loop_end_frame,
    })
}

impl StreamInfo {
    pub fn block_count(&self) -> u64 {
        self.sample_frames.div_ceil(BLOCK_FRAMES)
    }

    /// Decode bounded complete channel groups into interleaved signed PCM16.
    /// Headers reset predictor/index for every channel block, permitting direct
    /// seek and independent loop entry. No DSP coefficients/history are inferred.
    pub fn decode_blocks(
        &self,
        first_block: u64,
        count: usize,
        mut read: impl FnMut(u64, usize) -> Result<Vec<u8>>,
    ) -> Result<PcmChunk> {
        ensure!(
            (1..=8).contains(&self.channels) && (1000..=192_000).contains(&self.sample_rate),
            "invalid frontend stream format"
        );
        ensure!(
            count <= MAX_BLOCKS_PER_READ,
            "frontend stream decode work budget exceeded"
        );
        let end = first_block
            .checked_add(count as u64)
            .context("frontend stream block count overflow")?;
        ensure!(
            first_block <= self.block_count() && end <= self.block_count(),
            "frontend stream block range outside samples"
        );
        let first_frame = first_block
            .checked_mul(BLOCK_FRAMES)
            .context("frontend stream frame offset overflow")?;
        let frames = (self.sample_frames.saturating_sub(first_frame))
            .min(count as u64 * BLOCK_FRAMES) as usize;
        let channels = usize::from(self.channels);
        let group_bytes = BLOCK_BYTES * channels;
        let read_bytes = count
            .checked_mul(group_bytes)
            .context("frontend stream read size overflow")?;
        let byte_offset = first_block
            .checked_mul(group_bytes as u64)
            .context("frontend stream block offset overflow")?;
        ensure!(
            byte_offset
                .checked_add(read_bytes as u64)
                .context("frontend stream byte count overflow")?
                <= u64::from(self.data_bytes),
            "frontend stream block data outside payload"
        );
        let absolute = self
            .data_offset
            .checked_add(byte_offset)
            .context("frontend stream address overflow")?;
        ensure!(
            absolute
                .checked_add(read_bytes as u64)
                .context("frontend stream end overflow")?
                <= self.file_bytes,
            "frontend stream read outside resource"
        );
        let encoded = if count == 0 {
            Vec::new()
        } else {
            read_exact(&mut read, absolute, read_bytes)?
        };
        let mut samples = Vec::new();
        samples
            .try_reserve_exact(frames * channels)
            .context("allocating bounded frontend stream PCM")?;
        samples.resize(frames * channels, 0);
        for block in 0..count {
            let block_frames = frames
                .saturating_sub(block * BLOCK_FRAMES as usize)
                .min(BLOCK_FRAMES as usize);
            for channel in 0..channels {
                let start = block * group_bytes + channel * BLOCK_BYTES;
                let decoded = decode_channel(&encoded[start..start + BLOCK_BYTES])?;
                for (frame, &value) in decoded.iter().take(block_frames).enumerate() {
                    samples[(block * BLOCK_FRAMES as usize + frame) * channels + channel] = value;
                }
            }
        }
        Ok(PcmChunk {
            first_frame,
            sample_rate: self.sample_rate,
            channels: self.channels,
            frame_count: frames,
            samples,
        })
    }
}

// Standard IMA step/index facts. DAT8 consumes high nibble first and expands
// using separate shifts/additions, rather than the rounded multiply variation.
const STEPS: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66,
    73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449,
    494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272,
    2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630, 9493,
    10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794, 32767,
];
fn decode_channel(bytes: &[u8]) -> Result<[i16; 56]> {
    ensure!(bytes.len() == BLOCK_BYTES, "truncated DAT8 channel block");
    let mut predictor = i32::from(i16::from_le_bytes([bytes[0], bytes[1]]));
    let mut index = i32::from(bytes[2]);
    ensure!(index <= 88, "DAT8 IMA step index outside table");
    // Byte three is retained as codec-owned/reserved data (real streams use 0x41).
    // The predictor header is history, not an additional emitted sample.
    let mut result = [0; 56];
    for (sample, output) in result.iter_mut().enumerate() {
        let packed = bytes[4 + sample / 2];
        let code = if sample & 1 == 0 {
            packed >> 4
        } else {
            packed & 15
        };
        let step = STEPS[index as usize];
        let mut change = step >> 3;
        if code & 1 != 0 {
            change += step >> 2;
        }
        if code & 2 != 0 {
            change += step >> 1;
        }
        if code & 4 != 0 {
            change += step;
        }
        predictor = (predictor + if code & 8 == 0 { change } else { -change }).clamp(-32768, 32767);
        index = (index + [-1, -1, -1, -1, 2, 4, 6, 8][usize::from(code & 7)]).clamp(0, 88);
        *output = predictor as i16;
    }
    Ok(result)
}

#[cfg(test)]
#[path = "tests/frontend_stream.rs"]
mod tests;
