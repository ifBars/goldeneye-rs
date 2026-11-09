//! Authored collected-flag probes over initial placement matrices, not live records.
use anyhow::{Result, ensure};
use ge_assets::scene::Placement;
use ge_core::collision::frame::CollisionFrame;
use serde::Serialize;

#[derive(Default, Serialize)]
pub struct Audit {
    pub maps: usize,
    pub placements: usize,
    pub rigid_frames: usize,
    pub general_frames: usize,
    pub singular_general_frames: usize,
    pub ill_conditioned_general_frames: usize,
    pub validated_general_frames: usize,
    pub strict_inverse_frames: usize,
    pub rounding_inverse_frames: usize,
    pub query_probes: usize,
    pub strict_queries: usize,
    pub rounding_queries: usize,
    pub unvalidated_queries: usize,
    pub maximum_inverse_error_f64: f64,
    pub maximum_query_error_f64: f64,
}
impl Audit {
    pub fn include(&mut self, placements: &[Placement]) -> Result<()> {
        self.maps += 1;
        for p in placements {
            self.placements += 1;
            // These are two authored collected-record modes. Engine flag 0x4
            // is not a proven binding to final collected-record flag 0x2.
            for flags in [0, 2] {
                let frame = CollisionFrame::from_record(&p.model, flags)?;
                let expected = if flags == 0 {
                    self.rigid_frames += 1;
                    Some(rigid_reference(&p.model))
                } else {
                    self.general_frames += 1;
                    let inverse = inverse_reference(&p.model);
                    if frame.general_determinant == Some(0.) {
                        self.singular_general_frames += 1;
                        // Initial placements are affine. Exact zero in the
                        // native determinant must not conceal an invertible
                        // independent matrix in this corpus diagnostic.
                        ensure!(
                            inverse.is_none(),
                            "zero f32 determinant on invertible placement"
                        );
                        ensure!(
                            frame.local_from_world.iter().all(|&v| v == 0.),
                            "singular affine inverse is nonzero"
                        );
                        None
                    } else if let Some(inverse) = inverse {
                        if basis_norm(&p.model.map(f64::from)) * basis_norm(&inverse) > 10_000. {
                            self.ill_conditioned_general_frames += 1;
                            None
                        } else {
                            self.validated_general_frames += 1;
                            Some(inverse)
                        }
                    } else {
                        anyhow::bail!("nonzero f32 determinant on singular placement");
                    }
                };
                if let Some(expected) = expected {
                    let error = compare(&frame.local_from_world, &expected, 32.)?;
                    self.maximum_inverse_error_f64 = self.maximum_inverse_error_f64.max(error);
                    if error <= 0.00001 {
                        self.strict_inverse_frames += 1;
                    } else {
                        self.rounding_inverse_frames += 1;
                    }
                }
                for offset in [
                    [0.; 3],
                    [1., 0., 0.],
                    [0., 1., 0.],
                    [0., 0., 1.],
                    [1., 2., 3.],
                ] {
                    let world = std::array::from_fn(|i| p.position[i] + offset[i]);
                    let actual = frame.query_origin(world, p.position)?;
                    self.query_probes += 1;
                    let Some(inverse) = expected else {
                        self.unvalidated_queries += 1;
                        continue;
                    };
                    // Preserve the input's f32 subtraction, then compare the
                    // entire binding with independently computed f64 inverse.
                    let delta =
                        std::array::from_fn::<_, 3, _>(|i| f64::from(world[i] - p.position[i]));
                    let mut error = 0f64;
                    for row in 0..4 {
                        let terms = std::array::from_fn::<_, 3, _>(|col| {
                            inverse[col * 4 + row] * delta[col]
                        });
                        let reference = terms.iter().sum::<f64>();
                        let difference = (f64::from(actual[row]) - reference).abs();
                        let bound = 0.00001
                            + 64.
                                * f64::from(f32::EPSILON)
                                * terms.into_iter().map(f64::abs).sum::<f64>();
                        ensure!(
                            difference <= bound,
                            "frame query error {difference} exceeds {bound}"
                        );
                        error = error.max(difference);
                    }
                    self.maximum_query_error_f64 = self.maximum_query_error_f64.max(error);
                    if error <= 0.00001 {
                        self.strict_queries += 1;
                    } else {
                        self.rounding_queries += 1;
                    }
                }
            }
        }
        Ok(())
    }
}
fn compare(actual: &[f32; 16], expected: &[f64; 16], ulps: f64) -> Result<f64> {
    let mut error = 0f64;
    for (&actual, &expected) in actual.iter().zip(expected) {
        let difference = (f64::from(actual) - expected).abs();
        let bound = 0.00001 + ulps * f64::from(f32::EPSILON) * expected.abs();
        ensure!(
            difference <= bound,
            "frame inverse error {difference} exceeds {bound}"
        );
        error = error.max(difference);
    }
    Ok(error)
}
fn basis_norm(matrix: &[f64; 16]) -> f64 {
    (0..3)
        .map(|row| (0..3).map(|col| matrix[col * 4 + row].abs()).sum::<f64>())
        .fold(0f64, f64::max)
}
fn rigid_reference(matrix: &[f32; 16]) -> [f64; 16] {
    let mut inverse = [0.; 16];
    for col in 0..3 {
        for row in 0..3 {
            inverse[col * 4 + row] = f64::from(matrix[row * 4 + col]);
        }
    }
    for row in 0..3 {
        inverse[12 + row] = -(0..3)
            .map(|col| inverse[col * 4 + row] * f64::from(matrix[12 + col]))
            .sum::<f64>();
    }
    inverse[15] = 1.;
    inverse
}
// Independent partial-pivot elimination; no production minor/cofactor formula.
fn inverse_reference(matrix: &[f32; 16]) -> Option<[f64; 16]> {
    let mut augmented = [[0.; 8]; 4];
    for row in 0..4 {
        for col in 0..4 {
            augmented[row][col] = f64::from(matrix[col * 4 + row]);
        }
        augmented[row][row + 4] = 1.;
    }
    for col in 0..4 {
        let pivot = (col..4)
            .max_by(|&a, &b| augmented[a][col].abs().total_cmp(&augmented[b][col].abs()))?;
        if augmented[pivot][col] == 0. {
            return None;
        }
        augmented.swap(col, pivot);
        let divisor = augmented[col][col];
        for value in &mut augmented[col] {
            *value /= divisor;
        }
        let reference = augmented[col];
        for (row, values) in augmented.iter_mut().enumerate() {
            if row != col {
                let factor = values[col];
                for (value, pivot) in values.iter_mut().zip(reference) {
                    *value -= factor * pivot;
                }
            }
        }
    }
    Some(std::array::from_fn(|i| augmented[i % 4][i / 4 + 4]))
}

#[cfg(test)]
#[path = "tests/frame_audit.rs"]
mod tests;
