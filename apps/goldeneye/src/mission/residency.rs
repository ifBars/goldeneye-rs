//! Actual bounded EDB ownership in native file-discovery order. Completing this
//! stage establishes residency, not world-manager registration or player setup.
use super::metadata::{self, ResourceEvidence};
use anyhow::{Context, Result, bail, ensure};
use ge_assets::{edb::Edb, mission::maps, resource::GameAssets};
use ge_core::mission::residency::{self, Completion, Discovery, Link, Request, State};
use serde::Serialize;

#[derive(Debug, Clone, Copy)]
pub(super) struct Budget {
    pub files: usize,
    pub per_file_bytes: u32,
    pub total_bytes: u64,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            files: 32,
            per_file_bytes: 64 * 1024 * 1024,
            total_bytes: 128 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct ResidentEvidence {
    ordinal: usize,
    file_hash: u32,
    requested_map_reference: u32,
    resolved_map_hash: u32,
    map_directory_index: usize,
    payload_bytes: u32,
    base_bytes: u32,
    zones: usize,
    bsp_present: bool,
    bsp_nodes: usize,
    placements: usize,
    entity_descriptors: usize,
    script_descriptors: usize,
    textures: usize,
    next_link: Option<maps::LinkedMap>,
}

#[derive(Debug, Serialize)]
pub(super) struct Evidence {
    complete: bool,
    stop_reason: &'static str,
    repeated_file: Option<u32>,
    retained_files: usize,
    total_payload_bytes: u64,
    file_limit: usize,
    per_file_byte_limit: u32,
    total_byte_limit: u64,
    resources: Vec<ResidentEvidence>,
    scope: &'static str,
}

pub(super) struct Residency {
    resources: Vec<Edb>,
    requests: Vec<Request>,
    metadata: Vec<ResourceEvidence>,
    budget: Budget,
    completion: Completion,
}

impl Residency {
    pub fn load(assets: &mut GameAssets, root: u32) -> Result<Self> {
        Self::from_loader(root, Budget::default(), |file, maximum| {
            super::resource::load(assets, file, maximum)
        })
    }

    /// The loader receives the remaining aggregate allowance before it reads a
    /// full payload. Errors drop the entire local attempt and expose no context.
    pub fn from_loader(
        root: u32,
        budget: Budget,
        mut load: impl FnMut(u32, u32) -> Result<Edb>,
    ) -> Result<Self> {
        ensure!(
            budget.files <= 128,
            "mission residency file limit exceeds host budget"
        );
        ensure!(
            budget.per_file_bytes >= Edb::HEADER_BYTES as u32
                && budget.per_file_bytes <= 64 * 1024 * 1024,
            "mission residency per-file byte limit is unsupported"
        );
        ensure!(
            budget.total_bytes >= Edb::HEADER_BYTES as u64
                && budget.total_bytes <= 256 * 1024 * 1024,
            "mission residency total byte limit is unsupported"
        );
        let mut discovery = Discovery::new(
            root,
            residency::Budget {
                files: budget.files,
            },
        )?;
        let mut resources = Vec::new();
        let mut metadata = Vec::new();
        let mut total_bytes = 0u64;
        let completion = loop {
            let request = match discovery.state() {
                State::WaitingForResource(request) => request,
                State::Complete(completion) => break completion,
                state => bail!("unexpected mission residency state: {state:?}"),
            };
            let remaining = budget
                .total_bytes
                .checked_sub(total_bytes)
                .context("mission residency byte accounting overflow")?;
            let maximum = u32::try_from(remaining.min(u64::from(budget.per_file_bytes)))?;
            if maximum < Edb::HEADER_BYTES as u32 {
                discovery.resource_failed(request)?;
                bail!(
                    "mission residency total byte budget exhausted before resource {:#010x}",
                    request.file
                );
            }
            let resource = match load(request.file, maximum) {
                Ok(resource) => resource,
                Err(error) => {
                    discovery.resource_failed(request)?;
                    return Err(error).with_context(|| {
                        format!("loading mission resource {:#010x}", request.file)
                    });
                }
            };
            ensure!(
                resource.header.hash == request.file && resource.header.version == 263,
                "mission residency resource identity/version differs from request"
            );
            ensure!(
                resource.header.base_bytes >= Edb::HEADER_BYTES as u32
                    && resource.header.payload_bytes >= resource.header.base_bytes,
                "mission residency resource has invalid payload/base sizes"
            );
            ensure!(
                resource.header.payload_bytes <= maximum,
                "mission residency loader exceeded the preflight byte allowance"
            );
            let summary = metadata::read(&resource, request.map_reference)?;
            ensure!(
                summary.resolved_map_hash.is_some(),
                "mission residency coded map index zero is absent"
            );
            total_bytes = total_bytes
                .checked_add(u64::from(resource.header.payload_bytes))
                .context("mission residency total bytes overflow")?;
            resources
                .try_reserve(1)
                .context("retaining mission resource")?;
            metadata
                .try_reserve(1)
                .context("retaining mission metadata")?;
            resources.push(resource);
            metadata.push(summary);
            discovery.resource_ready(request)?;
            // Lookup follows actual ownership and map resolution. Its returned
            // map word is preserved as provenance, never substituted for index0.
            let link = match resources
                .last()
                .context("retained mission resource absent")?
                .mission_linked_map(
                    request.map_reference,
                    maps::Budget {
                        maps: 128,
                        rows: 65_536,
                        types: 65_536,
                    },
                ) {
                Ok(link) => link,
                Err(error) => {
                    discovery.link_failed(request)?;
                    return Err(error).context("reading retained mission linked-map trigger");
                }
            };
            metadata
                .last_mut()
                .context("retained mission metadata absent")?
                .next_link = link;
            discovery.linked(
                request,
                link.map(|link| Link {
                    file: link.file,
                    map: link.map,
                }),
            )?;
        };
        ensure!(
            resources.len() == discovery.entries().len(),
            "mission residency ownership count differs from discovery"
        );
        Ok(Self {
            resources,
            requests: discovery.entries().to_vec(),
            metadata,
            budget,
            completion,
        })
    }

    pub fn resources(&self) -> &[Edb] {
        &self.resources
    }

    pub fn requests(&self) -> &[Request] {
        &self.requests
    }

    pub fn root_metadata(&self) -> Result<&ResourceEvidence> {
        self.metadata
            .first()
            .context("mission root metadata is absent")
    }

    pub fn evidence(&self) -> Result<Evidence> {
        let resources = self
            .metadata
            .iter()
            .enumerate()
            .map(|(ordinal, source)| {
                let map = source
                    .maps
                    .first()
                    .context("retained mission coded map zero is absent")?;
                Ok(ResidentEvidence {
                    ordinal,
                    file_hash: source.resource_hash,
                    requested_map_reference: source.requested_map_reference,
                    resolved_map_hash: map.hash,
                    map_directory_index: map.directory_index,
                    payload_bytes: source.payload_bytes,
                    base_bytes: source.base_bytes,
                    zones: map.zones,
                    bsp_present: map.bsp_present,
                    bsp_nodes: map.bsp_nodes,
                    placements: map.placements,
                    entity_descriptors: source.entity_descriptors,
                    script_descriptors: source.script_descriptors,
                    textures: source.textures,
                    next_link: source.next_link,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Evidence {
            complete: true,
            stop_reason: match self.completion {
                Completion::NoLink => "no_link",
                Completion::RepeatedFile { .. } => "repeated_file",
            },
            repeated_file: match self.completion {
                Completion::NoLink => None,
                Completion::RepeatedFile { file } => Some(file),
            },
            retained_files: self.resources.len(),
            total_payload_bytes: self
                .resources
                .iter()
                .map(|edb| u64::from(edb.header.payload_bytes))
                .sum(),
            file_limit: self.budget.files,
            per_file_byte_limit: self.budget.per_file_bytes,
            total_byte_limit: self.budget.total_bytes,
            resources,
            scope: "Actual EDB owners retained in source file-discovery order. File-only deduplication and forced coded map index zero preserve the source stop rule. Total bytes count retained EDB payloads once, excluding catalog rows and host overhead. Residency completion is not world registration, final world-manager ordering, script activation, player initialization or original-runtime parity.",
        })
    }
}

#[cfg(test)]
#[path = "tests/residency.rs"]
mod tests;
