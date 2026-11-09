//! Stored instance bounds, their native affine update, and ordered component union.
use super::world::StoredBounds;
use anyhow::{Result, ensure};
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Bounds {
    pub minimum: [f32; 4],
    pub maximum: [f32; 4],
}
impl Bounds {
    /// Native missing-entity result is a finite inverted sentinel, not infinity,
    /// an identity-sized box, or permission to treat the world as clear.
    pub const MISSING_ENTITY: Self = Self {
        minimum: [1_000_000., 1_000_000., 1_000_000., 0.],
        maximum: [-1_000_000., -1_000_000., -1_000_000., -0.],
    };

    /// Each endpoint product and add rounds independently. Translation is the
    /// initial accumulator, followed by X, Y, Z. Input W/homogeneous row are unread.
    pub fn transformed(self, matrix: &[f32; 16]) -> Result<Self> {
        let mut result = Self {
            minimum: [0.; 4],
            maximum: [0.; 4],
        };
        for row in 0..3 {
            let translation = matrix[12 + row];
            ensure!(translation.is_finite(), "nonfinite bounds translation");
            let mut minimum = translation;
            let mut maximum = translation;
            for col in 0..3 {
                let coefficient = matrix[col * 4 + row];
                let first = coefficient * self.minimum[col];
                let second = coefficient * self.maximum[col];
                let difference = second - first;
                ensure!(
                    coefficient.is_finite()
                        && self.minimum[col].is_finite()
                        && self.maximum[col].is_finite()
                        && first.is_finite()
                        && second.is_finite()
                        && difference.is_finite(),
                    "nonfinite bounds endpoint product/difference"
                );
                let (low, high) = if difference >= 0. {
                    (first, second)
                } else {
                    (second, first)
                };
                minimum += low;
                maximum += high;
                ensure!(
                    minimum.is_finite() && maximum.is_finite(),
                    "bounds accumulation overflow"
                );
            }
            result.minimum[row] = minimum;
            result.maximum[row] = maximum;
        }
        Ok(result)
    }

    /// Paired subtract/select over all four lanes. A minimum tie chooses the
    /// incoming operand; a maximum tie retains the existing operand, including zeros.
    pub fn union(self, incoming: Self) -> Result<Self> {
        let mut result = self;
        for lane in 0..4 {
            let low_difference = self.minimum[lane] - incoming.minimum[lane];
            let high_difference = self.maximum[lane] - incoming.maximum[lane];
            ensure!(
                [
                    self.minimum[lane],
                    incoming.minimum[lane],
                    self.maximum[lane],
                    incoming.maximum[lane],
                    low_difference,
                    high_difference
                ]
                .into_iter()
                .all(f32::is_finite),
                "nonfinite bounds union value/difference"
            );
            result.minimum[lane] = if low_difference >= 0. {
                incoming.minimum[lane]
            } else {
                self.minimum[lane]
            };
            result.maximum[lane] = if high_difference >= 0. {
                self.maximum[lane]
            } else {
                incoming.maximum[lane]
            };
        }
        Ok(result)
    }

    pub fn contains_with_margin(self, origin: [f32; 3], margin: f32) -> Result<bool> {
        StoredBounds {
            minimum: self.minimum[..3].try_into().unwrap(),
            maximum: self.maximum[..3].try_into().unwrap(),
        }
        .contains_with_margin(origin, margin)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EntityBounds {
    pub base: Bounds,
    pub component_mask: u8,
}

/// Runtime placement update: base bounds, masked component slots 0..4 in order,
/// then optional slot-six aggregate bounds. Slot-six bounds require their own
/// established shape aggregation; they are not an ordinary entity header.
pub fn prepare_instance_bounds(
    matrix: &[f32; 16],
    entity: Option<EntityBounds>,
    mut component: impl FnMut(u8) -> Result<Bounds>,
    special: impl FnOnce() -> Result<Option<Bounds>>,
) -> Result<Bounds> {
    let Some(entity) = entity else {
        return Ok(Bounds::MISSING_ENTITY);
    };
    let mut result = entity.base.transformed(matrix)?;
    for slot in 0..5 {
        if entity.component_mask & (1 << slot) != 0 {
            result = result.union(component(slot)?.transformed(matrix)?)?;
        }
    }
    if entity.component_mask & 0x40 != 0
        && let Some(bounds) = special()?
    {
        result = result.union(bounds.transformed(matrix)?)?;
    }
    Ok(result)
}

#[cfg(test)]
#[path = "../tests/collision_bounds.rs"]
mod tests;
