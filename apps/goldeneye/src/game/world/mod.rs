//! Partial live trigger ownership. Unsupported rows retain their original slots
//! and block catalog completion; source metadata is never a ready game world.
mod animators;
mod evidence;
mod factory;
mod navigation;
pub(crate) mod post_trigger_resources;
pub(super) mod zones;

use anyhow::{Context, Result, ensure};
use ge_assets::{
    mission::common_triggers::{Admission, Common},
    supplied_data::SuppliedData,
};
use ge_core::mission::triggers::{
    self, CommonInput, Descriptor, MapContext, RowInput, Unsupported,
};

use crate::mission::MissionContext;
pub use evidence::Evidence;
pub use navigation::Evidence as NavigationEvidence;
pub use post_trigger_resources::Evidence as PostTriggerResourceEvidence;

pub(super) struct OwnedWorld {
    _data: SuppliedData,
    registry: triggers::Registry,
    animators: animators::Owned,
    evidence: Evidence,
    shapes: Vec<factory::ShapeSource>,
    definitions: super::actors::Definitions,
    _resource_states: super::resource_states::Attempt,
    navigation: Option<navigation::Owned>,
    post_trigger_resources: Option<post_trigger_resources::Owned>,
    _world_hooks: super::world_hooks::Attempt,
    loader: Option<super::world_hooks::loader::Run>,
    player: Option<ge_core::mission::player::identity::PlayerId>,
}

impl OwnedWorld {
    pub fn build(
        context: &MissionContext,
        data: SuppliedData,
        shared_resource_states: super::resource_states::Shared,
        assets: &mut ge_assets::resource::GameAssets,
        world_hooks: super::world_hooks::Attempt,
        work: ge_core::mission::loader::work::WorkOwner,
    ) -> Result<Self> {
        let mut loader = world_hooks.start_loader(work)?;
        let mut resource_states = super::resource_states::Attempt::new(shared_resource_states);
        let mut definitions = super::actors::Definitions::bind(context)?;
        let maps = context.trigger_maps()?;
        loader.construct_maps(&world_hooks, &maps)?;
        let mut registry = triggers::Registry::new(triggers::Budget {
            maps: 128,
            rows: 65_536,
            objects: 65_536,
            filter_words: 65_536,
            fixups: 65_536 * 8,
            catalog_entries: 65_536,
        });
        let mut animators = animators::Owned::default();
        let mut zones = zones::Worlds(&maps);
        let mut shapes = Vec::new();
        let mut audit = evidence::Audit::new(data.identity());
        let mut activated = 0;
        let mut bsp_work_remaining = 1_000_000;
        for map in &maps {
            if map.flags[2] == 0 {
                continue;
            }
            let reference = map.data.descriptor.canonical_reference;
            let rows = map
                .source
                .mission_common_triggers(
                    reference,
                    ge_assets::mission::common_triggers::Budget {
                        maps: 128,
                        rows: 65_536,
                        types: 65_536,
                    },
                )?
                .context("registered trigger map is absent")?;
            let map_context = MapContext {
                instance: map.loaded.instance,
                map_selector: i8::try_from(map.loaded.slot)?,
            };
            registry.begin_map(map_context, rows.row_count(), map.filter)?;
            audit.map(map_context, map.filter, rows.row_count());
            let mut factory = factory::Factory {
                source: map.source,
                reference,
                owned: &mut animators,
                map: map_context,
                map_index: activated,
                shapes: &mut shapes,
                definitions: &mut definitions,
                resource_states: &mut resource_states,
            };
            for ordinal in 0..rows.row_count() {
                let row = rows.row(ordinal)?;
                // The source world query precedes callback classification/filter.
                let position = row.query_position_bits()?;
                let zone = triggers::zone::select(
                    &mut zones,
                    [position[0], position[1], position[2]],
                    triggers::zone::Budget {
                        maps: 128,
                        nodes: bsp_work_remaining.min(65_536),
                    },
                )?;
                bsp_work_remaining -= zone.nodes_visited;
                let index = row.locator().native_type_index;
                audit.query(ordinal, index, zone);
                if !ge_assets::trigger_classes::supports(index) {
                    registry.mark_unsupported(
                        ordinal,
                        Unsupported::Unclassified {
                            native_type_index: index,
                        },
                    )?;
                    continue;
                }
                let descriptor = data.trigger_classes().descriptor(index)?;
                registry.visit(
                    RowInput {
                        ordinal,
                        zone: zone.callback_zone,
                        descriptor: Descriptor {
                            native_type_index: index,
                            class: descriptor.class,
                            subtype: descriptor.subtype,
                            enable_byte_raw: descriptor.enable_byte_raw as u8,
                        },
                    },
                    || {
                        let Admission::Admitted(common) =
                            row.read_common(&descriptor, map.filter, 28)?
                        else {
                            anyhow::bail!(
                                "common reader admission differs from runtime classification"
                            )
                        };
                        Ok(common_input(common))
                    },
                    &mut factory,
                )?;
            }
            registry.fix_references()?;
            registry.link_parents()?;
            registry.initialize_map()?;
            audit.slots(registry.slots(activated)?);
            activated += 1;
        }
        if registry.is_complete() {
            registry.rebuild_catalog()?;
        }
        world_hooks.bind_world_scratch(registry.scratch())?;
        let navigation = if registry.is_complete() {
            Some(navigation::Owned::build(context, &maps)?)
        } else {
            None
        };
        let post_trigger_resources = if navigation.is_some() {
            let operation = definitions.prepare_post_trigger(context)?;
            Some(post_trigger_resources::Owned::build(operation, assets)?)
        } else {
            None
        };
        let mut evidence = audit.finish(&registry, &animators, activated)?;
        evidence.navigation = navigation.as_ref().map(|owned| owned.evidence().clone());
        evidence.post_trigger_resources = post_trigger_resources
            .as_ref()
            .map(|owned| owned.evidence().clone());
        evidence.world_hooks = if post_trigger_resources
            .as_ref()
            .is_some_and(|owned| owned.is_complete())
        {
            Some(world_hooks.run()?)
        } else {
            None
        };
        let loader = if let Some(resources) = post_trigger_resources
            .as_ref()
            .filter(|owned| owned.is_complete())
        {
            loader.execute_to_phase19(&world_hooks, context, &mut registry, resources, assets)?;
            evidence.loader = Some(loader.evidence(&world_hooks)?);
            Some(loader)
        } else {
            None
        };
        let actors = registry
            .objects()
            .iter()
            .filter_map(|trigger| trigger.actor12())
            .collect::<Vec<_>>();
        for actor in &actors {
            if let Some(record) = actor.definition() {
                ensure!(
                    definitions.owns_definition(record),
                    "actor definition ownership differs"
                );
            }
        }
        let actors_ready = !actors.is_empty()
            && actors.iter().all(|actor| {
                actor.stage() == triggers::actor12::ConstructorStage::Complete
                    && actor.map_initialization() == triggers::actor12::MapInitialization::Complete
            });
        definitions.refresh(context, actors_ready)?;
        evidence.actor_definitions = Some(definitions.evidence().clone());
        evidence.resource_states = Some(resource_states.evidence()?);
        ensure!(
            evidence.shape_selectors_initialized == shapes.len(),
            "shape source ownership count differs"
        );
        for source in &shapes {
            let id = registry.slots(source.map_index)?[usize::from(source.ordinal)]
                .retained_trigger()
                .context("checked shape source has no retained trigger")?;
            let trigger = registry.trigger(id)?;
            ensure!(
                trigger.map() == source.map
                    && trigger
                        .common_shape()
                        .is_some_and(|binding| binding.selector() == source.binding.raw_selector
                            && binding.type_raw() == source.binding.type_raw),
                "checked shape source differs from owned common binding"
            );
        }
        ensure!(
            evidence.animators_initialized == animators.values.len(),
            "animator ownership differs"
        );
        Ok(Self {
            _data: data,
            registry,
            animators,
            evidence,
            shapes,
            definitions,
            _resource_states: resource_states,
            navigation,
            post_trigger_resources,
            _world_hooks: world_hooks,
            loader,
            player: None,
        })
    }

    pub fn evidence(&self) -> &Evidence {
        debug_assert_eq!(self.registry.objects().len(), self.evidence.owned_triggers);
        debug_assert_eq!(
            self.animators.values.len(),
            self.evidence.animators_initialized
        );
        debug_assert_eq!(self.shapes.len(), self.evidence.shape_selectors_initialized);
        debug_assert_eq!(
            self.navigation.as_ref().map(|owned| owned.evidence()),
            self.evidence.navigation.as_ref()
        );
        debug_assert_eq!(
            self.post_trigger_resources
                .as_ref()
                .map(|owned| owned.evidence()),
            self.evidence.post_trigger_resources.as_ref()
        );
        debug_assert_eq!(
            self.evidence.actor_definitions.as_ref(),
            Some(self.definitions.evidence())
        );
        &self.evidence
    }

    pub fn finish_session_load(
        &mut self,
        session: &mut ge_core::mission::session::Session,
        binding: &ge_core::mission::session::LoadBinding,
        context: &MissionContext,
        assets: &mut ge_assets::resource::GameAssets,
    ) -> Result<bool> {
        ensure!(
            context.registry().current_hash() == session.request().level_hash(),
            "loader/session level identity differs"
        );
        let Some(loader) = self.loader.as_mut() else {
            return Ok(false);
        };
        let resources = self
            .post_trigger_resources
            .as_ref()
            .context("loader mission resource owner is absent")?;
        loader.finish_session(
            &self._world_hooks,
            session,
            binding,
            &self.registry,
            resources,
            assets,
        )?;
        self.evidence.loader = Some(loader.evidence(&self._world_hooks)?);
        let (attempt, operations) =
            self._world_hooks
                .begin_player_attempt(session, binding, &self.registry)?;
        match attempt {
            ge_core::mission::session::player_load::PlayerAttemptOutcome::Started(request) => {
                ensure!(
                    self.player.is_none(),
                    "player constructor has already been dispatched"
                );
                let hook = &self._world_hooks;
                let (player, evidence, dependency) = hook.with_player_actors(|actors| {
                    let player = super::player::Owned::build(
                        request,
                        operations,
                        self.registry.catalog().len(),
                        super::player::Bindings {
                            session,
                            mission: context,
                            assets,
                        },
                        &mut self.definitions,
                        &mut self._resource_states,
                        hook,
                    )?;
                    let id = player.identity();
                    ensure!(
                        !actors.players.contains_key(&id),
                        "player identity is already retained"
                    );
                    // Install the real owner before any EXItem can reference it.
                    // A later failure/cancellation preserves that owner and links.
                    actors.players.insert(id, player);
                    let player = actors.players.get_mut(&id).expect("just inserted player");
                    player.initialize_virtual(
                        context,
                        session,
                        &mut actors.world,
                        &mut actors.registry,
                        hook,
                    )?;
                    player.initialize_components(
                        super::player::components::Sources {
                            session,
                            definitions: &self.definitions,
                            resources: self
                                .post_trigger_resources
                                .as_mut()
                                .context("player resource owner is absent")?,
                            assets,
                            identities: &mut actors.components,
                        },
                        &mut actors.world,
                        hook,
                    )?;
                    let evidence = player.evidence(
                        &actors.world,
                        &actors.registry,
                        &actors.components,
                        hook,
                    )?;
                    Ok((id, evidence, player.dependency()))
                })?;
                self.evidence.player_initialization = Some(evidence);
                self.player = Some(player);
                session.record_player_dependency(binding, dependency)?;
            }
            ge_core::mission::session::player_load::PlayerAttemptOutcome::InProgress(attempt) => {
                let id = self
                    .player
                    .context("player attempt lost its arena binding")?;
                self._world_hooks.with_player_actors(|actors| {
                    ensure!(
                        actors
                            .players
                            .get(&id)
                            .is_some_and(|player| player.matches_attempt(attempt)),
                        "player attempt lost its actual matching retained owner"
                    );
                    Ok(())
                })?;
            }
            ge_core::mission::session::player_load::PlayerAttemptOutcome::Gated(_) => {}
        }
        let actor_ready = self.definitions.evidence().native9_constructor_ready;
        self.definitions.refresh(context, actor_ready)?;
        self.evidence.actor_definitions = Some(self.definitions.evidence().clone());
        self.evidence.resource_states = Some(self._resource_states.evidence()?);
        Ok(true)
    }
}

fn common_input(value: Common) -> CommonInput {
    CommonInput {
        header_flags_raw: value.header_flags_raw,
        property_mask_raw: value.property_mask_raw,
        position_bits: value.position_bits,
        rotation_bits: value.rotation_bits,
        numeric: value.numeric,
        reference_ordinals: value.reference_ordinals,
        word_74_raw: value.word_74_raw,
        word_78_raw: value.word_78_raw,
        animator_present: value.animator_present,
        animator_index_raw: value.animator_index_raw,
        shape_index_raw: value.shape_index_raw,
    }
}
