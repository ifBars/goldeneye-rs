//! Actual outer loader operations. Host bank callbacks register retained bytes
//! before the source-shaped wait barriers can promote their queued phase.
mod assets;
mod banks;
mod evidence;
mod maps;
mod maps_source;

use anyhow::{Context, Result, ensure};
use ge_assets::resource::GameAssets;
use ge_core::mission::{
    loader::{
        checkpoint, outer,
        phase::{AttemptId, LoaderScalars, Phase8Byte, PhaseState, ResourcePoll, WaitPoll},
        work::{self, WorkOwner},
    },
    triggers::{Registry, TriggerId},
    world_hooks::buffer::Buffer,
};

use super::Attempt;
use crate::{game::world::post_trigger_resources, mission::MissionContext};
pub use evidence::Evidence;

pub(super) struct Persistent {
    outer: outer::Owner,
    checkpoint: Result<checkpoint::Owner>,
    banks: outer::BankOwner,
    service: banks::Service,
    phase8: Phase8Byte,
    scalars: LoaderScalars,
    contexts: ge_core::mission::maps::Contexts,
}
impl Persistent {
    pub fn new() -> Self {
        Self {
            outer: outer::Owner::from_constructor(),
            checkpoint: checkpoint::Owner::from_constructor(checkpoint::Budget {
                payload_bytes: checkpoint::PAYLOAD_BYTES,
            }),
            banks: outer::BankOwner::from_constructor(),
            service: banks::Service::new(),
            phase8: Phase8Byte::from_constructor(),
            scalars: LoaderScalars::unbound(),
            contexts: ge_core::mission::maps::Contexts::from_constructor(),
        }
    }
}

pub(crate) struct Run {
    attempt: AttemptId,
    phase: PhaseState,
    work: WorkOwner,
    pre: outer::PreResetReceipt,
    bank_receipt: Option<outer::BankReceipt>,
    batch: banks::Batch,
    checkpoint_skips: usize,
    promotions: usize,
    busy_polls: usize,
    resource_polls: usize,
    sparse_skips: usize,
    catalog_rebuilt: bool,
    catalog_entries: usize,
    readiness: Option<ge_core::mission::session::Phase19Receipt>,
    maps: Option<maps::Owned>,
}

impl Attempt {
    pub fn new_game_loader_handoff(&self) -> Result<()> {
        self.ensure_current()?;
        let mut state = self
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("loader owner lock is poisoned"))?;
        self.ensure_current()?;
        state.loader.phase8.reset_to_ten()
    }

    pub fn start_loader(&self, work: WorkOwner) -> Result<Run> {
        self.ensure_current()?;
        let attempt = AttemptId::new(self.identity)?;
        ensure!(
            work.attempt() == attempt && work.poll(attempt)? == work::WorkPoll::Complete,
            "loader inputs have not completed their owned provider callback"
        );
        let mut state = self
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("loader owner lock is poisoned"))?;
        self.ensure_current()?;
        state
            .loader
            .checkpoint
            .as_ref()
            .map_err(|error| anyhow::anyhow!("checkpoint construction failed: {error}"))?;
        let pre = state.loader.outer.run_pre_hooks();
        Ok(Run {
            attempt,
            phase: PhaseState::new(attempt, 7)?,
            work,
            pre,
            bank_receipt: None,
            batch: banks::Batch::new(),
            checkpoint_skips: 0,
            promotions: 0,
            busy_polls: 0,
            resource_polls: 0,
            sparse_skips: 0,
            catalog_rebuilt: false,
            catalog_entries: 0,
            readiness: None,
            maps: None,
        })
    }

    pub fn bind_world_scratch(&self, ids: &[TriggerId]) -> Result<()> {
        self.ensure_current()?;
        let count = u32::try_from(ids.len())?;
        ensure!(count <= 65_536, "loader scratch reference budget exceeded");
        let capacity = count
            .div_ceil(16)
            .checked_mul(16)
            .context("loader scratch capacity overflow")?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(capacity as usize * 8)?;
        for id in ids {
            bytes.extend_from_slice(&id.get().to_be_bytes());
        }
        let mut state = self
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("loader owner lock is poisoned"))?;
        self.ensure_current()?;
        let buffer = Buffer::from_owned(capacity, 16, count, (!ids.is_empty()).then_some(bytes))?;
        *state
            .loader
            .outer
            .world_buffer_mut(0)
            .context("world scratch header is absent")? = buffer;
        Ok(())
    }
}

impl Run {
    pub fn construct_maps(
        &mut self,
        hook: &Attempt,
        maps: &[crate::mission::world::TriggerMap<'_>],
    ) -> Result<()> {
        ensure!(
            self.maps.is_none() && self.phase.current() == 7,
            "loader maps already constructed or out of sequence"
        );
        self.maps = Some(maps::Owned::build(hook, maps)?);
        Ok(())
    }
    pub fn execute_to_phase19(
        &mut self,
        hook: &Attempt,
        context: &MissionContext,
        registry: &mut Registry,
        resources: &post_trigger_resources::Owned,
        assets: &mut GameAssets,
    ) -> Result<()> {
        ensure!(
            self.phase.current() == 7,
            "outer loader entry phase differs"
        );
        ensure!(
            self.maps.as_ref().is_some_and(maps::Owned::is_complete),
            "loader runtime maps are incomplete"
        );
        let mut sources = assets::Sources::new(assets);
        let needs_metadata = {
            let state = hook
                .shared
                .0
                .state
                .lock()
                .map_err(|_| anyhow::anyhow!("loader owner lock is poisoned"))?;
            state.loader.service.metadata.is_none()
        };
        let origin = if needs_metadata {
            sources.locate("File_USA.bin", banks::METADATA, 2 * 1024 * 1024)?
        } else {
            None
        };
        {
            hook.ensure_current()?;
            let mut state = hook
                .shared
                .0
                .state
                .lock()
                .map_err(|_| anyhow::anyhow!("loader owner lock is poisoned"))?;
            hook.ensure_current()?;
            let checkpoint = state
                .loader
                .checkpoint
                .as_ref()
                .map_err(|error| anyhow::anyhow!("checkpoint construction failed: {error}"))?;
            checkpoint.require_not_restoring()?;
            self.checkpoint_skips += 1;
            let enabled = state
                .final_owners
                .leaf()
                .byte(7)
                .context("bank enable byte is unknown")?;
            let Persistent { banks, service, .. } = &mut state.loader;
            self.batch
                .recover(service, &mut self.work, self.attempt, origin.as_ref())?;
            let mut provider = banks::Provider {
                context,
                service,
                batch: &mut self.batch,
                work: &mut self.work,
                attempt: self.attempt,
                metadata_origin: origin.as_ref(),
            };
            self.bank_receipt = Some(banks.run(enabled, &mut provider)?);
        }
        self.phase.queue(self.attempt, 8)?;
        self.wait(hook, registry, resources, &mut sources)?;
        {
            let mut state = hook
                .shared
                .0
                .state
                .lock()
                .map_err(|_| anyhow::anyhow!("loader owner lock is poisoned"))?;
            hook.ensure_current()?;
            self.phase
                .apply_case8(self.attempt, &mut state.loader.phase8)?;
        }
        self.wait(hook, registry, resources, &mut sources)?;
        // Actual retained restore-active state, not mode3, is the case10 gate.
        {
            let state = hook
                .shared
                .0
                .state
                .lock()
                .map_err(|_| anyhow::anyhow!("loader owner lock is poisoned"))?;
            hook.ensure_current()?;
            state
                .loader
                .checkpoint
                .as_ref()
                .map_err(|error| anyhow::anyhow!("checkpoint construction failed: {error}"))?
                .require_not_restoring()?;
            self.checkpoint_skips += 1;
        }
        self.phase.queue(self.attempt, 11)?;
        self.wait(hook, registry, resources, &mut sources)?;
        for next in [12, 13] {
            self.phase.queue(self.attempt, next)?;
            self.wait(hook, registry, resources, &mut sources)?;
        }
        let row = context.registry().selected_primary();
        let word = |offset: usize| row.map_or(0, |row| row.words_raw()[offset / 4]);
        ensure!(
            word(0x34) == 0,
            "case13 level resource provider branch is unsupported"
        );
        self.sparse_skips += 1;
        self.phase.queue(self.attempt, 14)?;
        self.wait(hook, registry, resources, &mut sources)?;
        let special = word(0x14);
        ensure!(
            special >> 16 != 0x0109 || special as u16 == 0,
            "case14/15 resource provider branch is unsupported"
        );
        for next in [15, 16] {
            self.sparse_skips += 1;
            self.phase.queue(self.attempt, next)?;
            self.wait(hook, registry, resources, &mut sources)?;
        }
        {
            let mut state = hook
                .shared
                .0
                .state
                .lock()
                .map_err(|_| anyhow::anyhow!("loader owner lock is poisoned"))?;
            hook.ensure_current()?;
            self.phase
                .apply_case16(self.attempt, &mut state.loader.scalars)?;
        }
        self.wait(hook, registry, resources, &mut sources)?;
        ensure!(
            word(0x38) == 0,
            "case17 level resource provider branch is unsupported"
        );
        self.sparse_skips += 1;
        self.phase.queue(self.attempt, 18)?;
        self.wait(hook, registry, resources, &mut sources)?;
        hook.ensure_current()?;
        ensure!(
            self.phase.current() == 18 && registry.is_complete(),
            "phase18 live registry is incomplete"
        );
        // This is the actual phase18 operation after the intervening providers
        // and barriers, separate from the earlier component cache preparation.
        registry.rebuild_catalog()?;
        self.catalog_rebuilt = true;
        self.catalog_entries = registry.catalog().len();
        self.phase.queue(self.attempt, 19)?;
        self.wait(hook, registry, resources, &mut sources)
    }

    fn wait(
        &mut self,
        hook: &Attempt,
        registry: &Registry,
        resources: &post_trigger_resources::Owned,
        sources: &mut assets::Sources<'_>,
    ) -> Result<()> {
        for _ in 0..128 {
            if let Err(error) = hook.ensure_current() {
                let _ = self.work.cancel(self.attempt);
                let _ = self.phase.cancel(self.attempt);
                return Err(error);
            }
            let resource_polls = &mut self.resource_polls;
            let polled = self.phase.poll(self.attempt, &self.work, || {
                *resource_polls += 1;
                ensure!(
                    resources.is_complete(),
                    "retained mission section provider is incomplete"
                );
                let state = hook
                    .shared
                    .0
                    .state
                    .lock()
                    .map_err(|_| anyhow::anyhow!("loader owner lock is poisoned"))?;
                ensure!(
                    state.loader.service.complete(state.loader.banks.fields()),
                    "host audio resource provider is incomplete"
                );
                // Read actual typed live membership. A reached unsupported7b
                // callback is an error, never an invented empty live list.
                for id in registry.membership() {
                    ensure!(
                        registry.trigger(*id)?.descriptor().subtype != 0x1f00_007b,
                        "live subtype7b resource callback is unsupported"
                    );
                }
                Ok(ResourcePoll::Complete)
            })?;
            match polled {
                WaitPoll::Promoted { .. } => {
                    self.promotions += 1;
                    return Ok(());
                }
                WaitPoll::ResourceBusy => anyhow::bail!("owned resource provider remains busy"),
                WaitPoll::WorkBusy => self.busy_polls += 1,
            }
            let batch = &self.batch;
            self.work
                .execute_next(self.attempt, |ticket| {
                    let task = batch
                        .tasks
                        .get(&ticket.id())
                        .context("host audio task callback is absent")?;
                    let (metadata, cached) =
                        {
                            let state =
                                hook.shared.0.state.lock().map_err(|_| {
                                    anyhow::anyhow!("loader owner lock is poisoned")
                                })?;
                            let metadata = state
                                .loader
                                .service
                                .metadata
                                .as_ref()
                                .map(|metadata| metadata.bytes.clone());
                            let cached = state
                                .loader
                                .service
                                .banks
                                .get(&ticket.key())
                                .map(|bank| bank.bytes.clone());
                            (metadata, cached)
                        };
                    let loaded = banks::load(task, sources, metadata, cached)?;
                    // Already-issued global audio commands retain immutable cache
                    // ownership across cancellation; no scene/user state is written.
                    let mut state = hook
                        .shared
                        .0
                        .state
                        .lock()
                        .map_err(|_| anyhow::anyhow!("loader owner lock is poisoned"))?;
                    banks::register(&mut state.loader.service, loaded)
                })?
                .context("busy host work has no executable callback")?;
        }
        anyhow::bail!("loader provider polling budget exhausted")
    }

    pub fn finish_session(
        &mut self,
        hook: &Attempt,
        session: &mut ge_core::mission::session::Session,
        binding: &ge_core::mission::session::LoadBinding,
        registry: &Registry,
        resources: &post_trigger_resources::Owned,
        assets: &mut GameAssets,
    ) -> Result<()> {
        hook.ensure_current()?;
        ensure!(
            self.maps.as_ref().is_some_and(maps::Owned::is_complete)
                && self.catalog_rebuilt
                && self.phase.current() == 19,
            "loader has no actual map/phase18 completion"
        );
        self.readiness = Some(session.finish_tutorial_phase19(binding, &mut self.phase)?);
        let mut sources = assets::Sources::new(assets);
        self.wait(hook, registry, resources, &mut sources)?;
        ensure!(
            self.phase.current() == 21,
            "loader readiness wait did not reach player phase"
        );
        hook.ensure_current()
    }

    pub fn evidence(&self, hook: &Attempt) -> Result<Evidence> {
        hook.ensure_current()?;
        let state = hook
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("loader owner lock is poisoned"))?;
        hook.ensure_current()?;
        evidence::build(self, &state)
    }
}
