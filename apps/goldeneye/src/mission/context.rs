use super::{
    metadata::ResourceEvidence, player::PlayerComponents, registration::PreparedRegistration,
    residency::Residency, spawn::PreparedSpawn, world::WorldBindings,
};
use anyhow::{Context, Result, ensure};
use ge_assets::{
    edb::Edb,
    mission::{self, LevelCatalog},
    resource::GameAssets,
};
use ge_core::mission::{Budget, Difficulty, InitialSelection, LevelRegistry, SelectionOutcome};
use serde::Serialize;

const ROW_BUDGET: Budget = Budget {
    primary: 4096,
    secondary: 1024,
};
const SCOPE: &str = "Actual level catalog rows and the selected mission EDB are retained. Initial selection is an isolated host request; original previous-level/profile history is unavailable. Map metadata, the first placement reference per map, the source linked-map trigger and declared script/entity counts are checked. The source file-discovery chain is loaded and retained using file-only deduplication and forced coded map index zero. An explicit one-initialization preference snapshot prepares registration order and filter metadata; original initializer timing/history is unverified. Owned map spatial/BSP and placement data are bound before prepared StartPoint selection. Source StartPoint/initial-pose planning retains actual private pose bits and publishes only a hash; live catalog membership, creation inputs and camera readiness remain unverified. The selected level character also binds its retained registry, first-person component and separate skin; owned CPU bone membership, constructor identity and camera-tag lookup are initialized. Optional prewarm sampling and camera-bone transforms are recorded separately; complete animation scheduling, auxiliary animator state, physics and the gameplay camera remain incomplete. These preparations do not activate or register worlds. No script is executed, no world is registered and no player is initialized. This is resource/selection evidence, not a rendered mission or original gameplay comparison.";

#[derive(Debug, Serialize)]
pub struct CatalogEvidence {
    resource_hash: u32,
    payload_bytes: u32,
    primary_sheet_hash: u32,
    secondary_sheet_hash: u32,
    primary_rows_read: usize,
    secondary_rows_read: usize,
    primary_rows_registered: usize,
    secondary_rows_registered: usize,
    duplicate_rows: usize,
}

#[derive(Debug, Serialize)]
pub struct SelectionEvidence {
    level_hash: u32,
    difficulty: u8,
    difficulty_label: &'static str,
    source_row_index: usize,
    source_row_address: usize,
    text_page_hash: u32,
    mission_object_resource_raw: u32,
    io_resource_raw: u32,
    intro_movie_hash: u32,
    outro_movie_hash: u32,
    bank_hashes: [u32; 3],
    tuning_hash: Option<u32>,
    character_request: u32,
}

#[derive(Debug, Serialize)]
pub struct Evidence {
    state: &'static str,
    catalog: CatalogEvidence,
    selection: SelectionEvidence,
    resource: ResourceEvidence,
    residency: super::residency::Evidence,
    registration: super::registration::RegistrationEvidence,
    spawn: super::spawn::SpawnEvidence,
    map_instances: super::world::Evidence,
    player_components: super::player::Evidence,
    resources_retained: bool,
    world_registered: bool,
    player_initialized: bool,
    reference_compared: bool,
    scope: &'static str,
}

pub struct MissionContext {
    registry: LevelRegistry,
    residency: Residency,
    registration: PreparedRegistration,
    spawn: PreparedSpawn,
    worlds: WorldBindings,
    player_components: PlayerComponents,
    evidence: Evidence,
}

struct ContextInput {
    registry: LevelRegistry,
    residency: Residency,
    player_components: PlayerComponents,
    difficulty: Difficulty,
    catalog_bytes: u32,
    read_counts: (usize, usize),
    first_row: usize,
    requested_id: u32,
}

impl MissionContext {
    pub fn load(assets: &mut GameAssets, level: u32, difficulty: Difficulty) -> Result<Self> {
        Self::load_with_start_point(assets, level, difficulty, 1)
    }

    pub fn load_with_start_point(
        assets: &mut GameAssets,
        level: u32,
        difficulty: Difficulty,
        requested_id: u32,
    ) -> Result<Self> {
        ensure!(
            assets.disc.identity.game_id == "SJBE52" && assets.disc.identity.revision == 0,
            "mission context requires SJBE52 revision zero"
        );
        let definition = super::resource::load(assets, mission::CATALOG_RESOURCE, 1024 * 1024)?;
        let catalog = LevelCatalog::read(
            &definition,
            mission::Budget {
                primary: ROW_BUDGET.primary,
                secondary: ROW_BUDGET.secondary,
            },
        )?;
        let source = catalog.locations();
        let read_counts = (catalog.primary().len(), catalog.secondary().len());
        let (primary, secondary) = catalog.into_rows();
        let mut registry = LevelRegistry::new(
            primary,
            secondary,
            ROW_BUDGET,
            InitialSelection {
                current_hash: u32::MAX,
                previous_hash: u32::MAX,
                primary_hash: None,
                secondary_hash: None,
            },
        )?;
        ensure!(
            matches!(registry.select(level), SelectionOutcome::Selected { .. }),
            "requested level {level:#010x} is absent from the mission catalog"
        );
        let residency = Residency::load(assets, level)?;
        let character_request = registry
            .selected_primary()
            .context("mission character request has no selected level")?
            .character_request();
        let player_components = PlayerComponents::load(assets, character_request)?;
        Self::from_residency(ContextInput {
            registry,
            residency,
            player_components,
            difficulty,
            catalog_bytes: definition.header.payload_bytes,
            read_counts,
            first_row: source.primary_first_row,
            requested_id,
        })
    }

    fn from_residency(input: ContextInput) -> Result<Self> {
        let ContextInput {
            registry,
            residency,
            player_components,
            difficulty,
            catalog_bytes,
            read_counts,
            first_row,
            requested_id,
        } = input;
        let mission = residency
            .resources()
            .first()
            .context("retained root mission resource absent")?;
        let selected = registry
            .selected_primary()
            .context("mission primary selection is absent")?;
        ensure!(
            registry.current_hash() == selected.hash() && mission.header.hash == selected.hash(),
            "retained mission resource differs from selected level"
        );
        ensure!(
            mission.header.version == 263 && mission.header.payload_bytes <= 64 * 1024 * 1024,
            "selected mission resource exceeds supported version or host budget"
        );
        let source_row_address = selected
            .source_index()
            .checked_mul(180)
            .and_then(|offset| first_row.checked_add(offset))
            .context("mission source-row address overflow")?;
        let raw = selected.words_raw();
        ensure!(
            player_components.selected_character()?.key() == selected.character_request(),
            "player component binding differs from selected level character request"
        );
        let selection = SelectionEvidence {
            level_hash: selected.hash(),
            difficulty: difficulty.value(),
            difficulty_label: difficulty.label(),
            source_row_index: selected.source_index(),
            source_row_address,
            text_page_hash: raw[0x10 / 4],
            mission_object_resource_raw: raw[0x14 / 4],
            io_resource_raw: raw[0x34 / 4],
            intro_movie_hash: selected.intro_movie_hash(),
            outro_movie_hash: selected.outro_movie_hash(),
            bank_hashes: selected.bank_hashes(),
            tuning_hash: registry.selected_secondary().map(|row| row.hash()),
            character_request: selected.character_request(),
        };
        let registration =
            PreparedRegistration::from_initialization_snapshot(residency.requests())?;
        let worlds = WorldBindings::bind(residency.resources(), registration.plan())?;
        let spawn =
            PreparedSpawn::prepare(registration.plan(), residency.resources(), requested_id)?;
        let evidence = Evidence {
            state: "resources_retained",
            catalog: CatalogEvidence {
                resource_hash: mission::CATALOG_RESOURCE,
                payload_bytes: catalog_bytes,
                primary_sheet_hash: mission::PRIMARY_SHEET,
                secondary_sheet_hash: mission::SECONDARY_SHEET,
                primary_rows_read: read_counts.0,
                secondary_rows_read: read_counts.1,
                primary_rows_registered: registry.primary().len(),
                secondary_rows_registered: registry.secondary().len(),
                duplicate_rows: registry.duplicates().len(),
            },
            selection,
            resource: residency.root_metadata()?.clone(),
            residency: residency.evidence()?,
            registration: registration.evidence().clone(),
            spawn: spawn.evidence().clone(),
            map_instances: worlds.evidence().clone(),
            player_components: player_components.evidence().clone(),
            resources_retained: true,
            world_registered: false,
            player_initialized: false,
            reference_compared: false,
            scope: SCOPE,
        };
        Ok(Self {
            registry,
            residency,
            registration,
            spawn,
            worlds,
            player_components,
            evidence,
        })
    }

    pub fn retained_resources(&self) -> &[Edb] {
        self.residency.resources()
    }

    pub(crate) fn actor_definition_source(&self) -> &Edb {
        self.player_components.actor_definition_source()
    }

    /// The actual retained group-one selection, checked against the owned level
    /// registry. Serialized diagnostics never authorize a character binding.
    pub(crate) fn selected_character(&self) -> Result<&ge_assets::character::CharacterRow> {
        let level = self
            .registry
            .selected_primary()
            .context("selected level is absent")?;
        ensure!(
            self.registry.current_hash() == level.hash(),
            "selected level identity differs"
        );
        let row = self.player_components.selected_character()?;
        ensure!(
            row.key() == level.character_request(),
            "selected character differs from level request"
        );
        Ok(row)
    }

    pub fn registry(&self) -> &LevelRegistry {
        &self.registry
    }

    pub fn registration_plan(&self) -> &ge_core::mission::registration::RegistrationPlan {
        self.registration.plan()
    }

    pub fn source_spawn_plan(&self) -> &ge_core::mission::spawn::SourceSpawnPlan {
        self.spawn.plan()
    }

    pub(super) fn map_instances(&self) -> &[super::world::MapDataInstance] {
        self.worlds.instances()
    }

    pub(crate) fn retained_map_count(&self) -> usize {
        self.worlds.instances().len()
    }

    pub(crate) fn prepared_start_point_count(&self) -> usize {
        self.spawn.prepared_entry_count()
    }
    pub(crate) fn trigger_maps(&self) -> Result<Vec<super::world::TriggerMap<'_>>> {
        self.worlds
            .trigger_maps(self.retained_resources(), self.registration_plan())
    }

    pub fn evidence(&self) -> &Evidence {
        // Read retained ownership rather than treating the evidence snapshot as
        // the loaded resource. Later world adapters consume these same owners.
        debug_assert_eq!(
            Some(self.registry().current_hash()),
            self.retained_resources().first().map(|edb| edb.header.hash)
        );
        debug_assert_eq!(
            self.registration_plan().rows().len(),
            self.retained_resources().len()
        );
        debug_assert_eq!(self.map_instances().len(), self.retained_resources().len());
        debug_assert_eq!(
            self.player_components.evidence().requested_character,
            self.registry
                .selected_primary()
                .expect("retained selected level")
                .character_request()
        );
        debug_assert_eq!(
            self.source_spawn_plan().pose().is_some(),
            self.source_spawn_plan()
                .selection()
                .selected_index()
                .is_some()
        );
        &self.evidence
    }

    pub fn prepare_animation(
        &mut self,
        requested: u32,
        previous: u32,
        fallback: u32,
    ) -> Result<()> {
        self.player_components
            .prepare_animation(requested, previous, fallback)?;
        self.evidence.player_components = self.player_components.evidence().clone();
        Ok(())
    }

    pub fn initialize_animation_layers(&mut self, assets: &mut GameAssets) -> Result<()> {
        self.player_components.initialize_layers(assets)?;
        self.evidence.player_components = self.player_components.evidence().clone();
        Ok(())
    }
    pub fn decode_initial_frame(&mut self) -> Result<()> {
        self.player_components.decode_initial_frame()?;
        self.evidence.player_components = self.player_components.evidence().clone();
        Ok(())
    }
    pub fn evaluate_prewarm_pose(&mut self) -> Result<()> {
        self.player_components.evaluate_prewarm_pose()?;
        self.evidence.player_components = self.player_components.evidence().clone();
        Ok(())
    }
    pub fn evaluate_uncached_camera_bone(&mut self) -> Result<()> {
        self.player_components.evaluate_uncached_camera_bone()?;
        self.evidence.player_components = self.player_components.evidence().clone();
        Ok(())
    }
    pub fn evaluate_cached_camera_bone(&mut self) -> Result<()> {
        self.player_components.evaluate_cached_camera_bone()?;
        self.evidence.player_components = self.player_components.evidence().clone();
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests/context.rs"]
mod tests;
