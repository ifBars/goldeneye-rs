use super::{
    Metrics,
    manifest::{Case, Sample},
    receipt::Failure,
};
use std::io::Cursor;

const MAX_OUTPUT: usize = 64 * 1024 * 1024;
struct Image {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}
fn decode(bytes: &[u8]) -> Result<Image, Failure> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_limits(png::Limits { bytes: MAX_OUTPUT });
    let mut reader = decoder
        .read_info()
        .map_err(|_| Failure::invalid("invalid_png"))?;
    let info = reader.info();
    if info.width == 0 || info.height == 0 || info.width > 4096 || info.height > 4096 {
        return Err(Failure::invalid("image_dimensions_exceed_limit"));
    }
    if info.bit_depth != png::BitDepth::Eight
        || !matches!(info.color_type, png::ColorType::Rgb | png::ColorType::Rgba)
        || info.animation_control.is_some()
        || info.trns.is_some()
    {
        return Err(Failure::unsupported("unsupported_png_format"));
    }
    let size = reader
        .output_buffer_size()
        .filter(|n| *n <= MAX_OUTPUT)
        .ok_or_else(|| Failure::invalid("image_output_exceeds_limit"))?;
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(size)
        .map_err(|_| Failure::invalid("image_allocation_failed"))?;
    pixels.resize(size, 0);
    let frame = reader
        .next_frame(&mut pixels)
        .map_err(|_| Failure::invalid("invalid_png_pixels"))?;
    reader
        .finish()
        .map_err(|_| Failure::invalid("invalid_png_end"))?;
    pixels.truncate(frame.buffer_size());
    let rgba = if frame.color_type == png::ColorType::Rgb {
        let size = (frame.width as usize)
            .checked_mul(frame.height as usize)
            .and_then(|n| n.checked_mul(4))
            .filter(|n| *n <= MAX_OUTPUT)
            .ok_or_else(|| Failure::invalid("image_output_exceeds_limit"))?;
        let mut rgba = Vec::new();
        rgba.try_reserve_exact(size)
            .map_err(|_| Failure::invalid("image_allocation_failed"))?;
        for rgb in pixels.chunks_exact(3) {
            rgba.extend_from_slice(rgb);
            rgba.push(255);
        }
        rgba
    } else {
        pixels
    };
    Ok(Image {
        width: frame.width,
        height: frame.height,
        rgba,
    })
}
pub(super) fn compare(
    reference: &[u8],
    candidate: &[u8],
    sample: &Sample,
    case: &Case,
) -> Result<(Metrics, bool), Failure> {
    let a = decode(reference)?;
    let b = decode(candidate)?;
    if [a.width, b.width] != [sample.width; 2] || [a.height, b.height] != [sample.height; 2] {
        return Err(Failure::failed("dimension_mismatch"));
    }
    let roi = sample.region();
    let mut sum = 0u64;
    let mut maximum = 0u8;
    let mut bad = 0u64;
    let mut signal = [0u64; 2];
    let mut minimum_rgb = [[u8::MAX; 3]; 2];
    let mut maximum_rgb = [[u8::MIN; 3]; 2];
    for y in roi.y..roi.y + roi.height {
        for x in roi.x..roi.x + roi.width {
            let at = ((y as usize) * a.width as usize + x as usize) * 4;
            let aa = &a.rgba[at..at + 4];
            let bb = &b.rgba[at..at + 4];
            let mut pixel_max = 0;
            for (&r, &c) in aa.iter().zip(bb) {
                let d = r.abs_diff(c);
                sum += u64::from(d);
                maximum = maximum.max(d);
                pixel_max = pixel_max.max(d);
            }
            if pixel_max > case.tolerances.bad_pixel_delta {
                bad += 1;
            }
            for (i, p) in [aa, bb].into_iter().enumerate() {
                for (channel, &value) in p[..3].iter().enumerate() {
                    minimum_rgb[i][channel] = minimum_rgb[i][channel].min(value);
                    maximum_rgb[i][channel] = maximum_rgb[i][channel].max(value);
                }
                if p[3] > 8 && p[..3].iter().any(|n| *n > 8) {
                    signal[i] += 1;
                }
            }
        }
    }
    let pixels = u64::from(roi.width) * u64::from(roi.height);
    let spatial_range: [u8; 2] = std::array::from_fn(|side| {
        (0..3)
            .map(|channel| maximum_rgb[side][channel] - minimum_rgb[side][channel])
            .max()
            .expect("three RGB channels")
    });
    let metrics = Metrics {
        pixels,
        mae: sum as f64 / (pixels * 4) as f64,
        max_channel_error: maximum,
        bad_pixel_fraction: bad as f64 / pixels as f64,
        reference_nonblack_fraction: signal[0] as f64 / pixels as f64,
        candidate_nonblack_fraction: signal[1] as f64 / pixels as f64,
        reference_spatial_range: spatial_range[0],
        candidate_spatial_range: spatial_range[1],
    };
    let passed = metrics.mae <= case.tolerances.max_mae
        && metrics.bad_pixel_fraction <= case.tolerances.max_bad_pixel_fraction
        && metrics.reference_nonblack_fraction >= sample.minimum_nonblack_fraction
        && metrics.candidate_nonblack_fraction >= sample.minimum_nonblack_fraction
        && metrics.reference_spatial_range >= sample.minimum_spatial_range
        && metrics.candidate_spatial_range >= sample.minimum_spatial_range;
    Ok((metrics, passed))
}
