use anyhow::{Result, ensure};

use crate::mission::{
    actor_definitions::{LookupBudget, Registry, Selection},
    catalog::LevelRow,
};

use super::{PreparationError, Request, SelectedDefinition, SkipReason, TailReset};

/// One invocation of the source hook. This is intentionally not Clone: make a
/// new operation only when actually invoking the hook again. Reusing requests
/// for host I/O retries must not consume another grouped definition selection.
#[derive(Debug)]
pub struct Operation {
    selected: Option<SelectedDefinition>,
    selection_reference: Option<u32>,
    requests: Option<[Request; 2]>,
    returned: usize,
    skip: Option<SkipReason>,
    tail: Option<TailReset>,
}

impl Operation {
    pub fn prepare(
        predicate_skip: bool,
        selected_level: Option<&LevelRow>,
        definitions: &mut Registry,
        budget: LookupBudget,
    ) -> Result<Self> {
        if predicate_skip {
            return Ok(Self::skipped(SkipReason::Predicate));
        }
        let Some(level) = selected_level else {
            return Ok(Self::skipped(SkipReason::NoSelectedLevel));
        };

        // Source selected primary row+84, then the one shared mutable lookup.
        let reference = level.words_raw()[0x84 / 4];
        let (id, group, member_index) =
            match definitions.selection_with_budget(reference, budget)? {
                Selection::Selected {
                    record,
                    group,
                    member_index,
                } => (record, group, member_index),
                Selection::RequiresDefault { reason } => {
                    return Err(PreparationError::RequiresDefault { reason }.into());
                }
                Selection::RequiresSpecial { kind } => {
                    return Err(PreparationError::RequiresSpecial { kind }.into());
                }
            };
        let record = definitions.record(id)?;
        let words = record.words();
        let file = words[0x18 / 4];
        Ok(Self {
            selected: Some(SelectedDefinition {
                record: id,
                key: record.key(),
                source_index: record.source_index(),
                group,
                member_index,
            }),
            selection_reference: Some(reference),
            requests: Some([
                Request {
                    ordinal: 0,
                    file,
                    target: words[0x1c / 4],
                },
                Request {
                    ordinal: 1,
                    file,
                    target: words[0x28 / 4],
                },
            ]),
            returned: 0,
            skip: None,
            tail: None,
        })
    }

    fn skipped(reason: SkipReason) -> Self {
        Self {
            selected: None,
            selection_reference: None,
            requests: None,
            returned: 0,
            skip: Some(reason),
            tail: None,
        }
    }

    pub fn selected_definition(&self) -> Option<SelectedDefinition> {
        self.selected
    }

    pub fn selection_reference(&self) -> Option<u32> {
        self.selection_reference
    }

    /// True means the retained shared lookup was performed for this operation,
    /// not that original prior group-cursor history is known.
    pub fn selection_performed(&self) -> bool {
        self.selected.is_some()
    }

    pub fn skip_reason(&self) -> Option<SkipReason> {
        self.skip
    }

    pub fn requests(&self) -> &[Request] {
        self.requests
            .as_ref()
            .map_or(&[], |requests| requests.as_slice())
    }

    pub fn next_request(&self) -> Option<&Request> {
        self.requests().get(self.returned)
    }

    pub fn returned_count(&self) -> usize {
        self.returned
    }

    /// Called only after this source helper invocation actually returns. Its
    /// returned NULL/non-NULL value is ignored by the source caller, so it is
    /// deliberately not represented as readiness here. A host failure that did
    /// not return leaves this operation unchanged for a retry of the same plan.
    pub fn record_returned(&mut self, ordinal: u8) -> Result<()> {
        ensure!(
            self.tail.is_none(),
            "post-trigger operation already finished"
        );
        let next = self
            .next_request()
            .ok_or_else(|| anyhow::anyhow!("post-trigger operation has no pending request"))?;
        ensure!(
            next.ordinal == ordinal,
            "post-trigger request returned out of order"
        );
        self.returned += 1;
        Ok(())
    }

    /// Produces only the known sparse source writes after both ordered calls,
    /// including NULL returns. Predicate/no-row skips reach this tail directly.
    pub fn finish(&mut self) -> Result<TailReset> {
        ensure!(
            self.tail.is_none(),
            "post-trigger operation already finished"
        );
        ensure!(
            self.next_request().is_none(),
            "post-trigger requests have not both returned"
        );
        let tail = TailReset {
            words_00_0c: [0; 4],
            word_338: u32::MAX,
            scalar_340_bits: 0,
            byte_344: 0,
        };
        self.tail = Some(tail);
        Ok(tail)
    }

    pub fn tail_reset(&self) -> Option<TailReset> {
        self.tail
    }

    pub fn is_finished(&self) -> bool {
        self.tail.is_some()
    }
}
