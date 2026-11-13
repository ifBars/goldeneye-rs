//! Drives one host and several clients over the simulated network.
//!
//! Network pumping and simulation ticks advance separately: the harness
//! pumps the sessions several times per tick, and a tick never depends on
//! how often the network was pumped.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ge_net::session::host::HostEvent;
use ge_net::transport::{pump_client, pump_host};
use ge_net::{
    AttemptId, ClientConfig, ClientEvent, ClientSession, CloseReason, CompatibilityDescriptor,
    DrainLimit, HostConfig, HostSession, NetInstant, PeerId, Phase, PlayerSlot, SessionBudget,
    Snapshot, SnapshotId, Tick, TickRate,
};

use super::link::{Endpoint, LinkProfile, SimNetwork};
use super::rng::SeededRng;
use super::simulation::{Command, World, digest};

/// An authored descriptor. Every value is synthetic.
pub fn descriptor() -> CompatibilityDescriptor {
    CompatibilityDescriptor {
        app_protocol: 1,
        simulation_schema: 1,
        build: [0x11; 32],
        edition: *b"SYNTH\0\0\0",
        revision: 0,
        content: [0x22; 32],
        ruleset: [0x33; 32],
        tick_rate: TickRate::per_second(60),
        supported_features: 0b0111,
        required_features: 0b0001,
        extensions: vec![7, 40],
    }
}

#[derive(Clone, Debug)]
pub struct HarnessConfig {
    pub seed: u64,
    pub clients: usize,
    pub profile: LinkProfile,
    pub budget: SessionBudget,
    /// A 60 Hz test profile; not a claim about any game's tick rate.
    pub tick_micros: u64,
    pub pumps_per_tick: u64,
    /// Unowned actors that change every tick.
    pub drifters: usize,
    pub slots_per_client: u8,
    pub sequence_origin: u32,
    /// How far past its newest snapshot a client schedules a command.
    pub command_lead_ticks: u64,
    /// Whether active clients submit a command per slot per tick.
    pub auto_commands: bool,
    pub drain_limit: DrainLimit,
    pub world_capacity: u16,
    /// Whether the synthetic application reports snapshots as applied.
    pub acknowledge: bool,
}

impl HarnessConfig {
    pub fn new(seed: u64, clients: usize) -> Self {
        Self {
            seed,
            clients,
            profile: LinkProfile::ideal(),
            budget: SessionBudget::small_session(),
            tick_micros: 16_666,
            pumps_per_tick: 2,
            drifters: 4,
            slots_per_client: 1,
            sequence_origin: 0,
            command_lead_ticks: 3,
            auto_commands: true,
            drain_limit: DrainLimit::UNLIMITED,
            world_capacity: 256,
            acknowledge: true,
        }
    }
}

#[derive(Debug)]
pub struct ClientNode {
    pub session: ClientSession,
    rng: SeededRng,
    /// A paused client stops consuming its transport and its events.
    pub paused: bool,
    pub accepted: Option<(PeerId, Vec<PlayerSlot>)>,
    /// The newest state the synthetic application applied.
    pub applied: Option<(SnapshotId, Tick, Arc<Snapshot>)>,
    pub activated: Vec<Tick>,
    pub closed: Vec<CloseReason>,
    pub submitted: BTreeSet<(u8, u32)>,
    pub submit_refused: u64,
    pub snapshots_seen: u64,
    /// Snapshots whose content differed from what the host published.
    pub digest_mismatches: u64,
    /// Snapshots whose tick did not exceed the previously applied one.
    pub tick_regressions: u64,
    last_command_tick: BTreeMap<PlayerSlot, u64>,
}

pub struct Harness {
    pub config: HarnessConfig,
    pub net: SimNetwork,
    pub host: HostSession,
    pub world: World,
    pub clients: Vec<ClientNode>,
    pub now: u64,
    pub tick: u64,
    pumps: u64,
    pub host_digests: BTreeMap<u64, u64>,
    pub host_events: Vec<HostEvent>,
    pub owners: BTreeMap<AttemptId, Vec<PlayerSlot>>,
    pub delivered_commands: u64,
    /// Per tick: the slot order commands were released in.
    pub release_order_violations: u64,
}

impl Harness {
    pub fn new(config: HarnessConfig) -> Self {
        let net = SimNetwork::new(config.seed, config.clients);
        let mut entropy = SeededRng::new(config.seed ^ 0x5eed);
        let host = HostSession::new(
            HostConfig {
                budget: config.budget.clone(),
                descriptor: descriptor(),
                sequence_origin: config.sequence_origin,
            },
            net.capabilities,
            &mut entropy,
            NetInstant::ZERO,
        )
        .expect("valid host configuration");
        let clients = (0..config.clients)
            .map(|index| ClientNode {
                session: ClientSession::new(
                    ClientConfig {
                        budget: config.budget.clone(),
                        descriptor: descriptor(),
                        requested_slots: config.slots_per_client,
                        sequence_origin: config.sequence_origin,
                    },
                    net.capabilities,
                    NetInstant::ZERO,
                )
                .expect("valid client configuration"),
                rng: SeededRng::new(config.seed.wrapping_mul(31).wrapping_add(index as u64)),
                paused: false,
                accepted: None,
                applied: None,
                activated: Vec::new(),
                closed: Vec::new(),
                submitted: BTreeSet::new(),
                submit_refused: 0,
                snapshots_seen: 0,
                digest_mismatches: 0,
                tick_regressions: 0,
                last_command_tick: BTreeMap::new(),
            })
            .collect();
        Self {
            world: World::new(config.world_capacity, config.drifters),
            config,
            net,
            host,
            clients,
            now: 0,
            tick: 0,
            pumps: 0,
            host_digests: BTreeMap::new(),
            host_events: Vec::new(),
            owners: BTreeMap::new(),
            delivered_commands: 0,
            release_order_violations: 0,
        }
    }

    /// Begin a new attempt for `client` over the default profile.
    pub fn connect(&mut self, client: usize) -> AttemptId {
        let profile = self.config.profile.clone();
        self.connect_with(client, profile)
    }

    pub fn connect_with(&mut self, client: usize, profile: LinkProfile) -> AttemptId {
        let attempt = self.net.connect(client, profile);
        let node = &mut self.clients[client];
        node.accepted = None;
        node.applied = None;
        node.last_command_tick.clear();
        node.session.begin(attempt).expect("client can begin");
        attempt
    }

    pub fn connect_all(&mut self) {
        for client in 0..self.clients.len() {
            self.connect(client);
        }
    }

    /// One network pump. Every `pumps_per_tick` pumps the simulation ticks.
    pub fn step(&mut self) {
        self.pumps += 1;
        self.now += self.config.tick_micros / self.config.pumps_per_tick;
        self.net.set_now(self.now);
        let now = NetInstant::from_micros(self.now);
        let limit = self.config.drain_limit;
        let ticking = self.pumps.is_multiple_of(self.config.pumps_per_tick);

        self.host.advance(now).expect("monotonic time");
        pump_host(
            &mut self.host,
            &mut self.net.endpoint(Endpoint::Host),
            limit,
        );
        self.drain_host_events();
        if ticking {
            self.tick += 1;
            self.simulate_tick();
            pump_host(
                &mut self.host,
                &mut self.net.endpoint(Endpoint::Host),
                limit,
            );
            self.drain_host_events();
        }

        for index in 0..self.clients.len() {
            if self.clients[index].paused {
                continue;
            }
            let node = &mut self.clients[index];
            node.session.advance(now).expect("monotonic time");
            pump_client(
                &mut node.session,
                &mut self.net.endpoint(Endpoint::Client(index)),
                limit,
            );
            self.drain_client_events(index);
            if ticking && self.config.auto_commands {
                self.submit_commands(index);
            }
        }
    }

    fn simulate_tick(&mut self) {
        let tick = Tick::new(self.tick);
        let commands = self.host.take_commands(tick).expect("ticks increase");
        if !commands.is_sorted_by_key(|command| command.slot) {
            self.release_order_violations += 1;
        }
        self.delivered_commands += commands.len() as u64;
        for command in &commands {
            self.world.apply(command);
        }
        self.world.step();
        let snapshot = self.world.snapshot();
        self.host_digests.insert(self.tick, digest(&snapshot));
        while self.host_digests.len() > 8_192 {
            self.host_digests.pop_first();
        }
        self.host
            .publish_snapshot(tick, snapshot)
            .expect("snapshot within budget");
    }

    fn drain_host_events(&mut self) {
        while let Some(event) = self.host.poll_event() {
            match &event {
                HostEvent::PeerAccepted { attempt, slots, .. } => {
                    self.owners.insert(*attempt, slots.clone());
                }
                HostEvent::PeerActive { attempt, .. } => {
                    for slot in self.owners.get(attempt).into_iter().flatten() {
                        self.world.join(*slot);
                    }
                }
                HostEvent::PeerClosed { attempt, .. } => {
                    for slot in self.owners.remove(attempt).unwrap_or_default() {
                        self.world.leave(slot);
                    }
                }
            }
            self.host_events.push(event);
        }
    }

    fn drain_client_events(&mut self, index: usize) {
        let acknowledge = self.config.acknowledge;
        let node = &mut self.clients[index];
        while let Some(event) = node.session.poll_event() {
            match event {
                ClientEvent::Accepted { peer, slots, .. } => node.accepted = Some((peer, slots)),
                ClientEvent::SyncSnapshot { id, tick, state }
                | ClientEvent::Snapshot {
                    id, tick, state, ..
                } => {
                    node.snapshots_seen += 1;
                    if self.host_digests.get(&tick.get()) != Some(&digest(&state)) {
                        node.digest_mismatches += 1;
                    }
                    if node
                        .applied
                        .as_ref()
                        .is_some_and(|(_, applied, _)| tick <= *applied)
                    {
                        node.tick_regressions += 1;
                    }
                    // "Applying" is storing the state; then say so.
                    node.applied = Some((id, tick, state));
                    if acknowledge && node.session.phase() == Phase::Synchronizing {
                        node.session
                            .acknowledge_applied(id)
                            .expect("held snapshot can be acknowledged");
                    }
                }
                ClientEvent::Activated { tick } => node.activated.push(tick),
                ClientEvent::Closed { reason } => node.closed.push(reason),
            }
        }
    }

    fn submit_commands(&mut self, index: usize) {
        let lead = self.config.command_lead_ticks;
        let node = &mut self.clients[index];
        if node.session.phase() != Phase::Active {
            return;
        }
        let Some((_, newest)) = node.session.latest_snapshot() else {
            return;
        };
        for slot in node.session.slots() {
            let last = node.last_command_tick.get(&slot).copied().unwrap_or(0);
            let tick = (newest.get() + lead).max(last + 1);
            let command = match node.rng.below(20) {
                0 => Command::Spawn,
                1 => Command::Despawn,
                _ => Command::Move {
                    dx: node.rng.below(3) as i8 - 1,
                    dy: node.rng.below(3) as i8 - 1,
                },
            };
            match node
                .session
                .submit_command(slot, Tick::new(tick), &command.encode())
            {
                Ok(sequence) => {
                    node.last_command_tick.insert(slot, tick);
                    node.submitted.insert((slot.get(), sequence.get()));
                }
                Err(_) => node.submit_refused += 1,
            }
        }
    }

    pub fn run_ticks(&mut self, ticks: u64) {
        for _ in 0..ticks * self.config.pumps_per_tick {
            self.step();
        }
    }

    /// Step until `done` holds; false if `max_ticks` passed first.
    pub fn run_until(&mut self, max_ticks: u64, done: impl Fn(&Self) -> bool) -> bool {
        for _ in 0..max_ticks * self.config.pumps_per_tick {
            if done(self) {
                return true;
            }
            self.step();
        }
        done(self)
    }

    pub fn all_active(&self) -> bool {
        self.clients
            .iter()
            .all(|node| node.session.phase() == Phase::Active)
    }

    /// How many ticks `client`'s applied state trails the host.
    pub fn lag_ticks(&self, client: usize) -> Option<u64> {
        let (_, tick, _) = self.clients[client].applied.as_ref()?;
        Some(self.tick - tick.get())
    }

    /// Every snapshot a client ever saw matched the host's for that tick,
    /// ticks only moved forward, and no command executed twice.
    pub fn assert_consistent(&self) {
        for (index, node) in self.clients.iter().enumerate() {
            assert_eq!(node.digest_mismatches, 0, "client {index} saw wrong state");
            assert_eq!(node.tick_regressions, 0, "client {index} went back in time");
        }
        assert_eq!(self.world.duplicate_executions, 0, "command executed twice");
        assert_eq!(
            self.release_order_violations, 0,
            "commands not slot-ordered"
        );
        let submitted: BTreeSet<(u8, u32)> = self
            .clients
            .iter()
            .flat_map(|node| node.submitted.iter().copied())
            .collect();
        // Sequences are per membership, so a rejoining client may reuse a
        // pair; the host can still never execute one nobody submitted.
        for executed in self.world.executed() {
            assert!(
                submitted.contains(executed),
                "executed unsubmitted {executed:?}"
            );
        }
    }
}
