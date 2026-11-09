//! Real host audio resource service. The shared source job registers SoundInfo;
//! backend bank commands have separate payload/index registration callbacks.
use anyhow::{Context, Result, ensure};
use ge_assets::{sound_bank, sound_info};
use ge_core::mission::loader::{
    outer,
    phase::AttemptId,
    work::{Ticket, WorkOwner},
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::Arc};

use super::assets::{Origin, Sources};

pub(super) const METADATA: u32 = 0x2d20_0000;

pub(super) struct Metadata {
    pub origin: Origin,
    pub bytes: Arc<Vec<u8>>,
    pub header: sound_info::Header,
    pub digest: [u8; 32],
}
pub(super) struct ResidentBank {
    pub origin: Origin,
    pub bytes: Arc<Vec<u8>>,
    pub sounds: usize,
    pub waves: usize,
    pub digest: [u8; 32],
}
#[derive(Debug, Clone, Copy)]
pub(super) struct Command {
    pub id: u64,
    pub token: u32,
    pub flags: u32,
    pub sequence: u32,
}
pub(super) struct Service {
    pub metadata_origin: Option<Origin>,
    pub metadata: Option<Metadata>,
    pub banks: BTreeMap<u32, ResidentBank>,
    pub pending: BTreeMap<u64, Command>,
    pub sequence: u32,
    next_command: u64,
}
impl Service {
    pub fn new() -> Self {
        Self {
            metadata_origin: None,
            metadata: None,
            banks: BTreeMap::new(),
            pending: BTreeMap::new(),
            sequence: 0,
            next_command: 1,
        }
    }
    pub fn complete(&self, fields: outer::BankFields) -> bool {
        self.metadata.is_some()
            && self.pending.is_empty()
            && [fields.word_4c, fields.word_50, fields.word_54]
                .into_iter()
                .filter(|&token| token != 0)
                .all(|token| self.banks.contains_key(&token))
    }
}

#[derive(Clone)]
pub(super) enum Task {
    SoundInfo(Origin),
    Bank(Command),
}
pub(super) struct Batch {
    pub tasks: BTreeMap<usize, Task>,
    metadata_ticket: Option<Ticket>,
}
impl Batch {
    pub fn new() -> Self {
        Self {
            tasks: BTreeMap::new(),
            metadata_ticket: None,
        }
    }

    pub fn ensure_metadata(
        &mut self,
        service: &mut Service,
        work: &mut WorkOwner,
        attempt: AttemptId,
        origin: Option<&Origin>,
    ) -> Result<outer::JobOutcome> {
        if service.metadata.is_some() || self.metadata_ticket.is_some() {
            return Ok(outer::JobOutcome::Existing);
        }
        let existed = service.metadata_origin.is_some();
        let origin = service.metadata_origin.as_ref().or(origin);
        let Some(origin) = origin else {
            return Ok(outer::JobOutcome::Unavailable);
        };
        let origin = origin.clone();
        let ticket = work.enqueue(attempt, METADATA)?;
        self.tasks
            .insert(ticket.id(), Task::SoundInfo(origin.clone()));
        self.metadata_ticket = Some(ticket);
        service.metadata_origin = Some(origin);
        Ok(if existed {
            outer::JobOutcome::Existing
        } else {
            outer::JobOutcome::Submitted
        })
    }

    /// A cancelled attempt does not erase commands already submitted to the
    /// global host backend. A new scope adopts their actual unfinished work.
    pub fn recover(
        &mut self,
        service: &mut Service,
        work: &mut WorkOwner,
        attempt: AttemptId,
        origin: Option<&Origin>,
    ) -> Result<()> {
        if service.pending.is_empty() {
            return Ok(());
        }
        self.ensure_metadata(service, work, attempt, origin)?;
        for command in service.pending.values() {
            let ticket = work.enqueue(attempt, command.token)?;
            self.tasks.insert(ticket.id(), Task::Bank(*command));
        }
        Ok(())
    }
}

pub(super) struct Provider<'a> {
    pub context: &'a crate::mission::MissionContext,
    pub service: &'a mut Service,
    pub batch: &'a mut Batch,
    pub work: &'a mut WorkOwner,
    pub attempt: AttemptId,
    pub metadata_origin: Option<&'a Origin>,
}
impl outer::BankProvider for Provider<'_> {
    fn selected_level(&mut self) -> Result<Option<outer::BankRow>> {
        Ok(self.context.registry().selected_primary().map(|row| {
            let words = row.words_raw();
            outer::BankRow {
                primary: words[0x50 / 4],
                optional: [words[0x54 / 4], words[0x58 / 4]],
            }
        }))
    }
    fn multiplayer_extra_required(&mut self) -> Result<bool> {
        // This service is reached by the admitted typed NewGame worker only:
        // its actual source descriptor+2c is1, independently of checkpoint state.
        Ok(false)
    }
    fn ensure_shared_type1_job(&mut self) -> Result<outer::JobOutcome> {
        self.batch
            .ensure_metadata(self.service, self.work, self.attempt, self.metadata_origin)
    }
    fn dispatch_bank(&mut self, token: u32, flags: u32) -> Result<outer::DispatchOutcome> {
        ensure!(
            self.service.pending.len() < 64,
            "host bank command budget exceeded"
        );
        let id = self.service.next_command;
        self.service.next_command = id
            .checked_add(1)
            .context("host bank command identity overflow")?;
        self.service.sequence = self.service.sequence.wrapping_add(1);
        let command = Command {
            id,
            token,
            flags: if flags == u32::MAX { 0 } else { flags },
            sequence: self.service.sequence,
        };
        self.service.pending.insert(id, command);
        let ticket = self.work.enqueue(self.attempt, token)?;
        self.batch.tasks.insert(ticket.id(), Task::Bank(command));
        Ok(outer::DispatchOutcome::Submitted)
    }
}

pub(super) enum Loaded {
    SoundInfo(Metadata),
    Bank(Command, ResidentBank),
}

pub(super) fn load(
    task: &Task,
    sources: &mut Sources<'_>,
    metadata: Option<Arc<Vec<u8>>>,
    cached_bank: Option<Arc<Vec<u8>>>,
) -> Result<Loaded> {
    match task {
        Task::SoundInfo(origin) => {
            let bytes = sources.read(origin)?;
            let view = sound_info::read(&bytes, 65_536)?;
            let header = *view.header();
            ensure!(
                header.resource_hash == METADATA,
                "shared audio job metadata identity differs"
            );
            let digest = Sha256::digest(&bytes).into();
            Ok(Loaded::SoundInfo(Metadata {
                origin: origin.clone(),
                bytes: Arc::new(bytes),
                header,
                digest,
            }))
        }
        Task::Bank(command) => {
            ensure!(command.flags == 0, "host bank flag branch is unsupported");
            let metadata = metadata.context("bank finalizer requires retained shared SoundInfo")?;
            let origin = sources
                .locate("File_COM.bin", command.token, 64 * 1024 * 1024)?
                .context("requested bank is absent from its actual archive index")?;
            let bytes = match cached_bank {
                Some(bytes) => bytes,
                None => Arc::new(sources.read(&origin)?),
            };
            let view =
                sound_bank::Bank::read(&bytes, command.token, sound_bank::Budget::default())?;
            let info = sound_info::read(&metadata, 65_536)?;
            for index in 0..view.sound_count() {
                ensure!(
                    info.lookup(view.sound(index)?.hash())?.is_some(),
                    "bank sound has no registered runtime metadata"
                );
            }
            let bank = ResidentBank {
                origin,
                bytes: bytes.clone(),
                sounds: view.sound_count(),
                waves: view.wave_count(),
                digest: Sha256::digest(bytes.as_slice()).into(),
            };
            Ok(Loaded::Bank(*command, bank))
        }
    }
}

pub(super) fn register(service: &mut Service, loaded: Loaded) -> Result<()> {
    match loaded {
        Loaded::SoundInfo(metadata) => {
            if let Some(current) = &service.metadata {
                ensure!(
                    current.digest == metadata.digest,
                    "shared audio metadata changed between scopes"
                );
            } else {
                service.metadata = Some(metadata);
            }
        }
        Loaded::Bank(command, bank) => {
            if let Some(current) = service.banks.get(&command.token) {
                ensure!(
                    current.digest == bank.digest,
                    "retained bank changed between scopes"
                );
            } else {
                let retained =
                    service
                        .banks
                        .values()
                        .try_fold(bank.bytes.len(), |total, bank| {
                            total
                                .checked_add(bank.bytes.len())
                                .context("retained bank byte count overflow")
                        })?;
                ensure!(
                    retained <= 64 * 1024 * 1024,
                    "retained bank byte budget exceeded"
                );
                service.banks.insert(command.token, bank);
            }
            service.pending.remove(&command.id);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests/banks.rs"]
mod tests;
