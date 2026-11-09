//! Bounded host work ownership. A backend read is not a completed task: the
//! executed provider callback must return successfully before completion.
//! This synchronous adapter does not reproduce native queue timing/history.
use std::collections::VecDeque;

use anyhow::{Context, Result, ensure};

use super::phase::AttemptId;

/// Exact 802dec10 arithmetic, kept separate from checked host accounting.
pub const fn source_busy(queued: u32, active: u32, scheduler_state: u32) -> bool {
    queued
        .wrapping_add(active)
        .wrapping_add((scheduler_state != 0) as u32)
        != 0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    pub tickets: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ticket {
    attempt: AttemptId,
    index: usize,
    key: u32,
}
impl Ticket {
    pub const fn attempt(self) -> AttemptId {
        self.attempt
    }
    pub const fn id(self) -> usize {
        self.index + 1
    }
    /// Caller-owned semantic task key; it is not a native pointer or status.
    pub const fn key(self) -> u32 {
        self.key
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Task {
    pub ticket: Ticket,
    pub state: TaskState,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    pub queued: usize,
    pub running: usize,
    pub completed: usize,
    pub failed: usize,
    pub cancelled: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkPoll {
    Busy,
    Complete,
}

#[derive(Debug)]
pub struct WorkOwner {
    attempt: AttemptId,
    budget: Budget,
    tasks: Vec<Task>,
    queue: VecDeque<usize>,
    cancelled: bool,
}

impl WorkOwner {
    pub fn new(attempt: AttemptId, budget: Budget) -> Result<Self> {
        ensure!(
            budget.tickets <= u32::MAX as usize,
            "work ticket budget exceeds checked counter range"
        );
        Ok(Self {
            attempt,
            budget,
            tasks: Vec::new(),
            queue: VecDeque::new(),
            cancelled: false,
        })
    }

    fn ensure_current(&self, attempt: AttemptId) -> Result<()> {
        ensure!(self.attempt == attempt, "work attempt identity differs");
        ensure!(!self.cancelled, "work attempt was cancelled");
        Ok(())
    }

    pub fn enqueue(&mut self, attempt: AttemptId, key: u32) -> Result<Ticket> {
        self.ensure_current(attempt)?;
        ensure!(
            self.tasks.len() < self.budget.tickets,
            "work ticket budget exhausted"
        );
        self.tasks.try_reserve(1).context("retaining work ticket")?;
        self.queue.try_reserve(1).context("queuing work ticket")?;
        let ticket = Ticket {
            attempt,
            index: self.tasks.len(),
            key,
        };
        self.tasks.push(Task {
            ticket,
            state: TaskState::Queued,
        });
        self.queue.push_back(ticket.index);
        Ok(ticket)
    }

    /// Execute the oldest task's real provider work, including its completion
    /// callback. There is deliberately no public `complete(bool)` setter.
    /// Failures remain retained and block completion, but subsequent queued
    /// callbacks can still execute when their source caller requires them.
    pub fn execute_next(
        &mut self,
        attempt: AttemptId,
        execute: impl FnOnce(Ticket) -> Result<()>,
    ) -> Result<Option<Ticket>> {
        self.ensure_current(attempt)?;
        let Some(index) = self.queue.pop_front() else {
            return Ok(None);
        };
        let task = &mut self.tasks[index];
        ensure!(
            task.state == TaskState::Queued,
            "work FIFO entry is not queued"
        );
        task.state = TaskState::Running;
        let ticket = task.ticket;
        let result = execute(ticket);
        self.tasks[index].state = if result.is_ok() {
            TaskState::Completed
        } else {
            TaskState::Failed
        };
        result
            .with_context(|| format!("work provider callback failed for ticket {}", ticket.id()))?;
        Ok(Some(ticket))
    }

    pub fn poll(&self, attempt: AttemptId) -> Result<WorkPoll> {
        self.ensure_current(attempt)?;
        let stats = self.stats();
        ensure!(
            stats.failed == 0,
            "work owner retains failed provider callbacks"
        );
        ensure!(stats.cancelled == 0, "work owner retains cancelled tasks");
        // Counts are actual retained task states under a bounded budget. Native
        // wrapping arithmetic is not used to turn overflow into host readiness.
        Ok(if stats.queued != 0 || stats.running != 0 {
            WorkPoll::Busy
        } else {
            WorkPoll::Complete
        })
    }

    pub fn cancel(&mut self, attempt: AttemptId) -> Result<()> {
        self.ensure_current(attempt)?;
        self.cancelled = true;
        self.queue.clear();
        for task in &mut self.tasks {
            if matches!(task.state, TaskState::Queued | TaskState::Running) {
                task.state = TaskState::Cancelled;
            }
        }
        Ok(())
    }

    pub fn stats(&self) -> Stats {
        let mut stats = Stats::default();
        for task in &self.tasks {
            match task.state {
                TaskState::Queued => stats.queued += 1,
                TaskState::Running => stats.running += 1,
                TaskState::Completed => stats.completed += 1,
                TaskState::Failed => stats.failed += 1,
                TaskState::Cancelled => stats.cancelled += 1,
            }
        }
        stats
    }
    pub fn tickets(&self) -> &[Task] {
        &self.tasks
    }
    pub const fn attempt(&self) -> AttemptId {
        self.attempt
    }
}

#[cfg(test)]
#[path = "tests/work.rs"]
mod tests;
