//! Persistent manager ownership shared by the native controller and its CPU
//! worker. Cancellation invalidates an attempt without recreating manager state.
mod animation;
mod clock;
mod evidence;
pub(super) mod loader;
mod player;

use anyhow::{Result, ensure};
use ge_core::mission::world_hooks::{clock_reset, final_reset};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

pub use evidence::Evidence;

struct State {
    clock: clock::Clock,
    reset: clock_reset::Owner,
    final_owners: final_reset::Owners,
    constructor_clock_reads: usize,
    loader: loader::Persistent,
    player: player::Persistent,
    animation_context: ge_core::animation::scratch::Context,
    animation_scratch: ge_core::animation::scratch::Arena,
    animation_callbacks: Arc<ge_core::mission::player::components::callbacks::Table>,
    animation_fallback: ge_core::animation::dispatch::Fallback,
}

struct SharedState {
    state: Mutex<State>,
    actors: Mutex<player::Actors>,
    current: AtomicU64,
}

#[derive(Clone)]
pub(super) struct Shared(Arc<SharedState>);

impl Shared {
    pub fn from_constructor() -> Self {
        let mut clock = clock::Clock::from_constructor();
        // The concrete boot clock is owned and infallible, initially positive
        // zero. This invokes the real constructor's sixteen reads exactly once.
        let reset = clock_reset::Owner::from_constructor(&mut clock)
            .expect("owned boot frame-clock constructor is infallible");
        let constructor_clock_reads = reset.total_reads();
        Self(Arc::new(SharedState {
            actors: Mutex::new(player::Actors::from_constructor()),
            state: Mutex::new(State {
                clock,
                reset,
                final_owners: final_reset::Owners::fresh(),
                constructor_clock_reads,
                loader: loader::Persistent::new(),
                player: player::Persistent::from_constructor(),
                animation_context: ge_core::animation::scratch::Context::default(),
                animation_scratch: ge_core::animation::scratch::Arena::from_constructor(1)
                    .expect("nonzero shared animation scratch identity"),
                animation_callbacks: Arc::new(
                    ge_core::mission::player::components::callbacks::Table::from_constructor(1)
                        .expect("nonzero startup animation callback table identity"),
                ),
                animation_fallback: ge_core::animation::dispatch::Fallback::default(),
            }),
            current: AtomicU64::new(0),
        }))
    }

    pub fn advance(&self, elapsed: Duration) -> Result<()> {
        self.0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("world-hook owner lock is poisoned"))?
            .clock
            .advance(elapsed)
    }

    pub fn begin(&self, identity: u64) -> Result<Attempt> {
        ensure!(identity != 0, "world-hook attempt identity is zero");
        self.0.current.store(identity, Ordering::SeqCst);
        Ok(Attempt {
            shared: self.clone(),
            identity,
        })
    }

    pub fn invalidate(&self) {
        self.0.current.store(0, Ordering::SeqCst);
    }
}

pub(super) struct Attempt {
    shared: Shared,
    identity: u64,
}

impl Attempt {
    pub fn ensure_current(&self) -> Result<()> {
        ensure!(
            self.shared.0.current.load(Ordering::SeqCst) == self.identity,
            "world-hook attempt was cancelled or superseded"
        );
        Ok(())
    }

    pub fn run(&self) -> Result<Evidence> {
        self.ensure_current()?;
        let mut state = self
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("world-hook owner lock is poisoned"))?;
        self.ensure_current()?;
        let mut clock = clock::SnapshotClock(state.clock.bits());
        state.reset.reset(&mut clock)?;
        state.final_owners.begin_operation();
        // Source order after the clock reset. The optional multiplayer resource
        // branch is skipped by this admitted mode3 / one-local-user path.
        state.final_owners.reach_noop(final_reset::Noop::First);
        state.final_owners.reach_noop(final_reset::Noop::Second);
        let leaf = state.final_owners.reset_leaf();
        let buffers = state.final_owners.reset_buffers()?;
        let two = state.final_owners.reset_two_buffers();
        state.final_owners.install_callback();
        let evidence = evidence::build(&state, &leaf, &buffers, &two)?;
        // A started hook is serialized with the next one. Cancellation never
        // publishes its result; an older worker cannot overwrite a newer hook.
        self.ensure_current()?;
        Ok(evidence)
    }
}

#[cfg(test)]
mod tests;
