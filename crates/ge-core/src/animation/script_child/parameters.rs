//! Direct class-four parameter writes. Changing speed does not resolve a file,
//! advance a script, sample a pose or replace the retained update history.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpeedWrite {
    pub previous_bits: u32,
    pub current_bits: u32,
    pub direction_changed: bool,
}
impl SpeedWrite {
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        emit(self.previous_bits);
        emit(self.current_bits);
        emit(u32::from(self.direction_changed));
    }
}

impl Store {
    pub(crate) fn set_speed(&mut self, id: NodeId, bits: u32) -> Result<SpeedWrite> {
        let h = self
            .headers
            .get_mut(&id)
            .context("script speed owner absent")?;
        ensure!(
            h.initializer_returned(),
            "script speed write precedes factory return"
        );
        let next = sign(bits);
        let previous = h.word(0x2c).context("previous script speed unknown")?;
        let changed = next != sign(previous);
        if changed {
            // Both source values are loaded before the first history store.
            let position = h
                .word(0x28)
                .context("script speed history position unknown")?;
            let phase = h.word(0x80).context("script speed history phase unknown")?;
            h.bytes[0x77] = Some(next as u8);
            h.w(0x68, position);
            h.w(0x84, phase);
        }
        h.w(0x2c, bits);
        Ok(SpeedWrite {
            previous_bits: previous,
            current_bits: bits,
            direction_changed: changed,
        })
    }
}

fn sign(bits: u32) -> i8 {
    let value = f32::from_bits(bits);
    // The native ordered comparison gives both zeros and NaN sign zero.
    if value > 0.0 {
        1
    } else if value < 0.0 {
        -1
    } else {
        0
    }
}

/// The two virtual-speed arguments for a supplied blend and reference bases.
/// Keep both divisions and every intervening operation separately rounded.
/// This helper performs no animation/FPS lookup and does not assume a unit rate.
pub fn blended_speeds(blend_bits: u32, first_basis_bits: u32, second_basis_bits: u32) -> [u32; 2] {
    let blend = f32::from_bits(blend_bits);
    let first = f32::from_bits(first_basis_bits);
    let second = f32::from_bits(second_basis_bits);
    let first_ratio = first / second;
    let second_ratio = second / first;
    let first_delta = first_ratio - 1.0f32;
    let second_delta = 1.0f32 - second_ratio;
    let first_term = blend * first_delta;
    let second_term = blend * second_delta;
    [
        (1.0f32 + first_term).to_bits(),
        (second_term + second_ratio).to_bits(),
    ]
}

#[cfg(test)]
#[path = "parameters/tests.rs"]
mod tests;
