//! Resident single-resource adapter for confirmed geometry collection contracts.
use anyhow::{Context, Result};
use ge_assets::geometry::GeometryView;
use ge_core::collision::{
    bounds::Bounds,
    collector::{GeometryFilter, GeometrySelection},
    dispatch::{GeometrySource, LocalSphere, Node},
};

pub struct Source<'a>(pub GeometryView<'a>);
fn node_key(address: usize) -> Result<Node> {
    u32::try_from(address).context("geometry address exceeds opaque node key range")
}
impl Source<'_> {
    /// The original top-level entity flag gate precedes all filter/component
    /// reads. A selected component is not gated again by its own entity flags.
    pub fn filter_node(&self, node: Node, filter: GeometryFilter) -> Result<Option<Node>> {
        let address = node as usize;
        if self.0.flags(address)? & 0x100 == 0 {
            return Ok(None);
        }
        let selected = filter.select(
            || self.0.component_mask(address),
            |slot| {
                self.0
                    .component_target(address, slot)?
                    .context("selected geometry component absent")
            },
        )?;
        match selected {
            GeometrySelection::Keep => Ok(Some(node)),
            GeometrySelection::Replace(address) => Ok(Some(node_key(address)?)),
            GeometrySelection::Reject => Ok(None),
        }
    }
    pub fn reference(&self, word: u32) -> Result<Option<Node>> {
        self.0.resolve_reference(word)?.map(node_key).transpose()
    }
}
impl GeometrySource for Source<'_> {
    fn node_type(&mut self, node: Node) -> Result<u32> {
        self.0.node_type(node as usize)
    }
    fn sphere(&mut self, node: Node) -> Result<LocalSphere> {
        let sphere = self.0.sphere(node as usize)?;
        Ok(LocalSphere {
            center: sphere.center,
            radius: sphere.radius,
        })
    }
    fn bounds(&mut self, node: Node) -> Result<Bounds> {
        let bounds = self.0.bounds(node as usize)?;
        Ok(Bounds {
            minimum: [
                bounds.minimum[0],
                bounds.minimum[1],
                bounds.minimum[2],
                f32::from_bits(bounds.packed_w_raw[0]),
            ],
            maximum: [
                bounds.maximum[0],
                bounds.maximum[1],
                bounds.maximum[2],
                f32::from_bits(bounds.packed_w_raw[1]),
            ],
        })
    }
    fn split_child_count(&mut self, node: Node) -> Result<u32> {
        self.0.split_child_count(node as usize)
    }
    fn split_child(&mut self, node: Node, index: u32) -> Result<Node> {
        node_key(self.0.split_child(node as usize, index)?)
    }
    fn link_count(&mut self, node: Node) -> Result<u8> {
        self.0.link_count(node as usize)
    }
    fn link_target(&mut self, node: Node, index: u8) -> Result<Option<Node>> {
        self.0
            .link_target(node as usize, index)?
            .map(node_key)
            .transpose()
    }
    fn optional_context_index(&mut self, node: Node) -> Result<u16> {
        self.0.optional_context_index(node as usize)
    }
}

#[cfg(test)]
#[path = "tests/geometry_source.rs"]
mod tests;
