//! The next source world hook, backed by actual synchronous supplied-data
//! residency. This owner does not model the original streaming scheduler.
mod evidence;
mod targets;
pub(crate) use targets::Target;

use anyhow::{Context, Result, ensure};
use ge_assets::{edb::Edb, resource::GameAssets, target_section};
use ge_core::mission::post_trigger::{Operation, Request, TailReset};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::sync::Arc;

pub use evidence::Evidence;

struct Section {
    index: usize,
    key: u32,
    start: usize,
    end: usize,
    sha256: [u8; 32],
}

struct File {
    edb: Arc<Edb>,
    sections: Vec<Arc<Section>>,
}

struct Resolution {
    directory_index: usize,
    stored_hash: u32,
    raw_section: u16,
    section_index: usize,
    key: u32,
    base: bool,
}

struct Returned {
    request: Request,
    resolution: Option<Resolution>,
    // A checked failure remains incomplete and privately inspectable. The
    // caller still performs its second request, just as for a NULL return.
    _failure: Option<anyhow::Error>,
}

pub(crate) struct Owned {
    operation: Operation,
    files: BTreeMap<u32, File>,
    returned: Vec<Returned>,
    tail: TailReset,
    evidence: Evidence,
}

impl Owned {
    pub(crate) fn is_complete(&self) -> bool {
        self.operation.is_finished()
            && self.returned.len() == self.operation.requests().len()
            && self
                .returned
                .iter()
                .all(|response| response.resolution.is_some())
    }
    pub fn build(operation: Operation, assets: &mut GameAssets) -> Result<Self> {
        Self::build_with(operation, |file| {
            crate::mission::resource::load(assets, file, 64 * 1024 * 1024)
        })
    }

    fn build_with(
        mut operation: Operation,
        mut load: impl FnMut(u32) -> Result<Edb>,
    ) -> Result<Self> {
        let mut files = BTreeMap::new();
        let mut returned = Vec::new();
        while let Some(&request) = operation.next_request() {
            let result = resolve(request, &mut files, &mut load);
            let (resolution, failure) = match result {
                Ok(resolution) => (Some(resolution), None),
                Err(error) => (None, Some(error)),
            };
            returned.push(Returned {
                request,
                resolution,
                _failure: failure,
            });
            // Source8005aa20 ignores the helper's NULL/non-NULL result. Actual
            // residency success is tracked separately and is required below.
            operation.record_returned(request.ordinal)?;
        }
        let tail = operation.finish()?;
        let evidence = evidence::build(&operation, &files, &returned, tail)?;
        Ok(Self {
            operation,
            files,
            returned,
            tail,
            evidence,
        })
    }

    pub fn evidence(&self) -> &Evidence {
        // The evidence is scoped to this finished source hook. Later player
        // requests reuse these actual wrappers without rewriting that snapshot.
        debug_assert_eq!(self.returned.len(), self.evidence.ordered_requests);
        debug_assert_eq!(self.operation.tail_reset(), Some(self.tail));
        &self.evidence
    }
}

fn resolve(
    request: Request,
    files: &mut BTreeMap<u32, File>,
    load: &mut impl FnMut(u32) -> Result<Edb>,
) -> Result<Resolution> {
    ensure!(
        request.is_file_request(),
        "post-trigger file prefix rejected"
    );
    if let std::collections::btree_map::Entry::Vacant(entry) = files.entry(request.file) {
        let edb = load(request.file)?;
        ensure!(
            edb.header.hash == request.file,
            "post-trigger file owner differs"
        );
        entry.insert(File {
            edb: Arc::new(edb),
            sections: Vec::new(),
        });
    }
    let file = files
        .get_mut(&request.file)
        .context("post-trigger file owner is absent")?;
    let target = file
        .edb
        .target_section(
            request.target,
            target_section::Budget {
                targets: 65_536,
                sections: 4096,
                section_bytes: 64 * 1024 * 1024,
            },
        )?
        .context("post-trigger target descriptor is absent")?;
    let resolution = Resolution {
        directory_index: target.directory_index(),
        stored_hash: target.stored_hash(),
        raw_section: target.raw_section(),
        section_index: target.section_index(),
        key: target.key(),
        base: target.is_base(),
    };
    if !target.is_base() {
        // The source manager only creates streamed records for section keys08.
        // Unknown branches remain incomplete rather than claiming residency.
        ensure!(
            target.key() >> 24 == 8,
            "post-trigger section kind is unsupported"
        );
        let span = target
            .span()
            .context("post-trigger section span is absent")?;
        let payload = target
            .payload()
            .context("post-trigger section payload is absent")?;
        if let Some(existing) = file
            .sections
            .iter()
            .find(|section| section.index == target.section_index())
        {
            ensure!(
                existing.key == target.key()
                    && existing.start == span.start
                    && existing.end == span.end,
                "post-trigger retained section identity changed"
            );
        } else {
            file.sections.push(Arc::new(Section {
                index: target.section_index(),
                key: target.key(),
                start: span.start,
                end: span.end,
                sha256: Sha256::digest(payload).into(),
            }));
        }
    }
    Ok(resolution)
}

#[cfg(test)]
mod tests;
