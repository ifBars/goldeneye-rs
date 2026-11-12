//! Checked v263 metadata and texture records. Unknown sections remain descriptors.
use crate::{
    binary::Bytes,
    tpl::{RgbaImage, Tpl},
};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
mod script_reference;

const TABLE_NAMES: [&str; 16] = [
    "sections",
    "references",
    "entities",
    "animations",
    "skins",
    "animation_scripts",
    "maps",
    "animation_modes",
    "animation_sets",
    "particles",
    "swooshes",
    "spreadsheets",
    "fonts",
    "force_feedback",
    "materials",
    "textures",
];

#[derive(Debug, Serialize)]
pub struct Header {
    pub hash: u32,
    pub version: u32,
    pub flags: u32,
    pub timestamp_raw: u32,
    pub payload_bytes: u32,
    pub base_bytes: u32,
    pub platform_words: [u32; 6],
    pub reserved_words: [u32; 3],
}

#[derive(Debug, Serialize)]
pub struct Table {
    pub name: &'static str,
    pub count: u16,
    pub hash_size_raw: i16,
    pub offset: usize,
}

#[derive(Debug, Serialize)]
pub struct Texture {
    pub hash: u32,
    pub section: u16,
    pub debug: u16,
    pub address: u32,
    pub runtime_pointer_raw: u32,
    pub width: u16,
    pub height: u16,
    pub game_flags: u32,
    pub flags: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Entity {
    pub hash: u32,
    pub section: u16,
    pub debug: u16,
    pub address: u32,
    pub runtime_pointer_raw: u32,
    pub unknown_word: u32,
    pub entity_type: u32,
}
#[derive(Debug, Serialize)]
pub struct EntityReference {
    pub directory_index: usize,
    pub entity: Entity,
}

/// Opaque table-five descriptor; the script body and live component are not decoded.
#[derive(Debug, Serialize)]
pub struct AnimationScriptReference {
    pub directory_index: usize,
    pub hash: u32,
    pub section: u16,
    pub debug: u16,
    pub address: u32,
    pub runtime_pointer_raw: u32,
}

#[derive(Debug, Serialize)]
pub struct TextureInfo {
    pub width: u16,
    pub height: u16,
    pub depth: u16,
    pub game_flags: u16,
    pub scroll: [i16; 2],
    pub frame_count: u8,
    pub image_count: u8,
    pub frame_rate: u8,
    pub values_used: u8,
    pub regions_count: u8,
    /// Number of levels after the base image (observed v263 Wii convention).
    pub extra_mips: u8,
    pub engine_format: u8,
    pub unknown_word: u32,
    pub color: [u8; 4],
    pub external_file: Option<u32>,
    pub data_bytes_per_image: u32,
    pub frames: Vec<u32>,
}

#[derive(Debug, Serialize)]
pub struct Edb {
    pub header: Header,
    pub tables: Vec<Table>,
    pub textures: Vec<Texture>,
    #[serde(skip)]
    data: Vec<u8>,
}

impl Edb {
    pub fn map_common_shape(
        &self,
        reference: u32,
        raw_selector: u32,
        budget: crate::mission::common_shapes::Budget,
    ) -> Result<Option<crate::mission::common_shapes::Binding>> {
        ensure!(
            self.header.version == 263,
            "common shape requires supported EDB version"
        );
        crate::mission::common_shapes::read(
            &self.data,
            self.tables.get(6).context("missing map directory")?,
            reference,
            raw_selector,
            budget,
        )
    }

    /// Source type0 fields for an explicit shape geometry consumer. The view
    /// borrows this owner; metadata-only common initialization remains lazy.
    pub fn map_common_shape_geometry(
        &self,
        reference: u32,
        raw_selector: u32,
        budget: crate::mission::common_shapes::Budget,
    ) -> Result<Option<crate::mission::common_shapes::Type0Geometry<'_>>> {
        ensure!(
            self.header.version == 263,
            "common shape geometry requires supported EDB version"
        );
        crate::mission::common_shapes::read_geometry(
            &self.data,
            self.tables.get(6).context("missing map directory")?,
            reference,
            raw_selector,
            budget,
        )
    }

    pub fn map_animator(
        &self,
        reference: u32,
        index: usize,
        budget: crate::map_animator::Budget,
    ) -> Result<Option<crate::map_animator::Animator<'_>>> {
        ensure!(
            self.header.version == 263,
            "map animator requires supported EDB version"
        );
        crate::map_animator::read(
            &self.data,
            self.tables.get(6).context("missing map directory")?,
            reference,
            index,
            budget,
        )
    }

    pub fn mission_common_triggers(
        &self,
        reference: u32,
        budget: crate::mission::common_triggers::Budget,
    ) -> Result<Option<crate::mission::common_triggers::View<'_>>> {
        ensure!(
            self.header.version == 263,
            "common triggers require supported EDB version"
        );
        crate::mission::common_triggers::read(
            &self.data,
            self.tables.get(6).context("missing map directory")?,
            reference,
            budget,
        )
    }
    /// Read skeletal metadata and its compressed span, without frame decoding.
    pub fn skeletal_animation(
        &self,
        reference: u32,
        budget: crate::skeletal_animation::Budget,
    ) -> Result<Option<crate::skeletal_animation::Animation>> {
        ensure!(
            self.header.version == 263,
            "skeletal animation requires supported EDB version"
        );
        crate::skeletal_animation::read(
            &self.data,
            self.tables.get(3).context("missing animation directory")?,
            self.header.hash,
            self.header.payload_bytes,
            reference,
            budget,
        )
    }

    pub(crate) fn skeletal_animation_payload(
        &self,
        animation: &crate::skeletal_animation::Animation,
    ) -> Result<&[u8]> {
        crate::skeletal_animation::payload(&self.data, animation)
    }

    /// Runtime view restricted to the logically resident base. Cached later
    /// sections are unavailable to its lazy directory/command reads.
    pub fn base_animation_modes(&self) -> Result<crate::animation_modes::View<'_>> {
        ensure!(
            self.header.version == 263,
            "animation modes require supported EDB version"
        );
        let data = self
            .data
            .get(..usize::try_from(self.header.base_bytes)?)
            .context("animation base exceeds retained file")?;
        crate::animation_modes::read(data, &self.tables)
    }

    /// Retained-owner lazy mode/set access. No animation is selected or sampled.
    pub fn animation_modes(&self) -> Result<crate::animation_modes::View<'_>> {
        ensure!(
            self.header.version == 263,
            "animation modes require supported EDB version"
        );
        crate::animation_modes::read(&self.data, &self.tables)
    }

    /// Bind a checked visual body without reading topology or runtime caches.
    pub fn skin_body(
        &self,
        reference: u32,
        maximum_skins: usize,
    ) -> Result<Option<crate::skin::Body>> {
        ensure!(
            self.header.version == 263,
            "visual skin requires supported EDB version"
        );
        crate::skin::body_read(
            &self.data,
            self.tables.get(4).context("missing skin directory")?,
            self.header.hash,
            reference,
            maximum_skins,
        )
    }

    /// Read skin topology from table four using the native coded/hash resolver.
    pub fn skin(
        &self,
        reference: u32,
        budget: crate::skin::Budget,
    ) -> Result<Option<crate::skin::Skin>> {
        ensure!(
            self.header.version == 263,
            "skin requires supported EDB version"
        );
        crate::skin::read(
            &self.data,
            self.tables.get(4).context("missing skin directory")?,
            self.header.hash,
            self.header.payload_bytes,
            reference,
            budget,
        )
    }

    /// Locate an observed skin/texture target's section in this same owner.
    /// A supplied payload view is not an original streaming-readiness result.
    pub fn target_section(
        &self,
        reference: u32,
        budget: crate::target_section::Budget,
    ) -> Result<Option<crate::target_section::View<'_>>> {
        ensure!(
            self.header.version == 263,
            "target section requires supported EDB version"
        );
        crate::target_section::read(
            &self.data,
            &self.tables,
            self.header.hash,
            reference,
            budget,
        )
    }

    pub(crate) fn skin_tag(
        &self,
        skin: &crate::skin::Skin,
        tag: u32,
        budget: usize,
    ) -> Result<Option<crate::skin::Tag>> {
        crate::skin::find_tag(&self.data, skin, tag, budget)
    }

    /// Read only the reached skin texture traversal, without loading topology.
    pub fn skin_texture_dependencies(
        &self,
        reference: u32,
        budget: crate::skin::TextureBudget,
    ) -> Result<Option<crate::skin::TextureDependencies>> {
        ensure!(
            self.header.version == 263,
            "skin textures require supported EDB version"
        );
        crate::skin::textures_read(&self.data, &self.tables, reference, budget)
    }
    pub(crate) fn skin_texture_dependencies_for_skin(
        &self,
        skin: &crate::skin::Skin,
        budget: crate::skin::TextureBudget,
    ) -> Result<crate::skin::TextureDependencies> {
        crate::skin::validate_skin(&self.data, skin)?;
        crate::skin::textures_enumerate_at(
            &self.data,
            self.tables.get(2).context("missing entity directory")?,
            self.tables.get(15).context("missing texture directory")?,
            skin.descriptor().address,
            budget,
        )
    }

    pub(crate) fn skin_auxiliary_count(&self, skin: &crate::skin::Skin) -> Result<u32> {
        crate::skin::auxiliary_count(&self.data, skin)
    }

    pub(crate) fn skin_fallback_translations(
        &self,
        skin: &crate::skin::Skin,
        maximum_bones: usize,
    ) -> Result<crate::skin::TranslationDefaults> {
        crate::skin::fallback_translations(&self.data, skin, maximum_bones)
    }

    pub(crate) fn skeletal_animation_root_bias(
        &self,
        animation: &crate::skeletal_animation::Animation,
    ) -> Result<[u32; 3]> {
        crate::skeletal_animation::root_bias(&self.data, animation)
    }

    pub(crate) fn skeletal_animation_consumer_flags(
        &self,
        animation: &crate::skeletal_animation::Animation,
    ) -> Result<u32> {
        crate::skeletal_animation::consumer_flags(&self.data, animation)
    }

    /// Source StartPoint records for an explicitly selected map. This only
    /// decodes data; registration filters and live catalog membership are separate.
    pub fn mission_start_points(
        &self,
        reference: u32,
        budget: crate::mission::startpoints::Budget,
    ) -> Result<Option<Vec<crate::mission::startpoints::StartPoint>>> {
        crate::mission::startpoints::read(
            &self.data,
            self.tables
                .get(6)
                .context("missing mission map directory")?,
            reference,
            budget,
        )
    }

    /// Native coded-map lookup and first source linked-map trigger. This does
    /// not activate triggers, load the returned resource or register worlds.
    pub fn mission_linked_map(
        &self,
        reference: u32,
        budget: crate::mission::maps::Budget,
    ) -> Result<Option<crate::mission::maps::LinkedMap>> {
        crate::mission::maps::read(
            &self.data,
            self.tables
                .get(6)
                .context("missing mission map directory")?,
            reference,
            budget,
        )
    }

    /// Read bounded English text through the resource's mapped text pages.
    pub fn frontend_text(
        &self,
        hash: u32,
        budget: crate::frontend_text::Budget,
    ) -> Result<Option<crate::frontend_text::Text>> {
        crate::frontend_text::read(
            &self.data,
            self.tables.first().context("missing section directory")?,
            self.tables.get(1).context("missing reference directory")?,
            self.tables
                .get(11)
                .context("missing spreadsheet directory")?,
            hash,
            budget,
        )
    }
    /// Resolve one checked frontend script view without interpreting commands.
    pub fn frontend_script(
        &self,
        reference: u32,
    ) -> Result<Option<crate::frontend_script::Script<'_>>> {
        crate::frontend_script::read(
            &self.data,
            self.tables.get(5).context("missing script directory")?,
            reference,
        )
    }
    /// Read a local font through its native table-twelve descriptor.
    pub fn frontend_font(
        &self,
        reference: u32,
    ) -> Result<Option<crate::frontend_font::FrontendFont>> {
        crate::frontend_font::read(
            &self.data,
            self.tables.get(12).context("missing font directory")?,
            reference,
        )
    }

    /// Lazy spreadsheet view through native table-eleven twenty-byte records.
    pub fn spreadsheet(
        &self,
        reference: u32,
    ) -> Result<Option<crate::spreadsheet::Spreadsheet<'_>>> {
        crate::spreadsheet::read(
            &self.data,
            self.tables
                .get(11)
                .context("missing spreadsheet directory")?,
            reference,
        )
    }
    /// Native table-five lookup with 16-byte descriptors. Type-0x04 actor scripts
    /// are distinct from table-two entity geometry, despite owning live actors.
    pub fn animation_script_reference(
        &self,
        reference: u32,
    ) -> Result<Option<AnimationScriptReference>> {
        let table = self
            .tables
            .get(5)
            .context("missing animation script directory")?;
        let Some(descriptor) = script_reference::metadata(&self.data, table, reference)? else {
            return Ok(None);
        };
        Bytes(&self.data)
            .range(descriptor.address as usize, 4)
            .context("script body address outside resource")?;
        Ok(Some(descriptor))
    }
    /// Metadata lookup against the logically resident base, before body access.
    pub fn base_animation_script_reference(
        &self,
        reference: u32,
    ) -> Result<Option<AnimationScriptReference>> {
        ensure!(
            self.header.version == 263,
            "live scripts require supported EDB version"
        );
        let base = self
            .data
            .get(..usize::try_from(self.header.base_bytes)?)
            .context("script base exceeds retained file")?;
        script_reference::metadata(
            base,
            self.tables
                .get(5)
                .context("missing animation script directory")?,
            reference,
        )
    }
    /// Reached cached descriptor's body. A cached later section grants no residence.
    pub fn base_animation_script_body(
        &self,
        index: usize,
    ) -> Result<Option<crate::frontend_script::Script<'_>>> {
        ensure!(
            index < 32767,
            "script descriptor index exceeds native directory bound"
        );
        let descriptor = self
            .base_animation_script_reference(0x8400_0000 | u32::try_from(index)?)?
            .context("cached script descriptor is absent")?;
        ensure!(
            descriptor.section & 0x7fff == 0,
            "live script requires non-base residency provider"
        );
        if descriptor.address == 0 {
            return Ok(None);
        }
        let base = self
            .data
            .get(..usize::try_from(self.header.base_bytes)?)
            .context("script base exceeds retained file")?;
        Ok(Some(crate::frontend_script::read_at(
            base,
            descriptor.address as usize,
        )?))
    }
    /// Native coded reference lookup, distinct from the CLI's strict literal hash selectors.
    pub fn entity_reference(&self, reference: u32) -> Result<Option<EntityReference>> {
        let table = self.tables.get(2).context("missing entity directory")?;
        let Some(directory_index) = crate::reference::resolve(&self.data, table, 20, reference)?
        else {
            return Ok(None);
        };
        let entity = self
            .entities()?
            .get(directory_index)
            .cloned()
            .context("resolved entity reference outside directory")?;
        Ok(Some(EntityReference {
            directory_index,
            entity,
        }))
    }
    /// Resolve a native entity reference without allocating the full descriptor
    /// list. Batch consumers can retain one checked directory and reuse it.
    pub fn entity_reference_index(&self, reference: u32) -> Result<Option<usize>> {
        crate::reference::resolve(
            &self.data,
            self.tables.get(2).context("missing entity directory")?,
            20,
            reference,
        )
    }
    /// Entity section metadata only; does not read the body address or type.
    pub fn entity_section_at(&self, index: usize) -> Result<usize> {
        crate::mission::map_runtime::entity_section(
            &self.data,
            self.tables.get(2).context("missing entity directory")?,
            index,
        )
    }
    /// Root entity descriptors only. Reading a type tag does not validate its body.
    pub fn entities(&self) -> Result<Vec<Entity>> {
        read_entities(&self.data, &self.tables)
    }
    /// Decode one selected entity through the same checked geometry reader as
    /// placements. Selection does not imply script placement or material parity.
    pub fn entity_geometry_at(&self, index: usize) -> Result<crate::scene::Geometry> {
        let entity = read_entity_at(&self.data, &self.tables, index)?;
        crate::scene::read_entity_geometry(
            &self.data,
            &self.tables,
            &self.textures,
            entity.address as usize,
        )
    }
    pub fn frontend_color_frames(
        &self,
        address: usize,
    ) -> Result<Option<crate::frontend_morph::ColorFrames<'_>>> {
        crate::frontend_morph::read_at(&self.data, address, 64)
    }

    /// Common stored bounds/flags for identified static entities. Does not
    /// compute live placement bounds or follow component/shape references.
    pub fn entity_spatial_at(&self, index: usize) -> Result<crate::entity_spatial::SpatialHeader> {
        crate::entity_spatial::read(&self.data, read_entity_at(&self.data, &self.tables, index)?)
    }

    /// Resolve one compact component slot without flattening or interpreting its body.
    pub fn entity_component_at(
        &self,
        index: usize,
        slot: u8,
    ) -> Result<Option<crate::entity_spatial::Component>> {
        crate::entity_spatial::component(
            &self.data,
            read_entity_at(&self.data, &self.tables, index)?,
            slot,
        )
    }
    /// Common bounds of a known static component. Unknown types and malformed
    /// addressed bodies are explicit errors; absent mask bits remain absent.
    pub fn component_spatial_at(
        &self,
        index: usize,
        slot: u8,
    ) -> Result<Option<crate::entity_spatial::SpatialHeader>> {
        self.entity_component_at(index, slot)?
            .map(|component| crate::entity_spatial::read_at(&self.data, component.address))
            .transpose()
    }
    /// Native slot-six bounds aggregate, separate from ordinary spatial headers
    /// and from a complete collision-shape decoder.
    pub fn entity_shape_bounds_at(
        &self,
        index: usize,
    ) -> Result<Option<crate::shape_bounds::ShapeBounds>> {
        self.entity_component_at(index, 6)?
            .map(|component| crate::shape_bounds::read(&self.data, component.address))
            .transpose()
    }

    /// Static mesh-local collision inputs, with literal hash selection.
    pub fn collision_mesh(&self, hash: u32) -> Result<crate::collision::CollisionMesh> {
        crate::collision::read(&self.data, read_entity(&self.data, &self.tables, hash)?)
    }
    /// Directory selection retains duplicate root hashes and avoids rescanning
    /// every descriptor when a corpus consumer already has a directory index.
    pub fn collision_mesh_at(&self, index: usize) -> Result<crate::collision::CollisionMesh> {
        crate::collision::read(&self.data, read_entity_at(&self.data, &self.tables, index)?)
    }
    /// Decode an addressed non-root body from a collected geometry record.
    /// No root directory identity or hash is manufactured for embedded meshes.
    pub fn collision_mesh_at_address(
        &self,
        address: usize,
    ) -> Result<crate::collision::CollisionMesh> {
        crate::collision::read_at(&self.data, address)
    }
    /// Static resident plane initialization reads included disabled corners too.
    pub fn collision_line_mesh_at_address(
        &self,
        address: usize,
    ) -> Result<crate::collision::CollisionMesh> {
        crate::collision::read_line_at(&self.data, address)
    }
    pub fn geometry_line_coarse_at_address(
        &self,
        address: usize,
    ) -> Result<Option<crate::coarse::View<'_>>> {
        crate::coarse::read_geometry_at(&self.data, address)
    }
    pub fn collision_coarse_at_address(
        &self,
        address: usize,
    ) -> Result<Option<crate::coarse::View<'_>>> {
        crate::coarse::read_at(&self.data, address)
    }
    pub fn collision_coarse_at(&self, index: usize) -> Result<Option<crate::coarse::View<'_>>> {
        crate::coarse::read(&self.data, read_entity_at(&self.data, &self.tables, index)?)
    }

    /// Decode 0x607 navigation mesh data without implementing original traversal behavior.
    pub fn entity_graph(&self, hash: u32) -> Result<crate::graph::TriangleGraph> {
        let entity = read_entity(&self.data, &self.tables, hash)?;
        crate::graph::read(&self.data, entity)
    }

    /// Select by root directory index when hashes are duplicated in the source data.
    pub fn entity_graph_at(&self, index: usize) -> Result<crate::graph::TriangleGraph> {
        let entity = self
            .entities()?
            .into_iter()
            .nth(index)
            .context("entity directory index out of bounds")?;
        crate::graph::read(&self.data, entity)
    }
    /// Static map-zone geometry only. Placements, actors, skies and effects remain separate.
    pub fn map_scene(&self, hash: u32) -> Result<crate::scene::Scene> {
        crate::scene::read(&self.data, &self.tables, &self.textures, hash)
    }
    pub fn map_scene_with_placements(
        &self,
        hash: u32,
        policy: crate::scene::PlacementPolicy,
    ) -> Result<crate::scene::Scene> {
        crate::scene::read_with_placements(&self.data, &self.tables, &self.textures, hash, policy)
    }
    pub fn map_placements(&self, hash: u32) -> Result<Vec<crate::scene::Placement>> {
        crate::scene::read_placements(&self.data, &self.tables, hash)
    }
    /// Normalize a native map reference and read its declared bounds without
    /// initializing trigger objects or fitting render geometry.
    pub fn mission_map_descriptor(
        &self,
        reference: u32,
        budget: crate::mission::world::Budget,
    ) -> Result<Option<crate::mission::world::MapDescriptor>> {
        crate::mission::world::read(
            &self.data,
            self.tables.get(6).context("missing map directory")?,
            reference,
            budget,
        )
    }
    pub fn mission_map_runtime(
        &self,
        reference: u32,
        budget: crate::mission::map_runtime::Budget,
    ) -> Result<Option<crate::mission::map_runtime::View<'_>>> {
        crate::mission::map_runtime::read(&self.data, &self.tables, reference, budget)
    }
    /// Static registrations for one selected map, using native coded-reference and key lookup.
    /// Does not establish loaded-map precedence, dynamic mutations or original runtime parity.
    pub fn map_navigation(&self, hash: u32) -> Result<crate::navigation::MapNavigation> {
        crate::navigation::read(&self.data, &self.tables, hash)
    }
    /// Checked BSP/zone inputs for one map. Does not collect world geometry or
    /// infer live animator transforms, portal clearance or collision results.
    pub fn map_spatial(&self, hash: u32) -> Result<crate::spatial::MapSpatial> {
        crate::spatial::read(&self.data, &self.tables, hash)
    }
    /// Only the selected zone reference, without decoding BSPs or other zones.
    pub fn map_zone_entity_reference(&self, hash: u32, zone: usize) -> Result<u32> {
        let map = crate::spatial::map_address(&self.data, &self.tables, hash)?;
        let b = Bytes(&self.data);
        let count = b.u32(map + 0x84)? as usize;
        ensure!(
            count <= 65536 && zone < count,
            "collection zone outside map directory"
        );
        b.u32(map + 0x88 + zone * 0x8c)
    }
    pub fn geometry(&self) -> crate::geometry::GeometryView<'_> {
        crate::geometry::GeometryView::new(&self.data, &self.tables)
    }
    /// Reachable initial hierarchy snapshot, including disabled entries. Runtime
    /// refresh/query preserves separate mutation and membership contracts.
    pub fn map_candidate_hierarchy(
        &self,
        hash: u32,
        zone: usize,
    ) -> Result<Option<crate::candidates::Hierarchy>> {
        crate::candidates::map_read(&self.data, &self.tables, hash, zone)
    }
    /// Ordered grouped portal inputs and only referenced quad/polygon shapes.
    /// No portal distance, world collection or rendered placement inference.
    pub fn map_portals(&self, hash: u32) -> Result<crate::portals::MapPortals> {
        crate::portals::read(&self.data, &self.tables, hash)
    }
    /// Query-only portal view: selected-zone rows and shapes are addressed lazily.
    pub fn map_portal_query(&self, hash: u32) -> Result<crate::portals::QueryView<'_>> {
        crate::portals::query_view(&self.data, &self.tables, hash)
    }
    /// Map hashes in directory order, preserving duplicates for explicit ambiguity handling.
    pub fn map_hashes(&self) -> Result<Vec<u32>> {
        let table = self.tables.get(6).context("missing map directory")?;
        let b = Bytes(&self.data);
        b.range(table.offset, table.count as usize * 16)?;
        (0..table.count as usize)
            .map(|i| b.u32(table.offset + i * 16))
            .collect()
    }
    pub const HEADER_BYTES: usize = 0xc8;
    pub fn read_header(data: &[u8]) -> Result<Header> {
        let b = Bytes(data);
        b.range(0, Self::HEADER_BYTES)?;
        ensure!(b.u32(0)? == 0x47454f4d, "EDB magic is not big-endian GEOM");
        let version = b.u32(8)?;
        ensure!(
            [262, 263].contains(&version),
            "unsupported EDB version {version}"
        );
        let payload_bytes = b.u32(0x14)?;
        let base_bytes = b.u32(0x18)?;
        ensure!(
            base_bytes >= Self::HEADER_BYTES as u32
                && payload_bytes >= base_bytes
                && payload_bytes <= 256 * 1024 * 1024,
            "invalid or oversized EDB payload/base size"
        );
        let mut platform_words = [0; 6];
        let mut reserved_words = [0; 3];
        for (i, word) in platform_words.iter_mut().enumerate() {
            *word = b.u32(0x1c + i * 4)?;
        }
        for (i, word) in reserved_words.iter_mut().enumerate() {
            *word = b.u32(0x34 + i * 4)?;
        }
        Ok(Header {
            hash: b.u32(4)?,
            version,
            flags: b.u32(12)?,
            timestamp_raw: b.u32(16)?,
            payload_bytes,
            base_bytes,
            platform_words,
            reserved_words,
        })
    }

    pub fn parse(data: Vec<u8>) -> Result<Self> {
        let header = Self::read_header(&data)?;
        ensure!(
            data.len() == header.payload_bytes as usize,
            "EDB payload size mismatch"
        );
        let b = Bytes(&data);
        let mut tables = Vec::new();
        for (i, name) in TABLE_NAMES.iter().enumerate() {
            let field = 0x40 + i * 8;
            let count = b.u16(field)?;
            ensure!(count <= i16::MAX as u16, "negative table count for {name}");
            let offset = b.relative(field + 4)?;
            if count > 0 {
                ensure!(
                    offset >= Self::HEADER_BYTES,
                    "{name} table overlaps the EDB header"
                );
                b.range(offset, 4)?;
            }
            tables.push(Table {
                name,
                count,
                hash_size_raw: b.u16(field + 2)? as i16,
                offset,
            });
        }
        let table = &tables[15];
        let records = b.range(table.offset, table.count as usize * 28)?;
        let records = Bytes(records);
        let mut textures = Vec::new();
        for i in 0..table.count as usize {
            let field = i * 28;
            let address = records.u32(field + 8)?;
            b.range(address as usize, 44)?;
            textures.push(Texture {
                hash: records.u32(field)?,
                section: records.u16(field + 4)?,
                debug: records.u16(field + 6)?,
                address,
                runtime_pointer_raw: records.u32(field + 12)?,
                width: records.u16(field + 16)?,
                height: records.u16(field + 18)?,
                game_flags: records.u32(field + 20)?,
                flags: records.u32(field + 24)?,
            });
        }
        Ok(Self {
            header,
            tables,
            textures,
            data,
        })
    }

    pub fn texture_info(&self, hash: u32) -> Result<TextureInfo> {
        let entry = self
            .textures
            .iter()
            .find(|t| t.hash == hash)
            .context("texture hash not found in EDB")?;
        let start = entry.address as usize;
        let b = Bytes(&self.data);
        let body = b.range(start, 44)?;
        let h = Bytes(body);
        let width = h.u16(0)?;
        let height = h.u16(2)?;
        ensure!(width > 0 && height > 0, "zero-sized EDB texture");
        ensure!(
            width == entry.width && height == entry.height,
            "texture table/body dimensions differ"
        );
        let external = h.u32(0x1c)?;
        let image_count = body[13];
        let mut frames = Vec::new();
        // External records retain the target animation's image count, but store one hash.
        let stored_count = if external == u32::MAX {
            image_count as usize
        } else {
            1
        };
        for i in 0..stored_count {
            let field = start + 44 + i * 4;
            frames.push(if external == u32::MAX {
                b.relative(field)? as u32
            } else {
                b.u32(field)?
            });
        }
        Ok(TextureInfo {
            width,
            height,
            depth: h.u16(4)?,
            game_flags: h.u16(6)?,
            scroll: [h.u16(8)? as i16, h.u16(10)? as i16],
            frame_count: body[12],
            image_count,
            frame_rate: body[14],
            values_used: body[16],
            regions_count: body[17],
            extra_mips: body[18],
            engine_format: body[19],
            unknown_word: h.u32(20)?,
            color: body[24..28].try_into()?,
            external_file: (external != u32::MAX).then_some(external),
            data_bytes_per_image: h.u32(40)?,
            frames,
        })
    }

    pub fn texture_tpl(&self, hash: u32, frame: usize) -> Result<Tpl<'_>> {
        let info = self.texture_info(hash)?;
        ensure!(
            info.external_file.is_none(),
            "external texture reference requires resolver (file {:?})",
            info.external_file
        );
        ensure!(info.depth == 1, "only 2D textures are supported");
        let start = *info
            .frames
            .get(frame)
            .context("texture frame out of range")? as usize;
        let data = Bytes(&self.data).range(start, info.data_bytes_per_image as usize)?;
        let tpl = Tpl::parse(data)?;
        ensure!(tpl.images.len() == 1, "expected one embedded TPL image");
        let image = &tpl.images[0];
        // Wii storage rounds logical source sizes up to powers of two, minimum 4.
        // Keep both sizes: sampling/UV policy is separate from decoding stored pixels.
        let expected = (
            u32::from(info.width).max(4).next_power_of_two(),
            u32::from(info.height).max(4).next_power_of_two(),
        );
        ensure!(
            (image.width, image.height) == (info.width, info.height)
                || (u32::from(image.width), u32::from(image.height)) == expected,
            "TPL/EDB dimensions differ: stored {}x{}, logical {}x{}, format {}",
            image.width,
            image.height,
            info.width,
            info.height,
            image.format
        );
        ensure!(
            image.max_lod == info.extra_mips,
            "TPL/EDB mip counts differ"
        );
        Ok(tpl)
    }

    pub fn decode_texture(&self, hash: u32, frame: usize, mip: u8) -> Result<RgbaImage> {
        let info = self.texture_info(hash)?;
        let stored = self.texture_tpl(hash, frame)?.decode(0, mip)?;
        let width = (u32::from(info.width) >> mip).max(1);
        let height = (u32::from(info.height) >> mip).max(1);
        if (stored.width, stored.height) == (width, height) {
            return Ok(stored);
        }
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height as usize {
            let row = y * stored.width as usize * 4;
            pixels.extend_from_slice(&stored.pixels[row..row + width as usize * 4]);
        }
        Ok(RgbaImage {
            width,
            height,
            pixels,
        })
    }
}

#[cfg(test)]
#[path = "tests/edb.rs"]
mod tests;

pub(crate) fn read_entities(data: &[u8], tables: &[Table]) -> Result<Vec<Entity>> {
    let b = Bytes(data);
    let table = tables.get(2).context("missing entity directory")?;
    b.range(table.offset, table.count as usize * 20)?;
    (0..table.count as usize)
        .map(|i| read_entity_at(data, tables, i))
        .collect()
}
fn read_entity_at(data: &[u8], tables: &[Table], index: usize) -> Result<Entity> {
    let b = Bytes(data);
    let table = tables.get(2).context("missing entity directory")?;
    ensure!(
        index < usize::from(table.count),
        "entity directory index out of bounds"
    );
    let p = table
        .offset
        .checked_add(index * 20)
        .context("entity descriptor offset overflow")?;
    b.range(p, 20)?;
    let address = b.u32(p + 8)?;
    Ok(Entity {
        hash: b.u32(p)?,
        section: b.u16(p + 4)?,
        debug: b.u16(p + 6)?,
        address,
        runtime_pointer_raw: b.u32(p + 12)?,
        unknown_word: b.u32(p + 16)?,
        entity_type: b.u32(address as usize)?,
    })
}
pub(crate) fn read_entity(data: &[u8], tables: &[Table], hash: u32) -> Result<Entity> {
    let mut entries = read_entities(data, tables)?
        .into_iter()
        .filter(|e| e.hash == hash);
    let entity = entries
        .next()
        .with_context(|| format!("entity hash {hash:#x} not found"))?;
    ensure!(entries.next().is_none(), "duplicate entity hash");
    Ok(entity)
}
