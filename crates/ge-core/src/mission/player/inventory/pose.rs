use super::Metadata;
use anyhow::{Result, ensure};

#[derive(Debug, Clone, Copy)]
pub struct PoseInputs {
    pub blend_bits: u32,
    pub c54_raw: Option<u8>,
    pub player_flags_raw: u16,
    pub flag_5ee_raw: u8,
    pub flag_95c_raw: u8,
    pub multiplayer_raw: u8,
}

#[allow(clippy::manual_clamp)] // Source ordered comparisons turn -0 into +0.
pub(super) fn normal(
    metadata: &Metadata,
    inputs: PoseInputs,
    retained: &mut Option<[u32; 5]>,
) -> Result<[u32; 5]> {
    ensure!(
        inputs.multiplayer_raw == 0
            && inputs.blend_bits == 0
            && inputs.player_flags_raw & 0x400 == 0
            && inputs.flag_5ee_raw == 0
            && inputs.flag_95c_raw == 0,
        "unsupported fresh normal weapon-values inputs"
    );
    let mut values = metadata.normal_values_bits.map(f32::from_bits);
    ensure!(
        values.iter().all(|value| value.is_finite()),
        "nonfinite normal weapon values"
    );
    let scale = f32::from_bits(0x3c23_d70a);
    for index in [0, 1, 2, 5, 6, 7] {
        values[index] *= scale;
    }
    let blend = f32::from_bits(inputs.blend_bits);
    let mut result = [0.; 5];
    for index in 0..5 {
        let difference = values[index + 5] - values[index];
        let scaled = blend * difference;
        result[index] = values[index] + scaled;
    }
    // The five initial stores precede the conditional c54 read. If that input
    // is unresolved, these reached writes remain owned without selecting an arm.
    *retained = Some(result.map(f32::to_bits));
    if inputs.c54_raw != Some(0) {
        let modifier = f32::from_bits(metadata.modifier_bits[0]);
        ensure!(
            modifier.is_finite(),
            "nonfinite conditional weapon modifier"
        );
        let mut modified = result;
        modified[0] *= modifier;
        modified[1] *= modifier;
        if modifier != 0. {
            modified[4] *= 1. / modifier;
        }
        if inputs.c54_raw.is_none() {
            ensure!(
                result.map(f32::to_bits) == modified.map(f32::to_bits),
                "unknown c54 changes reached weapon values"
            );
        }
        result = modified;
        *retained = Some(result.map(f32::to_bits));
    }
    ensure!(
        result.iter().all(|value| value.is_finite()),
        "weapon values arithmetic overflow"
    );
    // Source first clamps output1, then output0. Ordered comparisons preserve
    // the original zero behavior; do not replace with a NaN-normalizing rule.
    for index in [1, 0] {
        let value = result[index];
        result[index] = if value >= 1. {
            1.
        } else if value <= 0. {
            0.
        } else {
            value
        };
    }
    Ok(result.map(f32::to_bits))
}
