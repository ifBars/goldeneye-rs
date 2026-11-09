//! Ordered, clipped BSP segment zone binding. World membership is a caller concern.
use anyhow::{Result, ensure};
use serde::Serialize;

pub const OUTPUT_CAPACITY: usize = 16;
pub const MAX_PENDING_SEGMENTS: usize = 32;
const SPLIT_EPSILON: f32 = f32::from_bits(0x3727_c5ac);

/// Reads remain separate: only selected children are addressed. A crossing reads
/// its far child before its near child; a revisited node reads its plane again.
pub trait SegmentSource {
    fn plane(&mut self, node: u16) -> Result<[f32; 4]>;
    fn child(&mut self, node: u16, slot: u8) -> Result<i16>;
}

#[derive(Debug, Clone, Copy)]
pub struct SegmentLimits {
    pub node_tests: usize,
    /// Includes repeated leaves suppressed by the signed output-list scan.
    pub leaf_visits: usize,
    /// At most the source's 32 deferred far segments; exhaustion is an error.
    pub pending_segments: usize,
}

#[derive(Debug, Default, Serialize)]
pub struct SegmentResult {
    pub zones: Vec<usize>,
    pub node_tests: usize,
    pub leaf_visits: usize,
    pub maximum_pending_segments: usize,
    /// The sixteenth accepted append stops traversal, even with no pending work.
    /// This reports the stop condition, not whether another zone exists.
    pub capacity_reached: bool,
}

#[derive(Clone, Copy)]
struct Segment {
    child: i16,
    start: [f32; 4],
    end: [f32; 4],
}

fn finite(values: impl IntoIterator<Item = f32>) -> Result<()> {
    ensure!(
        values.into_iter().all(f32::is_finite),
        "nonfinite BSP segment input or arithmetic"
    );
    Ok(())
}
fn projection(point: [f32; 4], plane: [f32; 4]) -> Result<f32> {
    let xy = [point[0] * plane[0], point[1] * plane[1]];
    let zw = [
        point[2].mul_add(plane[2], xy[0]),
        point[3].mul_add(plane[3], xy[1]),
    ];
    let result = zw[0] + zw[1];
    finite(xy.into_iter().chain(zw).chain([result]))?;
    Ok(result)
}
fn clipped(start: [f32; 4], delta: [f32; 4], fraction: f32) -> Result<[f32; 4]> {
    let scaled = delta.map(|value| value * fraction);
    let point = std::array::from_fn(|i| start[i] + scaled[i]);
    finite(scaled.into_iter().chain(point))?;
    Ok(point)
}
// The source <=0 branch canonicalizes -0; f32::clamp preserves its sign.
#[allow(clippy::manual_clamp)]
fn clamp_fraction(value: f32) -> f32 {
    if value >= 1. {
        1.
    } else if value <= 0. {
        // The lower-bound load also canonicalizes a negative zero to +0.
        0.
    } else {
        value
    }
}

/// Bind a zero-margin segment, starting at BSP node zero. Input W lanes are
/// overwritten with one. Signed positive children are node indices; nonpositive
/// children encode negated zone indices. Near clipped work precedes deferred far
/// work. The first-seen output scan suppresses ordinary duplicate zones, while
/// node traversal itself is never deduplicated.
pub fn query<S: SegmentSource>(
    source: &mut S,
    mut start: [f32; 4],
    mut end: [f32; 4],
    zone_count: usize,
    limits: SegmentLimits,
) -> Result<SegmentResult> {
    ensure!(
        [limits.node_tests, limits.leaf_visits]
            .into_iter()
            .all(|value| value > 0 && value <= 1_000_000)
            && limits.pending_segments > 0
            && limits.pending_segments <= MAX_PENDING_SEGMENTS,
        "invalid BSP segment host budget"
    );
    ensure!(zone_count <= 65536, "invalid BSP segment zone count");
    start[3] = 1.;
    end[3] = 1.;
    finite(start.into_iter().chain(end))?;
    let mut result = SegmentResult::default();
    let mut pending = Vec::<Segment>::new();
    let mut current = Segment {
        child: 0,
        start,
        end,
    };
    // Root zero is always a node; zero subsequently read from a child is zone zero.
    let mut root = true;
    loop {
        if !root && current.child <= 0 {
            ensure!(
                result.leaf_visits < limits.leaf_visits,
                "BSP segment leaf budget exhausted"
            );
            result.leaf_visits += 1;
            let zone = (-i32::from(current.child)) as usize;
            ensure!(zone < zone_count, "BSP segment leaf outside zone directory");
            // Prior entries are loaded as signed halfwords. In particular, the
            // extreme encoding -32768 does not compare equal to a prior append.
            if !result
                .zones
                .iter()
                .any(|&old| i32::from(old as i16) == zone as i32)
            {
                result.zones.push(zone);
                if result.zones.len() == OUTPUT_CAPACITY {
                    result.capacity_reached = true;
                    return Ok(result);
                }
            }
            match pending.pop() {
                Some(next) => current = next,
                None => return Ok(result),
            }
            continue;
        }
        let node = if root { 0 } else { current.child as u16 };
        root = false;
        ensure!(
            result.node_tests < limits.node_tests,
            "BSP segment node budget exhausted"
        );
        result.node_tests += 1;
        let plane = source.plane(node)?;
        finite(plane)?;
        let first = projection(current.start, plane)?;
        let second = projection(current.end, plane)?;
        if first >= 0. && second >= 0. {
            current.child = source.child(node, 0)?;
        } else if first < 0. && second < 0. {
            current.child = source.child(node, 1)?;
        } else {
            let denominator = first - second;
            let inverse = 1. / denominator;
            let near_slot = u8::from(first <= second);
            // Distinct rounded operations and the asymmetric increasing-side
            // epsilon are intentional. Do not replace them with a line-plane t.
            let near_numerator = first + SPLIT_EPSILON;
            let far_numerator = if near_slot == 0 {
                first - SPLIT_EPSILON
            } else {
                first + SPLIT_EPSILON
            };
            let near_raw = inverse * near_numerator;
            let far_raw = inverse * far_numerator;
            finite([
                denominator,
                inverse,
                near_numerator,
                far_numerator,
                near_raw,
                far_raw,
            ])?;
            let near_fraction = clamp_fraction(near_raw);
            let far_fraction = clamp_fraction(far_raw);
            let delta = std::array::from_fn(|i| current.end[i] - current.start[i]);
            finite(delta)?;
            let far_start = clipped(current.start, delta, far_fraction)?;
            ensure!(
                pending.len() < limits.pending_segments,
                "BSP segment pending budget exhausted"
            );
            let far_child = source.child(node, 1 - near_slot)?;
            pending.push(Segment {
                child: far_child,
                start: far_start,
                end: current.end,
            });
            result.maximum_pending_segments = result.maximum_pending_segments.max(pending.len());
            let near_end = clipped(current.start, delta, near_fraction)?;
            current.child = source.child(node, near_slot)?;
            current.end = near_end;
        }
    }
}

#[cfg(test)]
#[path = "../tests/collision_bsp_segment.rs"]
mod tests;
