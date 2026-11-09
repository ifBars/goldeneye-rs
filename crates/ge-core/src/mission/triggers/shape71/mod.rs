//! Subtype71's owned shape constructor. Selector metadata is not geometry;
//! every reached stage remains explicit when a later host capability fails.
mod math;

use super::{Factory, MapContext, Trigger, TriggerId, Unsupported, actor12::ProviderOutcome};
use anyhow::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Request {
    pub trigger: TriggerId,
    pub map: MapContext,
    pub ordinal: u16,
    pub zone: u16,
    pub selector: u32,
}

/// Checked source fields only. The working record's unwritten padding has no
/// representation. Map/selector binding is checked by the supplying owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeometryInput {
    pub type_raw: u8,
    pub word_04_raw: u16,
    pub center_bits: [u32; 3],
    pub orientation_bits: [u32; 4],
    pub dimensions_bits: [u32; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Geometry {
    /// Source constructor writes only tag-1; no other shape bytes are known.
    Uninitialized,
    /// Sparse fallback: remaining in-place storage is unwritten, not zero.
    Type0 {
        center_bits: [u32; 4],
        radius_bits: u32,
    },
    Type2 {
        center_bits: [u32; 4],
        axis_a_bits: [u32; 4],
        axis_b_bits: [u32; 4],
        dimensions_bits: [u32; 3],
    },
}

impl Geometry {
    pub fn tag(&self) -> i32 {
        match self {
            Self::Uninitialized => -1,
            Self::Type0 { .. } => 0,
            Self::Type2 { .. } => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Uninitialized,
    Materialized,
    PlacementBuilt,
    Transformed,
    FallbackComplete,
}

#[derive(Debug, Clone)]
pub struct State {
    radius_bits: u32,
    radius_squared_bits: u32,
    geometry: Geometry,
    stage: Stage,
    placement: Option<[u32; 16]>,
}

impl State {
    pub fn from_serialized(radius_raw: u32) -> Self {
        let radius = super::numeric::signed_tenths(radius_raw);
        Self {
            radius_bits: radius.to_bits(),
            // Source stores9999 directly on zero, not9999 squared orFLT_MAX.
            radius_squared_bits: if radius == 0.0 {
                0x461c_3c00
            } else {
                (radius * radius).to_bits()
            },
            geometry: Geometry::Uninitialized,
            stage: Stage::Uninitialized,
            placement: None,
        }
    }
    pub fn radius_bits(&self) -> u32 {
        self.radius_bits
    }
    pub fn radius_squared_bits(&self) -> u32 {
        self.radius_squared_bits
    }
    pub fn geometry(&self) -> &Geometry {
        &self.geometry
    }
    pub fn geometry_stage(&self) -> Stage {
        self.stage
    }
    pub fn placement_matrix_bits(&self) -> Option<[u32; 16]> {
        self.placement
    }

    fn fallback(&mut self, center_bits: [u32; 4]) {
        self.geometry = Geometry::Type0 {
            center_bits,
            radius_bits: self.radius_bits,
        };
        self.stage = Stage::FallbackComplete;
    }

    fn prepare_present(
        &mut self,
        input: GeometryInput,
        position: [u32; 3],
        rotation: [u32; 3],
    ) -> Result<()> {
        self.geometry = math::materialize(input)?;
        self.stage = Stage::Materialized;
        let placement = math::placement(position, rotation)?;
        self.placement = Some(placement);
        self.stage = Stage::PlacementBuilt;
        self.geometry = math::transform(self.geometry, &placement)?;
        self.stage = Stage::Transformed;
        Ok(())
    }
}

impl Trigger {
    pub(super) fn initialize_shape71(
        &mut self,
        factory: &mut impl Factory,
    ) -> Result<Option<Unsupported>> {
        let state = State::from_serialized(self.common.numeric[0]);
        self.runtime_numeric[0] = state.radius_bits();
        self.shape71 = Some(state);
        let Some(selector) = self.common.shape_index_raw else {
            self.shape71
                .as_mut()
                .expect("owned shape state")
                .fallback(self.common.position_bits);
            return Ok(None);
        };
        let request = Request {
            trigger: self.id,
            map: self.map,
            ordinal: self.ordinal,
            zone: self.zone,
            selector,
        };
        let input = match factory.shape71_geometry(request)? {
            ProviderOutcome::Ready(input) => input,
            ProviderOutcome::Unavailable => {
                return Ok(Some(Unsupported::Shape71Geometry { trigger: self.id }));
            }
        };
        let position = std::array::from_fn(|i| self.common.position_bits[i]);
        let rotation = std::array::from_fn(|i| self.common.rotation_bits[i]);
        // These errors are explicit supported-host math/type limits, not asset
        // read failures. Earlier materialization/placement remains retained.
        if self
            .shape71
            .as_mut()
            .expect("owned shape state")
            .prepare_present(input, position, rotation)
            .is_err()
        {
            return Ok(Some(Unsupported::Shape71Geometry { trigger: self.id }));
        }
        Ok(None)
    }
}

#[cfg(test)]
#[path = "../tests/shape71_stages.rs"]
mod tests;
