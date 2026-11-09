//! Source request settings for one grenade ascent-box query, not a clearance backend.
use super::GrenadeClearanceVolume;
use crate::collision::OrientedBox;
use anyhow::{Result, ensure};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct GrenadeWorldBoxRequest {
    /// Original selected launch position used by world/BSP/portal/mesh pruning.
    /// It is separate from the padded primitive's center.
    pub world_origin: [f32; 3],
    /// Source apex-displacement length, not an enclosing radius of the padded box.
    pub radius: f32,
    pub world_from_query: [f32; 16],
    pub shape_type: u32,
    pub boxes: [OrientedBox; 1],
    pub filter_slot: u32,
    pub filter_flags: u32,
    /// Bit0x2 requests initial-zone portal expansion during world collection.
    pub request_flags_raw: u32,
    pub record_capacity: usize,
    pub contact_capacity: usize,
}
impl GrenadeWorldBoxRequest {
    /// Copies the confirmed WorldBox call's settings. Loaded-world membership,
    /// residency and any world/actor line backend must be provided separately.
    /// This constructor does not report clear space or advance grenade state.
    pub fn from_volume(volume: &GrenadeClearanceVolume) -> Result<Self> {
        ensure!(
            volume
                .selection_origin
                .into_iter()
                .chain([volume.selection_radius])
                .all(f32::is_finite),
            "nonfinite grenade world-box selection values"
        );
        volume.shape.validate()?;
        Ok(Self {
            world_origin: volume.selection_origin,
            radius: volume.selection_radius,
            world_from_query: [
                1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
            ],
            shape_type: 3,
            boxes: [volume.shape.clone()],
            filter_slot: 1,
            filter_flags: 3,
            request_flags_raw: 2,
            record_capacity: 64,
            contact_capacity: 512,
        })
    }
}
impl GrenadeClearanceVolume {
    pub fn world_box_request(&self) -> Result<GrenadeWorldBoxRequest> {
        GrenadeWorldBoxRequest::from_volume(self)
    }
}

#[cfg(test)]
#[path = "../tests/grenade_world_box_request.rs"]
mod tests;
