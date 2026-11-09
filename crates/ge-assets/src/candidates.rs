//! Initial serialized candidate hierarchies, separate from mutable runtime bounds.
use crate::{binary::Bytes, edb::Table};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize)]
pub struct Branch {
    pub target: usize,
    pub bounds: [[f32; 3]; 2],
}
#[derive(Debug, Clone, Serialize)]
pub struct Leaf {
    pub placement: u16,
    pub flags_raw: u8,
    pub unknown_byte: u8,
}
#[derive(Debug, Clone, Serialize)]
pub enum Entries {
    Branches(Vec<Branch>),
    Leaves(Vec<Leaf>),
}
#[derive(Debug, Clone, Serialize)]
pub struct Node {
    pub flags_raw: u8,
    pub entries: Entries,
}
#[derive(Debug, Clone, Serialize)]
pub struct Hierarchy {
    pub root: usize,
    /// Reachable nodes are stored once, while ordered edges retain sharing/cycles.
    /// A runtime refresh or query has its own non-deduplicating work budget.
    pub nodes: BTreeMap<usize, Node>,
}
pub(crate) fn map_read(
    data: &[u8],
    tables: &[Table],
    hash: u32,
    zone: usize,
) -> Result<Option<Hierarchy>> {
    let b = Bytes(data);
    let map = crate::spatial::map_address(data, tables, hash)?;
    let count = b.u32(map + 0x84)? as usize;
    ensure!(
        count <= 65536 && zone < count,
        "candidate zone outside map directory"
    );
    let field = map + 0x88 + zone * 0x8c + 0x28;
    if b.u32(field)? == 0 {
        return Ok(None);
    }
    let collection = b.relative(field)?;
    let field = collection
        .checked_add(8)
        .context("candidate collection offset overflow")?;
    if b.u32(field)? == 0 {
        return Ok(None);
    }
    read(data, b.relative(field)?).map(Some)
}
fn read(data: &[u8], root: usize) -> Result<Hierarchy> {
    let b = Bytes(data);
    let mut nodes = BTreeMap::new();
    let mut pending = vec![root];
    let mut entries = 0usize;
    while let Some(at) = pending.pop() {
        if nodes.contains_key(&at) {
            continue;
        }
        ensure!(
            nodes.len() < 65536,
            "candidate snapshot node budget exceeded"
        );
        let header = b.range(at, 4)?;
        let count = usize::from(header[0]);
        let body = if count == 0 {
            let count = usize::from(b.u16(at + 2)?);
            entries = entries
                .checked_add(count)
                .context("candidate entry count overflow")?;
            ensure!(
                entries <= 1_000_000,
                "candidate snapshot entry budget exceeded"
            );
            b.range(at + 4, count * 4)?;
            let mut leaves = Vec::with_capacity(count);
            for i in 0..count {
                let p = at + 4 + i * 4;
                leaves.push(Leaf {
                    placement: b.u16(p)?,
                    flags_raw: b.range(p + 2, 1)?[0],
                    unknown_byte: b.range(p + 3, 1)?[0],
                });
            }
            Entries::Leaves(leaves)
        } else {
            entries = entries
                .checked_add(count)
                .context("candidate entry count overflow")?;
            ensure!(
                entries <= 1_000_000,
                "candidate snapshot entry budget exceeded"
            );
            b.range(at + 4, count * 0x1c)?;
            let mut branches = Vec::with_capacity(count);
            for i in 0..count {
                let p = at + 4 + i * 0x1c;
                // Relative zero is the descriptor address itself, not absence.
                let target = b.relative(p)?;
                let mut bounds = [[0.; 3]; 2];
                for (side, vector) in bounds.iter_mut().enumerate() {
                    for (axis, value) in vector.iter_mut().enumerate() {
                        *value = b.f32(p + 4 + side * 12 + axis * 4)?;
                    }
                }
                branches.push(Branch { target, bounds });
                pending.push(target);
            }
            Entries::Branches(branches)
        };
        nodes.insert(
            at,
            Node {
                flags_raw: header[1],
                entries: body,
            },
        );
    }
    Ok(Hierarchy { root, nodes })
}

#[cfg(test)]
#[path = "tests/candidates.rs"]
mod tests;
