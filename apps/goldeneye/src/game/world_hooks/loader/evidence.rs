//! Receipts fingerprint actual retained providers and executed loader work.
//! Logical base publication is independent of immutable full-file caching.
use anyhow::{Context, Result, ensure};
use ge_core::mission::{
    loader::{checkpoint, outer, work::TaskState},
    session::{FlagAction, Phase19Receipt},
    world_hooks::buffer::Buffer,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::super::State;
use super::{Run, assets::Origin, banks};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Evidence {
    pub origin: &'static str,
    pub lifetime: &'static str,
    pub provider: &'static str,
    pub operation_complete: bool,
    pub entry_phase: u8,
    pub current_phase: u8,
    pub pending_phase: Option<u8>,
    pub pre_hooks_complete: bool,
    pub maps_constructed: usize,
    pub map_zones: usize,
    pub map_component_slots: usize,
    pub map_context_bindings: usize,
    pub map_sections: usize,
    pub map_sections_published: usize,
    pub map_zones_published: usize,
    pub map_bindings: usize,
    pub checkpoint_bound: bool,
    pub checkpoint_restore_active: bool,
    pub checkpoint_payload_initialized: bool,
    pub checkpoint_payload_bytes: usize,
    pub checkpoint_skips: usize,
    pub source_bank_calls: usize,
    pub metadata_jobs_submitted: usize,
    pub backend_commands: usize,
    pub backend_sequence: u32,
    pub retained_banks: usize,
    pub retained_bank_bytes: usize,
    pub sound_entries: usize,
    pub wave_entries: usize,
    pub sound_info_bound: bool,
    pub sound_info_rows: usize,
    pub sound_info_bytes: usize,
    pub work_completed: usize,
    pub work_pending: usize,
    pub work_failed: usize,
    pub barrier_promotions: usize,
    pub resource_polls: usize,
    pub work_busy_polls: usize,
    pub phase8_byte: u8,
    pub phase8_resets: usize,
    pub loader_c4_bits: u32,
    pub loader_c0_bits: u32,
    pub catalog_phase18_entries: usize,
    pub readiness_events: usize,
    pub readiness_local_stores: usize,
    pub readiness_owner_flags: u32,
    pub readiness_peer_flags: u32,
    pub readiness_user_flags: u32,
    pub readiness_local_flags: u32,
    pub world_ready_emitted: bool,
    pub player_created: bool,
    pub original_history_verified: bool,
    pub native_sdk_dispatch_verified: bool,
    pub native_work_timing_verified: bool,
    pub banks_playback_verified: bool,
    pub pre_state_sha256: String,
    pub maps_state_sha256: String,
    pub banks_state_sha256: String,
    pub phase_state_sha256: String,
    pub state_sha256: String,
    pub scope: &'static str,
}

pub(super) fn build(run: &Run, state: &State) -> Result<Evidence> {
    let checkpoint = state
        .loader
        .checkpoint
        .as_ref()
        .map_err(|error| anyhow::anyhow!("loader checkpoint owner failed: {error}"))?;
    let stats = run.work.stats();
    let pending = stats
        .queued
        .checked_add(stats.running)
        .context("loader pending work count overflow")?;
    let map_counts = run
        .maps
        .as_ref()
        .map(|maps| maps.counts())
        .unwrap_or_default();
    let maps_complete = run.maps.as_ref().is_some_and(|maps| maps.is_complete());
    let pre_complete = run.pre.context_writes == 4
        && run.pre.animator_word_writes == 1
        && run.pre.animator_byte_writes == 1;
    let service = &state.loader.service;
    let bank_receipt = run.bank_receipt;
    let backend_commands = run
        .batch
        .tasks
        .values()
        .filter(|task| matches!(task, banks::Task::Bank(_)))
        .count();
    let retained_bytes = sum(service.banks.values().map(|bank| bank.bytes.len()))?;
    let sounds = sum(service.banks.values().map(|bank| bank.sounds))?;
    let waves = sum(service.banks.values().map(|bank| bank.waves))?;
    let readiness = run.readiness.as_ref();
    let complete = pre_complete
        && maps_complete
        && run.phase.current() == 21
        && run.phase.pending().is_none()
        && run.catalog_rebuilt
        && run.promotions == 12
        && run.resource_polls == 12
        && run.checkpoint_skips == 2
        && !checkpoint.restore_active()
        && stats.failed == 0
        && stats.cancelled == 0
        && pending == 0
        && stats.completed > backend_commands
        && service.complete(state.loader.banks.fields())
        && readiness.is_some_and(|receipt| {
            receipt.changed_events == 2
                && receipt.peer_updates == 2
                && receipt.user_updates == 2
                && receipt.local_mirror_stores == 4
                && receipt.owner_flags_raw == 0x108
                && receipt.peer_flags_raw == 0x108
                && receipt.user_flags_raw == 0x108
                && receipt.local_mirror_flags_raw == 0x109
                && receipt.created_byte_raw == 0
        });
    let pre_digest = pre_fingerprint(run, state, checkpoint)?;
    let map_digest = if let Some(maps) = &run.maps {
        decode_hash(&maps.state_sha256())?
    } else {
        Fingerprint::new(b"ge_loader_maps_unbound_v1\0").finish()
    };
    let banks_digest = banks_fingerprint(run, state);
    let phase_digest = phase_fingerprint(run, state);
    let mut evidence = Evidence {
        origin: "fresh_constructor_snapshot",
        lifetime: "native_window_controller",
        provider: "scoped_supplied_audio_and_base_map",
        operation_complete: complete,
        entry_phase: 7,
        current_phase: run.phase.current(),
        pending_phase: run.phase.pending(),
        pre_hooks_complete: pre_complete,
        maps_constructed: map_counts.constructed,
        map_zones: map_counts.zones,
        map_component_slots: map_counts.component_slots,
        map_context_bindings: map_counts.context_bindings,
        map_sections: map_counts.sections,
        map_sections_published: map_counts.sections_published,
        map_zones_published: map_counts.zones_published,
        map_bindings: map_counts.bindings,
        checkpoint_bound: true,
        checkpoint_restore_active: checkpoint.restore_active(),
        checkpoint_payload_initialized: checkpoint.payload_initialized(),
        checkpoint_payload_bytes: checkpoint.payload_len(),
        checkpoint_skips: run.checkpoint_skips,
        source_bank_calls: bank_receipt.map_or(0, |receipt| receipt.source_calls),
        metadata_jobs_submitted: bank_receipt.map_or(0, |receipt| receipt.job_submissions),
        backend_commands,
        backend_sequence: service.sequence,
        retained_banks: service.banks.len(),
        retained_bank_bytes: retained_bytes,
        sound_entries: sounds,
        wave_entries: waves,
        sound_info_bound: service.metadata.is_some(),
        sound_info_rows: service
            .metadata
            .as_ref()
            .map_or(0, |metadata| metadata.header.record_count),
        sound_info_bytes: service
            .metadata
            .as_ref()
            .map_or(0, |metadata| metadata.bytes.len()),
        work_completed: stats.completed,
        work_pending: pending,
        work_failed: stats.failed,
        barrier_promotions: run.promotions,
        resource_polls: run.resource_polls,
        work_busy_polls: run.busy_polls,
        phase8_byte: state
            .loader
            .phase8
            .value()
            .context("loader phase8 byte owner is unbound")?,
        phase8_resets: state.loader.phase8.resets(),
        loader_c4_bits: state.loader.scalars.c4_bits().unwrap_or(0),
        loader_c0_bits: state.loader.scalars.c0_bits().unwrap_or(0),
        catalog_phase18_entries: run.catalog_entries,
        readiness_events: readiness.map_or(0, |receipt| receipt.changed_events),
        readiness_local_stores: readiness.map_or(0, |receipt| receipt.local_mirror_stores),
        readiness_owner_flags: readiness.map_or(0, |receipt| receipt.owner_flags_raw),
        readiness_peer_flags: readiness.map_or(0, |receipt| receipt.peer_flags_raw),
        readiness_user_flags: readiness.map_or(0, |receipt| receipt.user_flags_raw),
        readiness_local_flags: readiness.map_or(0, |receipt| receipt.local_mirror_flags_raw),
        world_ready_emitted: readiness.is_some(),
        player_created: readiness.is_some_and(|receipt| receipt.created_byte_raw != 0),
        original_history_verified: false,
        native_sdk_dispatch_verified: false,
        native_work_timing_verified: false,
        banks_playback_verified: false,
        pre_state_sha256: hex(&pre_digest),
        maps_state_sha256: hex(&map_digest),
        banks_state_sha256: hex(&banks_digest),
        phase_state_sha256: hex(&phase_digest),
        state_sha256: String::new(),
        scope: "Actual retained constructor contexts, map initialization, fresh base-section publication, source-shaped bank calls and checked SoundInfo/bank payload registration precede the ordered owned loader barriers. Full-file cache residency does not publish nonbase geometry. Phase18 rebuilds the actual live catalog; the same worker dispatches identity-bound phase19 deltas and completes the final wait before publishing player-phase readiness. Earlier operation cuts carry zero unavailable readiness receipt/scalar counters, not proof of initialized native globals. Checkpoint payload contents are not read when unwritten. Histories persist across cancellation; clock/command/reset fingerprints may differ by attempt. Original global history, SDK dispatch, native task timing, bank playback, player creation, a camera and original gameplay parity are not established. Original arrays, payloads, identities and native pointers remain private.",
    };
    let mut hash = Fingerprint::new(b"ge_loader_state_v1\0");
    for digest in [pre_digest, map_digest, banks_digest, phase_digest] {
        hash.0.update(digest);
    }
    for count in evidence.counts() {
        hash.count(count);
    }
    for flag in evidence.flags() {
        hash.flag(flag);
    }
    hash.optional_u8(evidence.pending_phase);
    evidence.state_sha256 = hex(&hash.finish());
    Ok(evidence)
}

impl Evidence {
    // Declaration order, excluding optional pending and fixed provider strings.
    fn counts(&self) -> [usize; 39] {
        [
            usize::from(self.entry_phase),
            usize::from(self.current_phase),
            self.maps_constructed,
            self.map_zones,
            self.map_component_slots,
            self.map_context_bindings,
            self.map_sections,
            self.map_sections_published,
            self.map_zones_published,
            self.map_bindings,
            self.checkpoint_payload_bytes,
            self.checkpoint_skips,
            self.source_bank_calls,
            self.metadata_jobs_submitted,
            self.backend_commands,
            self.backend_sequence as usize,
            self.retained_banks,
            self.retained_bank_bytes,
            self.sound_entries,
            self.wave_entries,
            self.sound_info_rows,
            self.sound_info_bytes,
            self.work_completed,
            self.work_pending,
            self.work_failed,
            self.barrier_promotions,
            self.resource_polls,
            self.work_busy_polls,
            usize::from(self.phase8_byte),
            self.phase8_resets,
            self.loader_c4_bits as usize,
            self.loader_c0_bits as usize,
            self.catalog_phase18_entries,
            self.readiness_events,
            self.readiness_local_stores,
            self.readiness_owner_flags as usize,
            self.readiness_peer_flags as usize,
            self.readiness_user_flags as usize,
            self.readiness_local_flags as usize,
        ]
    }
    fn flags(&self) -> [bool; 12] {
        [
            self.operation_complete,
            self.pre_hooks_complete,
            self.checkpoint_bound,
            self.checkpoint_restore_active,
            self.checkpoint_payload_initialized,
            self.sound_info_bound,
            self.world_ready_emitted,
            self.player_created,
            self.original_history_verified,
            self.native_sdk_dispatch_verified,
            self.native_work_timing_verified,
            self.banks_playback_verified,
        ]
    }
}

fn pre_fingerprint(run: &Run, state: &State, checkpoint: &checkpoint::Owner) -> Result<[u8; 32]> {
    let mut hash = Fingerprint::new(b"ge_loader_pre_state_v1\0");
    for count in [
        run.pre.context_writes,
        run.pre.animator_word_writes,
        run.pre.animator_byte_writes,
    ] {
        hash.count(count);
    }
    for receipt in run.pre.world_buffers {
        hash.flag(receipt.had_backing);
        hash.count(receipt.bytes_released);
        hash.u32(receipt.previous_capacity);
        hash.u32(receipt.previous_count);
    }
    for (index, slot) in state.loader.outer.contexts().iter().enumerate() {
        match slot {
            outer::ContextSlot::Unknown => hash.byte(0),
            outer::ContextSlot::Absent => hash.byte(1),
            outer::ContextSlot::Bound(instance) => {
                hash.byte(2);
                hash.u64(instance.get());
            }
        }
        for mask in [
            state
                .loader
                .contexts
                .first_mask(index)
                .context("loader first context mask absent")?,
            state
                .loader
                .contexts
                .second_mask(index)
                .context("loader second context mask absent")?,
        ] {
            for &word in mask {
                hash.u32(word);
            }
        }
    }
    let animator = state.loader.outer.animator();
    hash.optional_u32(animator.word);
    hash.optional_u8(animator.byte);
    hash.buffers(state.loader.outer.world_buffers());
    hash.byte(match checkpoint.origin() {
        checkpoint::Origin::FreshConstructor => 0,
        checkpoint::Origin::SuppliedSnapshot => 1,
    });
    hash.byte(checkpoint.valid_raw());
    hash.byte(checkpoint.restore_active_raw());
    hash.count(checkpoint.payload_len());
    hash.flag(checkpoint.payload_initialized());
    Ok(hash.finish())
}

fn banks_fingerprint(run: &Run, state: &State) -> [u8; 32] {
    let mut hash = Fingerprint::new(b"ge_loader_banks_state_v1\0");
    let owner = &state.loader.banks;
    let fields = owner.fields();
    for word in [fields.word_4c, fields.word_50, fields.word_54] {
        hash.u32(word);
    }
    hash.byte(fields.byte_58);
    match owner.stage() {
        outer::BankStage::Constructor => hash.byte(0),
        outer::BankStage::Disabled => hash.byte(1),
        outer::BankStage::Running(step) => {
            hash.byte(2);
            hash.bank_step(step);
        }
        outer::BankStage::Failed(step) => {
            hash.byte(3);
            hash.bank_step(step);
        }
        outer::BankStage::Complete => hash.byte(4),
    }
    hash.optional_u32(owner.last_rejected());
    hash.count(owner.calls().len());
    for call in owner.calls() {
        hash.count(call.ordinal);
        hash.u32(call.token);
        hash.u32(call.flags);
        hash.byte(match call.job {
            None => 0,
            Some(outer::JobOutcome::Existing) => 1,
            Some(outer::JobOutcome::Submitted) => 2,
            Some(outer::JobOutcome::Unavailable) => 3,
        });
        hash.byte(match call.valid_class {
            None => 0,
            Some(false) => 1,
            Some(true) => 2,
        });
        hash.byte(match call.dispatch {
            None => 0,
            Some(outer::DispatchOutcome::BackendAbsent) => 1,
            Some(outer::DispatchOutcome::Submitted) => 2,
        });
        hash.flag(call.returned);
    }
    let service = &state.loader.service;
    hash.u32(service.sequence);
    hash.flag(service.metadata_origin.is_some());
    if let Some(origin) = &service.metadata_origin {
        hash.origin(origin);
    }
    hash.flag(service.metadata.is_some());
    if let Some(metadata) = &service.metadata {
        hash.origin(&metadata.origin);
        hash.count(metadata.bytes.len());
        hash.0.update(metadata.digest);
        for word in [
            metadata.header.resource_hash,
            metadata.header.version,
            metadata.header.file_bytes,
            metadata.header.minimum_hash,
            metadata.header.maximum_hash,
        ] {
            hash.u32(word);
        }
        hash.count(metadata.header.record_count);
    }
    hash.count(service.banks.len());
    for (&token, bank) in &service.banks {
        hash.u32(token);
        hash.origin(&bank.origin);
        hash.count(bank.bytes.len());
        hash.0.update(bank.digest);
        hash.count(bank.sounds);
        hash.count(bank.waves);
    }
    hash.count(service.pending.len());
    for command in service.pending.values() {
        hash.command(command);
    }
    hash.count(run.batch.tasks.len());
    for (&ticket, task) in &run.batch.tasks {
        hash.count(ticket);
        match task {
            banks::Task::SoundInfo(origin) => {
                hash.byte(0);
                hash.origin(origin);
            }
            banks::Task::Bank(command) => {
                hash.byte(1);
                hash.command(command);
            }
        }
    }
    hash.finish()
}

fn phase_fingerprint(run: &Run, state: &State) -> [u8; 32] {
    let mut hash = Fingerprint::new(b"ge_loader_phase_state_v1\0");
    hash.u64(run.attempt.get());
    hash.byte(run.phase.current());
    hash.optional_u8(run.phase.pending());
    hash.flag(run.phase.is_cancelled());
    hash.count(run.work.tickets().len());
    for task in run.work.tickets() {
        hash.count(task.ticket.id());
        hash.u64(task.ticket.attempt().get());
        hash.u32(task.ticket.key());
        hash.byte(match task.state {
            TaskState::Queued => 0,
            TaskState::Running => 1,
            TaskState::Completed => 2,
            TaskState::Failed => 3,
            TaskState::Cancelled => 4,
        });
    }
    for count in [
        run.checkpoint_skips,
        run.promotions,
        run.busy_polls,
        run.resource_polls,
        run.sparse_skips,
        run.catalog_entries,
    ] {
        hash.count(count);
    }
    hash.flag(run.catalog_rebuilt);
    hash.optional_u8(state.loader.phase8.value());
    hash.count(state.loader.phase8.resets());
    hash.optional_u32(state.loader.scalars.c4_bits());
    hash.optional_u32(state.loader.scalars.c0_bits());
    hash.flag(run.readiness.is_some());
    if let Some(receipt) = &run.readiness {
        hash.readiness(receipt);
    }
    hash.finish()
}

fn sum(mut values: impl Iterator<Item = usize>) -> Result<usize> {
    values.try_fold(0usize, |total, value| {
        total
            .checked_add(value)
            .context("loader retained count overflow")
    })
}
fn decode_hash(text: &str) -> Result<[u8; 32]> {
    ensure!(
        text.len() == 64
            && text
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "map fingerprint is invalid"
    );
    let mut hash = [0; 32];
    for (index, byte) in hash.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16)?;
    }
    Ok(hash)
}
struct Fingerprint(Sha256);
impl Fingerprint {
    fn new(domain: &[u8]) -> Self {
        let mut hash = Sha256::new();
        hash.update(domain);
        Self(hash)
    }
    fn byte(&mut self, value: u8) {
        self.0.update([value]);
    }
    fn flag(&mut self, value: bool) {
        self.byte(u8::from(value));
    }
    fn u32(&mut self, value: u32) {
        self.0.update(value.to_be_bytes());
    }
    fn u64(&mut self, value: u64) {
        self.0.update(value.to_be_bytes());
    }
    fn count(&mut self, value: usize) {
        self.u64(value as u64);
    }
    fn optional_u8(&mut self, value: Option<u8>) {
        self.flag(value.is_some());
        if let Some(value) = value {
            self.byte(value);
        }
    }
    fn optional_u32(&mut self, value: Option<u32>) {
        self.flag(value.is_some());
        if let Some(value) = value {
            self.u32(value);
        }
    }
    fn text(&mut self, value: &str) {
        self.count(value.len());
        self.0.update(value.as_bytes());
    }
    fn origin(&mut self, origin: &Origin) {
        self.text(origin.index);
        self.u32(origin.hash);
        self.u32(origin.version);
        self.count(origin.bytes);
        self.text(&origin.archive);
        self.u64(origin.offset);
    }
    fn command(&mut self, command: &banks::Command) {
        self.u64(command.id);
        self.u32(command.token);
        self.u32(command.flags);
        self.u32(command.sequence);
    }
    fn bank_step(&mut self, step: outer::BankStep) {
        match step {
            outer::BankStep::ReadLevel => self.byte(0),
            outer::BankStep::EnsureJob { ordinal } => {
                self.byte(1);
                self.count(ordinal);
            }
            outer::BankStep::Dispatch { ordinal } => {
                self.byte(2);
                self.count(ordinal);
            }
            outer::BankStep::ReadSession => self.byte(3),
        }
    }
    fn buffers(&mut self, buffers: &[Buffer]) {
        self.count(buffers.len());
        for buffer in buffers {
            for word in [buffer.capacity(), buffer.growth(), buffer.count()] {
                self.u32(word);
            }
            self.flag(buffer.has_backing());
            self.count(buffer.backing_len());
            if let Some(bytes) = buffer.backing() {
                self.0.update(bytes);
            }
        }
    }
    fn readiness(&mut self, receipt: &Phase19Receipt) {
        for action in receipt.actions {
            match action.action {
                FlagAction::Clear(mask) => {
                    self.byte(0);
                    self.u32(mask);
                }
                FlagAction::Set(mask) => {
                    self.byte(1);
                    self.u32(mask);
                }
            }
            for word in [
                action.owner_before,
                action.owner_after,
                action.local_before,
                action.local_after,
            ] {
                self.u32(word);
            }
            self.flag(action.local_store);
            self.flag(action.event.is_some());
            if let Some(event) = action.event {
                self.u64(event.event.session.get());
                self.u64(event.event.source.get());
                self.u32(event.event.delta.set);
                self.u32(event.event.delta.clear);
                self.flag(event.peer_updated);
                self.flag(event.user_updated);
            }
        }
        for count in [
            receipt.changed_events,
            receipt.peer_updates,
            receipt.user_updates,
            receipt.local_mirror_stores,
            receipt.local_observer_callbacks,
            receipt.user_observer_callbacks,
        ] {
            self.count(count);
        }
        for word in [
            receipt.owner_flags_raw,
            receipt.peer_flags_raw,
            receipt.user_flags_raw,
            receipt.local_mirror_flags_raw,
        ] {
            self.u32(word);
        }
        self.byte(receipt.created_byte_raw);
        self.flag(receipt.native_timing_verified);
    }
    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}
fn hex(hash: &[u8; 32]) -> String {
    hash.iter().map(|byte| format!("{byte:02x}")).collect()
}
