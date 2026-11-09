use anyhow::{Context, Result, ensure};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlendInputs {
    /// Pointee of child+0x60, separate from child+0x28 normalized position.
    pub scalar_bits: u32,
    pub child_flags_raw: u32,
    /// Component+0x135. This is an explicit snapshot, not inferred history.
    pub component_flags_raw: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlendConstants {
    pub minimum_span_bits: u32,
    pub minimum_fraction_bits: u32,
    pub maximum_fraction_bits: u32,
    pub snap_fraction_bits: u32,
    pub pair_threshold_bits: u32,
    pub minimum_index: usize,
}

impl BlendConstants {
    /// Observed SJBE52r0 constants/global initialization, explicitly selected by
    /// the caller. This is not a default for arbitrary later native history.
    pub fn source_initial() -> Self {
        Self {
            minimum_span_bits: 0x3400_0000,
            minimum_fraction_bits: 0,
            maximum_fraction_bits: 0x3f80_0000,
            snap_fraction_bits: 0x3f7f_be77,
            pair_threshold_bits: 0x3a83_126f,
            minimum_index: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub source_index: usize,
    /// Present only when the sampler actually needs an adjacent clip request.
    pub second_index: Option<usize>,
    pub fraction_bits: u32,
    pub key_reads: usize,
}

/// Source strict ordered lookup. Repeated references/keys remain separate
/// entries. Only reached keys are validated; count one reads neither keys nor
/// scalar. Nonfinite reached arithmetic is an explicit supported-host limit,
/// not an invented native validation or a successful fallback selection.
pub fn select(
    keys: &[u32],
    inputs: BlendInputs,
    constants: BlendConstants,
    key_budget: usize,
) -> Result<Selection> {
    ensure!(!keys.is_empty(), "unsupported empty normal clip list");
    ensure!(
        keys.len() <= usize::from(u8::MAX),
        "clip count exceeds native byte representation"
    );
    // The sampler checks this threshold even after the helper's direct return.
    let pair_threshold = finite(constants.pair_threshold_bits, "clip pair threshold")?;
    ensure!(
        (0.0..=1.0).contains(&pair_threshold),
        "unsupported clip pair threshold"
    );
    if keys.len() == 1 {
        return Ok(Selection {
            source_index: 0,
            second_index: None,
            fraction_bits: 0,
            key_reads: 0,
        });
    }
    let scalar = finite(inputs.scalar_bits, "blend scalar")?;
    let mut reads = 0;
    let mut lower = 0;
    for ordinal in 0..keys.len() {
        let key = read_key(keys, ordinal, &mut reads, key_budget)?;
        if key <= scalar {
            lower = ordinal;
            continue;
        }
        let minimum_span = finite(constants.minimum_span_bits, "minimum clip span")?;
        let minimum = finite(constants.minimum_fraction_bits, "minimum clip fraction")?;
        let maximum = finite(constants.maximum_fraction_bits, "maximum clip fraction")?;
        let snap = finite(constants.snap_fraction_bits, "clip snap threshold")?;
        ensure!(
            minimum_span > 0.0
                && (0.0..=1.0).contains(&minimum)
                && (minimum..=1.0).contains(&maximum)
                && (minimum..=maximum).contains(&snap)
                && constants.minimum_index < keys.len(),
            "unsupported clip selection constants"
        );
        // Native rereads these fields in this order; budget counts source reads,
        // not merely distinct keys. Keep each single-precision subtraction.
        let low = read_key(keys, lower, &mut reads, key_budget)?;
        let high = read_key(keys, lower + 1, &mut reads, key_budget)?;
        let span = high - low;
        ensure!(span.is_finite(), "unsupported nonfinite clip span");
        let denominator = if minimum_span >= span {
            minimum_span
        } else {
            span
        };
        let low = read_key(keys, lower, &mut reads, key_budget)?;
        let numerator = scalar - low;
        let raw = numerator / denominator;
        ensure!(
            numerator.is_finite() && raw.is_finite(),
            "unsupported nonfinite clip fraction"
        );
        let mut fraction = if raw >= maximum {
            maximum
        } else if raw <= minimum {
            minimum
        } else {
            raw
        };
        if fraction >= snap {
            lower += 1;
            fraction = 0.0;
        }
        if inputs.child_flags_raw & 0x80 != 0 {
            fraction = 0.0;
        }
        lower = if lower >= keys.len() - 1 {
            keys.len() - 1
        } else {
            lower.max(constants.minimum_index)
        };
        if inputs.component_flags_raw & 4 != 0 {
            fraction = 0.0;
        }
        let second_index = if fraction > pair_threshold {
            ensure!(
                lower + 1 < keys.len(),
                "selected adjacent clip is out of range"
            );
            Some(lower + 1)
        } else {
            None
        };
        return Ok(Selection {
            source_index: lower,
            second_index,
            fraction_bits: fraction.to_bits(),
            key_reads: reads,
        });
    }
    Ok(Selection {
        source_index: lower,
        second_index: None,
        fraction_bits: 0,
        key_reads: reads,
    })
}

fn read_key(keys: &[u32], index: usize, reads: &mut usize, budget: usize) -> Result<f32> {
    ensure!(*reads < budget, "clip key-read budget exhausted");
    *reads += 1;
    finite(
        *keys.get(index).context("clip blend key is out of range")?,
        "clip blend key",
    )
}

pub(super) fn finite(bits: u32, label: &str) -> Result<f32> {
    let value = f32::from_bits(bits);
    ensure!(value.is_finite(), "unsupported nonfinite {label}");
    Ok(value)
}
