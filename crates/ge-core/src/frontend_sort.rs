//! Pure ordering for queued translucent frontend entity records.
//!
//! The caller supplies original stored-bound centers and the actual model/view
//! depth row. This module does not infer a camera, derive bounds from vertices,
//! flatten an entity's batches, or implement the opaque queue's reverse draw.
use anyhow::{Result, ensure};
use std::cmp::Ordering;

const BIAS_STEP: f32 = f32::from_bits(0x38d1_b717);

/// The key path selected before a translucent entity enters its draw queue.
#[derive(Debug, Clone, Copy)]
pub enum Depth {
    /// Zero sort override: transform the original stored local center.
    Computed {
        center: [f32; 3],
        /// Row coefficients X, Y, Z and translation, in that order.
        view_row: [f32; 4],
    },
    /// Already selected packet key. Explicit keys consume no running bias.
    Explicit(f32),
}

/// One geometry draw record, containing all batches of an entity.
#[derive(Debug, Clone, Copy)]
pub struct Entity {
    /// Opaque caller-owned group index; the sorter never addresses its batches.
    pub payload: usize,
    pub depth: Depth,
}

#[derive(Debug, Clone, Copy)]
pub struct OrderedEntity {
    pub payload: usize,
    pub source_index: usize,
    pub depth: f32,
}

/// Interpret a nonzero signed common-header sort override for queue type two.
/// Zero selects the computed-center path instead and is an explicit error here.
pub fn override_depth(sort_field: i32) -> Result<f32> {
    ensure!(
        sort_field != 0,
        "zero frontend sort field requires stored center"
    );
    let value = sort_field as f32;
    Ok(if sort_field < 0 {
        200_000. - value
    } else {
        -value - 200_000.
    })
}

/// Transform one stored center with separate f32 products and left-associated
/// additions. In particular, do not fuse a multiply/add or use a point helper
/// with different accumulation order.
pub fn center_depth(center: [f32; 3], view_row: [f32; 4]) -> Result<f32> {
    ensure!(
        center.iter().chain(&view_row).all(|v| v.is_finite()),
        "nonfinite frontend sort center or depth row"
    );
    let x = center[0] * view_row[0];
    let y = center[1] * view_row[1];
    let z = center[2] * view_row[2];
    let xy = x + y;
    let xyz = xy + z;
    let depth = xyz + view_row[3];
    ensure!(depth.is_finite(), "nonfinite transformed frontend depth");
    Ok(depth)
}

/// Return forward draw order for finite translucent keys, bounded by a caller
/// work budget. A fresh invocation starts one queue section's bias at +0.
///
/// Only computed records add and advance the running f32 bias, once per entity.
/// All of an entity's batches retain the resulting key and remain one group.
/// Finite keys sort numerically ascending, with -0 before +0. Exact negative-bit
/// ties reverse input order; positive-bit ties preserve it. The sign-bucket tie
/// behavior matters when the small bias rounds away at larger depth magnitudes.
pub fn order_entities(entities: &[Entity], max_records: usize) -> Result<Vec<OrderedEntity>> {
    ensure!(
        entities.len() <= max_records,
        "frontend sort record budget exhausted"
    );
    let mut bias = 0.0_f32;
    let mut ordered = Vec::with_capacity(entities.len());
    for (source_index, entity) in entities.iter().enumerate() {
        let depth = match entity.depth {
            Depth::Computed { center, view_row } => {
                let base = center_depth(center, view_row)?;
                let depth = base + bias;
                bias += BIAS_STEP;
                ensure!(bias.is_finite(), "frontend sort bias overflow");
                depth
            }
            Depth::Explicit(depth) => depth,
        };
        ensure!(depth.is_finite(), "nonfinite frontend sort key");
        ordered.push(OrderedEntity {
            payload: entity.payload,
            source_index,
            depth,
        });
    }
    ordered.sort_unstable_by(|a, b| match a.depth.total_cmp(&b.depth) {
        Ordering::Equal if a.depth.is_sign_negative() => b.source_index.cmp(&a.source_index),
        Ordering::Equal => a.source_index.cmp(&b.source_index),
        order => order,
    });
    Ok(ordered)
}

#[cfg(test)]
#[path = "tests/frontend_sort.rs"]
mod tests;
