//! A seeded, in-memory link simulator that honours the transport contract.
//!
//! One host endpoint and any number of client endpoints exchange messages
//! over simulated connections. Time, loss, duplication, delay, reordering,
//! bandwidth and partitions all derive from the recorded seed.
//!
//! The two delivery contracts are simulated separately:
//!
//! - Datagrams can be lost, duplicated, reordered by jitter, dropped by a
//!   full bottleneck queue, and dropped during a partition.
//! - Each reliable lane delivers every message once and in order. Loss only
//!   delays it by a retransmission penalty. A partition holds reliable data
//!   until it heals; one that outlasts the failure deadline ends the
//!   connection with an explicit error for both ends.
//!
//! These are logical-link tests. They are not packet-level evidence about
//! any real backend.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use ge_net::{
    AttemptId, Lane, SendOutcome, Transport, TransportCapabilities, TransportEvent,
    TransportFailure,
};

use super::rng::SeededRng;

/// Conditions of one simulated connection, applied in both directions.
#[derive(Clone, Debug)]
pub struct LinkProfile {
    /// One-way base delay.
    pub latency_micros: u64,
    /// Uniform extra one-way delay; reorders datagrams.
    pub jitter_micros: u64,
    /// Independent datagram loss.
    pub loss_per_mille: u32,
    /// Chance per datagram of entering a loss burst.
    pub burst_enter_per_mille: u32,
    /// Chance per datagram of leaving a loss burst.
    pub burst_exit_per_mille: u32,
    pub duplicate_per_mille: u32,
    /// Shared by all lanes of one direction; `None` is unlimited.
    pub bandwidth_bytes_per_second: Option<u64>,
    /// Bottleneck queue for datagrams, and send buffer of each reliable lane.
    pub queue_bytes: usize,
    /// Delay added to a reliable message for each simulated loss.
    pub retransmit_micros: u64,
    /// A partition longer than this fails the connection.
    pub failure_after_micros: u64,
}

impl LinkProfile {
    /// No delay, loss or limit.
    pub fn ideal() -> Self {
        Self {
            latency_micros: 0,
            jitter_micros: 0,
            loss_per_mille: 0,
            burst_enter_per_mille: 0,
            burst_exit_per_mille: 1_000,
            duplicate_per_mille: 0,
            bandwidth_bytes_per_second: None,
            queue_bytes: 256 * 1024,
            retransmit_micros: 200_000,
            failure_after_micros: 8_000_000,
        }
    }

    /// A round-trip time and loss rate, with jitter of a tenth of the delay.
    pub fn with_rtt_and_loss(rtt_millis: u64, loss_per_mille: u32) -> Self {
        let latency_micros = rtt_millis * 1_000 / 2;
        Self {
            latency_micros,
            jitter_micros: latency_micros / 10,
            loss_per_mille,
            retransmit_micros: (rtt_millis * 1_000).max(50_000),
            ..Self::ideal()
        }
    }
}

/// What a simulated connection did, for bandwidth and loss reports.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LinkStats {
    pub datagrams_sent: u64,
    pub datagrams_lost: u64,
    pub datagrams_duplicated: u64,
    pub datagrams_congested: u64,
    pub datagram_bytes: u64,
    pub reliable_messages: u64,
    pub reliable_bytes: u64,
    pub reliable_delayed: u64,
    pub reliable_blocked: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Endpoint {
    Host,
    Client(usize),
}

#[derive(Debug, Default)]
struct Direction {
    /// When the bottleneck finishes serializing what was already sent.
    busy_until: u64,
    /// Last scheduled arrival per reliable lane, to preserve order.
    last_arrival: [u64; 2],
    /// Reliable bytes sent and not yet consumed by the receiver, per lane.
    unconsumed_reliable: [usize; 2],
    unconsumed_datagram: usize,
    in_burst: bool,
}

#[derive(Debug)]
struct Connection {
    client: usize,
    host_attempt: AttemptId,
    client_attempt: AttemptId,
    profile: LinkProfile,
    partitions: Vec<(u64, u64)>,
    /// Index 0 carries client to host, index 1 host to client.
    directions: [Direction; 2],
    /// Per side (0 client, 1 host): closed locally or told it is closed.
    closed: [bool; 2],
    fail_at: Option<u64>,
    stats: LinkStats,
}

#[derive(Debug)]
struct Scheduled {
    at: u64,
    order: u64,
    connection: usize,
    /// Side that receives: 0 client, 1 host.
    side: usize,
    event: TransportEvent,
    /// Reliable lane index and length, to release send-buffer accounting.
    reliable: Option<(usize, usize)>,
    datagram_len: usize,
}

impl PartialEq for Scheduled {
    fn eq(&self, other: &Self) -> bool {
        (self.at, self.order) == (other.at, other.order)
    }
}

impl Eq for Scheduled {}

impl PartialOrd for Scheduled {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Scheduled {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.at, self.order).cmp(&(other.at, other.order))
    }
}

/// The simulated network: one host, many clients.
#[derive(Debug)]
pub struct SimNetwork {
    now: u64,
    rng: SeededRng,
    connections: Vec<Connection>,
    host_inbox: BinaryHeap<Reverse<Scheduled>>,
    client_inboxes: Vec<BinaryHeap<Reverse<Scheduled>>>,
    order: u64,
    next_host_attempt: u64,
    next_client_attempt: Vec<u64>,
    pub capabilities: TransportCapabilities,
}

const fn lane_index(lane: Lane) -> Option<usize> {
    match lane {
        Lane::Control => Some(0),
        Lane::Bulk => Some(1),
        Lane::Datagram => None,
    }
}

impl SimNetwork {
    pub fn new(seed: u64, clients: usize) -> Self {
        Self {
            now: 0,
            rng: SeededRng::new(seed),
            connections: Vec::new(),
            host_inbox: BinaryHeap::new(),
            client_inboxes: (0..clients).map(|_| BinaryHeap::new()).collect(),
            order: 0,
            next_host_attempt: 1,
            // Distinct ranges, so no test can confuse the two sides' identifiers.
            next_client_attempt: (0..clients)
                .map(|index| 1_000 * (index as u64 + 1))
                .collect(),
            capabilities: TransportCapabilities {
                reliable_lanes: 2,
                max_reliable_message_bytes: 64 * 1024,
                max_datagram_bytes: Some(1_200),
            },
        }
    }

    pub fn now_micros(&self) -> u64 {
        self.now
    }

    /// Advance simulated time and fail connections whose partition outlasted
    /// the failure deadline.
    pub fn set_now(&mut self, now: u64) {
        assert!(now >= self.now, "simulated time is monotonic");
        self.now = now;
        for index in 0..self.connections.len() {
            if let Some(at) = self.connections[index].fail_at.filter(|at| *at <= now) {
                self.connections[index].fail_at = None;
                // At the failure time itself, so nothing scheduled after it
                // on an abandoned lane can arrive before the failure.
                for side in 0..2 {
                    if !self.connections[index].closed[side] {
                        self.schedule_closed(index, side, at, TransportFailure::TimedOut);
                    }
                }
            }
        }
    }

    /// Start a connection from `client`. Both ends learn of it after one
    /// latency. Returns the client's attempt identifier.
    pub fn connect(&mut self, client: usize, profile: LinkProfile) -> AttemptId {
        let host_attempt = AttemptId::new(self.next_host_attempt);
        self.next_host_attempt += 1;
        let client_attempt = AttemptId::new(self.next_client_attempt[client]);
        self.next_client_attempt[client] += 1;
        let at = self.now + profile.latency_micros;
        let index = self.connections.len();
        self.connections.push(Connection {
            client,
            host_attempt,
            client_attempt,
            profile,
            partitions: Vec::new(),
            directions: Default::default(),
            closed: [false; 2],
            fail_at: None,
            stats: LinkStats::default(),
        });
        for side in 0..2 {
            let attempt = self.attempt_of(index, side);
            self.schedule(
                index,
                side,
                at,
                TransportEvent::Connected { attempt },
                None,
                0,
            );
        }
        client_attempt
    }

    /// Cut the client's newest connection between two instants.
    pub fn partition(&mut self, client: usize, from: u64, until: u64) {
        let index = self.newest(client).expect("client has a connection");
        let connection = &mut self.connections[index];
        connection.partitions.push((from, until));
        if until - from > connection.profile.failure_after_micros {
            let at = from + connection.profile.failure_after_micros;
            connection.fail_at = Some(connection.fail_at.map_or(at, |old| old.min(at)));
        }
        self.reconcile(index);
    }

    /// Apply the connection's partitions to messages already in flight:
    /// datagrams due during one are lost, and reliable messages wait for it
    /// to heal without overtaking each other.
    fn reconcile(&mut self, index: usize) {
        let client = self.connections[index].client;
        let mut pending = Vec::new();
        for side in 0..2 {
            let inbox = if side == 1 {
                &mut self.host_inbox
            } else {
                &mut self.client_inboxes[client]
            };
            let (mine, rest): (Vec<_>, Vec<_>) = std::mem::take(inbox)
                .into_iter()
                .partition(|Reverse(item)| item.connection == index);
            *inbox = rest.into_iter().collect();
            pending.extend(mine.into_iter().map(|Reverse(item)| item));
        }
        pending.sort_by_key(|item| (item.at, item.order));

        let profile = self.connections[index].profile.clone();
        // Latest arrival per receiving side and reliable lane.
        let mut last = [[0u64; 2]; 2];
        let mut orderly_closes = Vec::new();
        for mut item in pending {
            if matches!(
                item.event,
                TransportEvent::Closed {
                    failure: TransportFailure::RemoteClosed,
                    ..
                }
            ) {
                // Placed once every reliable arrival is known.
                orderly_closes.push(item);
                continue;
            }
            let partitioned = self.partition_end(index, item.at);
            match item.reliable {
                Some((lane, _)) => {
                    let mut at = item.at;
                    if let Some(end) = partitioned {
                        at = end + profile.retransmit_micros + profile.latency_micros;
                    }
                    at = at.max(last[item.side][lane]);
                    last[item.side][lane] = at;
                    // The sender's direction feeds this side.
                    let direction = &mut self.connections[index].directions[1 - item.side];
                    direction.last_arrival[lane] = direction.last_arrival[lane].max(at);
                    item.at = at;
                }
                None if partitioned.is_some()
                    && matches!(
                        item.event,
                        TransportEvent::Message {
                            lane: Lane::Datagram,
                            ..
                        }
                    ) =>
                {
                    let connection = &mut self.connections[index];
                    connection.directions[1 - item.side].unconsumed_datagram -= item.datagram_len;
                    connection.stats.datagrams_lost += 1;
                    continue;
                }
                None => {}
            }
            let inbox = if item.side == 1 {
                &mut self.host_inbox
            } else {
                &mut self.client_inboxes[client]
            };
            inbox.push(Reverse(item));
        }
        // An orderly close follows every reliable message already accepted
        // for that side, wherever the partition moved them.
        for mut item in orderly_closes {
            let flushed = last[item.side].iter().copied().max().unwrap_or(0);
            item.at = item.at.max(flushed + 1);
            let inbox = if item.side == 1 {
                &mut self.host_inbox
            } else {
                &mut self.client_inboxes[client]
            };
            inbox.push(Reverse(item));
        }
    }

    /// Replace the profile of the client's newest connection.
    pub fn set_profile(&mut self, client: usize, profile: LinkProfile) {
        let index = self.newest(client).expect("client has a connection");
        self.connections[index].profile = profile;
    }

    /// Statistics of the client's newest connection.
    pub fn stats(&self, client: usize) -> LinkStats {
        self.newest(client)
            .map(|index| self.connections[index].stats)
            .unwrap_or_default()
    }

    /// The host's identifier for the client's newest connection.
    pub fn host_attempt(&self, client: usize) -> Option<AttemptId> {
        self.newest(client)
            .map(|index| self.connections[index].host_attempt)
    }

    /// A transport view for one endpoint.
    pub fn endpoint(&mut self, endpoint: Endpoint) -> SimEndpoint<'_> {
        SimEndpoint {
            network: self,
            endpoint,
        }
    }

    fn newest(&self, client: usize) -> Option<usize> {
        self.connections
            .iter()
            .rposition(|connection| connection.client == client)
    }

    fn attempt_of(&self, connection: usize, side: usize) -> AttemptId {
        let connection = &self.connections[connection];
        if side == 0 {
            connection.client_attempt
        } else {
            connection.host_attempt
        }
    }

    fn find(&self, endpoint: Endpoint, attempt: AttemptId) -> Option<(usize, usize)> {
        self.connections
            .iter()
            .position(|connection| match endpoint {
                Endpoint::Host => connection.host_attempt == attempt,
                Endpoint::Client(client) => {
                    connection.client == client && connection.client_attempt == attempt
                }
            })
            .map(|index| (index, usize::from(endpoint == Endpoint::Host)))
    }

    fn schedule(
        &mut self,
        connection: usize,
        side: usize,
        at: u64,
        event: TransportEvent,
        reliable: Option<(usize, usize)>,
        datagram_len: usize,
    ) {
        self.order += 1;
        let scheduled = Reverse(Scheduled {
            at,
            order: self.order,
            connection,
            side,
            event,
            reliable,
            datagram_len,
        });
        if side == 1 {
            self.host_inbox.push(scheduled);
        } else {
            let client = self.connections[connection].client;
            self.client_inboxes[client].push(scheduled);
        }
    }

    fn schedule_closed(
        &mut self,
        connection: usize,
        side: usize,
        at: u64,
        failure: TransportFailure,
    ) {
        let attempt = self.attempt_of(connection, side);
        self.schedule(
            connection,
            side,
            at,
            TransportEvent::Closed { attempt, failure },
            None,
            0,
        );
    }

    /// End of the partition covering `at`, if any.
    fn partition_end(&self, connection: usize, at: u64) -> Option<u64> {
        self.connections[connection]
            .partitions
            .iter()
            .filter(|(from, until)| *from <= at && at < *until)
            .map(|(_, until)| *until)
            .max()
    }

    fn poll_endpoint(&mut self, endpoint: Endpoint) -> Option<TransportEvent> {
        loop {
            let inbox = match endpoint {
                Endpoint::Host => &mut self.host_inbox,
                Endpoint::Client(client) => &mut self.client_inboxes[client],
            };
            if inbox.peek().is_none_or(|next| next.0.at > self.now) {
                return None;
            }
            let Reverse(scheduled) = inbox.pop().expect("peeked");
            let connection = &mut self.connections[scheduled.connection];
            // The sender's direction feeds this side.
            let direction = &mut connection.directions[1 - scheduled.side];
            if let Some((lane, len)) = scheduled.reliable {
                direction.unconsumed_reliable[lane] -= len;
            }
            direction.unconsumed_datagram -= scheduled.datagram_len;
            if connection.closed[scheduled.side] {
                continue;
            }
            if matches!(scheduled.event, TransportEvent::Closed { .. }) {
                connection.closed[scheduled.side] = true;
            }
            return Some(scheduled.event);
        }
    }

    fn send_from(
        &mut self,
        endpoint: Endpoint,
        attempt: AttemptId,
        lane: Lane,
        message: &[u8],
    ) -> SendOutcome {
        let Some((index, side)) = self.find(endpoint, attempt) else {
            return SendOutcome::Closed;
        };
        if self.connections[index].closed.contains(&true) {
            return SendOutcome::Closed;
        }
        let now = self.now;
        let profile = self.connections[index].profile.clone();
        let len = message.len();
        let peer_side = 1 - side;
        let peer_attempt = self.attempt_of(index, peer_side);
        let event = TransportEvent::Message {
            attempt: peer_attempt,
            lane,
            bytes: message.to_vec(),
        };
        let serialize = |direction: &mut Direction| -> u64 {
            let start = direction.busy_until.max(now);
            direction.busy_until = match profile.bandwidth_bytes_per_second {
                Some(rate) => start + (len as u64 * 1_000_000).div_ceil(rate.max(1)),
                None => start,
            };
            direction.busy_until
        };

        if let Some(lane_slot) = lane_index(lane) {
            let direction = &mut self.connections[index].directions[side];
            if direction.unconsumed_reliable[lane_slot] + len > profile.queue_bytes {
                self.connections[index].stats.reliable_blocked += 1;
                return SendOutcome::Blocked;
            }
            let departure = serialize(direction);
            let mut delay = profile.latency_micros + self.rng.below(profile.jitter_micros + 1);
            let mut delayed = false;
            while self.rng.chance(profile.loss_per_mille) {
                delay += profile.retransmit_micros;
                delayed = true;
                if delay > profile.failure_after_micros {
                    // The backend gives up retransmitting: the connection
                    // fails for both ends, and nothing later on this lane
                    // may arrive before that failure.
                    let at = departure + profile.failure_after_micros;
                    let connection = &mut self.connections[index];
                    connection.fail_at = Some(connection.fail_at.map_or(at, |old| old.min(at)));
                    let last = &mut connection.directions[side].last_arrival[lane_slot];
                    *last = (*last).max(at + 1);
                    connection.stats.reliable_messages += 1;
                    connection.stats.reliable_delayed += 1;
                    return SendOutcome::Sent;
                }
            }
            let mut arrival = departure + delay;
            // A partition holds reliable data until it heals.
            for probe in [departure, arrival] {
                if let Some(end) = self.partition_end(index, probe) {
                    arrival = arrival.max(end + profile.retransmit_micros + profile.latency_micros);
                    delayed = true;
                }
            }
            let connection = &mut self.connections[index];
            let direction = &mut connection.directions[side];
            arrival = arrival.max(direction.last_arrival[lane_slot]);
            direction.last_arrival[lane_slot] = arrival;
            direction.unconsumed_reliable[lane_slot] += len;
            connection.stats.reliable_messages += 1;
            connection.stats.reliable_bytes += len as u64;
            connection.stats.reliable_delayed += u64::from(delayed);
            self.schedule(index, peer_side, arrival, event, Some((lane_slot, len)), 0);
            return SendOutcome::Sent;
        }

        // Datagram: every outcome below still reports `Sent`, as a real
        // unreliable lane would.
        self.connections[index].stats.datagrams_sent += 1;
        self.connections[index].stats.datagram_bytes += len as u64;
        let direction = &mut self.connections[index].directions[side];
        if direction.in_burst {
            if self.rng.chance(profile.burst_exit_per_mille) {
                direction.in_burst = false;
            }
        } else if self.rng.chance(profile.burst_enter_per_mille) {
            direction.in_burst = true;
        }
        let lost = direction.in_burst
            || self.rng.chance(profile.loss_per_mille)
            || self.partition_end(index, now).is_some();
        if lost {
            self.connections[index].stats.datagrams_lost += 1;
            return SendOutcome::Sent;
        }
        let direction = &mut self.connections[index].directions[side];
        let backlog_micros = direction.busy_until.saturating_sub(now);
        let backlog_bytes = profile
            .bandwidth_bytes_per_second
            .map_or(0, |rate| backlog_micros * rate / 1_000_000);
        if backlog_bytes as usize + len > profile.queue_bytes
            || direction.unconsumed_datagram + len > profile.queue_bytes
        {
            self.connections[index].stats.datagrams_congested += 1;
            return SendOutcome::Sent;
        }
        let departure = serialize(direction);
        let copies = 1 + usize::from(self.rng.chance(profile.duplicate_per_mille));
        self.connections[index].stats.datagrams_duplicated += copies as u64 - 1;
        for _ in 0..copies {
            let arrival =
                departure + profile.latency_micros + self.rng.below(profile.jitter_micros + 1);
            if self.partition_end(index, arrival).is_some() {
                self.connections[index].stats.datagrams_lost += 1;
                continue;
            }
            self.connections[index].directions[side].unconsumed_datagram += len;
            self.schedule(index, peer_side, arrival, event.clone(), None, len);
        }
        SendOutcome::Sent
    }

    fn close_from(&mut self, endpoint: Endpoint, attempt: AttemptId) {
        let Some((index, side)) = self.find(endpoint, attempt) else {
            return;
        };
        let connection = &mut self.connections[index];
        if connection.closed[side] {
            return;
        }
        connection.closed[side] = true;
        connection.fail_at = None;
        if connection.closed[1 - side] {
            return;
        }
        // Reliable messages already accepted are delivered before the close.
        let flushed = connection.directions[side]
            .last_arrival
            .iter()
            .copied()
            .max()
            .unwrap_or(0);
        let at = flushed.max(self.now + connection.profile.latency_micros) + 1;
        self.schedule_closed(index, 1 - side, at, TransportFailure::RemoteClosed);
    }
}

/// One endpoint's view of the network, usable as a [`Transport`].
pub struct SimEndpoint<'a> {
    network: &'a mut SimNetwork,
    endpoint: Endpoint,
}

impl Transport for SimEndpoint<'_> {
    fn capabilities(&self) -> TransportCapabilities {
        self.network.capabilities
    }

    fn poll(&mut self) -> Option<TransportEvent> {
        self.network.poll_endpoint(self.endpoint)
    }

    fn send(&mut self, attempt: AttemptId, lane: Lane, message: &[u8]) -> SendOutcome {
        self.network
            .send_from(self.endpoint, attempt, lane, message)
    }

    fn close(&mut self, attempt: AttemptId) {
        self.network.close_from(self.endpoint, attempt);
    }
}
