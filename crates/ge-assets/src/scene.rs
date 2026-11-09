//! Wii map-zone graphs, shared placement geometry, and indexed GX triangle strips.
//! Preserves unimplemented shading/normal data; never interprets gameplay scripts.
use crate::transform;
use crate::{
    binary::Bytes,
    edb::{Table, Texture},
};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize)]
pub struct Vertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
    pub color: [u8; 4],
    pub color_index: u16,
    pub position_index: u16,
    pub normal_index: u16,
    pub packed_normal_raw: u32,
}

#[derive(Debug, Serialize)]
pub struct MaterialReference {
    pub hash: u32,
    pub section: u16,
    pub debug: u16,
    pub runtime_pointer_raw: u32,
    pub file_hash: Option<u32>,
    pub texture_index: u32,
    pub texture_hash: Option<u32>,
    pub unknown_word: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Batch {
    pub model: [f32; 16],
    pub placement_index: Option<usize>,
    pub entity_address: usize,
    pub entity_flags: u32,
    pub index_format_raw: u32,
    pub binding_index: usize,
    pub raw_binding_index: u16,
    pub flags: u16,
    pub transparency: u16,
    pub declared_triangles: u16,
    pub unknown_words: [u32; 5],
    pub first_index: u32,
    pub index_count: u32,
}

#[derive(Debug, Serialize)]
pub struct Zone {
    pub reference: u32,
    pub identifier_offset: usize,
    pub bounds: [[f32; 3]; 2],
}

#[derive(Debug, Serialize)]
pub struct Scene {
    pub map_hash: u32,
    pub zones: Vec<Zone>,
    pub placements_not_loaded: u32,
    /// Decoded placement records, including separate navigation instances.
    pub placements_loaded: usize,
    pub render_placements: usize,
    pub navigation_placements: usize,
    /// Shared navmesh assets; placement status/model records locate instances.
    /// Use entity-graph for detailed JSON; scene summaries omit these arrays.
    #[serde(skip)]
    pub nav_meshes: Vec<crate::graph::TriangleGraph>,
    pub placements: Vec<Placement>,
    #[serde(flatten)]
    pub geometry: Geometry,
}

#[derive(Debug, Serialize)]
pub struct Geometry {
    /// GX batch indices address this texture table, not the material-reference table.
    pub texture_bindings: Vec<u32>,
    pub meshes: usize,
    pub splits: usize,
    pub zone_links: usize,
    pub shared_visits: usize,
    pub bounds: [[f32; 3]; 2],
    pub material_references: Vec<MaterialReference>,
    pub batches: Vec<Batch>,
    #[serde(skip)]
    pub vertices: Vec<Vertex>,
    #[serde(skip)]
    pub indices: Vec<u32>,
}

impl Scene {
    pub fn triangle_count(&self) -> usize {
        self.geometry
            .batches
            .iter()
            .map(|b| b.index_count as usize / 3)
            .sum()
    }
    pub fn vertex_count(&self) -> usize {
        self.geometry.vertices.len()
    }
}

impl Geometry {
    fn new(materials: Vec<MaterialReference>, textures: &[Texture]) -> Self {
        Self {
            texture_bindings: textures.iter().map(|t| t.hash).collect(),
            meshes: 0,
            splits: 0,
            zone_links: 0,
            shared_visits: 0,
            bounds: [[f32::INFINITY; 3], [f32::NEG_INFINITY; 3]],
            material_references: materials,
            batches: vec![],
            vertices: vec![],
            indices: vec![],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementPolicy {
    /// Inspect zones without reading placement records.
    StaticOnly,
    /// Reject the scene if any placement cannot be decoded.
    RequireAll,
    /// Report typed unsupported entities; corrupt or missing data still fails.
    AllowUnsupported,
}

#[derive(Debug, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum PlacementState {
    Loaded {
        first_batch: usize,
        batch_count: usize,
    },
    Navigation {
        nav_mesh: usize,
        vertices: usize,
        triangles: usize,
    },
    Unsupported {
        entity_type: u32,
        address: usize,
    },
}

#[derive(Debug, Serialize)]
pub struct Placement {
    pub hash: u32,
    pub position: [f32; 3],
    pub flags: u32,
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
    pub engine_flags: u16,
    pub map_on: u16,
    pub object_hash: u32,
    pub light_set: u16,
    pub group: i16,
    pub unknown_word: u32,
    pub model: [f32; 16],
    pub status: Option<PlacementState>,
}

#[derive(Debug, Clone, Copy)]
pub struct UnsupportedEntity {
    pub entity_type: u32,
    pub address: usize,
}
impl std::fmt::Display for UnsupportedEntity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "unsupported entity type {:#x} at {:#x}",
            self.entity_type, self.address
        )
    }
}
impl std::error::Error for UnsupportedEntity {}

pub(crate) fn read_with_placements(
    data: &[u8],
    tables: &[Table],
    textures: &[Texture],
    hash: u32,
    policy: PlacementPolicy,
) -> Result<Scene> {
    let mut scene = read(data, tables, textures, hash)?;
    if policy == PlacementPolicy::StaticOnly {
        return Ok(scene);
    }
    let mut placements = read_placements(data, tables, hash)?;
    let mut cache = BTreeMap::<u32, std::result::Result<CachedEntity, UnsupportedEntity>>::new();
    for (index, p) in placements.iter_mut().enumerate() {
        if let std::collections::btree_map::Entry::Vacant(slot) = cache.entry(p.object_hash) {
            let prototype = read_placed_entity(data, tables, textures, p.object_hash, &mut scene);
            let entry = match prototype {
                Ok(prototype) => Ok(prototype),
                Err(error) => {
                    if policy == PlacementPolicy::AllowUnsupported {
                        if let Some(unsupported) = error.downcast_ref::<UnsupportedEntity>() {
                            Err(*unsupported)
                        } else {
                            return Err(error).with_context(|| {
                                format!("placement {index} entity {:#x}", p.object_hash)
                            });
                        }
                    } else {
                        return Err(error).with_context(|| {
                            format!("placement {index} entity {:#x}", p.object_hash)
                        });
                    }
                }
            };
            slot.insert(entry);
        }
        match &cache[&p.object_hash] {
            Ok(CachedEntity::Geometry(prototype)) => {
                ensure!(
                    scene.geometry.batches.len() + prototype.batches.len() <= 65536,
                    "scene draw budget exceeded"
                );
                let first_batch = scene.geometry.batches.len();
                for b in &prototype.batches {
                    let mut draw = b.clone();
                    draw.model = p.model;
                    draw.placement_index = Some(index);
                    scene.geometry.batches.push(draw);
                }
                for v in &scene.geometry.vertices[prototype.vertices.clone()] {
                    let world = transform::point(&p.model, v.position)?;
                    for (axis, value) in world.into_iter().enumerate() {
                        scene.geometry.bounds[0][axis] = scene.geometry.bounds[0][axis].min(value);
                        scene.geometry.bounds[1][axis] = scene.geometry.bounds[1][axis].max(value);
                    }
                }
                p.status = Some(PlacementState::Loaded {
                    first_batch,
                    batch_count: prototype.batches.len(),
                });
                scene.placements_loaded += 1;
                scene.render_placements += 1;
            }
            Ok(CachedEntity::Navigation(nav_mesh)) => {
                let graph = &scene.nav_meshes[*nav_mesh];
                p.status = Some(PlacementState::Navigation {
                    nav_mesh: *nav_mesh,
                    vertices: graph.vertices.len(),
                    triangles: graph.triangles.len(),
                });
                scene.placements_loaded += 1;
                scene.navigation_placements += 1;
            }
            Err(unsupported) => {
                p.status = Some(PlacementState::Unsupported {
                    entity_type: unsupported.entity_type,
                    address: unsupported.address,
                })
            }
        }
    }
    scene.placements_not_loaded = placements.len() as u32 - scene.placements_loaded as u32;
    scene.placements = placements;
    Ok(scene)
}

struct CachedPrototype {
    batches: Vec<Batch>,
    vertices: std::ops::Range<usize>,
}

enum CachedEntity {
    Geometry(CachedPrototype),
    Navigation(usize),
}

fn read_placed_entity(
    data: &[u8],
    tables: &[Table],
    textures: &[Texture],
    hash: u32,
    scene: &mut Scene,
) -> Result<CachedEntity> {
    let entity = crate::edb::read_entity(data, tables, hash)?;
    if entity.entity_type == 0x607 {
        let graph = crate::graph::read(data, entity)?;
        let total_vertices = scene
            .nav_meshes
            .iter()
            .map(|g| g.vertices.len())
            .sum::<usize>()
            + graph.vertices.len();
        let total_triangles = scene
            .nav_meshes
            .iter()
            .map(|g| g.triangles.len())
            .sum::<usize>()
            + graph.triangles.len();
        ensure!(
            total_vertices <= 4_000_000 && total_triangles <= 4_000_000,
            "navigation geometry budget exceeded"
        );
        let index = scene.nav_meshes.len();
        scene.nav_meshes.push(graph);
        Ok(CachedEntity::Navigation(index))
    } else {
        let geometry = read_entity_geometry(data, tables, textures, entity.address as usize)?;
        Ok(CachedEntity::Geometry(append_prototype(
            &mut scene.geometry,
            geometry,
        )?))
    }
}

fn append_prototype(target: &mut Geometry, prototype: Geometry) -> Result<CachedPrototype> {
    ensure!(
        target.texture_bindings == prototype.texture_bindings,
        "prototype texture table differs from scene"
    );
    let vertex_base = target.vertices.len();
    let index_base = target.indices.len();
    ensure!(
        vertex_base + prototype.vertices.len() <= 4_000_000
            && index_base + prototype.indices.len() <= 12_000_000,
        "scene shared geometry budget exceeded"
    );
    let vertices = vertex_base..vertex_base + prototype.vertices.len();
    target.vertices.extend(prototype.vertices);
    target.indices.extend(
        prototype
            .indices
            .into_iter()
            .map(|i| i + vertex_base as u32),
    );
    target.meshes += prototype.meshes;
    target.splits += prototype.splits;
    target.zone_links += prototype.zone_links;
    target.shared_visits += prototype.shared_visits;
    let batches = prototype
        .batches
        .into_iter()
        .map(|mut b| {
            b.first_index += index_base as u32;
            b
        })
        .collect();
    Ok(CachedPrototype { batches, vertices })
}

pub(crate) fn map_address(b: Bytes<'_>, tables: &[Table], hash: u32) -> Result<usize> {
    ensure!(tables.len() == 16, "invalid EDB directory count");
    let table = &tables[6];
    b.range(table.offset, table.count as usize * 16)?;
    let record = (0..table.count as usize)
        .map(|i| table.offset + i * 16)
        .find(|&p| b.u32(p).ok() == Some(hash))
        .context("map hash not found")?;
    let address = b.u32(record + 8)? as usize;
    b.range(address, 0x88)?;
    ensure!(b.u32(address)? == 0x500, "unsupported map body type");
    Ok(address)
}

pub(crate) fn read_placements(data: &[u8], tables: &[Table], hash: u32) -> Result<Vec<Placement>> {
    let b = Bytes(data);
    let map = map_address(b, tables, hash)?;
    let count = b.u32(map + 0x48)? as usize;
    ensure!(count <= 1_000_000, "invalid placement count");
    let start = b.relative(map + 0x4c)?;
    b.range(start, count * 60)?;
    let mut placements = Vec::with_capacity(count);
    for i in 0..count {
        let p = start + i * 60;
        let position = vector(b, p + 4)?;
        let rotation = vector(b, p + 20)?;
        let scale = vector(b, p + 32)?;
        placements.push(Placement {
            hash: b.u32(p)?,
            position,
            flags: b.u32(p + 16)?,
            rotation,
            scale,
            engine_flags: b.u16(p + 44)?,
            map_on: b.u16(p + 46)?,
            object_hash: b.u32(p + 48)?,
            light_set: b.u16(p + 52)?,
            group: b.u16(p + 54)? as i16,
            unknown_word: b.u32(p + 56)?,
            model: transform::placement(position, rotation, scale)?,
            status: None,
        });
    }
    Ok(placements)
}

pub(crate) fn read_entity_geometry(
    data: &[u8],
    tables: &[Table],
    textures: &[Texture],
    address: usize,
) -> Result<Geometry> {
    let b = Bytes(data);
    let mut geometry = Geometry::new(read_material_references(b, tables, textures)?, textures);
    let refs = &tables[1];
    b.range(refs.offset, refs.count as usize * 16)?;
    Reader {
        b,
        refs,
        geometry: &mut geometry,
        active: BTreeSet::new(),
        seen: BTreeSet::new(),
    }
    .visit(address, 0)?;
    ensure!(
        !geometry.indices.is_empty(),
        "placement entity has no supported triangles"
    );
    Ok(geometry)
}

fn vector(b: Bytes<'_>, offset: usize) -> Result<[f32; 3]> {
    Ok([b.f32(offset)?, b.f32(offset + 4)?, b.f32(offset + 8)?])
}
fn bounds(b: Bytes<'_>, offset: usize) -> Result<[[f32; 3]; 2]> {
    Ok([vector(b, offset)?, vector(b, offset + 12)?])
}

pub(crate) fn read(
    data: &[u8],
    tables: &[Table],
    textures: &[Texture],
    hash: u32,
) -> Result<Scene> {
    ensure!(tables.len() == 16, "invalid EDB directory count");
    let b = Bytes(data);
    let maps = &tables[6];
    b.range(maps.offset, maps.count as usize * 16)?;
    let map = (0..maps.count as usize)
        .map(|i| maps.offset + i * 16)
        .find(|&p| b.u32(p).ok() == Some(hash))
        .context("map hash not found")?;
    let address = b.u32(map + 8)? as usize;
    b.range(address, 0x88)?;
    ensure!(b.u32(address)? == 0x500, "unsupported map body type");
    let zone_count = b.u32(address + 0x84)? as usize;
    ensure!(
        zone_count > 0 && zone_count <= 65536,
        "invalid map zone count"
    );
    b.range(address + 0x88, zone_count * 0x8c)?;
    let materials = read_material_references(b, tables, textures)?;
    let mut scene = Scene {
        map_hash: hash,
        zones: vec![],
        placements_not_loaded: b.u32(address + 0x48)?,
        placements_loaded: 0,
        render_placements: 0,
        navigation_placements: 0,
        nav_meshes: vec![],
        placements: vec![],
        geometry: Geometry::new(materials, textures),
    };
    let refs = &tables[1];
    b.range(refs.offset, refs.count as usize * 16)?;
    let mut reader = Reader {
        b,
        refs,
        geometry: &mut scene.geometry,
        active: BTreeSet::new(),
        seen: BTreeSet::new(),
    };
    for i in 0..zone_count {
        let zone = address + 0x88 + i * 0x8c;
        let reference = b.u32(zone)?;
        scene.zones.push(Zone {
            reference,
            identifier_offset: b.relative(zone + 4)?,
            bounds: bounds(b, zone + 0x68)?,
        });
        let target = reader.reference(reference)?;
        reader
            .visit(target, 0)
            .with_context(|| format!("map zone {i}, entity {target:#x}"))?;
    }
    ensure!(
        !scene.geometry.indices.is_empty(),
        "map contains no supported triangles"
    );
    Ok(scene)
}

struct Reader<'a, 'b> {
    b: Bytes<'a>,
    refs: &'a Table,
    geometry: &'b mut Geometry,
    active: BTreeSet<usize>,
    seen: BTreeSet<usize>,
}

impl Reader<'_, '_> {
    fn reference(&self, index: u32) -> Result<usize> {
        ensure!(
            index < self.refs.count as u32,
            "entity reference index out of bounds"
        );
        let entry = self.refs.offset + index as usize * 16;
        ensure!(
            self.b.u32(entry)? == 0,
            "external entity references are not implemented"
        );
        Ok(self.b.u32(entry + 8)? as usize)
    }
    fn visit(&mut self, address: usize, depth: usize) -> Result<()> {
        ensure!(
            depth < 128 && self.seen.len() < 65536,
            "entity graph limit exceeded"
        );
        ensure!(!self.active.contains(&address), "cyclic entity graph");
        if !self.seen.insert(address) {
            self.geometry.shared_visits += 1;
            return Ok(());
        }
        self.active.insert(address);
        match self.b.u32(address)? {
            0x601 => self.mesh(address)?,
            0x603 => {
                self.geometry.splits += 1;
                let count = self.b.u32(address + 0x54)? as usize;
                ensure!(count <= 4096, "invalid split child count");
                // Empty splits occur in mission logic maps. Native traversal
                // tests the count before touching optional data or child words.
                if count != 0 {
                    self.b.range(address + 0x5c, count * 4)?;
                    for i in 0..count {
                        let child = self.b.relative(address + 0x5c + i * 4)?;
                        self.visit(child, depth + 1)?;
                    }
                }
            }
            0x608 => {
                self.geometry.zone_links += 1;
                let target = self.reference(self.b.u32(address + 0x58)?)?;
                self.visit(target, depth + 1)?;
            }
            kind => {
                return Err(UnsupportedEntity {
                    entity_type: kind,
                    address,
                }
                .into());
            }
        }
        self.active.remove(&address);
        Ok(())
    }
    fn mesh(&mut self, address: usize) -> Result<()> {
        let b = self.b;
        b.range(address, 0xb4)?;
        let flags = b.u32(address + 4)?;
        let positions = b.relative(address + 0x5c)?;
        let uv_pool = b.relative(address + 0x60)?;
        let color_pool = b.relative(address + 0x64)?;
        let batches = b.u32(address + 0xa4)? as usize;
        let position_count = b.u32(address + 0xa8)? as usize;
        let format = b.u32(address + 0xb0)?;
        ensure!(
            format & 0x0fffffff == 0x203,
            "unsupported Wii vertex-index format {format:#x}"
        );
        ensure!(
            position_count > 0 && position_count <= 65536 && batches <= 65536,
            "invalid mesh counts"
        );
        b.range(positions, position_count * 16)?;
        let uv_scale = (1u32 << (16 - ((format >> 28) & 7))) as f32;
        let local_bindings = if flags & 1 != 0 {
            let list = b.relative(address + 0x54)?;
            let count = b.u16(list)? as usize;
            b.range(list + 2, count * 2)?;
            (0..count)
                .map(|i| b.u16(list + 2 + i * 2))
                .collect::<Result<Vec<_>>>()?
        } else {
            Vec::new()
        };
        let mut batch = b.relative(address + 0x58)?;
        for _ in 0..batches {
            b.range(batch, 32)?;
            let declared_triangles = b.u16(batch)?;
            let raw_binding_index = b.u16(batch + 2)?;
            let binding = if flags & 1 != 0 {
                *local_bindings
                    .get(raw_binding_index as usize)
                    .context("mesh local texture index out of bounds")?
            } else {
                raw_binding_index
            } as usize;
            ensure!(
                binding < self.geometry.texture_bindings.len(),
                "mesh texture index {binding} out of bounds at {address:#x}"
            );
            let data_bytes = b.u32(batch + 8)? as usize;
            let stream = b.range(batch + 32, data_bytes)?;
            let first_index = self.geometry.indices.len() as u32;
            let mut cursor = 0;
            while cursor < stream.len() {
                let s = Bytes(stream);
                let command = s.u16(cursor)?;
                if command == 0 {
                    ensure!(
                        stream[cursor..].iter().all(|&v| v == 0),
                        "nonzero data after strip padding"
                    );
                    break;
                }
                ensure!(
                    command == 0x98,
                    "unsupported indexed primitive {command:#x}"
                );
                let corners = s.u16(cursor + 2)? as usize;
                ensure!(corners >= 3, "strip has fewer than three corners");
                s.range(cursor + 4, corners * 8)?;
                ensure!(
                    self.geometry.vertices.len() + corners <= 4_000_000
                        && self.geometry.indices.len() + (corners - 2) * 3 <= 12_000_000,
                    "scene geometry budget exceeded"
                );
                let first = self.geometry.vertices.len() as u32;
                for i in 0..corners {
                    let c = cursor + 4 + i * 8;
                    let position_index = s.u16(c)?;
                    let normal_index = s.u16(c + 2)?;
                    let color_index = s.u16(c + 4)? as usize;
                    let uv_index = s.u16(c + 6)? as usize;
                    ensure!(
                        (position_index as usize) < position_count,
                        "mesh position index out of bounds"
                    );
                    let source = positions + position_index as usize * 16;
                    let position = vector(b, source)?;
                    let uv = [
                        b.u16(uv_pool + uv_index * 4)? as i16 as f32 / uv_scale,
                        b.u16(uv_pool + uv_index * 4 + 2)? as i16 as f32 / uv_scale,
                    ];
                    let color = b.range(color_pool + color_index * 4, 4)?.try_into()?;
                    for (axis, &value) in position.iter().enumerate() {
                        self.geometry.bounds[0][axis] = self.geometry.bounds[0][axis].min(value);
                        self.geometry.bounds[1][axis] = self.geometry.bounds[1][axis].max(value);
                    }
                    self.geometry.vertices.push(Vertex {
                        position,
                        uv,
                        color,
                        color_index: color_index as u16,
                        position_index,
                        normal_index,
                        packed_normal_raw: b.u32(source + 12)?,
                    });
                }
                for triangle in 0..corners as u32 - 2 {
                    let order = if triangle % 2 == 0 {
                        [triangle, triangle + 1, triangle + 2]
                    } else {
                        [triangle + 1, triangle, triangle + 2]
                    };
                    self.geometry.indices.extend(order.map(|i| first + i));
                }
                cursor += 4 + corners * 8;
            }
            let index_count = self.geometry.indices.len() as u32 - first_index;
            ensure!(
                index_count / 3 == declared_triangles as u32,
                "declared/decoded triangle counts differ"
            );
            self.geometry.batches.push(Batch {
                model: transform::IDENTITY,
                placement_index: None,
                entity_address: address,
                entity_flags: flags,
                index_format_raw: format,
                binding_index: binding,
                raw_binding_index,
                flags: b.u16(batch + 4)?,
                transparency: b.u16(batch + 6)?,
                declared_triangles,
                unknown_words: [
                    b.u32(batch + 12)?,
                    b.u32(batch + 16)?,
                    b.u32(batch + 20)?,
                    b.u32(batch + 24)?,
                    b.u32(batch + 28)?,
                ],
                first_index,
                index_count,
            });
            batch += 32 + data_bytes;
        }
        self.geometry.meshes += 1;
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests/scene.rs"]
mod tests;

fn read_material_references(
    b: Bytes<'_>,
    tables: &[Table],
    textures: &[Texture],
) -> Result<Vec<MaterialReference>> {
    let material_table = &tables[14];
    b.range(material_table.offset, material_table.count as usize * 16)?;
    let mut materials = Vec::new();
    for i in 0..material_table.count as usize {
        let entry = material_table.offset + i * 16;
        let body = b.u32(entry + 8)? as usize;
        b.range(body, 12)?;
        let file = b.u32(body)?;
        let texture_index = b.u32(body + 4)?;
        let texture_hash = if file == u32::MAX {
            Some(
                textures
                    .get(texture_index as usize)
                    .context("material texture index out of bounds")?
                    .hash,
            )
        } else {
            None
        };
        materials.push(MaterialReference {
            hash: b.u32(entry)?,
            section: b.u16(entry + 4)?,
            debug: b.u16(entry + 6)?,
            runtime_pointer_raw: b.u32(entry + 12)?,
            file_hash: (file != u32::MAX).then_some(file),
            texture_index,
            texture_hash,
            unknown_word: b.u32(body + 8)?,
        });
    }
    Ok(materials)
}
