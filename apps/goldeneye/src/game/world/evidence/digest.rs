use ge_core::mission::{
    animator::{CameraContext, EventOutcome, OwnerBinding, Runtime},
    triggers::{MapContext, Trigger, TriggerKind},
};
use sha2::{Digest, Sha256};

pub(super) struct Fingerprint(pub(super) Sha256);

impl Fingerprint {
    pub(super) fn trigger_state(&mut self, trigger: &Trigger) {
        self.byte(match trigger.kind() {
            TriggerKind::StartPoint => 0,
            TriggerKind::Generic => 1,
            TriggerKind::CommonBe => 2,
            TriggerKind::Resource08 => 3,
            TriggerKind::Active91 => 4,
            TriggerKind::Range0c => 5,
            TriggerKind::Radius06 => 6,
            TriggerKind::Actor12 => 7,
            TriggerKind::Range82 => 8,
            TriggerKind::A3 => 9,
            TriggerKind::Range27 => 10,
            TriggerKind::Common09 => 11,
            TriggerKind::Ad => 12,
            TriggerKind::Radius03 => 13,
            TriggerKind::Common8A => 14,
            TriggerKind::Radius1b => 15,
            TriggerKind::Radius74 => 16,
            TriggerKind::Shape71 => 17,
            TriggerKind::Radius88 => 18,
            TriggerKind::AuxAc => 19,
        });
        self.byte(trigger.factory_status().raw());
        self.flag(trigger.registration_status().is_some());
        if let Some(status) = trigger.registration_status() {
            self.byte(status.raw());
        }
        self.u32(trigger.runtime_header_flags_raw());
        self.words(trigger.runtime_numeric());
        self.flag(trigger.generic().is_some());
        if let Some(generic) = trigger.generic() {
            self.u32(generic.radius_bits());
            self.u32(generic.radius_squared_bits());
            self.u32(generic.resource_reference());
            self.flag(generic.mode_bytes().is_some());
            if let Some(bytes) = generic.mode_bytes() {
                self.0.update(bytes);
            }
        }
        self.flag(trigger.resource08().is_some());
        if let Some(resource) = trigger.resource08() {
            self.byte(resource.mode_high_byte());
            self.flag(resource.mode_low_byte().is_some());
            if let Some(value) = resource.mode_low_byte() {
                self.byte(value);
            }
            self.flag(resource.requested_resource().is_some());
            if let Some(value) = resource.requested_resource() {
                self.u32(value);
            }
            self.u32(resource.base_radius_squared_bits());
        }
        self.flag(trigger.range0c().is_some());
        if let Some(range) = trigger.range0c() {
            self.range(range);
        }
        self.flag(trigger.radius06().is_some());
        if let Some(radius) = trigger.radius06() {
            self.u32(radius.radius_bits());
            self.u32(radius.radius_squared_bits());
            self.u32(radius.resource_reference());
            self.flag(radius.requested_resource().is_some());
            if let Some(value) = radius.requested_resource() {
                self.u32(value);
            }
            self.flag(radius.provider_activation().is_some());
            if let Some(value) = radius.provider_activation() {
                self.flag(value);
            }
        }
        self.flag(trigger.actor12().is_some());
        if let Some(actor) = trigger.actor12() {
            self.actor(actor);
        }
        self.flag(trigger.range82().is_some());
        if let Some(range) = trigger.range82() {
            self.u32(range.radius_bits());
            self.u32(range.radius_squared_bits());
            self.u32(range.secondary_radius_bits());
            self.u32(range.secondary_radius_squared_bits());
            self.u32(range.animation_reference());
            self.u32(range.resource_reference());
            self.optional_word(range.requested_resource());
            self.byte(range.mode_high_byte());
            self.flag(range.mode_low_byte().is_some());
            if let Some(value) = range.mode_low_byte() {
                self.byte(value);
            }
            for word in [
                range.numeric5_bits(),
                range.numeric7_bits(),
                range.numeric8_reference(),
                range.word_11c(),
            ] {
                self.optional_word(word);
            }
            self.flag(range.diagnostic_reached().is_some());
            if let Some(value) = range.diagnostic_reached() {
                self.flag(value);
            }
        }
        self.flag(trigger.a3().is_some());
        if let Some(state) = trigger.a3() {
            self.u32(state.radius_squared_bits());
        }
        self.flag(trigger.range27().is_some());
        if let Some(state) = trigger.range27() {
            self.range(state.range0c_state());
        }
        self.flag(trigger.ad().is_some());
        if let Some(state) = trigger.ad() {
            self.u32(state.radius_bits());
            self.u32(state.radius_squared_bits());
            self.u32(state.numeric2_reference());
            self.u32(state.numeric15_reference());
        }
        self.flag(trigger.radius03().is_some());
        if let Some(state) = trigger.radius03() {
            self.u32(state.radius_squared_bits());
            self.u32(state.requested_resource().unwrap_or(0));
            self.byte(state.mode_high_byte());
            self.flag(state.mode_low_byte().is_some());
            if let Some(value) = state.mode_low_byte() {
                self.byte(value);
            }
        }
        self.flag(trigger.common8a().is_some());
        if let Some(state) = trigger.common8a() {
            use ge_core::mission::triggers::common8a::Gate;
            self.byte(match state.gate() {
                Gate::HeaderDisabled => 0,
                Gate::NumericBitClear => 1,
                Gate::RequiresHook => 2,
            });
        }
        self.flag(trigger.radius1b().is_some());
        if let Some(state) = trigger.radius1b() {
            self.u32(state.radius_squared_bits());
        }
        self.flag(trigger.radius74().is_some());
        if let Some(state) = trigger.radius74() {
            self.u32(state.radius_bits());
            self.u32(state.radius_squared_bits());
            self.u32(state.file_reference());
            self.u32(state.numeric2_raw());
        }
        self.flag(trigger.shape71().is_some());
        if let Some(state) = trigger.shape71() {
            use ge_core::mission::triggers::shape71::{Geometry, Stage};
            self.u32(state.radius_bits());
            self.u32(state.radius_squared_bits());
            self.u32(state.geometry().tag() as u32);
            match state.geometry() {
                Geometry::Uninitialized => {}
                Geometry::Type0 {
                    center_bits,
                    radius_bits,
                } => {
                    self.words(center_bits);
                    self.u32(*radius_bits);
                }
                Geometry::Type2 {
                    center_bits,
                    axis_a_bits,
                    axis_b_bits,
                    dimensions_bits,
                } => {
                    self.words(center_bits);
                    self.words(axis_a_bits);
                    self.words(axis_b_bits);
                    self.words(dimensions_bits);
                }
            }
            self.byte(match state.geometry_stage() {
                Stage::Uninitialized => 0,
                Stage::Materialized => 1,
                Stage::PlacementBuilt => 2,
                Stage::Transformed => 3,
                Stage::FallbackComplete => 4,
            });
            self.flag(state.placement_matrix_bits().is_some());
            if let Some(matrix) = state.placement_matrix_bits() {
                self.words(&matrix);
            }
        }
        self.flag(trigger.radius88().is_some());
        if let Some(state) = trigger.radius88() {
            self.byte(match state.stage() {
                ge_core::mission::triggers::radius88::Stage::Constructor => 0,
                ge_core::mission::triggers::radius88::Stage::PostCommon => 1,
            });
            self.u32(state.radius_squared_bits());
            self.optional_word(state.word_118());
        }
        self.flag(trigger.aux_ac().is_some());
        if let Some(state) = trigger.aux_ac() {
            self.byte(match state.stage() {
                ge_core::mission::triggers::aux_ac::Stage::Constructor => 0,
                ge_core::mission::triggers::aux_ac::Stage::PostCommon => 1,
            });
            self.u32(state.radius_squared_bits());
            self.usize(state.extra_words().len());
            for word in state.extra_words() {
                self.optional_word(*word);
            }
            self.usize(state.auxiliary_records().len());
            for auxiliary in state.auxiliary_records() {
                self.words(auxiliary.words());
                self.0.update(auxiliary.flag_bytes());
                for byte in auxiliary.padding() {
                    self.flag(byte.is_some());
                    if let Some(byte) = byte {
                        self.byte(byte);
                    }
                }
            }
        }
    }

    fn range(&mut self, range: &ge_core::mission::triggers::range0c::State) {
        self.u32(range.radius_bits());
        self.u32(range.radius_squared_bits());
        self.u32(range.secondary_radius_bits());
        self.u32(range.secondary_radius_squared_bits());
        self.u32(range.animation_reference());
        self.u32(range.resource_reference());
        self.optional_word(range.requested_resource());
        self.byte(range.mode_high_byte());
        self.flag(range.mode_low_byte().is_some());
        if let Some(value) = range.mode_low_byte() {
            self.byte(value);
        }
    }

    fn actor(&mut self, actor: &ge_core::mission::triggers::actor12::State) {
        use ge_core::mission::triggers::actor12::{
            ConstructorStage, MapInitialization, ProviderStage, SelectionBranch,
        };
        match actor.stage() {
            ConstructorStage::Provider(stage) => {
                self.byte(0);
                self.byte(match stage {
                    ProviderStage::DescriptorFallback => 0,
                    ProviderStage::LevelSelector => 1,
                    ProviderStage::Definition => 2,
                });
            }
            ConstructorStage::Complete => self.byte(1),
        }
        self.words(&actor.current_rotation_bits());
        self.flag(actor.rotation_repaired());
        self.flag(actor.level_selection().is_some());
        if let Some(selection) = actor.level_selection() {
            self.u32(selection.word);
            match selection.branch {
                SelectionBranch::AbsentContext => self.byte(0),
                SelectionBranch::Direct { selector } => {
                    self.byte(1);
                    self.byte(selector);
                }
                SelectionBranch::Random { selector } => {
                    self.byte(2);
                    self.byte(selector);
                }
            }
            self.flag(selection.sentinel_fallback);
        }
        self.flag(actor.definition().is_some());
        if let Some(id) = actor.definition() {
            self.u64(id.get());
        }
        self.optional_word(actor.radius_squared_bits());
        self.u32(actor.word_74());
        self.u32(actor.word_78());
        self.byte(actor.byte_11c());
        match actor.map_initialization() {
            MapInitialization::NotRun => self.byte(0),
            MapInitialization::Blocked { scan, ordinal } => {
                self.byte(1);
                self.byte(scan);
                self.optional_word(ordinal);
            }
            MapInitialization::Complete => self.byte(2),
        }
        let matches = actor.scan_matches();
        self.usize(matches.len());
        for found in matches {
            self.flag(found.is_some());
            if let Some(id) = found {
                self.u64(id.get());
            }
        }
    }
    fn optional_word(&mut self, value: Option<u32>) {
        self.flag(value.is_some());
        if let Some(word) = value {
            self.u32(word);
        }
    }
    pub(super) fn byte(&mut self, value: u8) {
        self.0.update([value]);
    }
    pub(super) fn flag(&mut self, value: bool) {
        self.byte(u8::from(value));
    }
    pub(super) fn u16(&mut self, value: u16) {
        self.0.update(value.to_be_bytes());
    }
    pub(super) fn u32(&mut self, value: u32) {
        self.0.update(value.to_be_bytes());
    }
    pub(super) fn u64(&mut self, value: u64) {
        self.0.update(value.to_be_bytes());
    }
    pub(super) fn usize(&mut self, value: usize) {
        self.u64(value as u64);
    }
    pub(super) fn text(&mut self, value: &str) {
        self.usize(value.len());
        self.0.update(value.as_bytes());
    }
    pub(super) fn words(&mut self, values: &[u32]) {
        self.usize(values.len());
        for value in values {
            self.u32(*value);
        }
    }
    pub(super) fn optional_usize(&mut self, value: Option<usize>) {
        self.flag(value.is_some());
        if let Some(value) = value {
            self.usize(value);
        }
    }
    pub(super) fn map(&mut self, value: MapContext) {
        self.u64(value.instance.get());
        self.byte(value.map_selector as u8);
    }
    pub(super) fn owner(&mut self, value: OwnerBinding) {
        match value {
            OwnerBinding::Unbound => self.byte(0),
            OwnerBinding::PlayerBasic(id) => {
                self.byte(1);
                self.u64(id.get());
            }
            OwnerBinding::Other(id) => {
                self.byte(2);
                self.u64(id.get());
            }
        }
    }
    pub(super) fn event(&mut self, value: EventOutcome) {
        match value {
            EventOutcome::Denied => self.byte(0),
            EventOutcome::Unmapped => self.byte(1),
            EventOutcome::UnsupportedImmediateExecution { state } => {
                self.byte(2);
                self.byte(state);
            }
        }
    }
    pub(super) fn animator(&mut self, value: &Runtime) {
        let h = value.header();
        self.u16(h.statement_count_raw);
        for byte in [
            h.variables,
            h.shared_variables,
            h.states,
            h.format,
            h.event_ff,
            h.event_create,
            h.event_105,
            h.event_10e,
        ] {
            self.byte(byte);
        }
        let c = value.context();
        self.u64(c.trigger_handle);
        self.byte(c.map_selector as u8);
        self.words(&c.position_bits);
        self.flag(c.manager_present);
        match c.camera {
            CameraContext::KnownMissing => self.byte(0),
            CameraContext::Bound(id) => {
                self.byte(1);
                self.u64(id.get());
            }
        }
        let s = value.source_state();
        self.byte(s.unique_states_raw);
        self.u32(s.execution_budget_raw);
        self.byte(s.callback_flags_raw);
        self.byte(s.selected_marker_raw);
        self.byte(s.secondary_marker_raw);
        self.usize(s.active_executions);
        self.owner(s.owner);
        self.event(s.constructor_create);
        self.event(s.registration_event);
        self.words(value.shared_variables());
        self.flag(value.extra_scratch().is_some());
        if let Some(words) = value.extra_scratch() {
            self.usize(words.len());
            for word in words {
                self.flag(word.is_some());
                if let Some(word) = word {
                    self.u32(*word);
                }
            }
        }
        self.usize(value.state_flags().len());
        self.0.update(value.state_flags());
        self.usize(value.execution_slots().len());
        for slot in value.execution_slots() {
            self.flag(slot.is_some());
            if let Some(id) = slot {
                self.u64(id.get());
            }
        }
    }
}
