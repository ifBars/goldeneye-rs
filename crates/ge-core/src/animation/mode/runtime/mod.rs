//! Resumable live selection. A matched command is acknowledged only after its
//! real effects return; fallback cannot run ahead of an unfinished command.
mod state;
use super::{AttemptKind, Budget, Inputs, Mode, Work};
use anyhow::{Context, Result, ensure};

pub trait Provider {
    fn resolve_mode(&mut self, reference: u32) -> Result<Option<usize>>;
    fn mode(&mut self, index: usize) -> Result<Mode>;
    fn transition_target(&mut self, index: usize, row: usize) -> Result<u32>;
    fn command_count(&mut self, index: usize, row: usize) -> Result<usize>;
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Invocation {
    pub attempt: usize,
    pub mode: usize,
    pub transition: usize,
    pub ordinal: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Resolving,
    CommandPending,
    Complete,
    Failed,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Target,
    Attempt,
    Scan,
    Commands,
    Pending,
    Complete,
    Failed,
}
#[derive(Debug)]
pub struct Attempt {
    pub kind: AttemptKind,
    pub source_index: Option<usize>,
    pub source_hash: Option<u32>,
    pub matched_transition: Option<usize>,
    pub command_count: Option<usize>,
    pub completed: usize,
}
#[derive(Debug)]
pub struct Cursor {
    inputs: Inputs,
    budget: Budget,
    work: Work,
    phase: Phase,
    target: Option<usize>,
    attempts: Vec<Attempt>,
    scan: usize,
    transitions: usize,
    pending: Option<Invocation>,
    returned: Option<u32>,
}
impl Cursor {
    pub fn new(inputs: Inputs, budget: Budget) -> Self {
        Self {
            inputs,
            budget,
            work: Work::default(),
            phase: if inputs.resource_available {
                Phase::Target
            } else {
                Phase::Complete
            },
            target: None,
            attempts: Vec::new(),
            scan: 0,
            transitions: 0,
            pending: None,
            returned: if inputs.resource_available {
                None
            } else {
                Some(0)
            },
        }
    }
    pub fn inputs(&self) -> Inputs {
        self.inputs
    }
    pub fn stage(&self) -> Stage {
        match self.phase {
            Phase::Pending => Stage::CommandPending,
            Phase::Complete => Stage::Complete,
            Phase::Failed => Stage::Failed,
            _ => Stage::Resolving,
        }
    }
    pub fn target(&self) -> Option<usize> {
        self.target
    }
    pub fn attempts(&self) -> &[Attempt] {
        &self.attempts
    }
    pub fn work(&self) -> Work {
        self.work
    }
    pub fn pending(&self) -> Option<Invocation> {
        self.pending
    }
    pub fn return_word(&self) -> Option<u32> {
        self.returned
    }
    pub fn completed_commands(&self) -> usize {
        self.attempts.iter().map(|attempt| attempt.completed).sum()
    }
    pub fn next_command(&mut self, source: &mut impl Provider) -> Result<Option<Invocation>> {
        ensure!(
            !matches!(self.phase, Phase::Pending | Phase::Failed),
            "live selector has an unfinished or failed command"
        );
        let result = self.advance(source);
        if result.is_err() {
            self.phase = Phase::Failed;
        }
        result
    }
    fn advance(&mut self, source: &mut impl Provider) -> Result<Option<Invocation>> {
        loop {
            match self.phase {
                Phase::Target => {
                    super::plan::charge(&mut self.work.lookups, 1, self.budget.lookups, "lookup")?;
                    self.target = source.resolve_mode(self.inputs.requested)?;
                    if let Some(target) = self.target {
                        let _ = u32::try_from(target)?;
                    } else {
                        self.finish(0);
                        continue;
                    }
                    self.phase = Phase::Attempt;
                }
                Phase::Attempt => {
                    let kind = if self.attempts.is_empty() {
                        AttemptKind::Previous
                    } else {
                        AttemptKind::Fallback
                    };
                    let reference = if kind == AttemptKind::Previous {
                        self.inputs.previous
                    } else {
                        self.inputs.fallback
                    };
                    self.attempts.push(Attempt {
                        kind,
                        source_index: None,
                        source_hash: None,
                        matched_transition: None,
                        command_count: None,
                        completed: 0,
                    });
                    super::plan::charge(&mut self.work.lookups, 1, self.budget.lookups, "lookup")?;
                    let index = source.resolve_mode(reference)?;
                    self.attempts
                        .last_mut()
                        .expect("retained attempt")
                        .source_index = index;
                    let Some(index) = index else {
                        self.finish_attempt(0);
                        continue;
                    };
                    let mode = source.mode(index)?;
                    self.attempts
                        .last_mut()
                        .expect("retained attempt")
                        .source_hash = Some(mode.hash);
                    self.scan = 0;
                    self.transitions = mode.transition_count;
                    self.phase = Phase::Scan;
                }
                Phase::Scan => {
                    if self.scan == self.transitions {
                        self.finish_attempt(0);
                        continue;
                    }
                    super::plan::charge(
                        &mut self.work.transition_rows,
                        1,
                        self.budget.transition_rows,
                        "transition row",
                    )?;
                    let index = self
                        .attempts
                        .last()
                        .expect("attempt")
                        .source_index
                        .context("live source mode absent")?;
                    let target = u32::try_from(self.target.context("live target mode absent")?)?;
                    let row = self.scan;
                    self.scan += 1;
                    if source.transition_target(index, row)? != target {
                        continue;
                    }
                    self.attempts
                        .last_mut()
                        .expect("attempt")
                        .matched_transition = Some(row);
                    let count = source.command_count(index, row)?;
                    self.attempts.last_mut().expect("attempt").command_count = Some(count);
                    ensure!(
                        count <= self.budget.commands.saturating_sub(self.work.commands),
                        "live mode command budget exhausted"
                    );
                    self.phase = Phase::Commands;
                }
                Phase::Commands => {
                    let attempt = self.attempts.last().expect("attempt");
                    if attempt.completed
                        == attempt.command_count.context("live command count absent")?
                    {
                        let hash = attempt.source_hash.context("live source hash absent")?;
                        self.finish_attempt(hash);
                        continue;
                    }
                    super::plan::charge(
                        &mut self.work.commands,
                        1,
                        self.budget.commands,
                        "command",
                    )?;
                    let attempt = self.attempts.last().expect("attempt");
                    let invocation = Invocation {
                        attempt: self.attempts.len() - 1,
                        mode: attempt.source_index.context("live source absent")?,
                        transition: attempt
                            .matched_transition
                            .context("live transition absent")?,
                        ordinal: attempt.completed,
                    };
                    self.pending = Some(invocation);
                    self.phase = Phase::Pending;
                    return Ok(Some(invocation));
                }
                Phase::Complete => return Ok(None),
                Phase::Pending | Phase::Failed => anyhow::bail!("invalid live selector advance"),
            }
        }
    }
    pub fn executed(&mut self, invocation: Invocation) -> Result<()> {
        ensure!(
            self.phase == Phase::Pending && self.pending == Some(invocation),
            "live command acknowledgement differs from pending invocation"
        );
        self.attempts.last_mut().expect("pending attempt").completed += 1;
        self.pending = None;
        self.phase = Phase::Commands;
        Ok(())
    }
    pub fn failed(&mut self, invocation: Invocation) -> Result<()> {
        ensure!(
            self.phase == Phase::Pending && self.pending == Some(invocation),
            "failed live command differs from pending invocation"
        );
        self.phase = Phase::Failed;
        Ok(())
    }
    fn finish_attempt(&mut self, value: u32) {
        if value != 0 || self.attempts.len() == 2 {
            self.finish(value);
        } else {
            self.phase = Phase::Attempt;
        }
    }
    fn finish(&mut self, value: u32) {
        self.returned = Some(value);
        self.phase = Phase::Complete;
    }
}

#[cfg(test)]
mod tests;
