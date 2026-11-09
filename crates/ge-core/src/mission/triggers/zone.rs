//! Source callback-zone selection over actual loaded-map inputs. Arithmetic
//! preserves the observed f32/FMA order; host numerical parity is unverified.
use anyhow::{Result, ensure};

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub maps: usize,
    pub nodes: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct Map {
    pub preregistration_index: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct Node {
    pub plane_bits: [u32; 4],
    pub children: [i16; 2],
}

pub trait Provider {
    fn loaded_count(&self) -> usize;
    /// Source instance bounds, not fitted mesh/zone bounds. Map zero stays
    /// unread when one map exists or all later-map candidates miss.
    fn bounds(&mut self, loaded_ordinal: usize) -> Result<[[u32; 3]; 2]>;
    /// None represents an actual missing selected instance/map body and yields
    /// the source ffff0000 sentinel, not zone zero. A missing BSP is a node error.
    fn map(&mut self, loaded_ordinal: usize) -> Result<Option<Map>>;
    fn node(&mut self, loaded_ordinal: usize, index: u16) -> Result<Node>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Context {
    pub loaded_ordinal: Option<usize>,
    pub zone_leaf: Option<u16>,
    pub callback_zone: u16,
    pub packed: u32,
    pub nodes_visited: usize,
}

impl Context {
    fn missing(loaded_ordinal: Option<usize>) -> Self {
        Self {
            loaded_ordinal,
            zone_leaf: None,
            callback_zone: 0xffff,
            packed: 0xffff_0000,
            nodes_visited: 0,
        }
    }
}

pub fn select(
    provider: &mut impl Provider,
    position_bits: [u32; 3],
    budget: Budget,
) -> Result<Context> {
    let count = provider.loaded_count();
    ensure!(count <= budget.maps, "callback world-map budget exhausted");
    if count == 0 {
        return Ok(Context::missing(None));
    }
    let point = position_bits.map(f32::from_bits);
    let mut selected = 0;
    for index in 1..count {
        if contains(provider.bounds(index)?, point) {
            selected = index;
            break;
        }
    }
    let Some(map) = provider.map(selected)? else {
        return Ok(Context::missing(Some(selected)));
    };
    let mut index = 0;
    let mut visits = 0;
    loop {
        ensure!(visits < budget.nodes, "callback BSP work budget exhausted");
        let node = provider.node(selected, index)?;
        visits += 1;
        let plane = node.plane_bits.map(f32::from_bits);
        let x = point[0] * plane[0];
        let y = point[1] * plane[1];
        let xz = point[2].mul_add(plane[2], x);
        // Native helper overwrites query W with positive1 before the plane dot.
        let yw = 1.0_f32.mul_add(plane[3], y);
        let dot = xz + yw;
        // Source CRLT selects +0x12; equality and unordered select +0x10.
        let child = node.children[usize::from(dot < 0.0)];
        if child > 0 {
            index = child as u16;
            continue;
        }
        let leaf = u16::try_from(-i32::from(child))?;
        let callback_zone = (u16::from(map.preregistration_index) << 8) | (leaf & 0xff);
        return Ok(Context {
            loaded_ordinal: Some(selected),
            zone_leaf: Some(leaf),
            callback_zone,
            packed: u32::from(callback_zone) << 16,
            nodes_visited: visits,
        });
    }
}

fn contains(bounds: [[u32; 3]; 2], point: [f32; 3]) -> bool {
    // Native ordered branches reject only strict out-of-bounds comparisons;
    // unordered compares therefore pass. Keep source subtraction order rather
    // than replacing it with an epsilon or generic inclusive-range predicate.
    for (axis, value) in point.iter().enumerate() {
        if 0.0 < f32::from_bits(bounds[0][axis]) - value {
            return false;
        }
    }
    for (axis, value) in point.iter().enumerate() {
        if -0.0 > f32::from_bits(bounds[1][axis]) - value {
            return false;
        }
    }
    true
}
