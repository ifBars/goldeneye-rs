use crate::mission::triggers::zone::*;

struct Source {
    bounds: Vec<[[u32; 3]; 2]>,
    maps: Vec<Option<Map>>,
    nodes: Vec<Node>,
    reads: Vec<String>,
}
impl Provider for Source {
    fn loaded_count(&self) -> usize {
        self.maps.len()
    }
    fn bounds(&mut self, index: usize) -> anyhow::Result<[[u32; 3]; 2]> {
        self.reads.push(format!("bounds{index}"));
        Ok(self.bounds[index])
    }
    fn map(&mut self, index: usize) -> anyhow::Result<Option<Map>> {
        self.reads.push(format!("map{index}"));
        Ok(self.maps[index])
    }
    fn node(&mut self, map: usize, index: u16) -> anyhow::Result<Node> {
        self.reads.push(format!("node{map}:{index}"));
        self.nodes
            .get(usize::from(index))
            .copied()
            .ok_or_else(|| anyhow::anyhow!("missing authored BSP node"))
    }
}
fn source(count: usize) -> Source {
    Source {
        bounds: vec![[[(-1.0_f32).to_bits(); 3], [1.0_f32.to_bits(); 3]]; count],
        maps: (0..count)
            .map(|index| {
                Some(Map {
                    preregistration_index: index as u8 + 1,
                })
            })
            .collect(),
        nodes: vec![Node {
            plane_bits: [1.0_f32.to_bits(), 0, 0, 0],
            children: [-3, -4],
        }],
        reads: Vec::new(),
    }
}
fn budget() -> Budget {
    Budget { maps: 4, nodes: 8 }
}

#[test]
fn later_world_first_match_uses_inclusive_bounds_and_unread_world_zero_bounds() {
    let mut data = source(3);
    let result = select(&mut data, [1.0_f32.to_bits(), 0, 0], budget()).unwrap();
    assert_eq!(result.loaded_ordinal, Some(1));
    assert_eq!(result.zone_leaf, Some(3));
    assert_eq!(result.callback_zone, 0x0203);
    assert_eq!(result.packed, 0x0203_0000);
    assert_eq!(data.reads, ["bounds1", "map1", "node1:0"]);
    data.reads.clear();
    let result = select(&mut data, [2.0_f32.to_bits(), 0, 0], budget()).unwrap();
    assert_eq!(result.loaded_ordinal, Some(0));
    assert_eq!(data.reads, ["bounds1", "bounds2", "map0", "node0:0"]);
}

#[test]
fn source_bsp_ties_negative_and_unordered_keep_observed_branches() {
    let mut data = source(1);
    let tie = select(&mut data, [0, 0, 0], budget()).unwrap();
    assert_eq!(tie.zone_leaf, Some(3));
    let negative = select(&mut data, [(-1.0_f32).to_bits(), 0, 0], budget()).unwrap();
    assert_eq!(negative.zone_leaf, Some(4));
    let nan = select(&mut data, [0x7fc0_0001, 0, 0], budget()).unwrap();
    assert_eq!(nan.zone_leaf, Some(3));
    assert!(!data.reads.iter().any(|read| read.starts_with("bounds")));
    data.nodes[0].children[0] = -300;
    assert_eq!(
        select(&mut data, [0, 0, 0], budget())
            .unwrap()
            .callback_zone,
        0x012c,
        "source packs only low8 leaf bits"
    );
}

#[test]
fn missing_world_or_map_is_sentinel_but_missing_bsp_is_error() {
    let mut empty = source(0);
    assert_eq!(
        select(&mut empty, [0; 3], budget()).unwrap().packed,
        0xffff_0000
    );
    assert!(empty.reads.is_empty());
    let mut data = source(1);
    data.maps[0] = None;
    assert_eq!(
        select(&mut data, [0; 3], budget()).unwrap().callback_zone,
        0xffff
    );
    data.maps[0] = Some(Map {
        preregistration_index: 7,
    });
    data.nodes.clear();
    assert!(select(&mut data, [0; 3], budget()).is_err());
}

#[test]
fn cycles_are_bounded_and_plane_w_is_forced_one() {
    let mut data = source(1);
    data.nodes[0] = Node {
        plane_bits: [0, 0, 0, (-1.0_f32).to_bits()],
        children: [0, -2],
    };
    assert_eq!(
        select(&mut data, [0; 3], budget()).unwrap().zone_leaf,
        Some(2)
    );
    data.nodes[0].children[1] = 1;
    data.nodes.push(data.nodes[0]);
    assert!(
        select(
            &mut data,
            [0; 3],
            Budget {
                nodes: 2,
                ..budget()
            }
        )
        .is_err()
    );
    assert!(
        select(
            &mut data,
            [0; 3],
            Budget {
                maps: 0,
                ..budget()
            }
        )
        .is_err()
    );
}
