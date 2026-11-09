//! Native record-frame preparation and consecutive owner/slot caching.
use anyhow::{Context, Result, ensure};
use serde::Serialize;

fn nmsub(a: f32, b: f32, c: f32) -> f32 {
    -a.mul_add(b, -c)
}
fn finite(values: &[f32]) -> Result<()> {
    ensure!(
        values.iter().all(|v| v.is_finite()),
        "nonfinite collision frame intermediate"
    );
    Ok(())
}
/// Ordered two-column minors for row pairs 01,02,03,12,13,23. The first
/// product rounds before negated fused subtraction of the second product.
fn minors(first: &[f32], second: &[f32]) -> Result<[f32; 6]> {
    let pairs = [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];
    let result = pairs.map(|(a, b)| nmsub(first[b], second[a], first[a] * second[b]));
    finite(&result)?;
    Ok(result)
}
fn add_chain(x: f32, a: f32, y: f32, b: f32, z: f32, c: f32) -> f32 {
    z.mul_add(c, nmsub(y, b, x * a))
}
fn neg_chain(x: f32, a: f32, y: f32, b: f32, z: f32, c: f32) -> f32 {
    nmsub(z, c, y.mul_add(b, nmsub(x, a, 0.)))
}
fn general(matrix: &[f32; 16]) -> Result<([f32; 16], f32)> {
    finite(matrix)?;
    let s = minors(&matrix[..4], &matrix[4..8])?;
    let t = minors(&matrix[8..12], &matrix[12..16])?;
    let left = nmsub(s[4], t[1], s[2].mul_add(t[3], s[0] * t[5]));
    let right = nmsub(s[5], -t[0], s[3].mul_add(t[2], -(s[1] * t[4])));
    let determinant = left + right;
    finite(&[left, right, determinant])?;
    let reciprocal = if determinant == 0. {
        0.
    } else {
        // Host seed replaces the Wii fres estimate; retain its one-step fused
        // residual/refinement order. Original estimate/FP-state parity is open.
        let seed = 1. / determinant;
        let residual = nmsub(seed, determinant, 1.);
        let value = seed.mul_add(residual, seed);
        finite(&[seed, residual, value])?;
        value
    };
    // Native arithmetic scales each 2x2 minor before the three-term cofactor
    // expression, rather than constructing an adjugate and dividing afterward.
    let positive_s = s.map(|v| reciprocal * v);
    let negative_s = s.map(|v| (-reciprocal) * v);
    let positive_t = t.map(|v| reciprocal * v);
    let negative_t = t.map(|v| (-reciprocal) * v);
    for values in [&positive_s, &negative_s, &positive_t, &negative_t] {
        finite(values)?;
    }
    let [a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p] = *matrix;
    let [s0, s1, s2, s3, s4, s5] = positive_s;
    let [ns0, ns1, ns2, ns3, ns4, ns5] = negative_s;
    let [t0, t1, t2, t3, t4, t5] = positive_t;
    let [nt0, nt1, nt2, nt3, nt4, nt5] = negative_t;
    let inverse = [
        add_chain(f, t5, g, t4, h, t3),
        add_chain(b, nt5, c, nt4, d, nt3),
        add_chain(n, s5, o, s4, p, s3),
        add_chain(j, ns5, k, ns4, l, ns3),
        neg_chain(e, t5, g, t2, h, t1),
        neg_chain(a, nt5, c, nt2, d, nt1),
        neg_chain(m, s5, o, s2, p, s1),
        neg_chain(i, ns5, k, ns2, l, ns1),
        add_chain(e, t4, f, t2, h, t0),
        add_chain(a, nt4, b, nt2, d, nt0),
        add_chain(m, s4, n, s2, p, s0),
        add_chain(i, ns4, j, ns2, l, ns0),
        neg_chain(e, t3, f, t1, g, t0),
        neg_chain(a, nt3, b, nt1, c, nt0),
        neg_chain(m, s3, n, s1, o, s0),
        neg_chain(i, ns3, j, ns1, k, ns0),
    ];
    finite(&inverse)?;
    Ok((inverse, determinant))
}
fn rigid(matrix: &[f32; 16]) -> Result<[f32; 16]> {
    let mut inverse = [0.; 16];
    for column in 0..3 {
        for row in 0..3 {
            let value = matrix[row * 4 + column];
            finite(&[value])?;
            inverse[column * 4 + row] = value;
        }
    }
    let [x, y, z] = [matrix[12], matrix[13], matrix[14]];
    finite(&[x, y, z])?;
    for row in 0..3 {
        let first = inverse[row] * x;
        let second = inverse[4 + row].mul_add(y, first);
        inverse[12 + row] = -inverse[8 + row].mul_add(z, second);
    }
    inverse[15] = 1.;
    finite(&inverse)?;
    Ok(inverse)
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InverseMethod {
    Rigid,
    General,
}

#[derive(Debug, Clone, Serialize)]
pub struct CollisionFrame {
    pub local_from_world: [f32; 16],
    pub method: InverseMethod,
    /// Only the general helper computes this. Exact zero remains a result,
    /// not an identity fallback, epsilon rejection or clear-world indication.
    pub general_determinant: Option<f32>,
}
impl CollisionFrame {
    pub fn from_record(matrix: &[f32; 16], flags_raw: u16) -> Result<Self> {
        if flags_raw & 2 != 0 {
            let (local_from_world, determinant) = general(matrix)?;
            Ok(Self {
                local_from_world,
                method: InverseMethod::General,
                general_determinant: Some(determinant),
            })
        } else {
            Ok(Self {
                local_from_world: rigid(matrix)?,
                method: InverseMethod::Rigid,
                general_determinant: None,
            })
        }
    }
    /// Mesh dispatch subtracts the CURRENT record translation first, then
    /// multiplies by the cached inverse with W=0. Do not use inverse-point W=1
    /// or silently substitute the translation of the last prepared record.
    pub fn query_origin(&self, world: [f32; 3], record_translation: [f32; 3]) -> Result<[f32; 4]> {
        finite(&world)?;
        finite(&record_translation)?;
        let relative = std::array::from_fn::<_, 3, _>(|i| world[i] - record_translation[i]);
        finite(&relative)?;
        let value = std::array::from_fn(|row| {
            let first = self.local_from_world[row] * relative[0];
            let second = self.local_from_world[4 + row].mul_add(relative[1], first);
            let third = self.local_from_world[8 + row].mul_add(relative[2], second);
            self.local_from_world[12 + row].mul_add(0., third)
        });
        finite(&value)?;
        Ok(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameKey {
    pub owner_key: u32,
    pub slot: i16,
}
pub struct FrameCache {
    last_key: FrameKey,
    frame: Option<CollisionFrame>,
}
impl FrameCache {
    /// Native traversal starts its key at pointer zero / signed slot -2. Any
    /// preexisting prepared frame must be explicit; unknown is not identity.
    pub fn new(initial_frame: Option<CollisionFrame>) -> Self {
        Self {
            last_key: FrameKey {
                owner_key: 0,
                slot: -2,
            },
            frame: initial_frame,
        }
    }
    pub fn resolve(
        &mut self,
        key: FrameKey,
        record: impl FnOnce() -> Result<([f32; 16], u16)>,
    ) -> Result<&CollisionFrame> {
        if self.last_key != key {
            let (matrix, flags) = record()?;
            let frame = CollisionFrame::from_record(&matrix, flags)?;
            // Host errors leave the previous valid cache intact. Native helper
            // failures/FP exceptions have no established recoverable contract.
            self.frame = Some(frame);
            self.last_key = key;
        }
        self.frame
            .as_ref()
            .context("matching frame key has no established cached inverse")
    }
}

#[cfg(test)]
#[path = "../tests/collision_frame.rs"]
mod tests;
