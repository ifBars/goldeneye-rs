//! Static included-batch bounds cache and ordered mesh/triangle pruning.
//! Caches are resident runtime state, never serialized mesh +0x70/+0x74 words.
use anyhow::{Context, Result, ensure};
use serde::Serialize;

const MAX_BATCHES: usize = 65535;
const MAX_CACHE_BOUNDS: usize = 0x2aa;
const MAX_TRIANGLES: usize = 4_000_000;
const MAX_COORDINATE_READS: usize = 72_000_000;

#[derive(Debug, Clone, Copy)]
pub struct BatchDescriptor {
    pub flags_raw: u16,
    pub material_raw: u16,
    pub declared_triangles: u16,
    pub first_triangle: usize,
    pub triangle_count: usize,
}
impl BatchDescriptor {
    pub fn included(self) -> bool {
        self.flags_raw & 0x101 == 0
    }
}
#[derive(Debug, Clone, Copy)]
pub struct TriangleDescriptor {
    pub flags_raw: u16,
    pub position_indices: [u16; 3],
    pub strip_in_batch: usize,
    pub triangle_in_strip: usize,
}
#[derive(Debug, Clone, Copy, Serialize)]
pub struct BatchBounds {
    pub minimum: [f32; 3],
    pub maximum: [f32; 3],
}
impl BatchBounds {
    pub const EMPTY: Self = Self {
        minimum: [999999.; 3],
        maximum: [-999999.; 3],
    };
}
#[derive(Debug, Clone, Serialize)]
pub struct StaticBatchCache {
    /// Compact storage: excluded batches have no entry and consume no slot.
    pub bounds: Vec<BatchBounds>,
}
#[derive(Debug, Clone, Copy)]
pub struct CacheLimits {
    pub triangle_visits: usize,
    pub coordinate_reads: usize,
}
impl Default for CacheLimits {
    fn default() -> Self {
        Self {
            triangle_visits: MAX_TRIANGLES,
            coordinate_reads: MAX_COORDINATE_READS,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheDisposition {
    SkippedFormat,
    Reused,
    NoIncludedBatches,
    Generated,
}
#[derive(Debug, Serialize)]
pub struct CachePreparationStats {
    pub disposition: CacheDisposition,
    pub included_batches: usize,
    pub triangle_visits: usize,
    pub position_visits: usize,
    pub coordinate_reads: usize,
}
impl CachePreparationStats {
    fn new(disposition: CacheDisposition) -> Self {
        Self {
            disposition,
            included_batches: 0,
            triangle_visits: 0,
            position_visits: 0,
            coordinate_reads: 0,
        }
    }
}

/// Generate mode-one static cache once, or retain its established resident state.
/// Format bit 0x8 skips preparation before the cache is inspected; it does not
/// clear a preexisting cache. A present cache is reused without metadata reads.
/// Dynamic-position contexts must be rejected by the calling static adapter.
/// Included metadata must come from validated strips whose actual count equals
/// the declared count. On host errors the old format/cache remain unchanged.
pub fn prepare_static_cache(
    format_raw: &mut u32,
    cache: &mut Option<StaticBatchCache>,
    batches: &[BatchDescriptor],
    triangle: impl FnMut(usize) -> Result<TriangleDescriptor>,
    coordinate: impl FnMut(u16, usize) -> Result<f32>,
    limits: CacheLimits,
) -> Result<CachePreparationStats> {
    prepare_cache(
        format_raw,
        cache,
        batches,
        triangle,
        coordinate,
        limits,
        CacheMode::Box,
    )
}

/// Generate mode-two line bounds in a separate resident cache (+0x74). Format
/// bit 0x10 skips before cache inspection; no included batches set that bit.
/// Extrema use the same compact traversal as mode one, then subtract f32 0.01
/// from minima and add it to maxima, including empty sentinel bounds. At large
/// sentinel magnitudes the rounded expansion can leave the bits unchanged.
/// The caller owns separate box/line cache slots and rejects dynamic contexts.
pub fn prepare_line_cache(
    format_raw: &mut u32,
    cache: &mut Option<StaticBatchCache>,
    batches: &[BatchDescriptor],
    triangle: impl FnMut(usize) -> Result<TriangleDescriptor>,
    coordinate: impl FnMut(u16, usize) -> Result<f32>,
    limits: CacheLimits,
) -> Result<CachePreparationStats> {
    prepare_cache(
        format_raw,
        cache,
        batches,
        triangle,
        coordinate,
        limits,
        CacheMode::Line,
    )
}

#[derive(Clone, Copy)]
enum CacheMode {
    Box,
    Line,
}
impl CacheMode {
    fn format_mask(self) -> u32 {
        match self {
            Self::Box => 8,
            Self::Line => 0x10,
        }
    }
    fn finish(self, mut bound: BatchBounds) -> BatchBounds {
        let margin = match self {
            Self::Box => 0.,
            Self::Line => f32::from_bits(0x3c23_d70a),
        };
        // Even +0 is an operation: maximum -0 becomes +0 on finalization.
        bound.minimum = bound.minimum.map(|value| value - margin);
        bound.maximum = bound.maximum.map(|value| value + margin);
        bound
    }
}

fn prepare_cache(
    format_raw: &mut u32,
    cache: &mut Option<StaticBatchCache>,
    batches: &[BatchDescriptor],
    mut triangle: impl FnMut(usize) -> Result<TriangleDescriptor>,
    mut coordinate: impl FnMut(u16, usize) -> Result<f32>,
    limits: CacheLimits,
    mode: CacheMode,
) -> Result<CachePreparationStats> {
    if *format_raw & mode.format_mask() != 0 {
        return Ok(CachePreparationStats::new(CacheDisposition::SkippedFormat));
    }
    if let Some(existing) = cache {
        let mut stats = CachePreparationStats::new(CacheDisposition::Reused);
        stats.included_batches = existing.bounds.len();
        return Ok(stats);
    }
    ensure!(
        batches.len() <= MAX_BATCHES,
        "static batch-count budget exceeded"
    );
    ensure!(
        limits.triangle_visits > 0
            && limits.triangle_visits <= MAX_TRIANGLES
            && limits.coordinate_reads > 0
            && limits.coordinate_reads <= MAX_COORDINATE_READS,
        "invalid static cache work budgets"
    );
    let mut stats = CachePreparationStats::new(CacheDisposition::Generated);
    let mut bounds = Vec::new();
    for batch in batches {
        if !batch.included() {
            continue;
        }
        ensure!(
            bounds.len() < MAX_CACHE_BOUNDS,
            "static batch-cache capacity exceeded"
        );
        ensure!(
            batch.triangle_count == usize::from(batch.declared_triangles),
            "static cache requires validated declared/actual triangle counts"
        );
        let mut bound = BatchBounds::EMPTY;
        let mut previous: Option<TriangleDescriptor> = None;
        let mut visited = 0u8;
        for local in 0..batch.triangle_count {
            ensure!(
                stats.triangle_visits < limits.triangle_visits,
                "static cache triangle budget exceeded"
            );
            stats.triangle_visits += 1;
            let index = batch
                .first_triangle
                .checked_add(local)
                .context("static cache triangle index overflow")?;
            let current = triangle(index)?;
            match previous {
                None => ensure!(
                    current.strip_in_batch == 0 && current.triangle_in_strip == 0,
                    "static cache strip must begin at zero"
                ),
                Some(prior) if current.strip_in_batch == prior.strip_in_batch => {
                    ensure!(
                        current.triangle_in_strip
                            == prior
                                .triangle_in_strip
                                .checked_add(1)
                                .context("static strip ordinal overflow")?
                            && current.position_indices[..2] == prior.position_indices[1..],
                        "static cache requires consecutive strip corners"
                    );
                }
                Some(prior) => {
                    ensure!(
                        current.strip_in_batch
                            == prior
                                .strip_in_batch
                                .checked_add(1)
                                .context("static strip index overflow")?
                            && current.triangle_in_strip == 0,
                        "static cache strip order mismatch"
                    );
                    visited = 0;
                }
            }
            if current.flags_raw & 0x200 == 0 {
                for (corner, position) in current.position_indices.into_iter().enumerate() {
                    let bit = 1 << corner;
                    if visited & bit != 0 {
                        continue;
                    }
                    stats.position_visits += 1;
                    for axis in 0..3 {
                        // The source reloads the coordinate before its maximum
                        // comparison. Preserve that order and strict tie retention.
                        let minimum =
                            read_coordinate(&mut coordinate, position, axis, &mut stats, limits)?;
                        if minimum < bound.minimum[axis] {
                            bound.minimum[axis] = minimum;
                        }
                        let maximum =
                            read_coordinate(&mut coordinate, position, axis, &mut stats, limits)?;
                        if maximum > bound.maximum[axis] {
                            bound.maximum[axis] = maximum;
                        }
                    }
                    visited |= bit;
                }
            }
            // Disabled triangles still advance the sliding corner window.
            visited >>= 1;
            previous = Some(current);
        }
        bounds.push(mode.finish(bound));
    }
    stats.included_batches = bounds.len();
    if bounds.is_empty() {
        *format_raw |= mode.format_mask();
        stats.disposition = CacheDisposition::NoIncludedBatches;
    } else {
        *cache = Some(StaticBatchCache { bounds });
    }
    Ok(stats)
}
fn read_coordinate(
    coordinate: &mut impl FnMut(u16, usize) -> Result<f32>,
    position: u16,
    axis: usize,
    stats: &mut CachePreparationStats,
    limits: CacheLimits,
) -> Result<f32> {
    ensure!(
        stats.coordinate_reads < limits.coordinate_reads,
        "static cache coordinate budget exceeded"
    );
    stats.coordinate_reads += 1;
    let value = coordinate(position, axis)?;
    ensure!(value.is_finite(), "nonfinite static cache coordinate");
    Ok(value)
}

#[derive(Debug, Clone, Copy)]
pub struct QueryCube {
    pub minimum: [f32; 3],
    pub maximum: [f32; 3],
}
impl QueryCube {
    /// Unchanged radius in mesh coordinates, including negative radii. Do not
    /// normalize scale, reorder endpoints, or substitute the oriented-box center.
    pub fn new(origin: [f32; 3], radius: f32) -> Result<Self> {
        ensure!(
            origin.into_iter().chain([radius]).all(f32::is_finite),
            "nonfinite static query cube input"
        );
        let minimum = origin.map(|value| (-radius) + value);
        let maximum = origin.map(|value| radius + value);
        ensure!(
            minimum.into_iter().chain(maximum).all(f32::is_finite),
            "static query cube overflow"
        );
        Ok(Self { minimum, maximum })
    }
}
fn finite_pair(first: f32, second: f32) -> Result<()> {
    ensure!(
        first.is_finite() && second.is_finite(),
        "nonfinite static pruning value"
    );
    Ok(())
}
/// Static-cache predicate only. The dynamic-context override is not modeled by
/// this operation. Exact minX=999999 rejects before other cache lanes are read.
pub fn cache_passes(bounds: &BatchBounds, cube: &QueryCube) -> Result<bool> {
    ensure!(
        bounds.minimum[0].is_finite(),
        "nonfinite static cache minimum X"
    );
    if bounds.minimum[0] == 999999. {
        return Ok(false);
    }
    for axis in 0..3 {
        finite_pair(bounds.minimum[axis], cube.maximum[axis])?;
        if bounds.minimum[axis] > cube.maximum[axis] {
            return Ok(false);
        }
    }
    for axis in 0..3 {
        finite_pair(bounds.maximum[axis], cube.minimum[axis])?;
        if bounds.maximum[axis] < cube.minimum[axis] {
            return Ok(false);
        }
    }
    Ok(true)
}
/// Disabled flag 0x200 rejects before XYZ. Each passing axis reads B,A,C in that
/// order, then tests both closed extrema; Y/Z remain unread after an X rejection.
pub fn triangle_passes(
    triangle: &TriangleDescriptor,
    cube: &QueryCube,
    mut coordinate: impl FnMut(u16, usize) -> Result<f32>,
) -> Result<bool> {
    if triangle.flags_raw & 0x200 != 0 {
        return Ok(false);
    }
    for axis in 0..3 {
        let b = coordinate(triangle.position_indices[1], axis)?;
        let a = coordinate(triangle.position_indices[0], axis)?;
        let c = coordinate(triangle.position_indices[2], axis)?;
        ensure!(
            [a, b, c].into_iter().all(f32::is_finite),
            "nonfinite static triangle coordinate"
        );
        let minimum = a.min(b).min(c);
        let maximum = a.max(b).max(c);
        finite_pair(minimum, cube.maximum[axis])?;
        if minimum > cube.maximum[axis] {
            return Ok(false);
        }
        finite_pair(maximum, cube.minimum[axis])?;
        if maximum < cube.minimum[axis] {
            return Ok(false);
        }
    }
    Ok(true)
}
/// Use when rejecting/skipping a whole batch: excluded batches consume zero
/// flags, included rejected batches consume DECLARED words without parsing strips.
/// Actual strip counts are a separate cursor path, equal only after validation.
pub fn skipped_flag_words(batch: &BatchDescriptor) -> usize {
    if batch.included() {
        usize::from(batch.declared_triangles)
    } else {
        0
    }
}

#[cfg(test)]
#[path = "../tests/collision_batch.rs"]
mod tests;
