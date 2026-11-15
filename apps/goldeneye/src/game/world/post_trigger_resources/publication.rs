//! A fresh host base publication and its actual same-wrapper request history.
//! Retained section bytes do not establish dependency or script-body fixups.
use super::LoadedFile;
use anyhow::{Context, Result, ensure};
use ge_assets::{edb::Edb, edb::script_publication::Section};
use ge_core::animation::script_child::Binding;
use serde::Serialize;
use std::{collections::BTreeMap, sync::Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RetainedSection {
    pub index: usize,
    pub key: u32,
    pub start: usize,
    pub end: usize,
}

struct History {
    // Present only for the checked section-zero callback of this newly
    // constructed wrapper. It is not copied from another resource or player.
    fresh_callback_mask: Option<u32>,
    requests: u64,
    active_requests: usize,
    unknown_history: bool,
    sections: BTreeMap<usize, RetainedSection>,
}

impl History {
    fn permits_fresh_null(&self) -> bool {
        self.fresh_callback_mask.is_some_and(|mask| mask & 8 != 0)
            && !self.unknown_history
            && self.active_requests == 0
            // Other sections' unresolved dependency/reference work can affect
            // this one. Host payload success cannot close those callbacks.
            && self.sections.is_empty()
    }
}

/// Current publication facts, separate from a child initializer's historical
/// return. No payloads, host addresses or native scheduling claims are exposed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Evidence {
    pub file: u32,
    pub generation: u32,
    pub descriptor_index: usize,
    pub section_index: usize,
    pub section_key: u32,
    pub callback_mask: Option<u32>,
    pub fresh_base: bool,
    pub request_count: u64,
    pub active_requests: usize,
    pub retained_sections: usize,
    pub history_unknown: bool,
    pub raw_job: Option<u32>,
    pub body_state: &'static str,
    pub native_history_verified: bool,
}

pub(super) struct Publication(Mutex<History>);

pub(super) enum ScriptBody<'a> {
    Absent,
    Pending,
    Resident(ge_assets::frontend_script::Script<'a>),
}

/// A dropped/failed operation keeps an unknown request prefix. No caller may
/// erase that prefix by retrying the file lookup or cloning a FileBinding.
pub(super) struct Request<'a> {
    owner: &'a Publication,
    finished: bool,
}

impl LoadedFile {
    /// Only the registry's vacant-entry path constructs a fresh wrapper. A
    /// subsequent lookup retains its Arc and cannot recreate this origin.
    pub(super) fn first_base_publication(edb: Edb) -> Self {
        let fresh_callback_mask = edb
            .script_publication()
            .and_then(|view| view.base_callback_mask())
            .ok();
        Self {
            edb,
            // The source constructor's -1 followed by first base completion.
            generation: 0,
            publication: Publication(Mutex::new(History {
                fresh_callback_mask,
                requests: 0,
                active_requests: 0,
                unknown_history: false,
                sections: BTreeMap::new(),
            })),
        }
    }

    pub(super) fn script_body(&self, binding: Binding) -> Result<ScriptBody<'_>> {
        let script = self.cached_script(binding)?;
        let descriptor = script.descriptor();
        if descriptor.section & 0x7fff == 0 {
            return Ok(
                match self
                    .edb
                    .base_animation_script_body(binding.directory_index)?
                {
                    Some(body) => ScriptBody::Resident(body),
                    None => ScriptBody::Absent,
                },
            );
        }
        let section = script.section()?;
        let key = section.key()?;
        // The current host section provider supports this streamed kind only.
        // No body/span read is reached for an untouched source NULL job.
        Ok(
            if key >> 24 == 8
                && key != 0x0800_0000
                && section.directory_index() != 0
                && self.publication.known_null(section)?
            {
                ScriptBody::Absent
            } else {
                ScriptBody::Pending
            },
        )
    }

    fn cached_script(
        &self,
        binding: Binding,
    ) -> Result<ge_assets::edb::script_publication::Script<'_>> {
        ensure!(
            binding.directory_index < 32767,
            "script body directory index exceeds native bound"
        );
        let view = self.edb.script_publication()?;
        let script = view
            .script(0x8400_0000 | u32::try_from(binding.directory_index)?)?
            .context("cached script descriptor disappeared")?;
        let descriptor = script.descriptor();
        ensure!(
            descriptor.hash == binding.hash,
            "cached script descriptor identity changed"
        );
        Ok(script)
    }

    pub(super) fn script_publication_evidence(&self, binding: Binding) -> Result<Option<Evidence>> {
        let script = self.cached_script(binding)?;
        if script.descriptor().section & 0x7fff == 0 {
            return Ok(None);
        }
        let section = script.section()?;
        let key = section.key()?;
        let state = self
            .publication
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("resource publication lock poisoned"))?;
        let raw_job = if state.permits_fresh_null()
            && key >> 24 == 8
            && key != 0x0800_0000
            && section.directory_index() != 0
        {
            Some(section.runtime_job_raw()?)
        } else {
            None
        };
        Ok(Some(Evidence {
            file: self.edb.header.hash,
            generation: self.generation,
            descriptor_index: binding.directory_index,
            section_index: section.directory_index(),
            section_key: key,
            callback_mask: state.fresh_callback_mask,
            fresh_base: state.fresh_callback_mask.is_some(),
            request_count: state.requests,
            active_requests: state.active_requests,
            retained_sections: state.sections.len(),
            history_unknown: state.unknown_history,
            raw_job,
            body_state: if raw_job == Some(0) {
                "known_null"
            } else {
                "pending"
            },
            native_history_verified: false,
        }))
    }

    pub(super) fn resident_script_body(
        &self,
        binding: Binding,
    ) -> Result<ge_assets::frontend_script::Script<'_>> {
        match self.script_body(binding)? {
            ScriptBody::Resident(script) => Ok(script),
            ScriptBody::Absent => anyhow::bail!("script runtime body is known NULL"),
            ScriptBody::Pending => anyhow::bail!("script runtime body publication is pending"),
        }
    }
}

impl Publication {
    #[cfg(test)]
    pub(super) fn has_unknown_history(&self) -> Result<bool> {
        Ok(self
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("resource publication lock poisoned"))?
            .unknown_history)
    }

    pub(super) fn begin_request(&self) -> Result<Request<'_>> {
        let mut state = self
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("resource publication lock poisoned"))?;
        state.requests = state
            .requests
            .checked_add(1)
            .context("resource request counter overflow")?;
        state.active_requests = state
            .active_requests
            .checked_add(1)
            .context("active resource requests overflow")?;
        Ok(Request {
            owner: self,
            finished: false,
        })
    }

    /// This closes only the source branch which explicitly stores NULL for a
    /// non-base descriptor whose same-wrapper section job is still absent.
    /// All observed requests and unimplemented section completion work remain
    /// Pending; no cached body is exposed by this narrow proof.
    fn known_null(&self, section: Section<'_>) -> Result<bool> {
        let state = self
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("resource publication lock poisoned"))?;
        if !state.permits_fresh_null() {
            return Ok(false);
        }
        Ok(section.runtime_job_raw()? == 0)
    }
}

impl Request<'_> {
    pub(super) fn finish(mut self, section: Option<RetainedSection>) -> Result<()> {
        let mut state = self
            .owner
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("resource publication lock poisoned"))?;
        ensure!(
            state.active_requests > 0,
            "resource request completion has no prefix"
        );
        if let Some(section) = section {
            if let Some(previous) = state.sections.get(&section.index) {
                ensure!(
                    *previous == section,
                    "resource publication section identity changed"
                );
            } else {
                state.sections.insert(section.index, section);
            }
        }
        state.active_requests -= 1;
        self.finished = true;
        Ok(())
    }
}

impl Drop for Request<'_> {
    fn drop(&mut self) {
        if !self.finished {
            // Poison keeps future readers failing closed. Never panic during
            // another failure's unwinding or restore an unrequested state.
            if let Ok(mut state) = self.owner.0.lock() {
                state.unknown_history = true;
                state.active_requests = state.active_requests.saturating_sub(1);
            }
        }
    }
}

#[cfg(test)]
mod tests;
