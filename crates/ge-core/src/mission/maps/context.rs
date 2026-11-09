use crate::mission::{
    loader::outer::{ContextSlot, Owner},
    world::InstanceId,
};
use anyhow::{Context, Result, ensure};

#[derive(Debug, Clone)]
pub struct SectionKeys {
    pub count_raw: i16,
    pub keys: Vec<u32>,
}
#[derive(Debug)]
pub struct Contexts {
    first: [[u32; 8]; 4],
    second: [[u32; 8]; 4],
}
impl Contexts {
    pub fn from_constructor() -> Self {
        Self {
            first: [[0; 8]; 4],
            second: [[0; 8]; 4],
        }
    }
    /// Explicit retained snapshot; this does not assert original global history.
    pub fn from_snapshot(first: [[u32; 8]; 4], second: [[u32; 8]; 4]) -> Self {
        Self { first, second }
    }
    pub fn clear_leading(&mut self, owner: &mut Owner) -> usize {
        owner.reset_contexts()
    }
    pub fn first_mask(&self, index: usize) -> Option<&[u32; 8]> {
        self.first.get(index)
    }
    pub fn second_mask(&self, index: usize) -> Option<&[u32; 8]> {
        self.second.get(index)
    }
    pub fn bind_primary(
        &mut self,
        owner: &mut Owner,
        instance: InstanceId,
        keys: &SectionKeys,
    ) -> Result<usize> {
        self.bind(owner, 0, instance, keys)
    }
    pub fn bind_secondary(
        &mut self,
        owner: &mut Owner,
        instance: InstanceId,
        keys: &SectionKeys,
    ) -> Result<usize> {
        let index = (1..4)
            .find(|&i| owner.contexts()[i] == ContextSlot::Absent)
            .context("no absent secondary context slot; native unbounded wait unsupported")?;
        self.bind(owner, index, instance, keys)
    }
    fn bind(
        &mut self,
        owner: &mut Owner,
        index: usize,
        instance: InstanceId,
        keys: &SectionKeys,
    ) -> Result<usize> {
        *owner
            .context_mut(index)
            .context("context index outside owner")? = ContextSlot::Bound(instance);
        let count = usize::try_from(keys.count_raw.max(0))?.min(256);
        ensure!(
            keys.keys.len() >= count,
            "reached section key snapshot truncated"
        );
        let mut mask = [0; 8];
        for &key in &keys.keys[..count] {
            if key & 0x7f00_0000 == 0x0800_0000 {
                let word = ((key & 0x00ff_ffff) >> 5) as usize;
                ensure!(word < 8, "section key exceeds supported context mask");
                mask[word] |= 1 << (key & 31);
            }
        }
        self.first[index] = mask;
        Ok(index)
    }
}
#[cfg(test)]
#[path = "tests/context.rs"]
mod tests;
