//! Receipts describe the completed operation cut. The live frame clock continues
//! advancing afterward; its actual samples are deliberately not deterministic.
use anyhow::{Result, ensure};
use ge_core::mission::world_hooks::{buffer::Buffer, clock_reset, final_reset};
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::State;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Evidence {
    pub origin: &'static str,
    pub lifetime: &'static str,
    pub clock_provider: &'static str,
    pub operation_complete: bool,
    pub clock_reset_complete: bool,
    pub clock_reads: usize,
    pub constructor_clock_reads: usize,
    pub completed_clock_resets: usize,
    pub clock_frame_updates: usize,
    pub clock_seconds_bits: u32,
    pub clock_timescale_bits: u32,
    pub clock_snapshot_bound: bool,
    pub scoped_buffers: usize,
    pub buffers_cleared: usize,
    pub buffers_preserved: usize,
    pub backings_released: usize,
    pub bytes_released: usize,
    pub retained_backing_bytes: usize,
    pub leaf_reset_complete: bool,
    pub literal_noops_reached: usize,
    pub binding_released: bool,
    pub callback_installed: bool,
    pub callback_invocations: usize,
    pub clock_state_sha256: String,
    pub final_state_sha256: String,
    pub state_sha256: String,
    pub original_history_verified: bool,
    pub native_clock_precision_verified: bool,
    pub native_timing_verified: bool,
    pub world_ready_emitted: bool,
    pub scope: &'static str,
}

pub(super) fn build(
    state: &State,
    leaf: &final_reset::LeafReset,
    buffers: &final_reset::BuffersReset,
    two: &final_reset::TwoBuffersReset,
) -> Result<Evidence> {
    let clock_complete = state.reset.stage() == clock_reset::Stage::Complete;
    ensure!(clock_complete, "world-hook clock reset did not complete");
    let clock_clears = state
        .reset
        .clears_in_attempt()
        .iter()
        .map(|receipt| receipt.as_ref().map(|receipt| receipt.stats));
    let clears = clock_clears
        .chain(buffers.cleared.iter().map(|receipt| Some(receipt.stats)))
        .chain(two.cleared.iter().map(|receipt| Some(receipt.stats)))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| anyhow::anyhow!("world-hook clear receipt is absent"))?;
    let all_buffers = state
        .reset
        .buffers()
        .iter()
        .chain(state.final_owners.buffers().top_buffers())
        .chain(state.final_owners.buffers().record_buffers())
        .chain(state.final_owners.two_buffers().buffers())
        .collect::<Vec<_>>();
    let leaf_complete = leaf.byte_writes == 6 && leaf.word_writes == 7;
    let callback_installed = state.final_owners.callback()
        == final_reset::CallbackSlot::Installed(final_reset::InstalledCallback::WorldTraversal);
    let noops = state.final_owners.noop_reaches();
    let correct_noops = noops
        .iter()
        .map(|receipt| receipt.hook)
        .eq([final_reset::Noop::First, final_reset::Noop::Second]);
    ensure!(
        clears.len() <= all_buffers.len(),
        "world-hook clear count exceeds scoped owners"
    );
    let complete = clock_complete
        && leaf_complete
        && buffers.complete
        && correct_noops
        && callback_installed
        && clears.len() == 22;
    let clock_digest = clock_fingerprint(state);
    let final_digest = final_fingerprint(state);
    let backings = clears.iter().filter(|receipt| receipt.had_backing).count();
    let released = clears.iter().try_fold(0usize, |total, receipt| {
        total
            .checked_add(receipt.bytes_released)
            .ok_or_else(|| anyhow::anyhow!("released byte count overflow"))
    })?;
    let retained = all_buffers.iter().try_fold(0usize, |total, buffer| {
        total
            .checked_add(buffer.backing_capacity())
            .ok_or_else(|| anyhow::anyhow!("retained byte count overflow"))
    })?;
    let mut digest = Fingerprint::new(b"ge_world_hook_segment_v1\0");
    digest.0.update(clock_digest);
    digest.0.update(final_digest);
    digest.u32(state.clock.bits());
    digest.u32(0x3f80_0000);
    for count in [
        state.reset.reads_in_attempt(),
        state.constructor_clock_reads,
        state.reset.completed_resets(),
        state.clock.updates(),
        all_buffers.len(),
        clears.len(),
        all_buffers.len() - clears.len(),
        backings,
        released,
        retained,
        noops.len(),
        0,
    ] {
        digest.count(count);
    }
    for flag in [
        complete,
        clock_complete,
        true,
        leaf_complete,
        buffers.release.is_some(),
        callback_installed,
    ] {
        digest.flag(flag);
    }
    Ok(Evidence {
        origin: "fresh_constructor_snapshot",
        lifetime: "native_window_controller",
        clock_provider: "host_frame_elapsed_f32",
        operation_complete: complete,
        clock_reset_complete: clock_complete,
        clock_reads: state.reset.reads_in_attempt(),
        constructor_clock_reads: state.constructor_clock_reads,
        completed_clock_resets: state.reset.completed_resets(),
        clock_frame_updates: state.clock.updates(),
        clock_seconds_bits: state.clock.bits(),
        clock_timescale_bits: 0x3f80_0000,
        clock_snapshot_bound: true,
        scoped_buffers: all_buffers.len(),
        buffers_cleared: clears.len(),
        buffers_preserved: all_buffers.len() - clears.len(),
        backings_released: backings,
        bytes_released: released,
        retained_backing_bytes: retained,
        leaf_reset_complete: leaf_complete,
        literal_noops_reached: noops.len(),
        binding_released: buffers.release.is_some(),
        callback_installed,
        callback_invocations: 0,
        clock_state_sha256: hex(&clock_digest),
        final_state_sha256: hex(&final_digest),
        state_sha256: hex(&digest.finish()),
        original_history_verified: false,
        native_clock_precision_verified: false,
        native_timing_verified: false,
        world_ready_emitted: false,
        scope: "Persistent controller-owned manager snapshots execute the sixteen-read clock reset, two literal no-ops, sparse leaf writes, actual owned backing cleanup and optional binding flag release, followed by typed callback installation without invocation. The host cached f32 frame clock advances through startup, menu and loading; Tutorial and cancellation do not reset it. A real cached-word lease supplies all sixteen ordered reads, separately from the constructor's sixteen reads. Scoped buffer counts omit unrelated untouched manager fields. Byte counts are host Vec allocation capacities. Original global/frame/precision/history and native scheduling remain unverified. This segment does not execute the remaining loader phases, provider barriers, readiness event, player or camera creation.",
    })
}

fn clock_fingerprint(state: &State) -> [u8; 32] {
    let mut hash = Fingerprint::new(b"ge_world_clock_reset_v1\0");
    hash.count(clock_reset::WORD_OFFSETS.len());
    for offset in clock_reset::WORD_OFFSETS {
        hash.count(offset);
        hash.word(state.reset.word(offset));
    }
    hash.buffers(state.reset.buffers());
    hash.finish()
}

fn final_fingerprint(state: &State) -> [u8; 32] {
    let owners = &state.final_owners;
    let mut hash = Fingerprint::new(b"ge_world_final_reset_v1\0");
    hash.count(owners.leaf().bytes().len());
    for (&offset, &value) in owners.leaf().bytes() {
        hash.0.update(offset.to_be_bytes());
        hash.byte(value);
    }
    hash.count(owners.leaf().words().len());
    for (&offset, &value) in owners.leaf().words() {
        hash.0.update(offset.to_be_bytes());
        hash.word(value);
    }
    let buffers = owners.buffers();
    hash.buffers(buffers.top_buffers());
    hash.buffers(buffers.record_buffers());
    let tail = buffers.tail();
    hash.byte(tail.flag_1c);
    hash.byte(tail.flag_3b0);
    for word in [tail.word_28, tail.word_e0, tail.word_e4] {
        hash.word(word);
    }
    match buffers.binding_slot() {
        final_reset::BindingSlot::Unknown => hash.0.update([0]),
        final_reset::BindingSlot::Absent => hash.0.update([1]),
        final_reset::BindingSlot::Bound(id) => {
            hash.0.update([2]);
            hash.u64(id.get());
        }
    }
    hash.count(buffers.bindings().len());
    for (id, binding) in buffers.bindings() {
        hash.u64(id.get());
        hash.0.update([binding.byte_40(), binding.byte_41()]);
    }
    hash.buffers(owners.two_buffers().buffers());
    hash.count(owners.noop_reaches().len());
    for receipt in owners.noop_reaches() {
        hash.0.update([match receipt.hook {
            final_reset::Noop::First => 0,
            final_reset::Noop::Second => 1,
        }]);
    }
    hash.0.update([match owners.callback() {
        final_reset::CallbackSlot::Unknown => 0,
        final_reset::CallbackSlot::Installed(final_reset::InstalledCallback::WorldTraversal) => 1,
    }]);
    hash.finish()
}

struct Fingerprint(Sha256);
impl Fingerprint {
    fn new(domain: &[u8]) -> Self {
        let mut hash = Sha256::new();
        hash.update(domain);
        Self(hash)
    }
    fn flag(&mut self, flag: bool) {
        self.0.update([u8::from(flag)]);
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
    fn byte(&mut self, value: Option<u8>) {
        self.flag(value.is_some());
        if let Some(value) = value {
            self.0.update([value]);
        }
    }
    fn word(&mut self, value: Option<u32>) {
        self.flag(value.is_some());
        if let Some(value) = value {
            self.u32(value);
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
    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}
fn hex(hash: &[u8; 32]) -> String {
    hash.iter().map(|b| format!("{b:02x}")).collect()
}
