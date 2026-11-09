use super::*;
use crate::{
    collision::world::StoredBounds,
    mission::world::{MapReference, WrapperId},
};

fn key(id: u64) -> Key {
    Key {
        wrapper: WrapperId::new(id).unwrap(),
        map_reference: MapReference::from_directory_index(0).unwrap(),
    }
}
fn unit() -> Bounds {
    Bounds {
        minimum: [0., 0., 0., 0.],
        maximum: [1., 1., 1., 0.],
    }
}
const IDENTITY: [f32; 16] = [
    1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
];

struct Fixture {
    key: Key,
    sections: Vec<usize>,
    links: Vec<Vec<usize>>,
    root: Root,
    entity: Option<EntityBounds>,
    scope: FreshScope,
    roots_read: usize,
    matrices_read: usize,
    trees_read: usize,
    components: Vec<u8>,
    special_reads: usize,
    fail_tree: bool,
}
impl Fixture {
    fn new(sections: Vec<usize>) -> Self {
        let count = sections.len();
        Self {
            key: key(1),
            sections,
            links: vec![vec![0]; count],
            root: Root {
                binding_count: 1,
                dynamic: 0,
            },
            entity: Some(EntityBounds {
                base: unit(),
                component_mask: 0,
            }),
            scope: FreshScope {
                live_contexts: 0,
                non_null_components: 0,
            },
            roots_read: 0,
            matrices_read: 0,
            trees_read: 0,
            components: Vec::new(),
            special_reads: 0,
            fail_tree: false,
        }
    }
}
impl RefreshHierarchy for Fixture {
    type Node = usize;
    fn child_count(&mut self, _: usize) -> Result<u8> {
        Ok(0)
    }
    fn leaf_count(&mut self, node: usize) -> Result<u16> {
        Ok(self.links[node].len() as u16)
    }
    fn leaf_placement(&mut self, node: usize, i: u16) -> Result<u16> {
        Ok(self.links[node][usize::from(i)] as u16)
    }
    fn placement_base_bounds(&mut self, _: u16) -> Result<Option<StoredBounds>> {
        Ok(Some(StoredBounds {
            minimum: [0.; 3],
            maximum: [1.; 3],
        }))
    }
    fn branch_bounds(&mut self, _: usize, _: u8) -> Result<StoredBounds> {
        anyhow::bail!("unreached branch bounds")
    }
    fn branch_target(&mut self, _: usize, _: u8) -> Result<usize> {
        anyhow::bail!("unreached branch target")
    }
    fn write_branch_bounds(&mut self, _: usize, _: u8, _: StoredBounds) -> Result<()> {
        anyhow::bail!("unreached branch write")
    }
}
impl Source for Fixture {
    fn key(&self) -> Key {
        self.key
    }
    fn fresh_scope(&self) -> FreshScope {
        self.scope
    }
    fn zone_section_index(&mut self, z: usize) -> Result<usize> {
        Ok(self.sections[z])
    }
    fn zone_root(&mut self, _: usize) -> Result<Root> {
        self.roots_read += 1;
        Ok(self.root)
    }
    fn zone_placement_count(&mut self, z: usize) -> Result<usize> {
        Ok(self.links[z].len())
    }
    fn zone_placement(&mut self, z: usize, i: usize) -> Result<usize> {
        Ok(self.links[z][i])
    }
    fn zone_tree_root(&mut self, z: usize) -> Result<usize> {
        self.trees_read += 1;
        ensure!(!self.fail_tree, "authored missing hierarchy provider");
        Ok(z)
    }
    fn placement_entity(&mut self, _: usize) -> Result<Option<EntityBounds>> {
        Ok(self.entity)
    }
    fn placement_matrix(&mut self, _: usize) -> Result<[f32; 16]> {
        self.matrices_read += 1;
        Ok(IDENTITY)
    }
    fn placement_component(&mut self, _: usize, slot: u8) -> Result<Bounds> {
        self.components.push(slot);
        let extent = f32::from(slot) + 2.;
        Ok(Bounds {
            minimum: [-extent, -extent, -extent, 0.],
            maximum: [extent, extent, extent, 0.],
        })
    }
    fn placement_special(&mut self, _: usize) -> Result<Option<Bounds>> {
        self.special_reads += 1;
        Ok(None)
    }
}

mod boundaries;
mod lifecycle;
