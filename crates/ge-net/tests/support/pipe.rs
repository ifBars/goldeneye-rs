//! A scripted host/client pair with hand-controlled delivery.
//!
//! Unlike the seeded link simulator, nothing here is random: the test holds,
//! drops, duplicates or reorders individual messages to reach one exact
//! protocol state.

use std::collections::VecDeque;
use std::sync::Arc;

use ge_net::protocol::envelope::{self, Envelope};
use ge_net::protocol::message::{CodecLimits, KindClass, Message, classify, decode_payload};
use ge_net::session::host::HostEvent;
use ge_net::{
    AttemptId, ClientConfig, ClientEvent, ClientSession, DrainLimit, HostConfig, HostSession,
    IngestError, Ingested, Lane, NetInstant, Outbound, Phase, SendOutcome, SessionBudget, Snapshot,
    SnapshotId, Tick, TransportCapabilities,
};

use super::harness::descriptor;
use super::rng::SeededRng;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Wire {
    pub lane: Lane,
    pub bytes: Vec<u8>,
}

impl Wire {
    pub fn envelope(&self) -> Envelope<'_> {
        envelope::decode(&self.bytes, usize::MAX).expect("session produced a valid envelope")
    }

    /// The decoded message, if this wire is a whole (unfragmented) one.
    pub fn message(&self) -> Option<Message<'_>> {
        let envelope = self.envelope();
        if envelope.header.fragment.is_some() {
            return None;
        }
        let KindClass::Known(kind) = classify(&envelope.header).ok()? else {
            return None;
        };
        decode_payload(kind, envelope.payload, &CODEC).ok()
    }
}

pub const CODEC: CodecLimits = CodecLimits {
    max_command_bytes: 1_024,
    max_slots_per_peer: 8,
};

pub const CAPABILITIES: TransportCapabilities = TransportCapabilities {
    reliable_lanes: 2,
    max_reliable_message_bytes: 64 * 1024,
    max_datagram_bytes: Some(1_200),
};

fn backpressured(result: &Result<Ingested, IngestError>) -> bool {
    matches!(
        result,
        Err(IngestError::InboundBudgetExhausted | IngestError::EventQueueFull)
    )
}

pub struct Pair {
    pub host: HostSession,
    pub client: ClientSession,
    pub host_attempt: AttemptId,
    pub client_attempt: AttemptId,
    pub to_client: VecDeque<Wire>,
    pub to_host: VecDeque<Wire>,
    pub now: u64,
    pub host_events: Vec<HostEvent>,
    pub client_events: Vec<ClientEvent>,
    /// The newest state the scripted application applied.
    pub applied: Option<(SnapshotId, Tick, Arc<Snapshot>)>,
    /// Apply and acknowledge snapshots as their events arrive.
    pub auto_apply: bool,
    pub host_disconnects: Vec<AttemptId>,
    pub client_disconnects: Vec<AttemptId>,
    pub last_host_ingest: Vec<Result<Ingested, IngestError>>,
    pub last_client_ingest: Vec<Result<Ingested, IngestError>>,
}

impl Pair {
    /// A host and one client whose transport has just connected.
    pub fn new(budget: SessionBudget, sequence_origin: u32, seed: u64) -> Self {
        Self::with_budgets(budget.clone(), budget, sequence_origin, seed)
    }

    /// As [`Pair::new`], with a different budget on each side.
    pub fn with_budgets(
        host_budget: SessionBudget,
        budget: SessionBudget,
        sequence_origin: u32,
        seed: u64,
    ) -> Self {
        let mut entropy = SeededRng::new(seed);
        let mut host = HostSession::new(
            HostConfig {
                budget: host_budget,
                descriptor: descriptor(),
                sequence_origin,
            },
            CAPABILITIES,
            &mut entropy,
            NetInstant::ZERO,
        )
        .expect("valid host");
        let mut client = ClientSession::new(
            ClientConfig {
                budget,
                descriptor: descriptor(),
                requested_slots: 1,
                sequence_origin,
            },
            CAPABILITIES,
            NetInstant::ZERO,
        )
        .expect("valid client");
        let host_attempt = AttemptId::new(11);
        let client_attempt = AttemptId::new(77);
        host.admit(host_attempt).expect("admitted");
        client.begin(client_attempt).expect("begun");
        client
            .transport_connected(client_attempt)
            .expect("connected");
        Self {
            host,
            client,
            host_attempt,
            client_attempt,
            to_client: VecDeque::new(),
            to_host: VecDeque::new(),
            now: 0,
            host_events: Vec::new(),
            client_events: Vec::new(),
            applied: None,
            auto_apply: true,
            host_disconnects: Vec::new(),
            client_disconnects: Vec::new(),
            last_host_ingest: Vec::new(),
            last_client_ingest: Vec::new(),
        }
    }

    /// Advance both sessions' clocks.
    pub fn advance(&mut self, micros: u64) {
        self.now += micros;
        let now = NetInstant::from_micros(self.now);
        self.host.advance(now).expect("monotonic");
        self.client.advance(now).expect("monotonic");
        self.collect_events();
    }

    /// Move the host's queued messages onto the wire towards the client.
    pub fn flush_host(&mut self) {
        let (wires, disconnects) = (&mut self.to_client, &mut self.host_disconnects);
        self.host.drain_outbound(DrainLimit::UNLIMITED, |outbound| {
            match outbound {
                Outbound::Message { lane, bytes, .. } => wires.push_back(Wire {
                    lane,
                    bytes: bytes.to_vec(),
                }),
                Outbound::Disconnect { attempt } => disconnects.push(attempt),
            }
            SendOutcome::Sent
        });
        self.collect_events();
    }

    /// Move the client's queued messages onto the wire towards the host.
    pub fn flush_client(&mut self) {
        let (wires, disconnects) = (&mut self.to_host, &mut self.client_disconnects);
        self.client
            .drain_outbound(DrainLimit::UNLIMITED, |outbound| {
                match outbound {
                    Outbound::Message { lane, bytes, .. } => wires.push_back(Wire {
                        lane,
                        bytes: bytes.to_vec(),
                    }),
                    Outbound::Disconnect { attempt } => disconnects.push(attempt),
                }
                SendOutcome::Sent
            });
        self.collect_events();
    }

    pub fn give_client(&mut self, wire: &Wire) -> Result<Ingested, IngestError> {
        let result = self
            .client
            .ingest(self.client_attempt, wire.lane, &wire.bytes);
        self.collect_events();
        result
    }

    pub fn give_host(&mut self, wire: &Wire) -> Result<Ingested, IngestError> {
        let result = self.host.ingest(self.host_attempt, wire.lane, &wire.bytes);
        self.collect_events();
        result
    }

    pub fn deliver_to_client(&mut self) {
        self.last_client_ingest.clear();
        while let Some(wire) = self.to_client.pop_front() {
            let result = self.give_client(&wire);
            if backpressured(&result) {
                // Not consumed: it stays first in line for the next update.
                self.to_client.push_front(wire);
                break;
            }
            self.last_client_ingest.push(result);
        }
    }

    pub fn deliver_to_host(&mut self) {
        self.last_host_ingest.clear();
        while let Some(wire) = self.to_host.pop_front() {
            let result = self.give_host(&wire);
            if backpressured(&result) {
                self.to_host.push_front(wire);
                break;
            }
            self.last_host_ingest.push(result);
        }
    }

    /// Flush and deliver both ways until nothing is in flight.
    pub fn settle(&mut self) {
        for _ in 0..512 {
            self.flush_host();
            self.flush_client();
            if self.to_client.is_empty() && self.to_host.is_empty() {
                return;
            }
            self.deliver_to_client();
            self.deliver_to_host();
            if !self.to_client.is_empty() || !self.to_host.is_empty() {
                // A per-update inbound budget was spent; start a new update.
                self.advance(1_000);
            }
        }
        panic!("pair did not settle");
    }

    /// Publish `snapshot` at `tick`, run a tick of time and settle.
    pub fn publish(&mut self, tick: u64, snapshot: Snapshot) -> SnapshotId {
        let id = self
            .host
            .publish_snapshot(Tick::new(tick), snapshot)
            .expect("publishable snapshot");
        self.advance(16_666);
        self.settle();
        id
    }

    /// Negotiate, synchronize on `snapshot` and reach the active phase.
    pub fn establish(&mut self, tick: u64, snapshot: Snapshot) -> SnapshotId {
        self.settle();
        let id = self.publish(tick, snapshot);
        for _ in 0..8 {
            if self.client.phase() == Phase::Active
                && self.host.phase(self.host_attempt) == Some(Phase::Active)
            {
                return id;
            }
            self.advance(16_666);
            self.settle();
        }
        panic!(
            "pair did not become active: client {:?}, host {:?}",
            self.client.phase(),
            self.host.phase(self.host_attempt)
        );
    }

    fn collect_events(&mut self) {
        while let Some(event) = self.host.poll_event() {
            self.host_events.push(event);
        }
        while let Some(event) = self.client.poll_event() {
            if self.auto_apply
                && let ClientEvent::SyncSnapshot { id, tick, state }
                | ClientEvent::Snapshot {
                    id, tick, state, ..
                } = &event
            {
                self.applied = Some((*id, *tick, state.clone()));
                if self.client.phase() == Phase::Synchronizing {
                    self.client
                        .acknowledge_applied(*id)
                        .expect("held snapshot can be acknowledged");
                }
            }
            self.client_events.push(event);
        }
    }
}
