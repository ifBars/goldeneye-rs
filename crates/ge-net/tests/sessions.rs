//! Host and two-client sessions over the seeded link simulator.
//!
//! Each scenario names its seed. Loss, delay and bandwidth figures are test
//! conditions, not promised real-network performance.

mod support;

use ge_net::replication::SubmitError;
use ge_net::session::host::{HostEvent, PublishError};
use ge_net::session::{StateError, TimeRegression};
use ge_net::{
    AttemptId, ChannelId, ClientConfig, ClientEvent, ClientSession, CloseReason, DrainLimit,
    DropReason, IngestError, Ingested, Lane, Mismatch, NetInstant, Outbound, Phase, PlayerSlot,
    SendOutcome, SessionBudget, Snapshot, Tick, TransportFailure,
};
use support::harness::{Harness, HarnessConfig, descriptor};
use support::link::{Endpoint, LinkProfile};
use support::pipe::{Pair, Wire};
use support::simulation::Command;

fn active_pair(seed: u64) -> Harness {
    let mut harness = Harness::new(HarnessConfig::new(seed, 2));
    harness.connect_all();
    assert!(
        harness.run_until(300, Harness::all_active),
        "seed {seed}: clients activate"
    );
    harness
}

fn closed_events(harness: &Harness) -> Vec<(AttemptId, CloseReason)> {
    harness
        .host_events
        .iter()
        .filter_map(|event| match event {
            HostEvent::PeerClosed {
                attempt, reason, ..
            } => Some((*attempt, *reason)),
            _ => None,
        })
        .collect()
}

#[test]
fn two_clients_join_and_converge_on_an_ideal_link() {
    let mut harness = active_pair(1);
    harness.run_ticks(300);
    harness.assert_consistent();
    for client in 0..2 {
        assert!(harness.lag_ticks(client).unwrap() <= 1);
        assert!(harness.clients[client].closed.is_empty());
        assert_eq!(harness.clients[client].activated.len(), 1);
        // Every submitted command ran exactly once on a lossless link.
        assert_eq!(harness.clients[client].submit_refused, 0);
    }
    let submitted: usize = harness.clients.iter().map(|c| c.submitted.len()).sum();
    assert!(submitted > 500);
    // The last few are still in flight or scheduled for future ticks.
    assert!(harness.world.executed().len() + 16 >= submitted);
    assert_eq!(
        harness.delivered_commands as usize,
        harness.world.executed().len()
    );
    assert_eq!(harness.host.diagnostics().drops(DropReason::CommandLate), 0);
    // Each accepted peer owns a distinct slot and avatar.
    let slots: Vec<_> = harness.owners.values().flatten().copied().collect();
    assert_eq!(slots.len(), 2);
    assert_ne!(slots[0], slots[1]);
    for slot in slots {
        assert!(harness.world.avatar(slot).is_some());
    }
}

#[test]
fn sessions_converge_across_the_loss_and_delay_profile_matrix() {
    for (row, loss_per_mille) in [0, 10, 50, 100].into_iter().enumerate() {
        for (column, rtt_millis) in [0, 50, 150, 300].into_iter().enumerate() {
            let seed = 100 + (row * 4 + column) as u64;
            let profile = LinkProfile::with_rtt_and_loss(rtt_millis, loss_per_mille);
            let label = format!("seed {seed}: {loss_per_mille}‰ loss, {rtt_millis} ms RTT");
            let mut config = HarnessConfig::new(seed, 2);
            config.profile = profile.clone();
            // Lead commands by the round trip so they arrive before their tick.
            config.command_lead_ticks = 4 + rtt_millis * 1_000 / config.tick_micros;
            let mut harness = Harness::new(config);
            harness.connect_all();
            assert!(
                harness.run_until(900, Harness::all_active),
                "{label}: activate"
            );
            harness.run_ticks(600);
            harness.assert_consistent();

            // Loss may leave the newest update missing. Once the loss stops,
            // state must catch up to within the one-way delay.
            let calm = LinkProfile {
                loss_per_mille: 0,
                ..profile
            };
            for client in 0..2 {
                harness.net.set_profile(client, calm.clone());
            }
            harness.run_ticks(120);
            harness.assert_consistent();
            let delay_ticks = (calm.latency_micros + calm.jitter_micros) / 16_666 + 2;
            for client in 0..2 {
                let node = &harness.clients[client];
                assert!(node.closed.is_empty(), "{label}: client {client} closed");
                assert_eq!(node.session.phase(), Phase::Active, "{label}");
                let lag = harness.lag_ticks(client).unwrap();
                assert!(
                    lag <= delay_ticks,
                    "{label}: client {client} lags {lag} ticks"
                );
                assert!(
                    node.snapshots_seen > 300,
                    "{label}: client {client} progressed"
                );
                let executed = harness
                    .world
                    .executed()
                    .iter()
                    .filter(|(slot, _)| *slot == client as u8)
                    .count();
                assert!(
                    executed > 300,
                    "{label}: client {client} ran {executed} commands"
                );
            }
            if loss_per_mille > 0 {
                assert!(harness.net.stats(0).datagrams_lost > 0, "{label}");
            }
        }
    }
}

#[test]
fn duplication_and_reordering_never_apply_anything_twice() {
    let seed = 21;
    let mut config = HarnessConfig::new(seed, 2);
    config.profile = LinkProfile {
        latency_micros: 20_000,
        jitter_micros: 60_000,
        duplicate_per_mille: 300,
        ..LinkProfile::ideal()
    };
    config.command_lead_ticks = 12;
    let mut harness = Harness::new(config);
    harness.connect_all();
    assert!(harness.run_until(300, Harness::all_active));
    harness.run_ticks(600);
    harness.assert_consistent();
    assert!(harness.net.stats(0).datagrams_duplicated > 100);
    let client = harness.clients[0].session.diagnostics();
    assert!(client.drops(DropReason::Stale) > 0, "reordering happened");
    assert!(
        client.drops(DropReason::Duplicate) > 0,
        "duplication happened"
    );
    let host = harness.host.diagnostics();
    assert!(host.drops(DropReason::CommandDuplicate) > 0);
    assert_eq!(harness.world.duplicate_executions, 0);
    for client in 0..2 {
        assert!(harness.clients[client].closed.is_empty());
        assert!(harness.lag_ticks(client).unwrap() <= 8);
    }
}

#[test]
fn fragmented_updates_survive_reordering_and_duplication() {
    let seed = 36;
    let mut config = HarnessConfig::new(seed, 2);
    // 150 changing actors: each update spans three datagrams.
    config.drifters = 150;
    config.profile = LinkProfile {
        latency_micros: 15_000,
        jitter_micros: 30_000,
        loss_per_mille: 20,
        duplicate_per_mille: 100,
        ..LinkProfile::ideal()
    };
    config.command_lead_ticks = 8;
    let mut harness = Harness::new(config);
    harness.connect_all();
    assert!(harness.run_until(600, Harness::all_active));
    harness.run_ticks(600);
    harness.assert_consistent();
    for client in 0..2 {
        let node = &harness.clients[client];
        assert!(node.closed.is_empty());
        let diagnostics = node.session.diagnostics();
        // Some updates completed from out-of-order fragments; some partial
        // ones were abandoned for a newer update.
        assert!(diagnostics.state_updates_applied > 200, "seed {seed}");
        assert!(
            diagnostics.drops(DropReason::PartialAbandoned) > 0,
            "seed {seed}"
        );
        assert!(diagnostics.drops(DropReason::Duplicate) > 0, "seed {seed}");
        assert!(harness.lag_ticks(client).unwrap() <= 12);
    }
    let sent = harness.host.diagnostics().channel(ChannelId::State);
    assert!(sent.messages_sent > 3 * 600, "updates were fragmented");
}

#[test]
fn burst_loss_is_survived_without_reconnecting() {
    let seed = 22;
    let mut config = HarnessConfig::new(seed, 2);
    config.profile = LinkProfile {
        latency_micros: 25_000,
        burst_enter_per_mille: 30,
        burst_exit_per_mille: 150,
        ..LinkProfile::ideal()
    };
    config.command_lead_ticks = 6;
    let mut harness = Harness::new(config);
    harness.connect_all();
    assert!(harness.run_until(600, Harness::all_active));
    harness.run_ticks(900);
    harness.assert_consistent();
    let stats = harness.net.stats(0);
    assert!(
        stats.datagrams_lost * 10 > stats.datagrams_sent,
        "bursts dropped over a tenth"
    );
    for client in 0..2 {
        assert!(harness.clients[client].closed.is_empty());
        assert_eq!(harness.clients[client].activated.len(), 1);
        assert!(harness.clients[client].snapshots_seen > 400);
    }
}

#[test]
fn a_congested_link_drops_state_but_stays_bounded_and_consistent() {
    let seed = 23;
    let mut config = HarnessConfig::new(seed, 2);
    // Forty changing actors need far more than this link carries.
    config.drifters = 40;
    config.profile = LinkProfile {
        latency_micros: 10_000,
        bandwidth_bytes_per_second: Some(12_000),
        queue_bytes: 6_000,
        ..LinkProfile::ideal()
    };
    config.command_lead_ticks = 30;
    let budget = config.budget.max_queued_bytes_per_peer as usize;
    let mut harness = Harness::new(config);
    harness.connect_all();
    assert!(harness.run_until(900, Harness::all_active));
    for _ in 0..1_200 {
        harness.step();
        for attempt in harness.host.attempts() {
            let queued: usize = harness
                .host
                .peer_diagnostics(attempt)
                .unwrap()
                .queued_bytes
                .iter()
                .sum();
            assert!(queued <= budget, "seed {seed}: queue of {queued} bytes");
        }
    }
    harness.assert_consistent();
    let stats = harness.net.stats(0);
    assert!(
        stats.datagrams_congested > 100,
        "the bottleneck dropped datagrams"
    );
    for client in 0..2 {
        assert!(
            harness.clients[client].closed.is_empty(),
            "congestion is not fatal"
        );
        assert!(harness.clients[client].snapshots_seen > 50);
    }
    // Relief: the same session converges once the bottleneck is gone.
    for client in 0..2 {
        harness.net.set_profile(client, LinkProfile::ideal());
    }
    harness.run_ticks(120);
    harness.assert_consistent();
    for client in 0..2 {
        assert!(harness.lag_ticks(client).unwrap() <= 1);
    }
}

#[test]
fn a_short_partition_heals_without_a_new_attempt() {
    let seed = 24;
    let mut harness = active_pair(seed);
    harness.run_ticks(60);
    // Three seconds: longer than any ping interval, shorter than a timeout.
    harness
        .net
        .partition(0, harness.now, harness.now + 3_000_000);
    harness.run_ticks(170);
    assert!(
        harness.lag_ticks(0).unwrap() > 100,
        "client 0 heard nothing"
    );
    assert!(
        harness.lag_ticks(1).unwrap() <= 1,
        "client 1 was unaffected"
    );
    harness.run_ticks(240);
    harness.assert_consistent();
    for client in 0..2 {
        assert!(harness.clients[client].closed.is_empty());
        assert_eq!(
            harness.clients[client].activated.len(),
            1,
            "no second activation"
        );
        assert!(harness.lag_ticks(client).unwrap() <= 1, "seed {seed}");
    }
    assert!(closed_events(&harness).is_empty());
}

#[test]
fn a_permanent_partition_fails_explicitly_and_frees_the_slot() {
    let seed = 25;
    let mut harness = active_pair(seed);
    harness.run_ticks(60);
    let lost_attempt = harness.net.host_attempt(0).unwrap();
    let lost_slots = harness.owners[&lost_attempt].clone();
    harness.net.partition(0, harness.now, u64::MAX / 2);
    // No convergence is asserted for the partitioned client: only that both
    // ends reach an explicit terminal state in bounded time.
    harness.run_ticks(60 * 12);
    assert_eq!(
        closed_events(&harness),
        vec![(
            lost_attempt,
            CloseReason::Transport(TransportFailure::TimedOut)
        )]
    );
    assert_eq!(
        harness.clients[0].closed,
        vec![CloseReason::Transport(TransportFailure::TimedOut)]
    );
    assert_eq!(harness.clients[0].session.phase(), Phase::Closed);
    assert_eq!(harness.host.phase(lost_attempt), None);
    for slot in lost_slots {
        assert_eq!(harness.world.avatar(slot), None, "the slot's avatar left");
    }
    assert_eq!(harness.host.diagnostics().disconnects.transport, 1);
    // The other client never noticed.
    harness.assert_consistent();
    assert!(harness.clients[1].closed.is_empty());
    assert!(harness.lag_ticks(1).unwrap() <= 1);
}

#[test]
fn a_client_that_stops_consuming_is_closed_and_never_grows_a_queue() {
    let seed = 26;
    let mut harness = active_pair(seed);
    harness.run_ticks(30);
    harness.clients[0].paused = true;
    let stalled = harness.net.host_attempt(0).unwrap();
    let budget = harness.config.budget.max_queued_bytes_per_peer as usize;
    let mut peak = 0;
    for _ in 0..60 * 12 * 2 {
        harness.step();
        if let Some(peer) = harness.host.peer_diagnostics(stalled) {
            peak = peak.max(peer.queued_bytes.iter().sum::<usize>());
        }
    }
    assert!(
        peak <= budget,
        "seed {seed}: stalled peer queued {peak} bytes"
    );
    assert_eq!(
        closed_events(&harness),
        vec![(stalled, CloseReason::IdleTimeout)]
    );
    assert_eq!(harness.host.diagnostics().disconnects.idle_timeout, 1);
    harness.assert_consistent();
    assert!(
        harness.lag_ticks(1).unwrap() <= 1,
        "the other client kept up"
    );
}

#[test]
fn a_connection_that_never_negotiates_times_out() {
    let mut config = HarnessConfig::new(27, 2);
    config.budget.max_handshakes = 1;
    let mut harness = Harness::new(config);
    // Raw transport connections with no client session behind them.
    harness.clients[0].paused = true;
    harness.clients[1].paused = true;
    harness.net.connect(0, LinkProfile::ideal());
    harness.run_ticks(2);
    let silent = harness.net.host_attempt(0).unwrap();
    assert_eq!(harness.host.phase(silent), Some(Phase::Negotiating));
    // The handshake budget is one: a second attempt is refused at admission.
    harness.net.connect(1, LinkProfile::ideal());
    harness.run_ticks(2);
    let refused = harness.net.host_attempt(1).unwrap();
    assert_eq!(harness.host.phase(refused), None);
    assert_eq!(harness.host.attempts(), vec![silent]);

    harness.run_ticks(60 * 5 + 5);
    assert_eq!(
        closed_events(&harness),
        vec![(silent, CloseReason::HandshakeTimeout)]
    );
    assert_eq!(harness.host.diagnostics().disconnects.handshake_timeout, 1);
    assert!(harness.host.attempts().is_empty());
    // The refused attempt stays refused even if its transport speaks later.
    assert_eq!(
        harness.host.ingest(refused, Lane::Control, &[1]),
        Err(IngestError::StaleAttempt)
    );
}

#[test]
fn a_client_that_never_applies_its_snapshot_never_becomes_active() {
    let mut config = HarnessConfig::new(28, 1);
    config.acknowledge = false;
    config.budget.sync_timeout = std::time::Duration::from_secs(3);
    let mut harness = Harness::new(config);
    harness.connect(0);
    harness.run_ticks(120);
    // The snapshot bytes arrived long ago; application work did not happen.
    assert!(harness.clients[0].applied.is_some());
    assert_eq!(harness.clients[0].session.phase(), Phase::Synchronizing);
    assert!(harness.clients[0].activated.is_empty());
    harness.run_ticks(120);
    let attempt = harness.net.host_attempt(0).unwrap();
    assert_eq!(
        closed_events(&harness),
        vec![(attempt, CloseReason::SyncTimeout)]
    );
    assert_eq!(harness.clients[0].session.phase(), Phase::Closed);
    assert!(matches!(
        harness.clients[0].closed[..],
        [CloseReason::RemoteClosed(5)] | [CloseReason::SyncTimeout]
    ));
    assert_eq!(harness.host.diagnostics().disconnects.sync_timeout, 1);
}

#[test]
fn graceful_close_frees_the_slot_and_a_reconnect_is_a_new_membership() {
    let seed = 29;
    let mut harness = active_pair(seed);
    harness.run_ticks(30);
    let first_attempt = harness.net.host_attempt(0).unwrap();
    let (first_peer, first_slots) = harness.clients[0].accepted.clone().unwrap();
    let first_client_attempt = harness.clients[0].session.attempt().unwrap();

    harness.clients[0].session.close().unwrap();
    assert_eq!(harness.clients[0].session.phase(), Phase::Closing);
    assert_eq!(
        harness.clients[0]
            .session
            .submit_command(first_slots[0], Tick::new(9_999), &[0]),
        Err(SubmitError::NotActive)
    );
    harness.run_ticks(10);
    assert_eq!(harness.clients[0].closed, vec![CloseReason::Requested]);
    assert_eq!(harness.clients[0].session.phase(), Phase::Closed);
    assert_eq!(
        closed_events(&harness),
        vec![(first_attempt, CloseReason::RemoteClosed(1))]
    );
    assert_eq!(harness.world.avatar(first_slots[0]), None);
    assert_eq!(harness.host.diagnostics().disconnects.remote_closed, 1);

    // Reconnect: new attempt on both sides, new peer identity, same slot free.
    let second_client_attempt = harness.connect(0);
    assert!(second_client_attempt > first_client_attempt);
    assert!(harness.run_until(300, Harness::all_active));
    harness.run_ticks(60);
    let (second_peer, second_slots) = harness.clients[0].accepted.clone().unwrap();
    assert_ne!(second_peer, first_peer);
    assert_eq!(second_slots, first_slots, "the freed slot was reused");
    assert_ne!(harness.net.host_attempt(0).unwrap(), first_attempt);
    assert_eq!(harness.clients[0].activated.len(), 2);
    harness.assert_consistent();
    assert!(harness.lag_ticks(0).unwrap() <= 1);
    assert!(harness.world.avatar(second_slots[0]).is_some());
}

#[test]
fn host_shutdown_closes_every_peer_and_refuses_new_attempts() {
    let mut harness = active_pair(30);
    harness.host.shutdown();
    assert!(!harness.host.is_shut_down());
    harness.run_ticks(10);
    assert!(harness.host.is_shut_down());
    for client in 0..2 {
        assert_eq!(
            harness.clients[client].closed,
            vec![CloseReason::RemoteClosed(2)]
        );
    }
    let reasons: Vec<_> = closed_events(&harness)
        .into_iter()
        .map(|(_, r)| r)
        .collect();
    assert_eq!(reasons, vec![CloseReason::Shutdown; 2]);
    assert_eq!(harness.host.diagnostics().disconnects.shutdown, 2);
    assert_eq!(
        harness.host.admit(AttemptId::new(10_000)),
        Err(StateError::WrongPhase(Phase::Closing))
    );
}

#[test]
fn an_incompatible_client_is_told_which_field_differs() {
    let mut harness = Harness::new(HarnessConfig::new(31, 2));
    let mut other = descriptor();
    other.content[7] ^= 0x40;
    harness.clients[1].session = ClientSession::new(
        ClientConfig {
            budget: SessionBudget::small_session(),
            descriptor: other,
            requested_slots: 1,
            sequence_origin: 0,
        },
        harness.net.capabilities,
        NetInstant::ZERO,
    )
    .unwrap();
    harness.connect_all();
    harness.run_ticks(60);
    let reason = CloseReason::Rejected(ge_net::protocol::message::RejectReason::Incompatible(
        Mismatch::Content,
    ));
    assert_eq!(harness.clients[1].closed, vec![reason]);
    assert_eq!(harness.clients[1].accepted, None);
    let rejected = harness.net.host_attempt(1).unwrap();
    assert_eq!(closed_events(&harness), vec![(rejected, reason)]);
    assert_eq!(harness.host.diagnostics().disconnects.rejected, 1);
    // The compatible client is unaffected.
    assert_eq!(harness.clients[0].session.phase(), Phase::Active);
    assert_eq!(harness.owners.len(), 1);
}

#[test]
fn peer_and_slot_limits_reject_with_a_reason() {
    use ge_net::protocol::message::RejectReason;
    let mut config = HarnessConfig::new(32, 2);
    config.budget.max_peers = 1;
    let mut harness = Harness::new(config);
    harness.connect(0);
    harness.run_ticks(30);
    harness.connect(1);
    harness.run_ticks(30);
    assert_eq!(
        harness.clients[1].closed,
        vec![CloseReason::Rejected(RejectReason::SessionFull)]
    );
    assert_eq!(harness.clients[0].session.phase(), Phase::Active);

    // Two slots exist and the first peer owns both: slots are not peers.
    let mut config = HarnessConfig::new(33, 2);
    config.budget.max_player_slots = 2;
    config.budget.max_slots_per_peer = 2;
    config.slots_per_client = 2;
    let mut harness = Harness::new(config);
    harness.connect(0);
    harness.run_ticks(30);
    harness.connect(1);
    harness.run_ticks(30);
    assert_eq!(
        harness.clients[1].closed,
        vec![CloseReason::Rejected(RejectReason::SlotsUnavailable)]
    );
    let (_, slots) = harness.clients[0].accepted.clone().unwrap();
    assert_eq!(slots, vec![PlayerSlot::new(0), PlayerSlot::new(1)]);
}

#[test]
fn one_peer_can_drive_several_slots_in_a_stable_order() {
    let mut config = HarnessConfig::new(34, 2);
    config.slots_per_client = 2;
    let mut harness = Harness::new(config);
    harness.connect_all();
    assert!(harness.run_until(300, Harness::all_active));
    harness.run_ticks(300);
    harness.assert_consistent();
    assert_eq!(harness.owners.values().flatten().count(), 4);
    for slot in 0..4u8 {
        assert!(harness.world.avatar(PlayerSlot::new(slot)).is_some());
        let executed = harness
            .world
            .executed()
            .iter()
            .filter(|(owner, _)| *owner == slot)
            .count();
        assert!(executed > 250, "slot {slot} ran {executed} commands");
    }
    assert_eq!(harness.release_order_violations, 0);
}

#[test]
fn identical_seeds_replay_identically_and_different_seeds_differ() {
    let run = |seed: u64| {
        let mut config = HarnessConfig::new(seed, 2);
        config.profile = LinkProfile::with_rtt_and_loss(80, 50);
        config.command_lead_ticks = 10;
        let mut harness = Harness::new(config);
        harness.connect_all();
        harness.run_ticks(400);
        (
            harness.host.diagnostics().clone(),
            harness.clients[0].session.diagnostics().clone(),
            harness.net.stats(0),
            harness.host_digests.clone(),
            harness.clients[1].snapshots_seen,
        )
    };
    assert_eq!(run(77), run(77));
    assert_ne!(run(77).2, run(78).2);
}

#[test]
fn round_trip_time_is_measured_from_probes() {
    let mut config = HarnessConfig::new(35, 1);
    config.profile = LinkProfile {
        latency_micros: 40_000,
        ..LinkProfile::ideal()
    };
    let mut harness = Harness::new(config);
    harness.connect(0);
    harness.run_ticks(600);
    let attempt = harness.net.host_attempt(0).unwrap();
    let peer = harness.host.peer_diagnostics(attempt).unwrap();
    let rtt = peer.rtt_micros.expect("probes were answered");
    // Eighty milliseconds, plus up to one pump interval at each end.
    assert!((80_000..=100_000).contains(&rtt), "host measured {rtt} µs");
    assert!(peer.jitter_micros < 20_000);
    assert!(!peer.rtt_samples_micros.is_empty());
    assert!(peer.rtt_samples_micros.len() <= 32, "samples are bounded");
    assert_eq!(peer.phase, Phase::Active);
    assert!(peer.baseline.is_some());
    assert!(peer.queue_high_water_bytes > 0);

    let client = harness.clients[0].session.peer_diagnostics().unwrap();
    let rtt = client.rtt_micros.expect("probes were answered");
    assert!(
        (80_000..=100_000).contains(&rtt),
        "client measured {rtt} µs"
    );
    assert_eq!(client.peer, harness.clients[0].session.peer());
    // Identities print without addresses or payloads.
    assert_eq!(attempt.to_string(), format!("attempt#{}", attempt.get()));
    assert_eq!(PlayerSlot::new(3).to_string(), "slot#3");
    let probes = harness.host.diagnostics().channel(ChannelId::Probe);
    assert!(probes.messages_sent > 10 && probes.messages_received > 10);
    assert!(probes.bytes_sent > 0 && probes.bytes_received > 0);
}

fn empty() -> Snapshot {
    Snapshot::empty()
}

fn command(dx: i8) -> Vec<u8> {
    Command::Move { dx, dy: 0 }.encode()
}

#[test]
fn commands_are_windowed_deduplicated_and_acknowledged() {
    let mut budget = SessionBudget::small_session();
    budget.command_future_ticks = 10;
    budget.max_pending_commands = 4;
    let mut pair = Pair::new(budget, 0, 40);
    pair.establish(1, empty());
    let slot = PlayerSlot::new(0);

    // Client-side refusals are typed errors, not silent drops.
    assert_eq!(
        pair.client
            .submit_command(PlayerSlot::new(5), Tick::new(3), &[0]),
        Err(SubmitError::UnknownSlot(PlayerSlot::new(5)))
    );
    assert_eq!(
        pair.client.submit_command(slot, Tick::new(3), &[0; 65]),
        Err(SubmitError::CommandTooLarge { len: 65, max: 64 })
    );
    let first = pair
        .client
        .submit_command(slot, Tick::new(3), &command(1))
        .unwrap();
    assert_eq!(
        pair.client.submit_command(slot, Tick::new(2), &command(1)),
        Err(SubmitError::TickRegression)
    );

    // The same datagram delivered three times is accepted once.
    pair.advance(16_666);
    pair.flush_client();
    let batch: Vec<Wire> = pair.to_host.drain(..).collect();
    for _ in 0..3 {
        for wire in &batch {
            pair.give_host(wire).unwrap();
        }
    }
    assert_eq!(pair.host.diagnostics().commands_accepted, 1);
    // Tick 2 has no commands; tick 3 releases exactly one.
    assert!(pair.host.take_commands(Tick::new(2)).unwrap().is_empty());
    let released = pair.host.take_commands(Tick::new(3)).unwrap();
    assert_eq!(released.len(), 1);
    assert_eq!(released[0].sequence, first);
    assert_eq!(released[0].slot, slot);
    assert_eq!(released[0].tick, Tick::new(3));
    assert_eq!(released[0].payload, command(1));
    assert_eq!(released[0].attempt, pair.host_attempt);
    assert_eq!(Some(released[0].peer), pair.client.peer());
    assert_eq!(
        pair.host.take_commands(Tick::new(3)),
        Err(PublishError::TickNotIncreasing { last: Tick::new(3) })
    );
    pair.publish(3, empty());
    assert_eq!(pair.client.pending_commands(slot), 0, "acknowledged");
    let acks = pair
        .client_events
        .iter()
        .rev()
        .find_map(|event| match event {
            ClientEvent::Snapshot { acks, .. } => Some(acks.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(acks.len(), 1);
    assert_eq!(acks[0].received_next, first.next());
    assert_eq!(acks[0].processed_next, first.next());

    // Late: its tick was already consumed. It is discarded, and
    // acknowledged so the client stops resending it.
    pair.client
        .submit_command(slot, Tick::new(3), &command(1))
        .unwrap();
    pair.advance(16_666);
    pair.settle();
    assert_eq!(pair.host.diagnostics().drops(DropReason::CommandLate), 1);
    pair.publish(4, empty());
    assert_eq!(pair.client.pending_commands(slot), 0);

    // Future: beyond the window it is not accepted yet, and stays pending.
    pair.client
        .submit_command(slot, Tick::new(40), &command(1))
        .unwrap();
    pair.advance(16_666);
    pair.settle();
    assert!(pair.host.diagnostics().drops(DropReason::CommandFuture) >= 1);
    pair.publish(5, empty());
    assert_eq!(pair.client.pending_commands(slot), 1);
    // The window reaches it once the host has consumed tick 30.
    pair.host.take_commands(Tick::new(30)).unwrap();
    pair.advance(16_666);
    pair.settle();
    pair.publish(30, empty());
    assert_eq!(pair.client.pending_commands(slot), 0);
    let released = pair.host.take_commands(Tick::new(40)).unwrap();
    assert_eq!(released.len(), 1);

    // A tick the application skipped discards its commands, counted.
    pair.client
        .submit_command(slot, Tick::new(45), &command(1))
        .unwrap();
    pair.advance(16_666);
    pair.settle();
    assert!(pair.host.take_commands(Tick::new(46)).unwrap().is_empty());
    assert_eq!(pair.host.diagnostics().drops(DropReason::CommandMissed), 1);
    pair.publish(46, empty());

    // Outbox pressure: four unacknowledged commands, the fifth is refused.
    for tick in 60..64 {
        pair.client
            .submit_command(slot, Tick::new(tick), &command(1))
            .unwrap();
    }
    assert_eq!(
        pair.client.submit_command(slot, Tick::new(64), &command(1)),
        Err(SubmitError::OutboxFull)
    );
    assert_eq!(pair.client.pending_commands(slot), 4);
    assert_eq!(pair.client.phase(), Phase::Active);
}

#[test]
fn transporting_a_command_does_not_make_it_valid() {
    let mut harness = active_pair(41);
    harness.config.auto_commands = false;
    harness.run_ticks(5);
    let (_, slots) = harness.clients[0].accepted.clone().unwrap();
    let avatar = harness.world.avatar(slots[0]).unwrap();
    let before = harness.world.actor(avatar).unwrap();
    let executed = harness.world.executed().len();
    let tick = Tick::new(harness.tick + 5);
    // Authenticated, well-formed, in-window, and against the rules.
    let teleport = Command::Move { dx: 100, dy: 0 }.encode();
    harness.clients[0]
        .session
        .submit_command(slots[0], tick, &teleport)
        .unwrap();
    harness.clients[0]
        .session
        .submit_command(slots[0], tick, &[0xee])
        .unwrap();
    // The most negative step, whose absolute value does not fit an `i8`.
    let wrapped = Command::Move { dx: i8::MIN, dy: 0 }.encode();
    harness.clients[0]
        .session
        .submit_command(slots[0], tick, &wrapped)
        .unwrap();
    harness.run_ticks(20);
    assert_eq!(harness.world.rejected_commands, 3);
    assert_eq!(
        harness.world.executed().len(),
        executed + 3,
        "released once each"
    );
    let after = harness.world.actor(avatar).unwrap();
    assert_eq!((after.x, after.y), (before.x, before.y));
}

#[test]
fn commands_for_a_slot_the_connection_does_not_own_close_it() {
    use ge_net::protocol::envelope;
    use ge_net::protocol::message::{CommandGroup, InputBatch, Message, WireCommand};
    let mut pair = Pair::new(SessionBudget::small_session(), 0, 42);
    pair.establish(1, empty());
    pair.client
        .submit_command(PlayerSlot::new(0), Tick::new(3), &command(1))
        .unwrap();
    pair.advance(16_666);
    pair.flush_client();
    let genuine = pair.to_host.pop_back().unwrap();
    let envelope = genuine.envelope();
    let Some(Message::InputBatch(batch)) = genuine.message() else {
        panic!("expected an input batch");
    };
    // The same authenticated connection claims another peer's slot.
    let forged = Message::InputBatch(InputBatch {
        ack: batch.ack,
        groups: vec![CommandGroup {
            slot: PlayerSlot::new(1),
            first: batch.groups[0].first,
            commands: vec![WireCommand {
                tick_offset: 0,
                payload: &[0],
            }],
        }],
    });
    let mut payload = Vec::new();
    forged.encode_payload(&mut payload).unwrap();
    let mut bytes = Vec::new();
    envelope::encode(&envelope.header, &payload, 1_200, &mut bytes).unwrap();
    let wire = Wire {
        lane: Lane::Datagram,
        bytes,
    };
    assert_eq!(
        pair.give_host(&wire),
        Ok(Ingested::Closed(CloseReason::Violation(
            ge_net::Violation::UnownedSlot(PlayerSlot::new(1))
        )))
    );
    assert_eq!(pair.host.diagnostics().commands_accepted, 0);
}

#[test]
fn inbound_work_and_events_are_bounded_per_update() {
    let mut budget = SessionBudget::small_session();
    budget.max_inbound_messages_per_update = 2;
    let mut pair = Pair::new(budget, 0, 43);
    pair.establish(1, empty());
    for tick in 2..6 {
        pair.host
            .publish_snapshot(Tick::new(tick), empty())
            .unwrap();
        pair.flush_host();
    }
    let wires: Vec<Wire> = pair.to_client.drain(..).collect();
    assert!(wires.len() >= 4);
    pair.advance(1_000);
    assert_eq!(pair.client.inbound_credit(), 2);
    assert!(pair.give_client(&wires[0]).is_ok());
    assert!(pair.give_client(&wires[1]).is_ok());
    assert_eq!(pair.client.inbound_credit(), 0);
    // Refused without being consumed: the same message succeeds later.
    assert_eq!(
        pair.give_client(&wires[2]),
        Err(IngestError::InboundBudgetExhausted)
    );
    pair.advance(1_000);
    assert_eq!(pair.give_client(&wires[2]), Ok(Ingested::Accepted));

    // Undrained events stop ingestion until the application polls them.
    let mut budget = SessionBudget::small_session();
    budget.max_pending_events = 1;
    let mut pair = Pair::new(budget, 0, 43);
    pair.flush_client();
    let hello = pair.to_host.pop_front().unwrap();
    assert_eq!(
        pair.host
            .ingest(pair.host_attempt, hello.lane, &hello.bytes),
        Ok(Ingested::Accepted)
    );
    // `PeerAccepted` is waiting; nothing more is taken until it is drained.
    assert_eq!(pair.host.inbound_credit(), 0);
    assert_eq!(
        pair.host.ingest(pair.host_attempt, Lane::Datagram, &[0]),
        Err(IngestError::EventQueueFull)
    );
    assert!(matches!(
        pair.host.poll_event(),
        Some(HostEvent::PeerAccepted { .. })
    ));
    assert!(pair.host.inbound_credit() > 0);
}

#[test]
fn draining_respects_caller_limits_and_transport_backpressure() {
    let mut pair = Pair::new(SessionBudget::small_session(), 0, 44);
    pair.establish(1, empty());
    for tick in 2..4 {
        pair.host
            .publish_snapshot(Tick::new(tick), empty())
            .unwrap();
    }
    pair.advance(600_000);
    // A limit of one message hands over exactly one and reports the rest.
    let mut seen = Vec::new();
    let report = pair.host.drain_outbound(
        DrainLimit {
            max_messages: 1,
            max_bytes: usize::MAX,
        },
        |outbound| {
            if let Outbound::Message { lane, .. } = outbound {
                seen.push(lane);
            }
            SendOutcome::Sent
        },
    );
    assert_eq!((report.messages, report.limited), (1, true));
    assert_eq!(seen.len(), 1);
    // A byte limit smaller than the next message hands over nothing.
    let report = pair.host.drain_outbound(
        DrainLimit {
            max_messages: 10,
            max_bytes: 4,
        },
        |_| SendOutcome::Sent,
    );
    assert_eq!(
        (report.messages, report.bytes, report.limited),
        (0, 0, true)
    );
    // A blocked lane keeps its message queued for the next drain.
    let report = pair
        .host
        .drain_outbound(DrainLimit::UNLIMITED, |_| SendOutcome::Blocked);
    assert_eq!((report.messages, report.blocked), (0, true));
    let report = pair
        .host
        .drain_outbound(DrainLimit::UNLIMITED, |_| SendOutcome::Sent);
    assert!(report.messages >= 1 && !report.blocked && !report.limited);
    assert_eq!(pair.host.phase(pair.host_attempt), Some(Phase::Active));
}

#[test]
fn time_must_not_go_backwards() {
    let mut pair = Pair::new(SessionBudget::small_session(), 0, 45);
    pair.advance(5_000);
    assert_eq!(
        pair.host.advance(NetInstant::from_micros(4_999)),
        Err(TimeRegression)
    );
    assert_eq!(
        pair.client.advance(NetInstant::from_micros(4_999)),
        Err(TimeRegression)
    );
    assert_eq!(pair.host.advance(NetInstant::from_micros(5_000)), Ok(()));
    assert_eq!(NetInstant::from_millis(5).as_micros(), 5_000);
}

#[test]
fn a_transport_that_reports_closed_on_send_ends_the_attempt() {
    let mut harness = active_pair(46);
    harness.run_ticks(5);
    // The host's transport drops client 0 without warning.
    let attempt = harness.net.host_attempt(0).unwrap();
    use ge_net::Transport;
    harness.net.endpoint(Endpoint::Host).close(attempt);
    harness.run_ticks(5);
    assert_eq!(
        harness.clients[0].closed,
        vec![CloseReason::Transport(TransportFailure::RemoteClosed)]
    );
    // The host still believes in the peer until its idle timer says otherwise.
    harness.run_ticks(60 * 11);
    assert_eq!(
        closed_events(&harness),
        vec![(attempt, CloseReason::IdleTimeout)]
    );
    assert_eq!(harness.clients[1].session.phase(), Phase::Active);
}
