//! Ordered loaded-world choice and mesh-independent BSP zone binding.
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct StoredBounds {
    pub minimum: [f32; 3],
    pub maximum: [f32; 3],
}
impl StoredBounds {
    /// Native zero-margin containment: minimum XYZ, then maximum XYZ, inclusive.
    /// Used values are checked lazily, after each earlier rejection.
    pub fn contains(self, origin: [f32; 3]) -> Result<bool> {
        self.contains_with_margin(origin, 0.)
    }
    /// Native rounded differences compared with +margin at each minimum and
    /// -margin at each maximum. Negative margins shrink; no clamping or sorting.
    pub fn contains_with_margin(self, origin: [f32; 3], margin: f32) -> Result<bool> {
        ensure!(margin.is_finite(), "nonfinite bounds margin");
        for (minimum, coordinate) in self.minimum.into_iter().zip(origin) {
            let difference = minimum - coordinate;
            ensure!(
                minimum.is_finite() && coordinate.is_finite() && difference.is_finite(),
                "nonfinite world minimum difference"
            );
            if difference > margin {
                return Ok(false);
            }
        }
        for (maximum, coordinate) in self.maximum.into_iter().zip(origin) {
            let difference = maximum - coordinate;
            ensure!(
                maximum.is_finite() && coordinate.is_finite() && difference.is_finite(),
                "nonfinite world maximum difference"
            );
            if difference < -margin {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

/// Scan worlds one onward, first containing match, otherwise world zero. The
/// callback does not address world-zero bounds, or worlds after a match. This
/// operation alone does not establish live world membership or collect geometry.
pub fn select_loaded_world(
    count: usize,
    origin: [f32; 3],
    mut bounds: impl FnMut(usize) -> Result<StoredBounds>,
) -> Result<Option<usize>> {
    ensure!(count <= 65536, "loaded-world host count budget exceeded");
    if count == 0 {
        return Ok(None);
    }
    for index in 1..count {
        if bounds(index)?.contains(origin)? {
            return Ok(Some(index));
        }
    }
    Ok(Some(0))
}

#[derive(Debug, Clone, Copy)]
pub struct BspNode {
    pub plane: [f32; 4],
    pub children: [i16; 2],
}
#[derive(Debug)]
pub struct MapBsp {
    pub nodes: BTreeMap<u16, BspNode>,
    pub zone_count: usize,
}
#[derive(Debug, Serialize)]
pub struct ZoneHit {
    pub zone: usize,
    pub nodes_tested: usize,
}
impl MapBsp {
    /// Root zero, positive children address nodes, nonpositive children encode
    /// -zone. Negative plane distance selects slot one; equality selects zero.
    pub fn zone_at(&self, origin: [f32; 3], node_budget: usize) -> Result<ZoneHit> {
        ensure!(
            node_budget > 0 && node_budget <= 1_000_000,
            "invalid BSP node-test budget"
        );
        ensure!(self.zone_count <= 65536, "invalid BSP zone count");
        ensure!(
            origin.into_iter().all(f32::is_finite),
            "nonfinite BSP origin"
        );
        let mut index = 0;
        for tested in 1..=node_budget {
            let node = self
                .nodes
                .get(&index)
                .context("missing addressed BSP node")?;
            let [x, y, z, w] = node.plane;
            ensure!(
                node.plane.into_iter().all(f32::is_finite),
                "nonfinite BSP plane"
            );
            // Paired dot: rounded X/Y products, fused Z into X and W into Y,
            // then sum the lanes. W is overwritten with one, never user input.
            let distance = origin[2].mul_add(z, origin[0] * x) + 1f32.mul_add(w, origin[1] * y);
            ensure!(distance.is_finite(), "BSP plane projection overflow");
            let child = node.children[usize::from(distance < 0.)];
            if child > 0 {
                index = child as u16;
            } else {
                let zone = (-i32::from(child)) as usize;
                ensure!(zone < self.zone_count, "BSP leaf outside zone directory");
                return Ok(ZoneHit {
                    zone,
                    nodes_tested: tested,
                });
            }
        }
        anyhow::bail!("BSP node-test budget exhausted")
    }
}

#[cfg(test)]
#[path = "../tests/collision_world.rs"]
mod tests;
