//! Source StartPoint records and common TriggerInit pose bits. This does not
//! create triggers or establish membership in a live world/catalog.
use super::{TriggerLocator, triggers};
use crate::edb::Table;
use anyhow::{Context, Result, ensure};

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub maps: usize,
    pub rows: usize,
    pub types: usize,
    pub points: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartPoint {
    pub id: u32,
    pub trigger_flags_raw: u32,
    pub property_mask_raw: u32,
    pub position_bits: [u32; 4],
    pub rotation_bits: [u32; 4],
    pub locator: TriggerLocator,
}

pub fn read(
    data: &[u8],
    maps: &Table,
    reference: u32,
    budget: Budget,
) -> Result<Option<Vec<StartPoint>>> {
    let Some(mut records) = triggers::records(
        data,
        maps,
        reference,
        triggers::Budget {
            maps: budget.maps,
            rows: budget.rows,
            types: budget.types,
        },
    )?
    else {
        return Ok(None);
    };
    let mut points = Vec::new();
    while let Some(record) = records.next_record()? {
        // SJBE52 r0 native descriptor28 is enabled class1e000003/subtype1f000081.
        // Different classes, including SpawnPoint, do not become StartPoints.
        if record.locator.native_type_index != 28 {
            continue;
        }
        ensure!(
            points.len() < budget.points,
            "StartPoint ownership budget exhausted"
        );
        let trigger_flags_raw = record.word(4)?;
        let property_mask_raw = record.word(8)?;
        let id = if property_mask_raw & 1 != 0 {
            record.word(0x30)?
        } else {
            0
        };
        // Common TriggerInit copies XYZ without arithmetic and supplies positive
        // zero as W. Preserve signed zeros/NaN-like payloads as raw words; no
        // native finite-value guard or quaternion interpretation is invented.
        let position_bits = [
            record.word(0x0c)?,
            record.word(0x10)?,
            record.word(0x14)?,
            0,
        ];
        let rotation_bits = [
            record.word(0x18)?,
            record.word(0x1c)?,
            record.word(0x20)?,
            0,
        ];
        points
            .try_reserve(1)
            .context("retaining bounded StartPoints")?;
        points.push(StartPoint {
            id,
            trigger_flags_raw,
            property_mask_raw,
            position_bits,
            rotation_bits,
            locator: record.locator,
        });
    }
    Ok(Some(points))
}

#[cfg(test)]
#[path = "tests/startpoints.rs"]
mod tests;
