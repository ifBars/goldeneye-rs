//! Source-backed primary actor definitions, sharing the already retained
//! character resource. This is not complete native manager or Native9 creation.
mod evidence;
mod fingerprint;

use crate::mission::MissionContext;
use anyhow::{Context, Result, ensure};
pub use evidence::Evidence;
use ge_assets::actor_definitions::{self as source, Definitions as View};
use ge_core::mission::{
    actor_definitions::{self as runtime, Registry},
    source_rng::{self, SourceRngOwner},
    triggers::actor12::{LevelContext, LevelOutcome, LevelSelection, ProviderOutcome},
};

pub(super) struct Definitions {
    registry: Registry,
    evidence: Evidence,
    level: LevelContext,
    rng: SourceRngOwner,
    selection_calls: usize,
}

impl Definitions {
    pub fn bind(context: &MissionContext) -> Result<Self> {
        let edb = context.actor_definition_source();
        let view = View::read(
            edb,
            source::Budget {
                primary_rows: 4096,
                secondary_rows: 4096,
                total_words: 4096 * 23,
                total_bytes: 4096 * 23 * 4,
            },
        )?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(view.primary_count().unwrap_or(0))
            .context("owning actor definition rows")?;
        for index in 0..view.primary_count().unwrap_or(0) {
            let row = view.primary(index)?;
            rows.push(runtime::Primary {
                source_index: row.source_index(),
                words: *row.words(),
            });
        }
        let registry = Registry::build(
            rows,
            runtime::Budget {
                rows: 4096,
                groups: 4096,
                members: 4096,
            },
        )?;
        let rng = SourceRngOwner::from_constructor();
        let evidence = Evidence::from_source(edb, &view, &registry, &rng, 0, false)?;
        let level = LevelContext::from_level_row(context.registry().selected_primary());
        Ok(Self {
            registry,
            evidence,
            level,
            rng,
            selection_calls: 0,
        })
    }

    pub fn descriptor_fallback(&self) -> u32 {
        self.level.descriptor_fallback()
    }

    /// Generic player handler construction consumes the same engine stream as
    /// the earlier Actor12 selector. It does not select another definition.
    pub fn draw_handler_seed(&mut self) -> Result<u16> {
        Ok(self.rng.draw_u32(source_rng::Budget { draws: 1_000_000 })? as u16)
    }

    pub fn level_selector(&mut self, token: u32) -> Result<ProviderOutcome<LevelSelection>> {
        Ok(
            match self.level.select(
                token,
                Some(&mut self.rng),
                source_rng::Budget { draws: 1_000_000 },
            )? {
                LevelOutcome::Selected(selection) => ProviderOutcome::Ready(selection),
                LevelOutcome::RequiresRng => ProviderOutcome::Unavailable,
            },
        )
    }

    pub fn definition(&mut self, reference: u32) -> Result<ProviderOutcome<runtime::RecordId>> {
        ensure!(
            self.selection_calls < 65_536,
            "actor definition call budget exhausted"
        );
        let selected = self.registry.selection_mut(reference)?;
        self.selection_calls += 1;
        Ok(match selected {
            runtime::Selection::Selected { record, .. } => {
                self.registry.record(record)?;
                ProviderOutcome::Ready(record)
            }
            runtime::Selection::RequiresDefault { .. }
            | runtime::Selection::RequiresSpecial { .. } => ProviderOutcome::Unavailable,
        })
    }

    pub fn owns_definition(&self, record: runtime::RecordId) -> bool {
        self.registry.record(record).is_ok()
    }

    pub fn prepare_post_trigger(
        &mut self,
        context: &MissionContext,
    ) -> Result<ge_core::mission::post_trigger::Operation> {
        ensure!(
            self.selection_calls < 65_536,
            "actor definition call budget exhausted"
        );
        // This adapter is reached only by the admitted New Game worker. Its
        // source descriptor+2c is 1, so 800c0b20 returns false. Other session
        // modes must supply their actual predicate rather than reuse this path.
        let operation = ge_core::mission::post_trigger::Operation::prepare(
            false,
            context.registry().selected_primary(),
            &mut self.registry,
            runtime::LookupBudget {
                comparisons: usize::BITS as usize,
            },
        )?;
        self.selection_calls += usize::from(operation.selection_performed());
        Ok(operation)
    }

    pub fn refresh(&mut self, context: &MissionContext, actor_ready: bool) -> Result<()> {
        let edb = context.actor_definition_source();
        let view = View::read(
            edb,
            source::Budget {
                primary_rows: 4096,
                secondary_rows: 4096,
                total_words: 4096 * 23,
                total_bytes: 4096 * 23 * 4,
            },
        )?;
        self.evidence = Evidence::from_source(
            edb,
            &view,
            &self.registry,
            &self.rng,
            self.selection_calls,
            actor_ready,
        )?;
        Ok(())
    }

    pub fn evidence(&self) -> &Evidence {
        debug_assert_eq!(
            self.registry.record_count(),
            self.evidence.registered_primary
        );
        &self.evidence
    }
}
