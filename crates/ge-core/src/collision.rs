//! Native type-three oriented boxes and mesh-local triangle contact construction.
use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize};
pub mod batch;
pub mod bounds;
pub mod bsp_segment;
pub mod candidates;
pub mod coarse;
pub mod collector;
mod contact;
pub mod dispatch;
pub mod frame;
pub mod line_bounds;
pub mod line_coarse;
pub mod line_frame;
pub mod line_geometry;
pub mod line_triangle;
mod math;
pub mod mesh;
pub mod portal;
pub mod refresh;
pub mod result_binding;
mod sat;
pub mod world;

#[derive(Debug, Clone, Serialize)]
pub struct BoxTriangleCandidate {
    pub axis: u8,
    pub normal: [f32; 3],
    pub depth: f32,
}
#[derive(Debug, Serialize)]
pub struct BoxTriangleContact {
    pub axis: u8,
    pub normal: [f32; 3],
    pub point: [f32; 3],
    /// Source wrapper's other output point is one stored normal vector away;
    /// it is not an independently clamped physical witness on the box.
    pub point_minus_normal: [f32; 3],
    pub depth: f32,
}
#[derive(Debug, Serialize)]
pub struct BoxTriangleResult {
    /// IDs 1..13 are ordered separating axes; 99 means the computed f32 triangle
    /// normal has zero length (not an independent geometric area classification).
    pub separation_axis: Option<u8>,
    pub candidate: Option<BoxTriangleCandidate>,
    pub contact: Option<BoxTriangleContact>,
}

/// Center, two stored axes and three half extents. The third axis is their cross
/// product; source data is not silently orthogonalized or normalized.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrientedBox {
    pub center: [f32; 3],
    pub axes: [[f32; 3]; 2],
    pub half_extents: [f32; 3],
}
impl OrientedBox {
    /// Complete mesh-local type-three triangle contact over ordered vertices.
    /// Host sqrt/reciprocal remain provisional relative to Wii estimates/FP state.
    pub fn query_triangle(&self, vertices: [[f32; 3]; 3]) -> Result<BoxTriangleResult> {
        let t = math::Triangle::new(vertices)?;
        match sat::select(self, &t)? {
            sat::Outcome::Separated(axis) => Ok(BoxTriangleResult {
                separation_axis: Some(axis),
                candidate: None,
                contact: None,
            }),
            sat::Outcome::Candidate(candidate) => {
                let contact = contact::build(self, &t, &candidate)?;
                Ok(BoxTriangleResult {
                    separation_axis: None,
                    candidate: Some(candidate),
                    contact,
                })
            }
        }
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.center
                .into_iter()
                .chain(self.axes.into_iter().flatten())
                .chain(self.half_extents)
                .all(f32::is_finite),
            "nonfinite oriented box"
        );
        ensure!(
            self.half_extents.into_iter().all(|v| v >= 0.),
            "negative oriented-box half extent"
        );
        Ok(())
    }
    /// Original axis accessor uses rounded products followed by fused subtracts.
    pub fn axis(&self, index: usize) -> Result<[f32; 3]> {
        self.validate()?;
        let value = match index {
            0 | 1 => self.axes[index],
            2 => {
                let [a, b] = self.axes;
                [
                    a[1].mul_add(b[2], -(a[2] * b[1])),
                    a[2].mul_add(b[0], -(a[0] * b[2])),
                    a[0].mul_add(b[1], -(a[1] * b[0])),
                ]
            }
            _ => bail!("oriented-box axis outside 0..3"),
        };
        ensure!(
            value.into_iter().all(f32::is_finite),
            "oriented-box cross product overflow"
        );
        Ok(value)
    }
}
#[cfg(test)]
#[path = "tests/collision_contact.rs"]
mod tests;
