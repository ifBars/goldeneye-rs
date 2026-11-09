use crate::mission::world::TriggerMap;
use anyhow::{Context, Result, ensure};
use ge_core::mission::triggers::zone;

pub(super) struct Worlds<'a, 'b>(pub &'a [TriggerMap<'b>]);

pub(in crate::game) fn player_origin(
    context: &crate::mission::MissionContext,
    position: [u32; 4],
) -> Result<zone::Context> {
    let maps = context.trigger_maps()?;
    zone::select(
        &mut Worlds(&maps),
        [position[0], position[1], position[2]],
        zone::Budget {
            maps: 128,
            nodes: 65_536,
        },
    )
}

impl zone::Provider for Worlds<'_, '_> {
    fn loaded_count(&self) -> usize {
        self.0.len()
    }
    fn bounds(&mut self, index: usize) -> Result<[[u32; 3]; 2]> {
        Ok(self
            .0
            .get(index)
            .context("zone world ordinal absent")?
            .data
            .descriptor
            .declared_bounds_bits)
    }
    fn map(&mut self, index: usize) -> Result<Option<zone::Map>> {
        Ok(Some(zone::Map {
            preregistration_index: self
                .0
                .get(index)
                .context("zone map ordinal absent")?
                .loaded
                .slot,
        }))
    }
    fn node(&mut self, map: usize, index: u16) -> Result<zone::Node> {
        let spatial = &self
            .0
            .get(map)
            .context("zone map ordinal absent")?
            .data
            .spatial;
        ensure!(spatial.bsp_present, "selected zone map BSP absent");
        let node = spatial
            .nodes
            .get(&index)
            .context("reached zone BSP node absent")?;
        Ok(zone::Node {
            plane_bits: node.plane.map(f32::to_bits),
            children: node.children,
        })
    }
}
