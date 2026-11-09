//! Mesh-local collision input from static Wii 0x601 bodies. No render textures,
//! winding repair, instance transforms, skinning, broad phase or contact solving.
use crate::{binary::Bytes, edb::Entity};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct CollisionTriangle {
    /// Original consecutive strip corners, including odd and degenerate triangles.
    pub position_indices: [u16; 3],
    /// Byte offset in this resource of the first raw eight-byte corner record.
    /// This is source provenance, not an invented original-runtime pointer.
    pub first_corner_offset: usize,
    /// Global flag ordinal across included batches only, including disabled triangles.
    pub flag_index: usize,
    pub flags_raw: u16,
    pub command_raw: u16,
    pub batch_index: usize,
    pub strip_in_batch: usize,
    pub triangle_in_strip: usize,
}
impl CollisionTriangle {
    pub fn enabled(&self) -> bool {
        self.flags_raw & 0x200 == 0
    }
}
#[derive(Debug, Serialize)]
pub struct CollisionBatch {
    pub flags_raw: u16,
    pub material_raw: u16,
    pub declared_triangles: u16,
    pub first_triangle: usize,
    /// Excluded batches leave this cursor unchanged and decode zero triangles.
    pub first_flag_index: usize,
    pub triangle_count: usize,
    pub unconsumed_stream_bytes: usize,
}
impl CollisionBatch {
    pub fn included(&self) -> bool {
        self.flags_raw & 0x101 == 0
    }
}
#[derive(Debug, Serialize)]
pub struct CollisionMesh {
    /// Present only when the caller selected an actual root directory descriptor.
    /// Collected non-root bodies have no inferred hash, registration or descriptor.
    pub entity: Option<Entity>,
    pub address: usize,
    pub index_format_raw: u32,
    pub declared_positions: usize,
    pub positions_read: usize,
    pub disabled_triangles: usize,
    pub skipped_batches: usize,
    pub batches: Vec<CollisionBatch>,
    /// Contact reads load enabled corners only; line-plane reads additionally
    /// load included disabled corners. Unreferenced positions remain unread.
    #[serde(skip)]
    pub positions: Vec<Option<[f32; 3]>>,
    #[serde(skip)]
    pub triangles: Vec<CollisionTriangle>,
}
impl CollisionMesh {
    pub fn triangle_vertices(&self, index: usize) -> Result<Option<[[f32; 3]; 3]>> {
        let triangle = self
            .triangles
            .get(index)
            .context("collision triangle index out of bounds")?;
        if !triangle.enabled() {
            return Ok(None);
        }
        let mut points = [[0.; 3]; 3];
        for (point, index) in points.iter_mut().zip(triangle.position_indices) {
            *point = self
                .positions
                .get(index as usize)
                .copied()
                .flatten()
                .context("collision position not decoded")?;
        }
        Ok(Some(points))
    }
}

pub(crate) fn read(data: &[u8], entity: Entity) -> Result<CollisionMesh> {
    ensure!(
        entity.entity_type == 0x601,
        "collision mesh requires a root 0x601 body"
    );
    let address = entity.address as usize;
    read_body(data, address, Some(entity), false)
}
/// Decode an addressed static mesh body without searching or manufacturing a
/// root entity descriptor. The actual body tag is still required to be 0x601.
pub(crate) fn read_at(data: &[u8], address: usize) -> Result<CollisionMesh> {
    read_body(data, address, None, false)
}
/// Resident line-plane preparation addresses disabled triangles' positions too.
/// Keep this separate from the contact reader's disabled-position laziness.
pub(crate) fn read_line_at(data: &[u8], address: usize) -> Result<CollisionMesh> {
    read_body(data, address, None, true)
}
fn read_body(
    data: &[u8],
    address: usize,
    entity: Option<Entity>,
    line_positions: bool,
) -> Result<CollisionMesh> {
    let b = Bytes(data);
    b.range(address, 0xb4)?;
    ensure!(
        b.u32(address)? == 0x601,
        "collision mesh requires an addressed 0x601 body"
    );
    let batch_count = b.u32(address + 0xa4)? as usize;
    let position_count = b.u32(address + 0xa8)? as usize;
    ensure!(
        batch_count < 65536 && position_count <= 65536,
        "invalid collision mesh counts"
    );
    let format = b.u32(address + 0xb0)?;
    let mut result = CollisionMesh {
        entity,
        address,
        index_format_raw: format,
        declared_positions: position_count,
        positions_read: 0,
        disabled_triangles: 0,
        skipped_batches: 0,
        batches: Vec::new(),
        positions: vec![None; position_count],
        triangles: Vec::new(),
    };
    let mut batch = if batch_count == 0 {
        0
    } else {
        ensure!(
            b.u32(address + 0x58)? != 0,
            "missing collision batch stream"
        );
        b.relative(address + 0x58)?
    };
    let mut flag_cursor = 0usize;
    let mut flags_start = None;
    let mut positions_start = None;
    for batch_index in 0..batch_count {
        b.range(batch, 32)?;
        let flags_raw = b.u16(batch + 4)?;
        let declared = b.u16(batch)?;
        let bytes = b.u32(batch + 8)? as usize;
        let stream_start = batch
            .checked_add(32)
            .context("collision stream offset overflow")?;
        let stream = Bytes(b.range(stream_start, bytes)?);
        let mut meta = CollisionBatch {
            flags_raw,
            material_raw: b.u16(batch + 2)?,
            declared_triangles: declared,
            first_triangle: result.triangles.len(),
            first_flag_index: flag_cursor,
            triangle_count: 0,
            unconsumed_stream_bytes: bytes,
        };
        if !meta.included() {
            result.skipped_batches += 1;
            // Native skip does not advance the global triangle-flag cursor.
        } else {
            let mut cursor = 0usize;
            let mut strip = 0usize;
            while meta.triangle_count < usize::from(declared) {
                ensure!(
                    format & 0x0fff_ffff == 0x203,
                    "unsupported collision corner format {format:#x}"
                );
                let command = stream.u16(cursor)?;
                let corners = usize::from(stream.u16(cursor + 2)?);
                ensure!(corners >= 3, "collision strip has fewer than three corners");
                let triangles = corners - 2;
                ensure!(
                    meta.triangle_count + triangles <= usize::from(declared),
                    "collision strip exceeds declared triangle count"
                );
                let corners_start = cursor
                    .checked_add(4)
                    .context("collision corner offset overflow")?;
                stream.range(corners_start, corners * 8)?;
                ensure!(
                    result.triangles.len() + triangles <= 4_000_000,
                    "collision triangle budget exceeded"
                );
                result
                    .triangles
                    .try_reserve(triangles)
                    .context("allocating collision triangles")?;
                for triangle_in_strip in 0..triangles {
                    let mut indices = [0u16; 3];
                    for (j, index) in indices.iter_mut().enumerate() {
                        *index = stream.u16(corners_start + (triangle_in_strip + j) * 8)?;
                    }
                    let flags = match flags_start {
                        Some(start) => start,
                        None => {
                            ensure!(
                                b.u32(address + 0x68)? != 0,
                                "missing collision triangle flags"
                            );
                            let start = b.relative(address + 0x68)?;
                            flags_start = Some(start);
                            start
                        }
                    };
                    let flag_index = flag_cursor;
                    let flags_raw = b.u16(
                        flags
                            .checked_add(flag_cursor * 2)
                            .context("collision flag offset overflow")?,
                    )?;
                    flag_cursor += 1;
                    if flags_raw & 0x200 != 0 {
                        result.disabled_triangles += 1;
                    }
                    if line_positions || flags_raw & 0x200 == 0 {
                        let positions = match positions_start {
                            Some(start) => start,
                            None => {
                                ensure!(
                                    b.u32(address + 0x5c)? != 0,
                                    "missing collision position pool"
                                );
                                let start = b.relative(address + 0x5c)?;
                                positions_start = Some(start);
                                start
                            }
                        };
                        for index in indices {
                            let slot = result
                                .positions
                                .get_mut(index as usize)
                                .context("collision position index out of bounds")?;
                            if slot.is_none() {
                                let p = positions
                                    .checked_add(usize::from(index) * 16)
                                    .context("collision position offset overflow")?;
                                *slot = Some([b.f32(p)?, b.f32(p + 4)?, b.f32(p + 8)?]);
                                result.positions_read += 1;
                            }
                        }
                    }
                    result.triangles.push(CollisionTriangle {
                        position_indices: indices,
                        first_corner_offset: stream_start
                            .checked_add(corners_start)
                            .and_then(|at| at.checked_add(triangle_in_strip * 8))
                            .context("collision first corner offset overflow")?,
                        flag_index,
                        flags_raw,
                        command_raw: command,
                        batch_index,
                        strip_in_batch: strip,
                        triangle_in_strip,
                    });
                }
                meta.triangle_count += triangles;
                cursor = corners_start + corners * 8;
                strip += 1;
            }
            // Collision traversal stops at the declared count, not GX padding.
            meta.unconsumed_stream_bytes = bytes - cursor;
        }
        result.batches.push(meta);
        batch = stream_start
            .checked_add(bytes)
            .context("collision next batch overflow")?;
    }
    Ok(result)
}
#[cfg(test)]
#[path = "tests/collision.rs"]
mod tests;
