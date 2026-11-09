mod digest;
use anyhow::{Result, ensure};
use digest::Fingerprint;
use ge_assets::supplied_data::Identity;
use ge_core::mission::{
    animator::EventOutcome,
    triggers::{
        AnimatorState, MapContext, Reference, Registry, Slot, TriggerKind, Unsupported, zone,
    },
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

const SCOPE: &str = "Owned source-backed trigger constructors, derived ranges/resource state and conditional activation with distinct source factory lifecycles and fresh StartPoint base animators. Original ordinal slots, subtype filters, zone queries and blocked dependencies are retained. Unsupported shapes, factories or references prevent catalog completion; fresh StartPoint create is denied before program execution; inactive constructors stop an absent animator without executing its serialized selector. Native zone precision, complete world behavior, player creation and gameplay are not established. Private state is fingerprinted, never serialized as original arrays or poses.";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Evidence {
    pub maps_activated: usize,
    pub original_slots: usize,
    pub disabled_slots: usize,
    pub filtered_slots: usize,
    pub source_null_slots: usize,
    pub unclassified_slots: usize,
    pub unsupported_factory_slots: usize,
    pub unsupported_shape_slots: usize,
    pub shape_selectors_initialized: usize,
    pub actor_definitions: Option<crate::game::ActorDefinitionEvidence>,
    pub resource_states: Option<crate::game::ResourceStateEvidence>,
    pub navigation: Option<crate::game::NavigationEvidence>,
    pub post_trigger_resources: Option<crate::game::PostTriggerResourceEvidence>,
    pub world_hooks: Option<crate::game::WorldHookEvidence>,
    pub loader: Option<crate::game::LoaderEvidence>,
    pub player_initialization: Option<crate::game::PlayerInitializationEvidence>,
    pub owned_triggers: usize,
    pub common_scratch: usize,
    pub membership_entries: usize,
    pub active_triggers: usize,
    pub start_points: usize,
    pub generic_triggers: usize,
    pub common_be_triggers: usize,
    pub resource08_triggers: usize,
    pub active91_triggers: usize,
    pub range0c_triggers: usize,
    pub radius06_triggers: usize,
    pub actor12_triggers: usize,
    pub range82_triggers: usize,
    pub a3_triggers: usize,
    pub range27_triggers: usize,
    pub common09_triggers: usize,
    pub ad_triggers: usize,
    pub radius03_triggers: usize,
    pub common8a_triggers: usize,
    pub radius1b_triggers: usize,
    pub radius74_triggers: usize,
    pub shape71_triggers: usize,
    pub materialized_shapes: usize,
    pub radius88_triggers: usize,
    pub auxac_triggers: usize,
    pub map_initialization_blocked: usize,
    pub staged_triggers: usize,
    pub stopped_animators: usize,
    pub resolved_references: usize,
    pub blocked_references: usize,
    pub animators_initialized: usize,
    pub animators_failed: usize,
    pub shared_variables: usize,
    pub unwritten_scratch_words: usize,
    pub state_flags: usize,
    pub execution_slots: usize,
    pub active_executions: usize,
    pub create_events_denied: usize,
    pub registration_events_unmapped: usize,
    pub zone_queries: usize,
    pub catalog_entries: usize,
    pub catalog_complete: bool,
    pub native_zone_precision_verified: bool,
    pub state_sha256: String,
    pub scope: &'static str,
}

struct PendingMap {
    rows: usize,
    queries: usize,
}

pub(super) struct Audit {
    hash: Fingerprint,
    maps: usize,
    rows: usize,
    queries: usize,
    pending: Option<PendingMap>,
    invalid: bool,
    counts: [usize; 7],
    staged: BTreeSet<u64>,
}

impl Audit {
    pub fn new(identity: &Identity) -> Self {
        let mut hash = Fingerprint(Sha256::new());
        hash.0.update(b"ge_world_trigger_state_v9\0");
        hash.byte(1);
        hash.text(&identity.game_id);
        hash.byte(identity.revision);
        hash.u64(identity.main_dol_partition_offset);
        hash.u64(identity.dol_bytes);
        hash.text(&identity.dol_sha256);
        hash.usize(identity.data_section_index);
        hash.u32(identity.data_virtual_address);
        hash.usize(identity.data_bytes);
        hash.text(&identity.data_sha256);
        Self {
            hash,
            maps: 0,
            rows: 0,
            queries: 0,
            pending: None,
            invalid: false,
            counts: [0; 7],
            staged: BTreeSet::new(),
        }
    }

    pub fn map(&mut self, context: MapContext, filter: Option<&[u32]>, row_count: usize) {
        self.invalid |= self.pending.is_some();
        increment(&mut self.maps, 1, &mut self.invalid);
        increment(&mut self.rows, row_count, &mut self.invalid);
        self.pending = Some(PendingMap {
            rows: row_count,
            queries: 0,
        });
        self.hash.byte(2);
        self.hash.map(context);
        self.hash.flag(filter.is_some());
        if let Some(words) = filter {
            self.hash.words(words);
        }
        self.hash.usize(row_count);
    }

    pub fn query(&mut self, ordinal: usize, index: u32, zone: zone::Context) {
        if let Some(map) = self.pending.as_mut() {
            self.invalid |= ordinal != map.queries || ordinal >= map.rows;
            increment(&mut map.queries, 1, &mut self.invalid);
        } else {
            self.invalid = true;
        }
        increment(&mut self.queries, 1, &mut self.invalid);
        self.hash.byte(3);
        self.hash.usize(ordinal);
        self.hash.u32(index);
        self.hash.optional_usize(zone.loaded_ordinal);
        self.hash.flag(zone.zone_leaf.is_some());
        if let Some(leaf) = zone.zone_leaf {
            self.hash.u16(leaf);
        }
        self.hash.u16(zone.callback_zone);
        self.hash.u32(zone.packed);
        self.hash.usize(zone.nodes_visited);
        self.invalid |= zone.packed != u32::from(zone.callback_zone) << 16;
    }

    pub fn slots(&mut self, slots: &[Slot]) {
        let Some(map) = self.pending.take() else {
            self.invalid = true;
            return;
        };
        self.invalid |= slots.len() != map.rows || map.queries != map.rows;
        self.hash.byte(4);
        self.hash.usize(slots.len());
        for slot in slots {
            let counter = match *slot {
                Slot::Unvisited => {
                    self.invalid = true;
                    self.hash.byte(0);
                    continue;
                }
                Slot::Disabled => {
                    self.hash.byte(1);
                    0
                }
                Slot::Filtered => {
                    self.hash.byte(2);
                    1
                }
                Slot::SourceNull => {
                    self.hash.byte(17);
                    6
                }
                Slot::Unsupported(Unsupported::Unclassified { native_type_index }) => {
                    self.hash.byte(3);
                    self.hash.u32(native_type_index);
                    2
                }
                Slot::Unsupported(Unsupported::Factory { class, subtype }) => {
                    self.hash.byte(4);
                    self.hash.u32(class);
                    self.hash.u32(subtype);
                    3
                }
                Slot::Unsupported(Unsupported::GenericResource { trigger, resource }) => {
                    self.invalid |= !self.staged.insert(trigger.get());
                    self.hash.byte(7);
                    self.hash.u64(trigger.get());
                    self.hash.u32(resource);
                    3
                }
                Slot::Unsupported(Unsupported::Resource08Provider { trigger, resource }) => {
                    self.invalid |= !self.staged.insert(trigger.get());
                    self.hash.byte(8);
                    self.hash.u64(trigger.get());
                    self.hash.u32(resource);
                    3
                }
                Slot::Unsupported(Unsupported::Range0cProvider { trigger, resource }) => {
                    self.invalid |= !self.staged.insert(trigger.get());
                    self.hash.byte(9);
                    self.hash.u64(trigger.get());
                    self.hash.u32(resource);
                    3
                }
                Slot::Unsupported(Unsupported::Radius06Provider { trigger, resource }) => {
                    self.invalid |= !self.staged.insert(trigger.get());
                    self.hash.byte(10);
                    self.hash.u64(trigger.get());
                    self.hash.u32(resource);
                    3
                }
                Slot::Unsupported(Unsupported::Actor12Provider { trigger, stage }) => {
                    self.invalid |= !self.staged.insert(trigger.get());
                    self.hash.byte(11);
                    self.hash.u64(trigger.get());
                    self.hash.byte(match stage {
                        ge_core::mission::triggers::actor12::ProviderStage::DescriptorFallback => 0,
                        ge_core::mission::triggers::actor12::ProviderStage::LevelSelector => 1,
                        ge_core::mission::triggers::actor12::ProviderStage::Definition => 2,
                    });
                    3
                }
                Slot::Unsupported(Unsupported::Range82Provider { trigger, resource }) => {
                    self.invalid |= !self.staged.insert(trigger.get());
                    self.hash.byte(12);
                    self.hash.u64(trigger.get());
                    self.hash.u32(resource);
                    3
                }
                Slot::Unsupported(Unsupported::Range27Provider { trigger, resource }) => {
                    self.invalid |= !self.staged.insert(trigger.get());
                    self.hash.byte(13);
                    self.hash.u64(trigger.get());
                    self.hash.u32(resource);
                    3
                }
                Slot::Unsupported(Unsupported::Radius03Provider { trigger, resource }) => {
                    self.invalid |= !self.staged.insert(trigger.get());
                    self.hash.byte(14);
                    self.hash.u64(trigger.get());
                    self.hash.u32(resource);
                    3
                }
                Slot::Unsupported(Unsupported::Common8AHook { trigger }) => {
                    self.invalid |= !self.staged.insert(trigger.get());
                    self.hash.byte(15);
                    self.hash.u64(trigger.get());
                    3
                }
                Slot::Unsupported(Unsupported::Shape71Geometry { trigger }) => {
                    self.invalid |= !self.staged.insert(trigger.get());
                    self.hash.byte(16);
                    self.hash.u64(trigger.get());
                    3
                }
                Slot::Unsupported(Unsupported::Shape { index }) => {
                    self.hash.byte(5);
                    self.hash.u32(index);
                    4
                }
                Slot::Owned(id) => {
                    self.hash.byte(6);
                    self.hash.u64(id.get());
                    5
                }
            };
            increment(&mut self.counts[counter], 1, &mut self.invalid);
        }
    }

    pub fn finish(
        mut self,
        registry: &Registry,
        animators: &super::animators::Owned,
        activated: usize,
    ) -> Result<Evidence> {
        ensure!(
            !self.invalid && self.pending.is_none(),
            "world audit callback sequence differs"
        );
        ensure!(
            self.maps == activated && self.rows == self.queries,
            "world audit map/query count differs"
        );
        let slot_sum = self
            .counts
            .iter()
            .try_fold(0usize, |sum, n| sum.checked_add(*n));
        ensure!(
            slot_sum == Some(self.rows),
            "world audit slot totals differ"
        );
        ensure!(
            self.counts[5] + self.staged.len() == registry.objects().len(),
            "world audit owned trigger count differs"
        );
        ensure!(
            registry
                .scratch()
                .iter()
                .copied()
                .eq(registry.objects().iter().map(|object| object.id()))
                && registry.membership().iter().copied().eq(registry
                    .scratch()
                    .iter()
                    .copied()
                    .filter(|id| !self.staged.contains(&id.get()))),
            "world audit source membership differs"
        );
        if registry.catalog_ready() {
            let expected: Vec<_> = registry
                .objects()
                .iter()
                .filter(|trigger| trigger.descriptor().is_start_point())
                .map(|trigger| trigger.id())
                .collect();
            ensure!(
                registry.catalog() == expected.as_slice(),
                "world audit completed catalog was not rebuilt"
            );
        } else {
            ensure!(
                registry.catalog().is_empty(),
                "world audit has a stale catalog"
            );
        }
        self.hash.byte(5);
        self.hash.usize(activated);
        self.hash.byte(6);
        self.hash.usize(registry.scratch().len());
        for id in registry.scratch() {
            self.hash.u64(id.get());
        }
        self.hash.usize(registry.membership().len());
        for id in registry.membership() {
            self.hash.u64(id.get());
        }
        self.hash.usize(registry.objects().len());
        let mut blocked = 0;
        let mut resolved = 0;
        let mut active_triggers = 0;
        let mut start_points = 0;
        let mut generic_triggers = 0;
        let mut common_be_triggers = 0;
        let mut resource08_triggers = 0;
        let mut active91_triggers = 0;
        let mut range0c_triggers = 0;
        let mut radius06_triggers = 0;
        let mut actor12_triggers = 0;
        let mut range82_triggers = 0;
        let mut a3_triggers = 0;
        let mut range27_triggers = 0;
        let mut common09_triggers = 0;
        let mut ad_triggers = 0;
        let mut radius03_triggers = 0;
        let mut common8a_triggers = 0;
        let mut radius1b_triggers = 0;
        let mut radius74_triggers = 0;
        let mut shape71_triggers = 0;
        let mut materialized_shapes = 0;
        let mut radius88_triggers = 0;
        let mut auxac_triggers = 0;
        let mut map_initialization_blocked = 0;
        let mut stopped_animators = 0;
        let mut shape_selectors_initialized = 0;
        let mut failed = 0;
        let mut initialized = BTreeSet::new();
        for trigger in registry.objects() {
            let staged = self.staged.contains(&trigger.id().get());
            ensure!(
                trigger.is_registered() != staged,
                "world audit registration stage differs"
            );
            ensure!(
                trigger.factory_status() == trigger.kind().factory_status(),
                "world audit factory status differs from kind"
            );
            ensure!(
                trigger.generic().is_some() == (trigger.kind() == TriggerKind::Generic),
                "world audit derived generic state differs from kind"
            );
            ensure!(
                (trigger
                    .generic()
                    .is_some_and(|state| state.requires_resource_provider())
                    || trigger
                        .resource08()
                        .is_some_and(|state| state.requires_resource_provider())
                    || trigger
                        .range0c()
                        .is_some_and(|state| state.requires_resource_provider())
                    || trigger
                        .radius06()
                        .is_some_and(|state| state.requires_resource_provider())
                    || trigger
                        .range82()
                        .is_some_and(|state| state.requested_resource().is_some())
                    || trigger
                        .range27()
                        .is_some_and(|state| state.requires_resource_provider())
                    || trigger
                        .radius03()
                        .is_some_and(|state| state.mode_low_byte().is_none())
                    || trigger.common8a().is_some_and(|state| state.gate()
                        == ge_core::mission::triggers::common8a::Gate::RequiresHook)
                    || trigger.shape71().is_some_and(|state| !matches!(
                        state.geometry_stage(),
                        ge_core::mission::triggers::shape71::Stage::Transformed
                            | ge_core::mission::triggers::shape71::Stage::FallbackComplete
                    ))
                    || trigger.actor12().is_some_and(|state| matches!(
                        state.stage(),
                        ge_core::mission::triggers::actor12::ConstructorStage::Provider(_)
                    )))
                    == staged,
                "world audit constructor stage differs"
            );
            ensure!(
                trigger.resource08().is_some() == (trigger.kind() == TriggerKind::Resource08),
                "world audit resource08 state differs from kind"
            );
            ensure!(
                trigger.range0c().is_some() == (trigger.kind() == TriggerKind::Range0c),
                "world audit range0c state differs from kind"
            );
            ensure!(
                trigger.radius06().is_some() == (trigger.kind() == TriggerKind::Radius06),
                "world audit radius06 state differs from kind"
            );
            ensure!(
                trigger.registration_status().is_some() == trigger.is_registered(),
                "world audit callback status differs from membership"
            );
            ensure!(
                trigger.actor12().is_some() == (trigger.kind() == TriggerKind::Actor12),
                "world audit actor12 state differs from kind"
            );
            ensure!(
                trigger.range82().is_some() == (trigger.kind() == TriggerKind::Range82),
                "world audit range82 state differs from kind"
            );
            ensure!(
                trigger.a3().is_some() == (trigger.kind() == TriggerKind::A3),
                "world audit a3 state differs from kind"
            );
            ensure!(
                trigger.range27().is_some() == (trigger.kind() == TriggerKind::Range27),
                "world audit range27 state differs from kind"
            );
            ensure!(
                trigger.ad().is_some() == (trigger.kind() == TriggerKind::Ad),
                "world audit ad state differs from kind"
            );
            ensure!(
                trigger.radius03().is_some() == (trigger.kind() == TriggerKind::Radius03),
                "world audit radius03 state differs from kind"
            );
            ensure!(
                trigger.common8a().is_some() == (trigger.kind() == TriggerKind::Common8A),
                "world audit common8a state differs from kind"
            );
            ensure!(
                trigger.radius1b().is_some() == (trigger.kind() == TriggerKind::Radius1b),
                "world audit radius1b state differs from kind"
            );
            ensure!(
                trigger.radius74().is_some() == (trigger.kind() == TriggerKind::Radius74),
                "world audit radius74 state differs from kind"
            );
            ensure!(
                trigger.shape71().is_some() == (trigger.kind() == TriggerKind::Shape71),
                "world audit shape71 state differs from kind"
            );
            ensure!(
                trigger.radius88().is_some() == (trigger.kind() == TriggerKind::Radius88),
                "world audit radius88 state differs from kind"
            );
            ensure!(
                trigger.aux_ac().is_some() == (trigger.kind() == TriggerKind::AuxAc),
                "world audit auxiliary state differs from kind"
            );
            if let Some(state) = trigger.shape71() {
                materialized_shapes += usize::from(state.geometry().tag() >= 0);
            }
            if let Some(actor) = trigger.actor12() {
                use ge_core::mission::triggers::actor12::MapInitialization;
                match actor.map_initialization() {
                    MapInitialization::NotRun => {
                        ensure!(staged, "actor map initialization pass missing")
                    }
                    MapInitialization::Blocked { .. } => map_initialization_blocked += 1,
                    MapInitialization::Complete => {}
                }
            }
            self.hash.u64(trigger.id().get());
            self.hash.map(trigger.map());
            self.hash.u16(trigger.zone());
            self.hash.u16(trigger.ordinal());
            let descriptor = trigger.descriptor();
            self.hash.u32(descriptor.native_type_index);
            self.hash.u32(descriptor.class);
            self.hash.u32(descriptor.subtype);
            self.hash.byte(descriptor.enable_byte_raw);
            match trigger.kind() {
                TriggerKind::StartPoint => start_points += 1,
                TriggerKind::Generic => generic_triggers += 1,
                TriggerKind::CommonBe => common_be_triggers += 1,
                TriggerKind::Resource08 => resource08_triggers += 1,
                TriggerKind::Active91 => active91_triggers += 1,
                TriggerKind::Range0c => range0c_triggers += 1,
                TriggerKind::Radius06 => radius06_triggers += 1,
                TriggerKind::Actor12 => actor12_triggers += 1,
                TriggerKind::Range82 => range82_triggers += 1,
                TriggerKind::A3 => a3_triggers += 1,
                TriggerKind::Range27 => range27_triggers += 1,
                TriggerKind::Common09 => common09_triggers += 1,
                TriggerKind::Ad => ad_triggers += 1,
                TriggerKind::Radius03 => radius03_triggers += 1,
                TriggerKind::Common8A => common8a_triggers += 1,
                TriggerKind::Radius1b => radius1b_triggers += 1,
                TriggerKind::Radius74 => radius74_triggers += 1,
                TriggerKind::Shape71 => shape71_triggers += 1,
                TriggerKind::Radius88 => radius88_triggers += 1,
                TriggerKind::AuxAc => auxac_triggers += 1,
            }
            active_triggers += usize::from(trigger.runtime_flags_raw() & 1 != 0);
            self.hash.trigger_state(trigger);
            if let Some(binding) = trigger.common_shape() {
                ensure!(
                    binding.type_raw() == 0
                        && trigger.common().shape_index_raw == Some(binding.selector()),
                    "common shape binding differs from source selector"
                );
                shape_selectors_initialized += 1;
            } else {
                ensure!(
                    trigger.common().shape_index_raw.is_none(),
                    "required common shape binding absent"
                );
            }
            let common = trigger.common();
            self.hash.u32(common.header_flags_raw);
            self.hash.u32(common.property_mask_raw);
            self.hash.words(&common.position_bits);
            self.hash.words(&common.rotation_bits);
            self.hash.words(&common.numeric);
            self.hash.words(&common.reference_ordinals);
            self.hash.u32(common.word_74_raw);
            self.hash.u32(common.word_78_raw);
            self.hash.flag(common.animator_present);
            self.hash.u32(common.animator_index_raw as u32);
            self.hash.flag(common.shape_index_raw.is_some());
            if let Some(selector) = common.shape_index_raw {
                self.hash.u32(selector);
            }
            self.hash.flag(trigger.common_shape().is_some());
            if let Some(binding) = trigger.common_shape() {
                self.hash.u32(binding.selector());
                self.hash.byte(binding.type_raw());
            }
            self.hash.words(&trigger.original_position_bits());
            self.hash.words(&trigger.original_rotation_bits());
            self.hash.u32(trigger.runtime_flags_raw());
            self.hash.usize(trigger.references().len());
            for reference in trigger.references() {
                match *reference {
                    Reference::Unresolved => {
                        ensure!(staged, "world audit reference pass unfinished");
                        self.hash.byte(3);
                    }
                    Reference::Null => self.hash.byte(0),
                    Reference::Trigger(id) => {
                        resolved += 1;
                        registry.trigger(id)?;
                        self.hash.byte(1);
                        self.hash.u64(id.get());
                    }
                    Reference::Blocked { ordinal } => {
                        blocked += 1;
                        self.hash.byte(2);
                        self.hash.u32(ordinal);
                    }
                }
            }
            self.hash.flag(trigger.parent().is_some());
            if let Some(parent) = trigger.parent() {
                registry.trigger(parent)?;
                self.hash.u64(parent.get());
            }
            self.hash.usize(trigger.children().len());
            for child in trigger.children() {
                registry.trigger(*child)?;
                self.hash.u64(child.get());
            }
            match trigger.animator() {
                AnimatorState::NotRequired => self.hash.byte(0),
                AnimatorState::StoppedAbsent => {
                    stopped_animators += 1;
                    self.hash.byte(3);
                }
                AnimatorState::Pending => {
                    anyhow::bail!("world audit animator initialization unfinished")
                }
                AnimatorState::Failed => {
                    failed += 1;
                    self.hash.byte(1);
                }
                AnimatorState::Initialized(id) => {
                    ensure!(
                        initialized.insert(id.get()),
                        "world audit animator identity reused"
                    );
                    let index = usize::try_from(id.get() - 1)?;
                    let runtime = animators
                        .values
                        .get(index)
                        .ok_or_else(|| anyhow::anyhow!("world audit animator ownership absent"))?;
                    let context = runtime.context();
                    ensure!(
                        context.trigger_handle == trigger.id().get()
                            && context.map_selector == trigger.map().map_selector
                            && context.position_bits == common.position_bits,
                        "world audit animator trigger binding differs"
                    );
                    self.hash.byte(2);
                    self.hash.u64(id.get());
                }
            }
        }
        ensure!(
            initialized.len() == animators.values.len(),
            "world audit has unreferenced animators"
        );
        let mut shared = 0;
        let mut unwritten = 0;
        let mut state_flags = 0;
        let mut slots = 0;
        let mut active = 0;
        let mut denied = 0;
        let mut unmapped = 0;
        self.hash.byte(7);
        self.hash.usize(animators.values.len());
        for runtime in &animators.values {
            shared += runtime.shared_variables().len();
            unwritten += runtime.extra_scratch().map_or(0, |words| {
                words.iter().filter(|word| word.is_none()).count()
            });
            state_flags += runtime.state_flags().len();
            slots += runtime.execution_slots().len();
            let source = runtime.source_state();
            let occupied = runtime
                .execution_slots()
                .iter()
                .filter(|slot| slot.is_some())
                .count();
            ensure!(
                occupied == source.active_executions,
                "world audit execution slot count differs"
            );
            active += occupied;
            denied += usize::from(source.constructor_create == EventOutcome::Denied);
            unmapped += usize::from(source.registration_event == EventOutcome::Unmapped);
            self.hash.animator(runtime);
        }
        self.hash.byte(8);
        self.hash.flag(registry.catalog_ready());
        self.hash.usize(registry.catalog().len());
        for id in registry.catalog() {
            registry.trigger(*id)?;
            self.hash.u64(id.get());
        }
        Ok(Evidence {
            maps_activated: activated,
            original_slots: self.rows,
            disabled_slots: self.counts[0],
            filtered_slots: self.counts[1],
            source_null_slots: self.counts[6],
            unclassified_slots: self.counts[2],
            unsupported_factory_slots: self.counts[3],
            unsupported_shape_slots: self.counts[4],
            shape_selectors_initialized,
            actor_definitions: None,
            resource_states: None,
            navigation: None,
            post_trigger_resources: None,
            world_hooks: None,
            loader: None,
            player_initialization: None,
            owned_triggers: registry.objects().len(),
            common_scratch: registry.scratch().len(),
            membership_entries: registry.membership().len(),
            active_triggers,
            start_points,
            generic_triggers,
            common_be_triggers,
            resource08_triggers,
            active91_triggers,
            range0c_triggers,
            radius06_triggers,
            actor12_triggers,
            range82_triggers,
            a3_triggers,
            range27_triggers,
            common09_triggers,
            ad_triggers,
            radius03_triggers,
            common8a_triggers,
            radius1b_triggers,
            radius74_triggers,
            shape71_triggers,
            materialized_shapes,
            radius88_triggers,
            auxac_triggers,
            map_initialization_blocked,
            staged_triggers: self.staged.len(),
            stopped_animators,
            resolved_references: resolved,
            blocked_references: blocked,
            animators_initialized: initialized.len(),
            animators_failed: failed,
            shared_variables: shared,
            unwritten_scratch_words: unwritten,
            state_flags,
            execution_slots: slots,
            active_executions: active,
            create_events_denied: denied,
            registration_events_unmapped: unmapped,
            zone_queries: self.queries,
            catalog_entries: registry.catalog().len(),
            catalog_complete: registry.catalog_ready(),
            native_zone_precision_verified: false,
            state_sha256: format!("{:x}", self.hash.0.finalize()),
            scope: SCOPE,
        })
    }
}

fn increment(value: &mut usize, amount: usize, invalid: &mut bool) {
    if let Some(sum) = value.checked_add(amount) {
        *value = sum;
    } else {
        *invalid = true;
    }
}

#[cfg(test)]
#[path = "../tests/evidence.rs"]
mod tests;
