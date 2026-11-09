use anyhow::{Context, Result, ensure};
use ge_core::mission::post_trigger::{Operation, TailReset};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

use super::{File, Returned};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Evidence {
    pub origin: &'static str,
    pub provider: &'static str,
    pub operation_complete: bool,
    pub selection_performed: bool,
    pub selected_definition_bound: bool,
    pub ordered_requests: usize,
    pub resolved_requests: usize,
    pub retained_files: usize,
    pub retained_sections: usize,
    pub retained_file_bytes: usize,
    /// Union of reached validated section spans in each shared file payload.
    pub retained_section_bytes: usize,
    pub tail_reset_applied: bool,
    pub requests_sha256: String,
    pub residency_sha256: String,
    pub state_sha256: String,
    pub original_cursor_history_verified: bool,
    pub original_rng_history_verified: bool,
    pub original_stream_history_verified: bool,
    pub native_stream_timing_verified: bool,
    pub native_stream_readiness_verified: bool,
    pub scope: &'static str,
}

pub(super) fn build(
    operation: &Operation,
    files: &BTreeMap<u32, File>,
    returned: &[Returned],
    tail: TailReset,
) -> Result<Evidence> {
    ensure!(
        operation.tail_reset() == Some(tail),
        "post-trigger tail owner differs"
    );
    let mut requests = Fingerprint::new(b"ge_post_trigger_requests_v1\0");
    requests.flag(operation.selection_reference().is_some());
    if let Some(reference) = operation.selection_reference() {
        requests.u32(reference);
    }
    requests.flag(operation.selected_definition().is_some());
    if let Some(selected) = operation.selected_definition() {
        requests.u64(selected.record.get());
        requests.u32(selected.key);
        requests.count(selected.source_index);
        requests.flag(selected.group.is_some());
        if let Some(group) = selected.group {
            requests.0.update(group.to_be_bytes());
        }
        requests.flag(selected.member_index.is_some());
        if let Some(index) = selected.member_index {
            requests.count(index);
        }
    }
    requests.count(operation.requests().len());
    for request in operation.requests() {
        requests.0.update([request.ordinal]);
        requests.u32(request.file);
        requests.u32(request.target);
    }
    let mut residency = Fingerprint::new(b"ge_post_trigger_residency_v1\0");
    residency.count(files.len());
    let mut file_bytes = 0usize;
    let mut section_bytes = 0usize;
    let mut sections = 0usize;
    for (&key, file) in files {
        ensure!(
            key == file.edb.header.hash,
            "post-trigger file receipt identity differs"
        );
        for value in [
            key,
            file.edb.header.version,
            file.edb.header.base_bytes,
            file.edb.header.payload_bytes,
        ] {
            residency.u32(value);
        }
        file_bytes = file_bytes
            .checked_add(file.edb.header.payload_bytes as usize)
            .context("post-trigger file byte count overflow")?;
        residency.count(file.sections.len());
        for section in &file.sections {
            residency.count(section.index);
            residency.u32(section.key);
            residency.count(section.start);
            residency.count(section.end);
            residency.0.update(section.sha256);
        }
        sections += file.sections.len();
        section_bytes = section_bytes
            .checked_add(covered_bytes(file)?)
            .context("post-trigger section byte count overflow")?;
    }
    residency.count(returned.len());
    for response in returned {
        residency.0.update([response.request.ordinal]);
        residency.u32(response.request.file);
        residency.u32(response.request.target);
        residency.flag(response.resolution.is_some());
        if let Some(target) = &response.resolution {
            residency.count(target.directory_index);
            residency.u32(target.stored_hash);
            residency.0.update(target.raw_section.to_be_bytes());
            residency.count(target.section_index);
            residency.u32(target.key);
            residency.flag(target.base);
        }
    }
    let resolved = returned
        .iter()
        .filter(|response| response.resolution.is_some())
        .count();
    let complete = operation.is_finished()
        && returned.len() == operation.requests().len()
        && resolved == returned.len();
    let requests = requests.finish();
    let residency = residency.finish();
    let mut state = Fingerprint::new(b"ge_post_trigger_source_hook_state_v1\0");
    state.0.update(requests);
    state.0.update(residency);
    for flag in [
        complete,
        operation.selection_performed(),
        operation.selected_definition().is_some(),
        operation.is_finished(),
    ] {
        state.flag(flag);
    }
    for count in [
        returned.len(),
        resolved,
        files.len(),
        sections,
        file_bytes,
        section_bytes,
    ] {
        state.count(count);
    }
    for word in tail.words_00_0c {
        state.u32(word);
    }
    state.u32(tail.word_338);
    state.u32(tail.scalar_340_bits);
    state.0.update([tail.byte_344]);
    Ok(Evidence {
        origin: "source_hook_snapshot",
        provider: "synchronous_supplied_data_residency",
        operation_complete: complete,
        selection_performed: operation.selection_performed(),
        selected_definition_bound: operation.selected_definition().is_some(),
        ordered_requests: returned.len(),
        resolved_requests: resolved,
        retained_files: files.len(),
        retained_sections: sections,
        retained_file_bytes: file_bytes,
        retained_section_bytes: section_bytes,
        tail_reset_applied: operation.is_finished(),
        requests_sha256: hex(&requests),
        residency_sha256: hex(&residency),
        state_sha256: hex(&state.finish()),
        original_cursor_history_verified: false,
        original_rng_history_verified: false,
        original_stream_history_verified: false,
        native_stream_timing_verified: false,
        native_stream_readiness_verified: false,
        scope: "The source post-trigger hook consumes one selection from the same retained actor-definition owner, performs both ordered target requests and retains only its proven sparse reset writes. Actual checked supplied-data reads share immutable full-file payloads and validated section spans; covered section bytes count their union per file. Missing targets or failed reads remain incomplete even though the source caller ignores NULL returns and reaches its reset. This synchronous host residency adaptation does not implement original stream relocation, scheduler, prior manager history, later world hooks, readiness events, player or camera creation.",
    })
}

fn covered_bytes(file: &File) -> Result<usize> {
    let mut spans = file
        .sections
        .iter()
        .map(|section| section.start..section.end)
        .collect::<Vec<_>>();
    spans.sort_by_key(|span| span.start);
    let (mut end, mut count) = (0usize, 0usize);
    for span in spans {
        ensure!(
            span.start <= span.end && span.end <= file.edb.header.payload_bytes as usize,
            "post-trigger retained section span differs"
        );
        count = count
            .checked_add(span.end.saturating_sub(end.max(span.start)))
            .context("post-trigger covered byte count overflow")?;
        end = end.max(span.end);
    }
    Ok(count)
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
    fn u32(&mut self, word: u32) {
        self.0.update(word.to_be_bytes());
    }
    fn u64(&mut self, word: u64) {
        self.0.update(word.to_be_bytes());
    }
    fn count(&mut self, count: usize) {
        self.u64(count as u64);
    }
    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}
fn hex(hash: &[u8; 32]) -> String {
    hash.iter().map(|b| format!("{b:02x}")).collect()
}
