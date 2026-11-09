//! Checked THP video/audio frame access, JPEG adaptation and DSP ADPCM decode.
//! No original executable or external video process is required for decoding.
use crate::{binary::Bytes, tpl::RgbaImage};
use anyhow::{Context, Result, ensure};
use jpeg_decoder::{Decoder, PixelFormat};
use serde::Serialize;
use std::io::Cursor;

const MAX_FRAME_BYTES: u32 = 16 * 1024 * 1024;
const MAX_PIXELS: u64 = 16 * 1024 * 1024;
#[derive(Debug, Clone, Serialize)]
pub struct Header {
    pub version: u32,
    pub fps: f32,
    pub frame_count: u32,
    pub width: u32,
    pub height: u32,
    pub maximum_frame_bytes: u32,
    pub first_frame_bytes: u32,
    pub first_frame_offset: u64,
    pub last_frame_offset: u64,
    pub data_bytes: u64,
    pub file_bytes: u64,
    pub audio: Option<AudioInfo>,
}
#[derive(Debug, Clone, Serialize)]
pub struct AudioInfo {
    pub channels: u32,
    pub sample_rate: u32,
    pub total_samples: u32,
    pub tracks: u32,
    pub maximum_frame_samples: u32,
}
#[derive(Debug, Clone, Serialize)]
pub struct Frame {
    pub index: u32,
    pub offset: u64,
    pub total_bytes: u32,
    pub next_frame_bytes: u32,
    pub previous_frame_bytes: u32,
    pub video_offset: u64,
    pub video_bytes: u32,
    pub audio_offset: Option<u64>,
    pub audio_bytes: u32,
}
fn exact(
    read: &mut impl FnMut(u64, usize) -> Result<Vec<u8>>,
    offset: u64,
    count: usize,
) -> Result<Vec<u8>> {
    let data = read(offset, count)?;
    ensure!(data.len() == count, "truncated THP read");
    Ok(data)
}
/// Read only the fixed header and addressed component metadata. Supports one
/// video component followed by optional mono/stereo, single-track DSP audio.
pub fn read_header(
    file_bytes: u64,
    mut read: impl FnMut(u64, usize) -> Result<Vec<u8>>,
) -> Result<Header> {
    ensure!(file_bytes >= 48, "truncated THP header");
    let header = exact(&mut read, 0, 48)?;
    let b = Bytes(&header);
    ensure!(b.range(0, 4)? == b"THP\0", "invalid THP magic");
    let version = b.u32(4)?;
    ensure!(
        version == 0x10000 || version == 0x11000,
        "unsupported THP version"
    );
    let component_offset = u64::from(b.u32(32)?);
    ensure!(
        component_offset >= 48
            && component_offset
                .checked_add(20)
                .is_some_and(|end| end <= file_bytes),
        "THP component metadata outside file"
    );
    let mut components = exact(&mut read, component_offset, 20)?;
    let c = Bytes(&components);
    let count = c.u32(0)?;
    ensure!(
        (count == 1 && c.range(4, 1)? == [0]) || (count == 2 && c.range(4, 2)? == [0, 1]),
        "unsupported THP component order/types"
    );
    let video_bytes = if version == 0x11000 { 12 } else { 8 };
    let audio_bytes = if count == 2 {
        if version == 0x11000 { 16 } else { 12 }
    } else {
        0
    };
    let info_bytes = video_bytes + audio_bytes;
    ensure!(
        component_offset + 20 + info_bytes as u64 <= file_bytes,
        "THP component metadata outside file"
    );
    components.extend(exact(&mut read, component_offset + 20, info_bytes)?);
    parse_header(&header, &components, file_bytes)
}
fn parse_header(header: &[u8], components: &[u8], file_bytes: u64) -> Result<Header> {
    let b = Bytes(header);
    let c = Bytes(components);
    let version = b.u32(4)?;
    let fps = b.f32(16)?;
    ensure!((0.1..=1000.).contains(&fps), "invalid THP frame rate");
    let frame_count = b.u32(20)?;
    let maximum_frame_bytes = b.u32(8)?;
    let first_frame_bytes = b.u32(24)?;
    let width = c.u32(20)?;
    let height = c.u32(24)?;
    let audio = if c.u32(0)? == 2 {
        let offset = if version == 0x11000 { 32 } else { 28 };
        let channels = c.u32(offset)?;
        let sample_rate = c.u32(offset + 4)?;
        let total_samples = c.u32(offset + 8)?;
        let tracks = if version == 0x11000 {
            c.u32(offset + 12)?
        } else {
            1
        };
        let maximum_frame_samples = b.u32(12)?;
        ensure!(
            (1..=2).contains(&channels)
                && (8000..=192000).contains(&sample_rate)
                && tracks == 1
                && maximum_frame_samples > 0
                && maximum_frame_samples <= 1_000_000,
            "unsupported/invalid THP audio format"
        );
        Some(AudioInfo {
            channels,
            sample_rate,
            total_samples,
            tracks,
            maximum_frame_samples,
        })
    } else {
        None
    };
    ensure!(
        width > 0
            && height > 0
            && width <= 4096
            && height <= 4096
            && u64::from(width) * u64::from(height) <= MAX_PIXELS,
        "THP image dimensions exceed host budget"
    );
    ensure!(
        frame_count > 0 && frame_count <= 1_000_000,
        "THP frame count exceeds host budget"
    );
    ensure!(
        (12..=MAX_FRAME_BYTES).contains(&maximum_frame_bytes)
            && first_frame_bytes >= 12
            && first_frame_bytes <= maximum_frame_bytes,
        "invalid THP frame sizes"
    );
    let first_frame_offset = u64::from(b.u32(40)?);
    let last_frame_offset = u64::from(b.u32(44)?);
    let data_bytes = u64::from(b.u32(28)?);
    let component_end = u64::from(b.u32(32)?) + components.len() as u64;
    ensure!(
        first_frame_offset >= component_end
            && data_bytes >= u64::from(first_frame_bytes)
            && first_frame_offset
                .checked_add(data_bytes)
                .is_some_and(|end| end <= file_bytes)
            && last_frame_offset >= first_frame_offset
            && last_frame_offset < first_frame_offset + data_bytes,
        "THP frame data outside file"
    );
    Ok(Header {
        version,
        fps,
        frame_count,
        width,
        height,
        maximum_frame_bytes,
        first_frame_bytes,
        first_frame_offset,
        last_frame_offset,
        data_bytes,
        file_bytes,
        audio,
    })
}
/// Follow exactly the requested prefix of the frame chain. Prefer frame_index
/// once or FrameCursor for repeated/sequential access instead of repeated scans.
pub fn locate_frame(
    header: &Header,
    index: u32,
    mut read: impl FnMut(u64, usize) -> Result<Vec<u8>>,
) -> Result<Frame> {
    ensure!(
        index < header.frame_count,
        "THP frame index outside bounded sequence"
    );
    let mut cursor = FrameCursor::new(header)?;
    loop {
        let frame = cursor
            .next_frame(&mut read)?
            .context("THP selected frame absent")?;
        if frame.index == index {
            return Ok(frame);
        }
    }
}

/// Sequential header cursor. Errors leave its position unchanged; EOF performs
/// no reads. It owns a small metadata copy and does not retain movie payloads.
pub struct FrameCursor {
    header: Header,
    index: u32,
    offset: u64,
    total: u32,
    previous: Option<u32>,
}
impl FrameCursor {
    pub fn new(header: &Header) -> Result<Self> {
        ensure!(
            header.frame_count > 0 && header.frame_count <= 1_000_000,
            "THP frame count exceeds host budget"
        );
        Ok(Self {
            header: header.clone(),
            index: 0,
            offset: header.first_frame_offset,
            total: header.first_frame_bytes,
            previous: None,
        })
    }
    pub fn next_frame(
        &mut self,
        mut read: impl FnMut(u64, usize) -> Result<Vec<u8>>,
    ) -> Result<Option<Frame>> {
        if self.index == self.header.frame_count {
            return Ok(None);
        }
        let end = self
            .header
            .first_frame_offset
            .checked_add(self.header.data_bytes)
            .context("THP data end overflow")?;
        let header_bytes = if self.header.audio.is_some() { 16 } else { 12 };
        ensure!(
            end <= self.header.file_bytes
                && self.total >= header_bytes
                && self.total <= self.header.maximum_frame_bytes
                && self.total <= MAX_FRAME_BYTES
                && self
                    .offset
                    .checked_add(u64::from(self.total))
                    .is_some_and(|next| next <= end),
            "THP frame outside data span"
        );
        let data = exact(&mut read, self.offset, header_bytes as usize)?;
        let b = Bytes(&data);
        let next_frame_bytes = b.u32(0)?;
        let previous_frame_bytes = b.u32(4)?;
        let video_bytes = b.u32(8)?;
        let audio_bytes = if self.header.audio.is_some() {
            b.u32(12)?
        } else {
            0
        };
        if let Some(previous) = self.previous {
            ensure!(
                previous_frame_bytes == previous,
                "THP previous frame size mismatch"
            );
        }
        ensure!(
            video_bytes >= 4
                && u64::from(video_bytes) + u64::from(audio_bytes)
                    <= u64::from(self.total - header_bytes),
            "THP video/audio payload outside frame"
        );
        if self.index + 1 == self.header.frame_count {
            ensure!(
                self.offset == self.header.last_frame_offset,
                "THP final frame offset differs from header"
            );
        }
        let video_offset = self.offset + u64::from(header_bytes);
        let frame = Frame {
            index: self.index,
            offset: self.offset,
            total_bytes: self.total,
            next_frame_bytes,
            previous_frame_bytes,
            video_offset,
            video_bytes,
            audio_offset: self
                .header
                .audio
                .as_ref()
                .map(|_| video_offset + u64::from(video_bytes)),
            audio_bytes,
        };
        self.index += 1;
        self.offset += u64::from(self.total);
        self.previous = Some(self.total);
        self.total = next_frame_bytes;
        Ok(Some(frame))
    }
}
/// Build the complete checked frame index using exactly one header read/frame.
/// JPEG and audio payloads remain unread until the caller selects them.
pub fn frame_index(
    header: &Header,
    mut read: impl FnMut(u64, usize) -> Result<Vec<u8>>,
) -> Result<Vec<Frame>> {
    let mut cursor = FrameCursor::new(header)?;
    let mut frames = Vec::new();
    frames
        .try_reserve_exact(header.frame_count as usize)
        .context("allocating THP frame index")?;
    while let Some(frame) = cursor.next_frame(&mut read)? {
        frames.push(frame);
    }
    Ok(frames)
}

#[derive(Debug)]
pub struct PcmAudio {
    pub sample_rate: u32,
    pub channels: u32,
    pub samples_per_channel: u32,
    /// Frame-major interleaved signed16 samples, original channel order.
    pub samples: Vec<i16>,
}
/// One movie's sequential DSP state. First decode (including an explicit seek)
/// initializes from that packet's stored histories; following packets retain
/// decoded histories. Reset before restarting or changing movies.
#[derive(Default)]
pub struct AudioDecoder {
    history: Option<Vec<[i16; 2]>>,
    next_index: Option<u32>,
    format: Option<(u32, u32)>,
}
impl AudioDecoder {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn decode(&mut self, header: &Header, frame: &Frame, payload: &[u8]) -> Result<PcmAudio> {
        let audio = header
            .audio
            .as_ref()
            .context("THP has no audio component")?;
        ensure!(
            (1..=2).contains(&audio.channels) && audio.tracks == 1,
            "unsupported THP audio channels/tracks"
        );
        ensure!(
            frame.audio_offset.is_some() && payload.len() == frame.audio_bytes as usize,
            "THP selected audio length differs"
        );
        if let Some(expected) = self.next_index {
            ensure!(
                frame.index == expected,
                "THP audio frames must be sequential; reset before seek"
            );
        }
        if let Some(format) = self.format {
            ensure!(
                format == (audio.sample_rate, audio.channels),
                "THP audio format changed without reset"
            );
        }
        let channels = audio.channels as usize;
        let mut result = PcmAudio {
            sample_rate: audio.sample_rate,
            channels: audio.channels,
            samples_per_channel: 0,
            samples: Vec::new(),
        };
        if payload.is_empty() {
            self.next_index = frame.index.checked_add(1);
            self.format = Some((audio.sample_rate, audio.channels));
            return Ok(result);
        }
        ensure!(
            payload.len() <= MAX_FRAME_BYTES as usize,
            "THP audio packet exceeds host budget"
        );
        let b = Bytes(payload);
        let channel_bytes = b.u32(0)? as usize;
        let count = b.u32(4)?;
        ensure!(
            count > 0 && count <= audio.maximum_frame_samples && count <= 1_000_000,
            "invalid THP audio sample count"
        );
        let data_offset = 8 + channels * 36;
        let minimum_bytes = (count as usize / 14) * 8
            + if count % 14 == 0 {
                0
            } else {
                1 + (count as usize % 14).div_ceil(2)
            };
        ensure!(
            channel_bytes >= minimum_bytes,
            "THP audio channel shorter than sample count"
        );
        b.range(
            data_offset,
            channel_bytes
                .checked_mul(channels)
                .context("THP audio channel size overflow")?,
        )?;
        let mut history = if let Some(history) = &self.history {
            ensure!(
                history.len() == channels,
                "THP audio history channel count differs"
            );
            history.clone()
        } else {
            (0..channels)
                .map(|channel| {
                    Ok([
                        b.u16(8 + channels * 32 + channel * 4)? as i16,
                        b.u16(10 + channels * 32 + channel * 4)? as i16,
                    ])
                })
                .collect::<Result<Vec<_>>>()?
        };
        let mut decoded = Vec::new();
        for (channel, prior) in history.iter_mut().enumerate() {
            let mut coefficients = [0i16; 16];
            for (index, value) in coefficients.iter_mut().enumerate() {
                *value = b.u16(8 + channel * 32 + index * 2)? as i16;
            }
            let data = b.range(data_offset + channel * channel_bytes, channel_bytes)?;
            let (samples, next) = decode_dsp_channel(data, count as usize, coefficients, *prior)?;
            *prior = next;
            decoded.push(samples);
        }
        result
            .samples
            .try_reserve(count as usize * channels)
            .context("allocating THP interleaved audio")?;
        for index in 0..count as usize {
            for channel in &decoded {
                result.samples.push(channel[index]);
            }
        }
        result.samples_per_channel = count;
        // Publish decoder state only after every channel decoded successfully.
        self.history = Some(history);
        self.next_index = frame.index.checked_add(1);
        self.format = Some((audio.sample_rate, audio.channels));
        Ok(result)
    }
}
/// Independently decode one packet using its own stored predictor histories.
/// For continuous playback retain AudioDecoder instead of resetting each frame.
pub fn decode_audio(header: &Header, frame: &Frame, payload: &[u8]) -> Result<PcmAudio> {
    AudioDecoder::default().decode(header, frame, payload)
}
fn decode_dsp_channel(
    data: &[u8],
    count: usize,
    coefficients: [i16; 16],
    mut history: [i16; 2],
) -> Result<(Vec<i16>, [i16; 2])> {
    ensure!(count <= 1_000_000, "DSP sample budget exceeded");
    let mut output = Vec::new();
    output
        .try_reserve(count)
        .context("allocating DSP channel samples")?;
    for index in 0..count {
        let block = index / 14;
        let ordinal = index % 14;
        let header = *data.get(block * 8).context("truncated DSP block header")?;
        let pair = usize::from((header >> 4) & 7) * 2;
        let exponent = header & 15;
        let packed = *data
            .get(block * 8 + 1 + ordinal / 2)
            .context("truncated DSP sample nibble")?;
        let nibble = if ordinal % 2 == 0 {
            packed >> 4
        } else {
            packed & 15
        };
        let signed = if nibble >= 8 {
            i64::from(nibble) - 16
        } else {
            i64::from(nibble)
        };
        let prediction = (i64::from(history[0]) * i64::from(coefficients[pair])
            + i64::from(history[1]) * i64::from(coefficients[pair + 1]))
            >> 11;
        let value = (prediction + signed * (1i64 << exponent))
            .clamp(i64::from(i16::MIN), i64::from(i16::MAX)) as i16;
        output.push(value);
        history = [value, history[0]];
    }
    Ok((output, history))
}

/// Convert a baseline THP JPEG-like payload into a standard JPEG. Marker
/// segments before SOS are retained; raw entropy FF bytes are stuffed. The final
/// EOI is identified after stripping only zero alignment padding, so marker-like
/// bytes inside the raw scan remain entropy data.
pub fn jpeg_payload(payload: &[u8]) -> Result<Vec<u8>> {
    ensure!(
        payload.len() <= MAX_FRAME_BYTES as usize,
        "THP JPEG payload exceeds host budget"
    );
    let b = Bytes(payload);
    ensure!(b.range(0, 2)? == [0xff, 0xd8], "THP video missing JPEG SOI");
    let mut position = 2;
    let mut baseline = false;
    let scan = loop {
        let marker = b.range(position, 4)?;
        ensure!(marker[0] == 0xff, "THP JPEG marker boundary invalid");
        let kind = marker[1];
        let length = usize::from(b.u16(position + 2)?);
        ensure!(length >= 2, "THP JPEG marker length invalid");
        b.range(position + 2, length)?;
        if kind == 0xc0 {
            baseline = true;
        } else if (0xc1..=0xcf).contains(&kind) && kind != 0xc4 {
            anyhow::bail!("unsupported THP JPEG frame coding");
        }
        if kind == 0xdd {
            ensure!(
                length == 4 && b.u16(position + 4)? == 0,
                "THP JPEG restart intervals unsupported"
            );
        }
        position = position
            .checked_add(2 + length)
            .context("THP JPEG marker offset overflow")?;
        if kind == 0xda {
            ensure!(baseline, "THP JPEG lacks baseline frame header");
            break position;
        }
        ensure!(position < payload.len(), "THP JPEG missing scan");
    };
    let end = payload
        .iter()
        .rposition(|byte| *byte != 0)
        .context("THP JPEG empty payload")?
        + 1;
    ensure!(
        end >= scan + 2 && b.range(end - 2, 2)? == [0xff, 0xd9],
        "THP JPEG missing final EOI"
    );
    let entropy = &payload[scan..end - 2];
    let stuffed = entropy.iter().filter(|&&byte| byte == 0xff).count();
    let mut jpeg = Vec::new();
    jpeg.try_reserve(end + stuffed)
        .context("allocating adapted THP JPEG")?;
    jpeg.extend_from_slice(&payload[..scan]);
    for &byte in entropy {
        jpeg.push(byte);
        if byte == 0xff {
            jpeg.push(0);
        }
    }
    jpeg.extend_from_slice(&[0xff, 0xd9]);
    Ok(jpeg)
}
pub fn decode_frame(header: &Header, frame: &Frame, payload: &[u8]) -> Result<RgbaImage> {
    ensure!(
        payload.len() == frame.video_bytes as usize,
        "THP selected video length differs"
    );
    let jpeg = jpeg_payload(payload)?;
    let mut decoder = Decoder::new(Cursor::new(jpeg));
    decoder.set_max_decoding_buffer_size((MAX_PIXELS * 4) as usize);
    decoder
        .read_info()
        .context("reading adapted THP JPEG header")?;
    let info = decoder.info().context("THP JPEG metadata missing")?;
    ensure!(
        u32::from(info.width) == header.width && u32::from(info.height) == header.height,
        "THP component/JPEG dimensions differ"
    );
    ensure!(
        matches!(info.pixel_format, PixelFormat::RGB24 | PixelFormat::L8),
        "unsupported THP JPEG pixel format"
    );
    let decoded = decoder.decode().context("decoding adapted THP JPEG")?;
    let channels = info.pixel_format.pixel_bytes();
    let pixels = header.width as usize * header.height as usize;
    ensure!(
        decoded.len() == pixels * channels,
        "THP decoded pixel count differs"
    );
    let mut rgba = Vec::new();
    rgba.try_reserve(pixels * 4)
        .context("allocating THP RGBA pixels")?;
    for pixel in decoded.chunks_exact(channels) {
        if channels == 1 {
            rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], 255]);
        } else {
            rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
        }
    }
    Ok(RgbaImage {
        width: header.width,
        height: header.height,
        pixels: rgba,
    })
}

#[cfg(test)]
#[path = "tests/thp.rs"]
mod tests;
