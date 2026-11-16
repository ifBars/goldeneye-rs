//! Nonempty shared evaluator. It owns one retained scope over the actual arena:
//! reserve, region/context stores, the slot loop, optional commit and release.
//! The source is synchronous; Pending is the host's honest pause at a reached
//! provider. Resumption continues at the retained cursor and never repeats a
//! completed store, lookup or node invocation. Cancellation is explicit and may
//! come from a cloned lease after the owning Pending has been abandoned.
mod lease;
mod trace;
use super::{Arena, Context, Refresh, refresh::REFRESH_BYTES, wide};
use crate::animation::layers::NodeId;
use anyhow::{Context as _, Result, bail, ensure};
pub use lease::CancellationLease;
pub use trace::{Boundary, Branch, Progress, Slot, Stage};

/// Source single-precision epsilon; slots skip only on ordered less-than.
pub const WEIGHT_EPSILON_BITS: u32 = 0x3a83_126f;
pub const CURRENT_LAYER: u32 = 0x8000;
pub const COMMIT: u32 = 0x4000;

/// A read of an actual owner field; Unknown is never a default value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Read<T> {
    Known(T),
    Unknown,
}
/// A reached provider either returned or must pause at this exact point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step<T> {
    Done(T),
    Pending,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tail {
    Null,
    Node(NodeId),
}
/// Actual animator owner bridge. Implementations must read real retained
/// fields and must not substitute live receivers or defaults.
pub trait Layers {
    /// Animator+114 skeletal instance.
    fn skeleton_present(&mut self) -> Result<bool>;
    /// Current active-slot byte, re-read at every loop test.
    fn active_count(&mut self) -> Result<Read<u8>>;
    fn weight_bits(&mut self, slot: u8) -> Result<Read<u32>>;
    fn tail(&mut self, slot: u8) -> Result<Read<Tail>>;
    /// Skeleton sample preparation with the caller's copyDefaults 0.
    fn prepare(&mut self, slot: u8) -> Result<Step<()>>;
    /// Node virtual +2c; returns the raw byte the caller sign-extends.
    fn invoke(&mut self, node: NodeId, slot: u8, successes: u32) -> Result<Step<u8>>;
    fn previous(&mut self, node: NodeId) -> Result<Read<Option<NodeId>>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cursor {
    Count(u8),
    Weight(u8),
    Tail(u8),
    Prepare(u8, NodeId),
    Node(u8, NodeId, u32),
    Previous(u8, NodeId, u32),
    Layer(u8),
    Commit,
    Done,
}
#[derive(Debug)]
struct Scope {
    lease: CancellationLease,
    start: usize,
}
#[derive(Debug)]
pub struct Evaluation {
    owner: u64,
    scope: Option<Scope>,
    stage: Stage,
    cursor: Cursor,
    node_budget: usize,
    node_visits: usize,
    slots: Vec<Slot>,
    nonempty_layers: u32,
    advanced_context: bool,
    commit_register: u32,
    released: bool,
    cursor_after: Option<usize>,
    live_after: Option<usize>,
}
impl Evaluation {
    /// NULL skeleton returns without a reservation. Otherwise the actual shared
    /// arena reserves 0x2200 bytes; exhaustion fails before any mutation.
    pub fn begin(
        context: &mut Context,
        arena: &mut Arena,
        layers: &mut impl Layers,
        node_budget: usize,
    ) -> Result<Self> {
        let ordinal = context.next_ordinal()?;
        let mut evaluation = Self {
            owner: arena.owner,
            scope: None,
            stage: Stage::Running,
            cursor: Cursor::Done,
            node_budget,
            node_visits: 0,
            slots: Vec::new(),
            nonempty_layers: 0,
            advanced_context: false,
            commit_register: 0,
            released: false,
            cursor_after: None,
            live_after: None,
        };
        if !layers.skeleton_present()? {
            return Ok(evaluation);
        }
        let mut reservation = arena
            .reserve(REFRESH_BYTES)?
            .context("shared animation scratch exhausted")?;
        let start = reservation
            .start
            .expect("successful reservation has a start");
        if let Err(error) = context.store_regions(arena, &reservation) {
            arena.release(&mut reservation)?;
            return Err(error);
        }
        context.refreshes = ordinal;
        context.holder = Some(ordinal);
        // The lease exists before begin returns, so a host can register it
        // while still holding its shared-state lock.
        evaluation.scope = Some(Scope {
            lease: CancellationLease::new(arena, context, ordinal, reservation),
            start,
        });
        evaluation.cursor = Cursor::Count(0);
        Ok(evaluation)
    }
    pub fn advance(
        &mut self,
        context: &mut Context,
        arena: &mut Arena,
        layers: &mut impl Layers,
    ) -> Result<Progress> {
        if self.lease_aborted()? {
            // A reaped stale attempt never resumes and never releases again.
            self.stage = Stage::Aborted;
            bail!("animation evaluation scope was cancelled");
        }
        ensure!(
            matches!(self.stage, Stage::Running | Stage::Pending(_)),
            "animation evaluation is not resumable"
        );
        ensure!(
            arena.owner == self.owner
                && (self.released
                    || self
                        .scope
                        .as_ref()
                        .is_none_or(|scope| { context.holder == Some(scope.lease.ordinal()) })),
            "animation evaluation resumed with a different shared owner"
        );
        self.stage = Stage::Running;
        let result = self.run(context, arena, layers);
        self.stage = match &result {
            Ok(Progress::Returned) => Stage::Returned,
            Ok(Progress::Pending(boundary)) => Stage::Pending(*boundary),
            Err(_) => Stage::Failed,
        };
        result
    }
    /// Explicit host cancellation of a retained pending/failed scope. It is a
    /// terminal release, never a replay and never an evaluator return.
    /// Equivalent to aborting the cloned lease; an already reaped scope is
    /// observed without a second release.
    pub fn abort(&mut self, context: &mut Context, arena: &mut Arena) -> Result<()> {
        if !self.lease_aborted()? {
            ensure!(
                matches!(self.stage, Stage::Pending(_) | Stage::Failed),
                "only a retained pending or failed evaluation can be aborted"
            );
            if let Some(scope) = &self.scope {
                scope.lease.abort(context, arena)?;
                self.cursor_after = Some(arena.cursor());
                self.live_after = Some(arena.live_reservations());
            }
        }
        self.stage = Stage::Aborted;
        Ok(())
    }
    fn lease_aborted(&self) -> Result<bool> {
        match &self.scope {
            Some(scope) => scope.lease.is_aborted(),
            None => Ok(false),
        }
    }
    fn run(
        &mut self,
        context: &mut Context,
        arena: &mut Arena,
        layers: &mut impl Layers,
    ) -> Result<Progress> {
        loop {
            match self.cursor {
                Cursor::Count(slot) => {
                    let Read::Known(count) = layers.active_count()? else {
                        return Ok(Progress::Pending(Boundary::ActiveCount));
                    };
                    if slot >= count {
                        self.cursor = Cursor::Commit;
                        continue;
                    }
                    self.slots.try_reserve(1)?;
                    self.slots.push(Slot::new(slot));
                    self.cursor = Cursor::Weight(slot);
                }
                Cursor::Weight(slot) => {
                    let Read::Known(bits) = layers.weight_bits(slot)? else {
                        return Ok(Progress::Pending(Boundary::Weight));
                    };
                    self.current()?.weight_bits = Some(bits);
                    // Ordered compare: NaN is not less and therefore qualifies.
                    if f32::from_bits(bits) < f32::from_bits(WEIGHT_EPSILON_BITS) {
                        self.current()?.branch = Some(Branch::LowWeight);
                        self.cursor = Cursor::Count(next(slot)?);
                    } else {
                        self.cursor = Cursor::Tail(slot);
                    }
                }
                Cursor::Tail(slot) => match layers.tail(slot)? {
                    Read::Unknown => return Ok(Progress::Pending(Boundary::Tail)),
                    Read::Known(Tail::Null) => {
                        self.current()?.branch = Some(Branch::EmptyTail);
                        self.clear_current_layer(context)?;
                        self.cursor = Cursor::Count(next(slot)?);
                    }
                    Read::Known(Tail::Node(node)) => {
                        self.current()?.branch = Some(Branch::Nonempty(node));
                        self.cursor = Cursor::Prepare(slot, node);
                    }
                },
                Cursor::Prepare(slot, node) => {
                    if layers.prepare(slot)? == Step::Pending {
                        return Ok(Progress::Pending(Boundary::Prepare));
                    }
                    self.current()?.prepared = true;
                    context.clear_masks();
                    self.current()?.masks_cleared = true;
                    self.cursor = Cursor::Node(slot, node, 0);
                }
                Cursor::Node(slot, node, successes) => {
                    ensure!(
                        self.node_visits < self.node_budget,
                        "animation node walk budget exhausted"
                    );
                    let Step::Done(byte) = layers.invoke(node, slot, successes)? else {
                        return Ok(Progress::Pending(Boundary::Node));
                    };
                    self.node_visits += 1;
                    let successes = successes + u32::from(byte as i8 != 0);
                    let current = self.current()?;
                    current.nodes.try_reserve(1)?;
                    current.nodes.push((node, byte));
                    current.successes = successes;
                    self.cursor = Cursor::Previous(slot, node, successes);
                }
                Cursor::Previous(slot, node, successes) => match layers.previous(node)? {
                    Read::Unknown => return Ok(Progress::Pending(Boundary::Previous)),
                    Read::Known(Some(previous)) => {
                        self.cursor = Cursor::Node(slot, previous, successes)
                    }
                    Read::Known(None) => self.cursor = Cursor::Layer(slot),
                },
                Cursor::Layer(slot) => {
                    let flags = context.flags.context("shared animation flags unknown")?;
                    if self.nonempty_layers != 0 && flags & CURRENT_LAYER != 0 {
                        return Ok(Progress::Pending(Boundary::LayerBlend));
                    }
                    self.nonempty_layers = self
                        .nonempty_layers
                        .checked_add(1)
                        .context("nonempty layer count overflow")?;
                    self.clear_current_layer(context)?;
                    self.cursor = Cursor::Count(next(slot)?);
                }
                Cursor::Commit => {
                    // The source tests its register copy of the last stored
                    // flags (0 when no clear was reached), not a fresh read.
                    if self.commit_register & COMMIT != 0 {
                        return Ok(Progress::Pending(Boundary::Commit));
                    }
                    self.release(context, arena)?;
                    self.cursor = Cursor::Done;
                }
                Cursor::Done => return Ok(Progress::Returned),
            }
        }
    }
    fn current(&mut self) -> Result<&mut Slot> {
        self.slots
            .last_mut()
            .context("evaluation slot trace absent")
    }
    fn clear_current_layer(&mut self, context: &mut Context) -> Result<()> {
        let flags = context.flags.context("shared animation flags unknown")? & !CURRENT_LAYER;
        context.flags = Some(flags);
        self.commit_register = flags;
        self.current()?.flags_after = Some(flags);
        if self.nonempty_layers != 0 {
            self.advanced_context = true;
        }
        Ok(())
    }
    /// Normal return: the evaluator consumes its own reservation.
    fn release(&mut self, context: &mut Context, arena: &mut Arena) -> Result<()> {
        if let Some(scope) = &self.scope {
            scope.lease.consume(context, arena)?;
        }
        self.released = true;
        self.cursor_after = Some(arena.cursor());
        self.live_after = Some(arena.live_reservations());
        Ok(())
    }
    /// Reports Aborted once the cloned lease was reaped, even before this
    /// owner observes it. A poisoned lease reports the retained stage.
    pub fn stage(&self) -> Stage {
        if self.stage != Stage::Returned && self.lease_aborted().unwrap_or(false) {
            Stage::Aborted
        } else {
            self.stage
        }
    }
    /// Cloneable cancellation handle; None when no scope was reserved.
    pub fn lease(&self) -> Option<CancellationLease> {
        self.scope.as_ref().map(|scope| scope.lease.clone())
    }
    pub fn reserved(&self) -> bool {
        self.scope.is_some()
    }
    /// Normal-return release only. Cancellation is reported by `stage`.
    pub fn released(&self) -> bool {
        self.released
    }
    pub fn ordinal(&self) -> Option<u64> {
        self.scope.as_ref().map(|scope| scope.lease.ordinal())
    }
    pub fn allocation_ordinal(&self) -> Option<u64> {
        self.scope
            .as_ref()
            .map(|scope| scope.lease.allocation_ordinal())
    }
    pub fn reservation_start(&self) -> Option<usize> {
        self.scope.as_ref().map(|scope| scope.start)
    }
    pub fn slots(&self) -> &[Slot] {
        &self.slots
    }
    pub fn node_visits(&self) -> usize {
        self.node_visits
    }
    pub fn nonempty_layers(&self) -> u32 {
        self.nonempty_layers
    }
    pub fn successful_samples(&self) -> u64 {
        self.slots
            .iter()
            .map(|slot| u64::from(slot.successes))
            .sum()
    }
    pub fn advanced_context(&self) -> bool {
        self.advanced_context
    }
    pub fn commit_register(&self) -> u32 {
        self.commit_register
    }
    /// No blend or skeletal commit is implemented; reaching either pauses.
    pub fn blended_layers(&self) -> u32 {
        0
    }
    pub fn committed(&self) -> bool {
        false
    }
    pub fn cursor_after(&self) -> Option<usize> {
        self.cursor_after
    }
    pub fn live_after(&self) -> Option<usize> {
        self.live_after
    }
    /// Both evaluator invocations come from the same retained arena, in order.
    pub fn follows_refresh(&self, previous: &Refresh) -> bool {
        self.owner == previous.owner
            && self.scope.as_ref().is_some_and(|scope| {
                scope.lease.ordinal() > previous.ordinal
                    && scope.lease.allocation_ordinal() > previous.allocation
            })
    }
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        wide(&mut emit, self.owner);
        trace::stage_words(self.stage(), &mut emit);
        emit(u32::from(self.scope.is_some()));
        if let Some(scope) = &self.scope {
            wide(&mut emit, scope.lease.ordinal());
            wide(&mut emit, scope.lease.allocation_ordinal());
            emit(REFRESH_BYTES as u32);
            emit(scope.start as u32);
        }
        wide(&mut emit, self.node_budget as u64);
        wide(&mut emit, self.node_visits as u64);
        emit(self.slots.len() as u32);
        for slot in &self.slots {
            slot.visit_state_words(&mut emit);
        }
        emit(self.nonempty_layers);
        emit(u32::from(self.advanced_context));
        emit(self.commit_register);
        emit(u32::from(self.released));
        for value in [self.cursor_after, self.live_after] {
            emit(u32::from(value.is_some()));
            if let Some(value) = value {
                emit(value as u32);
            }
        }
    }
}
fn next(slot: u8) -> Result<u8> {
    slot.checked_add(1).context("animation slot index overflow")
}

#[cfg(test)]
#[path = "evaluation/tests.rs"]
mod tests;
