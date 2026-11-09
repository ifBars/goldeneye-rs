//! Explicit raw type-102 frame decoding, before root bias, format conversion,
//! skin mapping, blending or skeleton commit. The first supported format is
//! 1..128 frames with no auxiliary channels. Host f32/FMA ordering follows the
//! source polynomial path; this does not establish native floating-point parity.
mod stream;
mod types;
pub use types::{Budget, DecodedFrame, Layout, Precision, Rotation, Translation, Work};

use anyhow::{Context, Result, ensure};
use stream::Stream;

pub fn decode(
    payload: &[u8],
    layout: Layout,
    frame: u32,
    requested_mask: [u32; 4],
    budget: Budget,
) -> Result<DecodedFrame> {
    ensure!(
        (1..=128).contains(&layout.frame_count),
        "unsupported skeletal frame count; supported range is 1..128"
    );
    ensure!(
        frame < u32::from(layout.frame_count),
        "skeletal frame index is out of range"
    );
    ensure!(
        layout.rotation_count > 0 && layout.rotation_count <= 128,
        "unsupported skeletal rotation count"
    );
    ensure!(
        layout.auxiliary_count == 0,
        "unsupported skeletal auxiliary channels"
    );
    for bone in u16::from(layout.rotation_count)..128 {
        ensure!(
            !bit(layout.translation_mask, bone),
            "skeletal translation mask exceeds rotation channels"
        );
    }
    let translations = layout
        .translation_mask
        .iter()
        .map(|n| n.count_ones() as usize)
        .sum::<usize>();
    let scalars = (usize::from(layout.rotation_count) + translations)
        .checked_mul(3)
        .context("skeletal scalar count overflow")?;
    ensure!(
        scalars <= budget.scalar_records,
        "skeletal scalar record budget exhausted"
    );
    let mut stream = Stream::new(payload, budget)?;
    let mut result = DecodedFrame {
        frame,
        rotations: Vec::new(),
        translations: Vec::new(),
        valid_mask: [0; 4],
        work: Work::default(),
    };
    result
        .rotations
        .try_reserve_exact(usize::from(layout.rotation_count))
        .context("allocating bounded skeletal rotations")?;
    result
        .translations
        .try_reserve_exact(translations)
        .context("allocating bounded skeletal translations")?;
    let t = frame as f32 / f32::from(layout.frame_count);
    let mut powers = [t; 5];
    for i in 1..5 {
        powers[i] = t * powers[i - 1];
    }
    for bone in 0..u16::from(layout.rotation_count) {
        let requested = bit(requested_mask, bone);
        if bit(layout.translation_mask, bone) {
            let mut values = [0; 4];
            for lane in values.iter_mut().take(3) {
                *lane = stream
                    .scalar(frame, powers, requested)?
                    .unwrap_or(0.0)
                    .to_bits();
            }
            values[3] = 1.0f32.to_bits();
            if requested {
                result.translations.push(Translation {
                    bone_index: bone,
                    translation_bits: values,
                });
            }
        }
        let mut values = [0.0; 3];
        for value in &mut values {
            *value = stream.scalar(frame, powers, requested)?.unwrap_or(0.0);
        }
        if requested {
            let (quaternion_bits, precision) = quaternion(values);
            result.rotations.push(Rotation {
                bone_index: bone,
                quaternion_bits,
                precision,
            });
            result.valid_mask[usize::from(bone / 32)] |= 1 << (bone % 32);
        }
    }
    result.work = stream.work();
    Ok(result)
}

fn bit(mask: [u32; 4], bone: u16) -> bool {
    mask[usize::from(bone / 32)] & (1 << (bone % 32)) != 0
}

fn quaternion([x, y, packed_z]: [f32; 3]) -> ([u32; 4], Precision) {
    let negative = packed_z >= 2.0;
    let z = if negative { packed_z - 4.0 } else { packed_z };
    // Source paired lanes: x*x and y*y, then z*z + x*x (fused), then
    // lane sum. The temporary fourth lane is positive zero.
    let xx = x * x;
    let yy = y * y;
    let squared = z.mul_add(z, xx) + yy;
    let radicand = 1.0 - squared;
    let (w, precision) = if radicand <= 0.0 {
        (radicand, Precision::ExactBranch)
    } else if radicand <= f32::from_bits(0x3400_0000) {
        (if negative { -0.0f32 } else { 0.0 }, Precision::ExactBranch)
    } else {
        // Deliberately labelled host approximation. Ordinary sqrt is NOT an
        // implementation of PPC frsqrte plus the source's Newton refinement.
        let root = radicand.sqrt();
        (
            if negative { -root } else { root },
            Precision::HostSqrtUnverified,
        )
    };
    (
        [x.to_bits(), y.to_bits(), z.to_bits(), w.to_bits()],
        precision,
    )
}

#[cfg(test)]
mod tests;
