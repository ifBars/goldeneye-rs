//! Slot-six aggregate bounds from the native helper's addressed point streams.
//! This intentionally preserves repeated first-record accesses. It is not a
//! generic union of every shape or a collision/clearance implementation.
use crate::binary::Bytes;
use anyhow::{Context, Result, ensure};
use serde::Serialize;

const MAX_OUTER_ITERATIONS: usize = 1_000_000;
const MAX_POINT_UPDATES: usize = 1_000_000;

#[derive(Debug, Serialize)]
pub struct ShapeBounds {
    pub minimum: [f32; 4],
    pub maximum: [f32; 4],
    pub first_count_raw: u32,
    pub second_count_raw: u32,
    pub first_point_updates: usize,
    pub second_point_updates: usize,
}
impl ShapeBounds {
    fn include(&mut self, xyz: [f32; 3]) -> Result<()> {
        let point = [xyz[0], xyz[1], xyz[2], 0.];
        for (lane, value) in point.into_iter().enumerate() {
            let minimum_difference = self.minimum[lane] - value;
            let maximum_difference = self.maximum[lane] - value;
            ensure!(
                value.is_finite()
                    && minimum_difference.is_finite()
                    && maximum_difference.is_finite(),
                "nonfinite shape bounds value/difference"
            );
            if minimum_difference >= 0. {
                self.minimum[lane] = value;
            }
            if maximum_difference < 0. {
                self.maximum[lane] = value;
            }
        }
        Ok(())
    }
}

fn outer_iterations(raw: u32) -> Result<usize> {
    let iterations = (raw as i32).max(0) as usize;
    ensure!(
        iterations <= MAX_OUTER_ITERATIONS,
        "shape bounds outer-iteration budget exceeded"
    );
    Ok(iterations)
}
fn update_count(outer: usize, inner: usize, previous: usize) -> Result<usize> {
    let count = outer
        .checked_mul(inner)
        .context("shape bounds point-count overflow")?;
    ensure!(
        previous
            .checked_add(count)
            .is_some_and(|total| total <= MAX_POINT_UPDATES),
        "shape bounds point-update budget exceeded"
    );
    Ok(count)
}
fn target(b: Bytes<'_>, at: usize, raw: u32) -> Result<usize> {
    ensure!(
        raw != 0,
        "addressed shape bounds stream has a null relative pointer"
    );
    b.relative(at)
}
fn point(b: Bytes<'_>, at: usize) -> Result<[f32; 3]> {
    Ok([b.f32(at)?, b.f32(at + 4)?, b.f32(at + 8)?])
}

/// Read the aggregate used by placement bounds, not the complete shape set.
/// Positive signed counts repeat the first stream record/polygon. Nonpositive
/// counts address no points. Unused pointer targets and point W lanes stay unread.
pub(crate) fn read(data: &[u8], address: usize) -> Result<ShapeBounds> {
    let b = Bytes(data);
    b.range(address, 8)?;
    let first_count_raw = b.u32(address)?;
    let first_pointer_raw = b.u32(address + 4)?;
    let mut result = ShapeBounds {
        minimum: [1_000_000., 1_000_000., 1_000_000., 0.],
        maximum: [-1_000_000., -1_000_000., -1_000_000., -0.],
        first_count_raw,
        second_count_raw: 0,
        first_point_updates: 0,
        second_point_updates: 0,
    };
    let first_outer = outer_iterations(first_count_raw)?;
    if first_outer != 0 {
        let first = target(b, address + 4, first_pointer_raw)?;
        let inner = usize::from(b.range(first, 1)?[0]) + 1;
        result.first_point_updates = update_count(first_outer, inner, 0)?;
        // The observed helper never advances the first source pointer in
        // either loop. Later points/records must not be interpreted here.
        for _ in 0..result.first_point_updates {
            result.include(point(b, first + 4)?)?;
        }
    }
    result.second_count_raw = b.u32(address + 8)?;
    let second_outer = outer_iterations(result.second_count_raw)?;
    if second_outer != 0 {
        let at = address + 12;
        let polygon = target(b, at, b.u32(at)?)?;
        let vertices = usize::from(b.range(polygon, 1)?[0]).saturating_sub(2);
        result.second_point_updates =
            update_count(second_outer, vertices, result.first_point_updates)?;
        // Each outer iteration restarts at the same first polygon. Only its
        // inner vertex index advances, at a sixteen-byte source stride.
        for _ in 0..second_outer {
            for vertex in 0..vertices {
                result.include(point(b, polygon + 16 + vertex * 16)?)?;
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
#[path = "tests/shape_bounds.rs"]
mod tests;
