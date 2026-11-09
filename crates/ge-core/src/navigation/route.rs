//! Read-only consumers of world waypoint lists, reconstructed from static evidence.
use anyhow::{Result, ensure};

/// Checked borrowed route. Point zero is the route origin; point one is the target.
/// This does not advance points, move an actor, or establish physical clearance.
pub struct WaypointRoute<'a> {
    points: &'a [[f32; 3]],
}

impl<'a> WaypointRoute<'a> {
    pub fn new(points: &'a [[f32; 3]]) -> Result<Self> {
        ensure!(
            points.iter().flatten().all(|v| v.is_finite()),
            "nonfinite waypoint route"
        );
        Ok(Self { points })
    }

    /// The actor getter returns point one only when at least two points exist.
    pub fn target(&self) -> Option<[f32; 3]> {
        self.points.get(1).copied()
    }

    /// Requires EXACTLY two points and horizontal distance squared strictly below 0.0625.
    /// Height is ignored; a longer route is not arrived even at its final point.
    pub fn arrived(&self, position: [f32; 3]) -> Result<bool> {
        finite_position(position)?;
        if self.points.len() != 2 {
            return Ok(false);
        }
        let x = self.points[1][0] - position[0];
        let z = self.points[1][2] - position[2];
        let squared = z.mul_add(z, x * x);
        ensure!(squared.is_finite(), "waypoint arrival distance overflow");
        Ok(squared < 0.0625)
    }

    /// Ordered full-3D segment-length sum. Ordinary sqrt is provisional relative
    /// to the original reciprocal-square-root estimate/Newton pipeline.
    pub fn length(&self) -> Result<f32> {
        let mut sum = 0.;
        for pair in self.points.windows(2) {
            let d = sub(pair[0], pair[1])?;
            let squared = dot(d, d)?;
            // Only exact zero is bypassed here; the path-cost EPS cutoff does not apply.
            let length = if squared == 0. { 0. } else { squared.sqrt() };
            sum += length;
            ensure!(sum.is_finite(), "waypoint route length overflow");
        }
        Ok(sum)
    }

    /// First segment whose clamped 3D distance squared is STRICTLY below radius².
    /// Segment zero also requires a nonnegative forward dot, even within its end cap.
    /// Later segments have both end caps. None scans all segments; Some(0) scans none.
    /// Signed radius is squared, matching the original helper. Returned index is diagnostic.
    pub fn corridor_segment(
        &self,
        position: [f32; 3],
        radius: f32,
        max_segments: Option<usize>,
    ) -> Result<Option<usize>> {
        finite_position(position)?;
        let radius_squared = radius * radius;
        ensure!(
            radius.is_finite() && radius_squared.is_finite(),
            "invalid waypoint corridor radius"
        );
        let count = self.points.len().saturating_sub(1);
        for (index, pair) in self
            .points
            .windows(2)
            .take(max_segments.unwrap_or(count))
            .enumerate()
        {
            let direction = sub(pair[1], pair[0])?;
            let relative = sub(position, pair[0])?;
            let projection = dot(relative, direction)?;
            let distance = if projection <= 0. {
                relative
            } else {
                let squared = dot(direction, direction)?;
                if projection >= squared {
                    sub(relative, direction)?
                } else {
                    let t = projection / squared;
                    let projected = direction.map(|v| v * t);
                    sub(relative, projected)?
                }
            };
            if dot(distance, distance)? >= radius_squared {
                continue;
            }
            if index == 0 {
                // This consumer uses a different paired-single accumulation order
                // from the distance helper: fuse z into the x product, then add y.
                let forward = relative[2].mul_add(direction[2], relative[0] * direction[0])
                    + relative[1] * direction[1];
                ensure!(forward.is_finite(), "waypoint forward-dot overflow");
                if forward < 0. {
                    continue;
                }
            }
            return Ok(Some(index));
        }
        Ok(None)
    }
}

fn finite_position(position: [f32; 3]) -> Result<()> {
    ensure!(
        position.into_iter().all(f32::is_finite),
        "nonfinite waypoint probe position"
    );
    Ok(())
}
fn sub(a: [f32; 3], b: [f32; 3]) -> Result<[f32; 3]> {
    let d = std::array::from_fn(|i| a[i] - b[i]);
    ensure!(
        d.iter().all(|v: &f32| v.is_finite()),
        "waypoint displacement overflow"
    );
    Ok(d)
}
fn dot(a: [f32; 3], b: [f32; 3]) -> Result<f32> {
    let value = a[2].mul_add(b[2], a[0] * b[0] + a[1] * b[1]);
    ensure!(value.is_finite(), "waypoint dot-product overflow");
    Ok(value)
}

#[cfg(test)]
#[path = "../tests/route.rs"]
mod tests;
