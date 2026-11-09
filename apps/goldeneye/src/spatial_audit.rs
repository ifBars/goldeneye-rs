//! Static BSP/zone reader-to-runtime checks. No world geometry is collected here.
use anyhow::{Context, Result, ensure};
use ge_assets::spatial::MapSpatial;
use ge_core::collision::world::{BspNode, MapBsp};
use serde::Serialize;

#[derive(Default, Serialize)]
pub struct Audit {
    pub maps: usize,
    pub decoded: usize,
    pub bsp_maps: usize,
    pub absent_bsp_maps: usize,
    pub zones: usize,
    pub reachable_nodes: usize,
    pub queries: usize,
    pub validated_queries: usize,
    pub ambiguous_queries: usize,
    pub maximum_nodes_tested: usize,
    pub portals: crate::portal_audit::Audit,
}
impl Audit {
    pub fn include(
        &mut self,
        source: &MapSpatial,
        portals: &ge_assets::portals::MapPortals,
    ) -> Result<()> {
        ensure!(
            portals.zone_groups.len() == source.zones.len(),
            "portal/BSP zone directories differ"
        );
        self.portals.include(portals);
        self.decoded += 1;
        self.zones += source.zones.len();
        self.reachable_nodes += source.nodes.len();
        if !source.bsp_present {
            self.absent_bsp_maps += 1;
            return Ok(());
        }
        self.bsp_maps += 1;
        let map = MapBsp {
            zone_count: source.zones.len(),
            nodes: source
                .nodes
                .iter()
                .map(|(&index, n)| {
                    (
                        index,
                        BspNode {
                            plane: n.plane,
                            children: n.children,
                        },
                    )
                })
                .collect(),
        };
        for zone in source.zones.iter().take(8) {
            let center = std::array::from_fn(|i| {
                ((f64::from(zone.bounds[0][i]) + f64::from(zone.bounds[1][i])) * 0.5) as f32
            });
            // Bounds are sample positions only. A zone's AABB center need not
            // actually lie in that BSP leaf, so never assert its source zone ID.
            for origin in [center, zone.bounds[0], zone.bounds[1]] {
                let hit = map.zone_at(origin, 65536)?;
                self.queries += 1;
                self.maximum_nodes_tested = self.maximum_nodes_tested.max(hit.nodes_tested);
                let (expected, ambiguous) = oracle(&map, origin)?;
                if ambiguous {
                    self.ambiguous_queries += 1;
                } else {
                    ensure!(
                        hit.zone == expected,
                        "BSP zone differs from independent f64 traversal"
                    );
                    self.validated_queries += 1;
                }
                self.portals.query(portals, hit.zone, origin, ambiguous)?;
            }
        }
        Ok(())
    }
}
fn oracle(map: &MapBsp, origin: [f32; 3]) -> Result<(usize, bool)> {
    let mut index = 0;
    let mut ambiguous = false;
    for _ in 0..65536 {
        let n = map
            .nodes
            .get(&index)
            .context("independent BSP addressed missing node")?;
        let terms = [
            f64::from(n.plane[0]) * f64::from(origin[0]),
            f64::from(n.plane[1]) * f64::from(origin[1]),
            f64::from(n.plane[2]) * f64::from(origin[2]),
            f64::from(n.plane[3]),
        ];
        let value = terms.into_iter().sum::<f64>();
        let bound =
            1e-6 + 8. * f64::from(f32::EPSILON) * terms.into_iter().map(f64::abs).sum::<f64>();
        ambiguous |= value.abs() <= bound;
        let child = n.children[usize::from(value < 0.)];
        if child <= 0 {
            let zone = (-i32::from(child)) as usize;
            ensure!(
                zone < map.zone_count,
                "independent BSP leaf outside directory"
            );
            return Ok((zone, ambiguous));
        }
        index = child as u16;
    }
    anyhow::bail!("independent BSP traversal exhausted its budget")
}
