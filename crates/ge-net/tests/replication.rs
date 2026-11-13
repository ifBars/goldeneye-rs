//! Acknowledged baselines, deltas, late join, bounded catch-up and recovery.

mod support;

use std::collections::BTreeSet;
use std::time::Duration;

use ge_net::protocol::envelope;
use ge_net::protocol::message::{Message, StateUpdate};
use ge_net::replication::{DeltaError, SnapshotError, apply_delta, encode_delta};
use ge_net::session::host::{HostEvent, PublishError};
use ge_net::{
    AttemptId, ClientEvent, CloseReason, DrainLimit, DropReason, EntityAllocator, EntityId,
    EntityRecord, Ingested, Lane, Outbound, Phase, SendOutcome, SessionBudget, Snapshot,
    SnapshotId, Tick, Violation,
};
use support::harness::{Harness, HarnessConfig};
use support::link::LinkProfile;
use support::pipe::{Pair, Wire};
use support::rng::SeededRng;

fn record(index: u16, generation: u32, state: &[u8]) -> EntityRecord {
    EntityRecord {
        id: EntityId::new(index, generation),
        state: state.to_vec(),
    }
}

fn world(records: Vec<EntityRecord>) -> Snapshot {
    Snapshot::from_records(records).unwrap()
}

/// A snapshot of `count` entities whose state encodes `value`.
fn filled(count: u16, value: u8) -> Snapshot {
    world((0..count).map(|i| record(i, 0, &[value; 16])).collect())
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

fn state_updates(wires: &[Wire]) -> Vec<(SnapshotId, Option<SnapshotId>)> {
    wires
        .iter()
        .filter_map(|wire| match wire.message() {
            Some(Message::StateUpdate(update)) => Some((update.snapshot, update.baseline)),
            _ => None,
        })
        .collect()
}

#[test]
fn snapshots_validate_their_own_structure_and_limits() {
    assert_eq!(
        Snapshot::from_records(vec![record(3, 0, &[]), record(3, 1, &[])]),
        Err(SnapshotError::DuplicateIndex(3))
    );
    let snapshot = world(vec![record(9, 2, &[1, 2]), record(1, 0, &[3])]);
    // Records are canonical: ascending by index whatever the input order.
    let order: Vec<u16> = snapshot.records().iter().map(|r| r.id.index).collect();
    assert_eq!(order, vec![1, 9]);
    assert_eq!(snapshot.len(), 2);
    assert!(!snapshot.is_empty() && Snapshot::empty().is_empty());
    assert_eq!(snapshot.get(EntityId::new(9, 2)), Some(&[1, 2][..]));
    assert_eq!(snapshot.get(EntityId::new(9, 1)), None, "wrong generation");
    assert_eq!(snapshot.at_index(4), None);

    let limits = ge_net::replication::ReplicationLimits {
        max_entities: 10,
        max_entity_bytes: 2,
        max_snapshot_bytes: 64,
    };
    assert_eq!(snapshot.check(&limits), Ok(snapshot.encoded_len()));
    let cases = [
        (
            world((0..11).map(|i| record(i, 0, &[])).collect()),
            SnapshotError::TooManyEntities { count: 11, max: 10 },
        ),
        (
            world(vec![record(10, 0, &[])]),
            SnapshotError::IndexOutOfRange(10),
        ),
        (
            world(vec![record(1, 0, &[0; 3])]),
            SnapshotError::EntityTooLarge {
                id: EntityId::new(1, 0),
                len: 3,
                max: 2,
            },
        ),
        (
            world((0..7).map(|i| record(i, 0, &[0; 2])).collect()),
            SnapshotError::SnapshotTooLarge { bytes: 81, max: 64 },
        ),
    ];
    for (snapshot, expected) in cases {
        assert_eq!(snapshot.check(&limits), Err(expected));
        assert!(!expected.to_string().is_empty());
    }
}

#[test]
fn random_snapshot_pairs_round_trip_through_deltas() {
    let limits = ge_net::replication::ReplicationLimits {
        max_entities: 64,
        max_entity_bytes: 32,
        max_snapshot_bytes: 8_192,
    };
    let mut rng = SeededRng::new(0xde17a);
    let mut allocator = EntityAllocator::new(64);
    let mut live: Vec<(EntityId, Vec<u8>)> = Vec::new();
    let mut previous = Snapshot::empty();
    for round in 0..400 {
        // Destroy, create and mutate a random subset.
        for _ in 0..rng.below(4) {
            if !live.is_empty() {
                let at = rng.below(live.len() as u64) as usize;
                let (id, _) = live.swap_remove(at);
                allocator.free(id).unwrap();
                assert!(!allocator.is_live(id));
            }
        }
        for _ in 0..rng.below(5) {
            if let Ok(id) = allocator.allocate() {
                let mut state = vec![0; rng.below(33) as usize];
                rng.fill(&mut state);
                live.push((id, state));
            }
        }
        for (_, state) in &mut live {
            if rng.chance(300) {
                state.resize(rng.below(33) as usize, 0);
                rng.fill(state);
            }
        }
        assert_eq!(allocator.live_count(), live.len());
        let next = world(
            live.iter()
                .map(|(id, state)| EntityRecord {
                    id: *id,
                    state: state.clone(),
                })
                .collect(),
        );
        let mut delta = Vec::new();
        encode_delta(Some(&previous), &next, &mut delta);
        assert_eq!(
            apply_delta(Some(&previous), &delta, &limits).as_ref(),
            Ok(&next),
            "round {round}"
        );
        // The same delta against the wrong baseline never yields `next`
        // silently: it either fails or the baselines were equivalent.
        if let Ok(other) = apply_delta(None, &delta, &limits) {
            assert_eq!(other, next, "round {round}: delta carried the whole state");
        }
        let mut full = Vec::new();
        encode_delta(None, &next, &mut full);
        assert_eq!(apply_delta(None, &full, &limits).as_ref(), Ok(&next));
        previous = next;
    }
    assert_eq!(allocator.capacity(), 64);
}

#[test]
fn entity_identities_are_never_reissued() {
    let mut allocator = EntityAllocator::new(2);
    let first = allocator.allocate().unwrap();
    let second = allocator.allocate().unwrap();
    assert_eq!(
        allocator.allocate(),
        Err(ge_net::identity::EntityAllocError::Exhausted)
    );
    allocator.free(first).unwrap();
    assert_eq!(
        allocator.free(first),
        Err(ge_net::identity::EntityAllocError::NotLive(first))
    );
    let replacement = allocator.allocate().unwrap();
    assert_eq!(replacement.index, first.index);
    assert_eq!(replacement.generation, first.generation + 1);
    assert!(allocator.is_live(replacement) && allocator.is_live(second));
    assert!(!allocator.is_live(first), "the old generation stays dead");
    assert_eq!(first.to_string(), "entity#0.0");
}

#[test]
fn a_lost_update_never_blocks_the_next_one() {
    let mut pair = Pair::new(SessionBudget::small_session(), 0, 50);
    pair.establish(1, filled(4, 1));
    // The update for tick 2 is lost on the wire.
    pair.host
        .publish_snapshot(Tick::new(2), filled(4, 2))
        .unwrap();
    pair.advance(16_666);
    pair.flush_host();
    assert_eq!(state_updates(pair.to_client.make_contiguous()).len(), 1);
    pair.to_client.clear();
    // Tick 3 is a delta against the acknowledged baseline, not against the
    // lost update, so it applies on its own.
    pair.publish(3, filled(4, 3));
    let (_, tick, state) = pair.applied.clone().unwrap();
    assert_eq!(tick, Tick::new(3));
    assert_eq!(*state, filled(4, 3));
    assert_eq!(pair.client.diagnostics().recovery_requests, 0);
    assert_eq!(
        pair.client.diagnostics().drops(DropReason::MissingBaseline),
        0
    );
}

#[test]
fn the_host_only_references_baselines_the_client_acknowledged() {
    let mut pair = Pair::new(SessionBudget::small_session(), 0, 51);
    let sync = pair.establish(1, filled(8, 0));
    let mut rng = SeededRng::new(51);
    let mut acknowledged: BTreeSet<u32> = BTreeSet::from([sync.get()]);
    let mut referenced = 0;
    for tick in 2..400u64 {
        pair.host
            .publish_snapshot(Tick::new(tick), filled(8, tick as u8))
            .unwrap();
        pair.advance(16_666);
        pair.flush_host();
        pair.flush_client();
        for (_, baseline) in state_updates(pair.to_client.make_contiguous()) {
            let baseline = baseline.expect("an active peer has a baseline");
            assert!(
                acknowledged.contains(&baseline.get()),
                "tick {tick}: {baseline} was never acknowledged"
            );
            referenced += 1;
        }
        // A third of the traffic is lost in each direction.
        pair.to_client.retain(|_| !rng.chance(333));
        pair.to_host.retain(|_| !rng.chance(333));
        for wire in &pair.to_host {
            match wire.message() {
                Some(Message::StateAck(id)) => acknowledged.insert(id.get()),
                Some(Message::InputBatch(batch)) => {
                    batch.ack.is_some_and(|id| acknowledged.insert(id.get()))
                }
                _ => false,
            };
        }
        pair.deliver_to_client();
        pair.deliver_to_host();
    }
    assert!(referenced > 300);
    assert!(acknowledged.len() > 100, "baselines kept advancing");
    assert_eq!(pair.client.phase(), Phase::Active);
    assert_eq!(pair.client.diagnostics().recovery_requests, 0);
}

/// Host and client budgets that differ only in baseline history.
fn uneven_history(host: u8, client: u8) -> (SessionBudget, SessionBudget) {
    let mut host_budget = SessionBudget::small_session();
    host_budget.baseline_history = host;
    let mut client_budget = SessionBudget::small_session();
    client_budget.baseline_history = client;
    (host_budget, client_budget)
}

/// Publish a tick and deliver only host-to-client traffic.
fn publish_one_way(pair: &mut Pair, tick: u64, snapshot: Snapshot) {
    pair.host
        .publish_snapshot(Tick::new(tick), snapshot)
        .unwrap();
    pair.flush_host();
    pair.deliver_to_client();
    // The client's periodic acknowledgement now names this snapshot.
    pair.advance(16_666);
    pair.flush_client();
}

#[test]
fn an_evicted_baseline_recovers_from_a_snapshot_the_client_still_holds() {
    // The client keeps fewer snapshots than the host believes.
    let (host_budget, client_budget) = uneven_history(16, 2);
    let mut pair = Pair::with_budgets(host_budget, client_budget, 0, 52);
    pair.establish(1, filled(4, 1));

    publish_one_way(&mut pair, 2, filled(4, 2));
    // The acknowledgement of tick 2 is delayed in the network.
    let delayed: Vec<Wire> = pair.to_host.drain(..).collect();
    for tick in 3..7 {
        publish_one_way(&mut pair, tick, filled(4, tick as u8));
        pair.to_host.clear();
    }
    // It arrives now. The host adopts tick 2 as the baseline, which the
    // client has since evicted.
    for wire in &delayed {
        pair.give_host(wire).unwrap();
    }
    pair.host
        .publish_snapshot(Tick::new(7), filled(4, 7))
        .unwrap();
    pair.advance(16_666);
    pair.flush_host();
    pair.deliver_to_client();
    assert!(
        pair.last_client_ingest
            .contains(&Ok(Ingested::Dropped(DropReason::MissingBaseline))),
        "{:?}",
        pair.last_client_ingest
    );
    assert_eq!(
        pair.applied.as_ref().unwrap().1,
        Tick::new(6),
        "not applied"
    );
    assert_eq!(pair.client.diagnostics().recovery_requests, 1);

    // One bounded request; the host still has what the client holds, so the
    // repair is a delta rather than a full transfer.
    pair.settle();
    assert_eq!(pair.host.diagnostics().recovery_requests, 1);
    assert_eq!(
        pair.host.diagnostics().full_transfers_started,
        1,
        "only the join"
    );
    let (_, tick, state) = pair.applied.clone().unwrap();
    assert_eq!((tick, &*state), (Tick::new(7), &filled(4, 7)));
    pair.publish(8, filled(4, 8));
    assert_eq!(pair.applied.as_ref().unwrap().1, Tick::new(8));
    assert_eq!(pair.client.diagnostics().recovery_requests, 1, "no repeat");
    assert_eq!(pair.client.phase(), Phase::Active);
}

/// Reach the state where the host's baseline and the client's newest
/// snapshot are both gone from the other side. The client is left at tick 4
/// having just refused the update for tick 8.
fn lose_every_shared_baseline(pair: &mut Pair, make: impl Fn(u8) -> Snapshot) {
    pair.establish(1, make(1));
    publish_one_way(pair, 2, make(2));
    // The acknowledgement of tick 2 is delayed in the network.
    let delayed: Vec<Wire> = pair.to_host.drain(..).collect();
    for tick in 3..5 {
        publish_one_way(pair, tick, make(tick as u8));
        pair.to_host.clear();
    }
    // It arrives after the client evicted tick 2.
    for wire in &delayed {
        pair.give_host(wire).unwrap();
    }
    // Ticks 5 to 7 are lost on the way to the client, and by then the host
    // has evicted everything the client holds.
    for tick in 5..8 {
        pair.host
            .publish_snapshot(Tick::new(tick), make(tick as u8))
            .unwrap();
        pair.advance(16_666);
        pair.flush_host();
        pair.to_client.clear();
    }
    pair.host.publish_snapshot(Tick::new(8), make(8)).unwrap();
    pair.advance(16_666);
    pair.flush_host();
    pair.deliver_to_client();
    assert!(
        pair.last_client_ingest
            .contains(&Ok(Ingested::Dropped(DropReason::MissingBaseline))),
        "{:?}",
        pair.last_client_ingest
    );
    assert_eq!(pair.applied.as_ref().unwrap().1, Tick::new(4));
}

#[test]
fn a_dropped_baseline_recovers_with_one_full_state_in_datagrams() {
    let (host_budget, client_budget) = uneven_history(3, 2);
    let mut pair = Pair::with_budgets(host_budget, client_budget, 0, 53);
    lose_every_shared_baseline(&mut pair, |value| filled(4, value));
    pair.settle();
    // Small enough for the datagram lane: no bulk transfer was needed.
    assert_eq!(
        pair.host.diagnostics().full_transfers_started,
        1,
        "only the join"
    );
    assert_eq!(pair.host.diagnostics().recovery_requests, 1);
    for tick in 9..14 {
        pair.publish(tick, filled(4, tick as u8));
    }
    let (_, tick, state) = pair.applied.clone().unwrap();
    assert_eq!((tick, &*state), (Tick::new(13), &filled(4, 13)));
    assert_eq!(pair.client.diagnostics().recovery_requests, 1);
    assert_eq!(pair.client.phase(), Phase::Active);
    // Recovery replaced the baseline: the next update is a delta again.
    pair.host
        .publish_snapshot(Tick::new(14), filled(4, 14))
        .unwrap();
    pair.advance(16_666);
    pair.flush_host();
    let updates = state_updates(pair.to_client.make_contiguous());
    assert!(matches!(updates[..], [(_, Some(_))]), "{updates:?}");
}

#[test]
fn a_dropped_baseline_recovers_with_a_bounded_bulk_transfer() {
    let (mut host_budget, mut client_budget) = uneven_history(3, 2);
    // One datagram per update: a full state of 200 entities cannot fit.
    host_budget.max_update_fragments = 1;
    client_budget.max_update_fragments = 1;
    let mut pair = Pair::with_budgets(host_budget, client_budget, 0, 54);
    // Only a few entities change per tick, so ordinary deltas still fit.
    let make = |value: u8| {
        world(
            (0..200)
                .map(|i| record(i, 0, &[if i < 4 { value } else { 0 }; 16]))
                .collect(),
        )
    };
    lose_every_shared_baseline(&mut pair, make);
    // The bulk queue is kept shallow, so the transfer spans several updates.
    for tick in 9..24 {
        pair.publish(tick, make(tick as u8));
    }
    assert_eq!(pair.host.diagnostics().recovery_requests, 1);
    assert_eq!(
        pair.host.diagnostics().full_transfers_started,
        2,
        "join and recovery"
    );
    assert_eq!(pair.host.diagnostics().full_transfers_completed, 2);
    assert_eq!(pair.client.diagnostics().full_transfers_completed, 2);
    let (_, tick, state) = pair.applied.clone().unwrap();
    assert_eq!(tick, Tick::new(23));
    assert!(*state == make(23), "the recovered state matches the host");
    assert_eq!(pair.client.diagnostics().recovery_requests, 1);
    assert_eq!(pair.client.phase(), Phase::Active);
    assert_eq!(pair.host.phase(pair.host_attempt), Some(Phase::Active));
}

#[test]
fn a_recovery_that_cannot_complete_closes_the_peer_explicitly() {
    let (mut host_budget, mut client_budget) = uneven_history(3, 2);
    host_budget.max_update_fragments = 1;
    client_budget.max_update_fragments = 1;
    host_budget.sync_timeout = Duration::from_secs(2);
    let mut pair = Pair::with_budgets(host_budget, client_budget, 0, 55);
    let make = |value: u8| {
        world(
            (0..200)
                .map(|i| record(i, 0, &[if i < 4 { value } else { 0 }; 16]))
                .collect(),
        )
    };
    lose_every_shared_baseline(&mut pair, make);
    pair.flush_client();
    pair.deliver_to_host();
    assert_eq!(pair.host.diagnostics().full_transfers_started, 2);

    // The bulk lane is stuck: the transfer never reaches the client. The
    // peer stays alive on the other lanes, so only the recovery bound can
    // end this.
    let mut queued_peak = 0;
    for step in 0..200 {
        pair.host
            .publish_snapshot(Tick::new(9 + step), make(step as u8))
            .unwrap();
        pair.advance(16_666);
        let wires = &mut pair.to_client;
        pair.host
            .drain_outbound(DrainLimit::UNLIMITED, |outbound| match outbound {
                Outbound::Message {
                    lane: Lane::Bulk, ..
                } => SendOutcome::Blocked,
                Outbound::Message { lane, bytes, .. } => {
                    wires.push_back(Wire {
                        lane,
                        bytes: bytes.to_vec(),
                    });
                    SendOutcome::Sent
                }
                Outbound::Disconnect { .. } => SendOutcome::Sent,
            });
        if let Some(peer) = pair.host.peer_diagnostics(pair.host_attempt) {
            queued_peak = queued_peak.max(peer.queued_bytes.iter().sum::<usize>());
        }
        pair.deliver_to_client();
        pair.flush_client();
        pair.deliver_to_host();
        if pair.host.phase(pair.host_attempt).is_none() {
            break;
        }
    }
    assert!(
        queued_peak < 16 * 1024,
        "the stuck transfer queued {queued_peak} bytes"
    );
    assert!(matches!(
        pair.host_events.last(),
        Some(HostEvent::PeerClosed {
            reason: CloseReason::SyncTimeout,
            ..
        })
    ));
    assert!(
        pair.now < 4_000_000,
        "closed within the bound, at {} µs",
        pair.now
    );
    assert_eq!(pair.client.phase(), Phase::Closed);
    assert!(matches!(
        pair.client_events.last(),
        Some(ClientEvent::Closed {
            reason: CloseReason::RemoteClosed(5)
        })
    ));
}

#[test]
fn an_update_for_an_unknown_baseline_is_never_applied() {
    let mut pair = Pair::new(SessionBudget::small_session(), 0, 56);
    pair.establish(1, world(vec![record(0, 0, &[1])]));
    pair.host
        .publish_snapshot(Tick::new(2), world(vec![record(0, 0, &[2])]))
        .unwrap();
    pair.advance(16_666);
    pair.flush_host();
    let genuine = pair
        .to_client
        .iter()
        .find(|wire| matches!(wire.message(), Some(Message::StateUpdate(_))))
        .cloned()
        .unwrap();
    pair.to_client.clear();
    let envelope = genuine.envelope();
    let Some(Message::StateUpdate(update)) = genuine.message() else {
        unreachable!()
    };
    // An update of entity 5 arrives before anything that creates it: it
    // names a baseline the client was never given.
    let delta = [0, 0, 0, 1, 1, 0, 5, 0, 0, 0, 0, 0, 1, 9];
    let early = Message::StateUpdate(StateUpdate {
        snapshot: update.snapshot.offset(5),
        baseline: Some(update.snapshot.offset(4)),
        acks: update.acks.clone(),
        delta: &delta,
    });
    let mut payload = Vec::new();
    early.encode_payload(&mut payload).unwrap();
    let mut header = envelope.header;
    header.sequence = header.sequence.offset(5);
    let mut bytes = Vec::new();
    envelope::encode(&header, &payload, 1_200, &mut bytes).unwrap();
    let wire = Wire {
        lane: Lane::Datagram,
        bytes,
    };
    assert_eq!(
        pair.give_client(&wire),
        Ok(Ingested::Dropped(DropReason::MissingBaseline))
    );
    let (_, tick, state) = pair.applied.clone().unwrap();
    assert_eq!(tick, Tick::new(1));
    assert_eq!(
        state.at_index(5),
        None,
        "nothing was created from the update"
    );
    assert_eq!(pair.client.phase(), Phase::Active);
    assert_eq!(pair.client.diagnostics().recovery_requests, 1);
    // A second one inside the retry interval does not ask again.
    let mut header = envelope.header;
    header.sequence = header.sequence.offset(6);
    let mut bytes = Vec::new();
    envelope::encode(&header, &payload, 1_200, &mut bytes).unwrap();
    assert_eq!(
        pair.give_client(&Wire {
            lane: Lane::Datagram,
            bytes
        }),
        Ok(Ingested::Dropped(DropReason::MissingBaseline))
    );
    assert_eq!(
        pair.client.diagnostics().recovery_requests,
        1,
        "rate limited"
    );
    assert_eq!(pair.applied.as_ref().unwrap().1, Tick::new(1));
}

#[test]
fn a_delayed_update_cannot_resurrect_a_destroyed_entity() {
    let mut pair = Pair::new(SessionBudget::small_session(), 0, 57);
    pair.establish(1, world(vec![record(0, 0, &[1])]));

    // Tick 2: entity 3.0 exists. Its update is delayed in the network.
    let alive = world(vec![record(0, 0, &[1]), record(3, 0, &[7])]);
    pair.host.publish_snapshot(Tick::new(2), alive).unwrap();
    pair.advance(16_666);
    pair.flush_host();
    let delayed: Vec<Wire> = pair.to_client.drain(..).collect();
    assert_eq!(state_updates(&delayed).len(), 1);

    // Tick 3 destroys it; tick 4 reuses index 3 with a new generation.
    pair.publish(3, world(vec![record(0, 0, &[1])]));
    pair.publish(4, world(vec![record(0, 0, &[1]), record(3, 1, &[9])]));
    let (_, tick, state) = pair.applied.clone().unwrap();
    assert_eq!(tick, Tick::new(4));
    assert_eq!(state.get(EntityId::new(3, 1)), Some(&[9][..]));

    // The delayed update arrives after the destruction and the replacement.
    for wire in &delayed {
        assert_eq!(
            pair.give_client(wire),
            Ok(Ingested::Dropped(DropReason::Stale))
        );
    }
    let (_, tick, state) = pair.applied.clone().unwrap();
    assert_eq!(tick, Tick::new(4));
    assert_eq!(
        state.get(EntityId::new(3, 0)),
        None,
        "the old generation stays dead"
    );
    assert_eq!(state.get(EntityId::new(3, 1)), Some(&[9][..]));
    assert_eq!(pair.client.phase(), Phase::Active);
}

#[test]
fn a_retired_generation_is_rejected_even_in_a_newer_snapshot() {
    // The authority itself may not reuse a retired generation.
    let mut pair = Pair::new(SessionBudget::small_session(), 0, 58);
    pair.establish(1, world(vec![record(3, 4, &[1])]));
    pair.publish(2, world(vec![]));
    assert_eq!(
        pair.host
            .publish_snapshot(Tick::new(3), world(vec![record(3, 4, &[1])])),
        Err(PublishError::Snapshot(SnapshotError::GenerationRegression(
            EntityId::new(3, 4)
        )))
    );
    assert_eq!(
        pair.host
            .publish_snapshot(Tick::new(3), world(vec![record(3, 2, &[1])])),
        Err(PublishError::Snapshot(SnapshotError::GenerationRegression(
            EntityId::new(3, 2)
        )))
    );
    // The refused snapshots consumed no tick: tick 3 is still available.
    pair.publish(3, world(vec![record(3, 5, &[1])]));
    assert_eq!(pair.applied.as_ref().unwrap().1, Tick::new(3));
    assert_eq!(
        pair.host.publish_snapshot(Tick::new(3), world(vec![])),
        Err(PublishError::TickNotIncreasing { last: Tick::new(3) })
    );

    // A client holds the same floor against a misbehaving authority. Forge
    // a well-formed newer update that recreates the retired generation.
    pair.host
        .publish_snapshot(Tick::new(4), world(vec![]))
        .unwrap();
    pair.advance(16_666);
    pair.flush_host();
    let genuine = pair
        .to_client
        .iter()
        .find(|wire| matches!(wire.message(), Some(Message::StateUpdate(_))))
        .cloned()
        .unwrap();
    pair.deliver_to_client();
    let envelope = genuine.envelope();
    let Some(Message::StateUpdate(update)) = genuine.message() else {
        unreachable!()
    };
    let delta = [0, 0, 0, 1, 0, 0, 3, 0, 0, 0, 5, 0, 1, 1];
    let forged = Message::StateUpdate(StateUpdate {
        snapshot: update.snapshot.next(),
        baseline: Some(update.snapshot),
        acks: update.acks.clone(),
        delta: &delta,
    });
    let mut payload = Vec::new();
    forged.encode_payload(&mut payload).unwrap();
    let mut header = envelope.header;
    header.sequence = header.sequence.next();
    header.tick = Some(Tick::new(5));
    let mut bytes = Vec::new();
    envelope::encode(&header, &payload, 1_200, &mut bytes).unwrap();
    assert_eq!(
        pair.give_client(&Wire {
            lane: Lane::Datagram,
            bytes
        }),
        Ok(Ingested::Closed(CloseReason::Violation(
            Violation::StaleGeneration(EntityId::new(3, 5))
        )))
    );
}

#[test]
fn an_inconsistent_delta_closes_the_connection() {
    let mut pair = Pair::new(SessionBudget::small_session(), 0, 59);
    pair.establish(1, world(vec![record(0, 0, &[1])]));
    pair.host
        .publish_snapshot(Tick::new(2), world(vec![record(0, 0, &[2])]))
        .unwrap();
    pair.advance(16_666);
    pair.flush_host();
    let genuine = pair
        .to_client
        .iter()
        .find(|wire| matches!(wire.message(), Some(Message::StateUpdate(_))))
        .cloned()
        .unwrap();
    pair.to_client.clear();
    let envelope = genuine.envelope();
    let Some(Message::StateUpdate(update)) = genuine.message() else {
        unreachable!()
    };
    // Against the acknowledged baseline, destroy an entity it never held.
    let delta = [0, 1, 0, 9, 0, 0, 0, 0, 0, 0];
    let forged = Message::StateUpdate(StateUpdate {
        delta: &delta,
        ..update.clone()
    });
    let mut payload = Vec::new();
    forged.encode_payload(&mut payload).unwrap();
    let mut bytes = Vec::new();
    envelope::encode(&envelope.header, &payload, 1_200, &mut bytes).unwrap();
    assert_eq!(
        pair.give_client(&Wire {
            lane: Lane::Datagram,
            bytes
        }),
        Ok(Ingested::Closed(CloseReason::Violation(Violation::Delta(
            DeltaError::DestroyMissing(EntityId::new(9, 0))
        ))))
    );
    assert_eq!(
        pair.applied.as_ref().unwrap().1,
        Tick::new(1),
        "nothing applied"
    );
}

#[test]
fn a_late_joiner_synchronizes_while_the_others_keep_playing() {
    let seed = 60;
    let mut config = HarnessConfig::new(seed, 3);
    config.drifters = 200;
    // A slow bulk lane: the join takes about a second of transfer.
    config.budget.bulk_bytes_per_second = 3_000;
    config.budget.bulk_burst_bytes = 1_100;
    let mut harness = Harness::new(config);
    harness.connect(0);
    harness.connect(1);
    assert!(harness.run_until(600, |h| {
        h.clients[..2]
            .iter()
            .all(|c| c.session.phase() == Phase::Active)
    }));
    harness.run_ticks(120);
    let published_before = harness.host.diagnostics().snapshots_published;

    harness.connect(2);
    let mut joined_at = None;
    let mut worst_lag = 0;
    let mut progress_seen = false;
    for tick in 0..600 {
        harness.run_ticks(1);
        for client in 0..2 {
            worst_lag = worst_lag.max(harness.lag_ticks(client).unwrap());
        }
        if let Some(attempt) = harness.net.host_attempt(2)
            && let Some(peer) = harness.host.peer_diagnostics(attempt)
            && let Some(progress) = peer.sync_progress
        {
            assert!(progress.transferred_bytes <= progress.total_bytes);
            progress_seen = true;
        }
        if joined_at.is_none() && harness.clients[2].session.phase() == Phase::Active {
            joined_at = Some(tick);
        }
    }
    let joined_at = joined_at.expect("the late joiner became active");
    assert!(
        joined_at > 30,
        "the transfer was rate limited: {joined_at} ticks"
    );
    assert!(progress_seen, "synchronization progress was observable");
    assert!(
        worst_lag <= 1,
        "seed {seed}: active clients lagged {worst_lag} ticks"
    );
    harness.assert_consistent();
    assert!(harness.lag_ticks(2).unwrap() <= 1);
    // The host was never restarted or paused for the joiner.
    assert_eq!(
        harness.host.diagnostics().snapshots_published,
        published_before + 600
    );
    assert!(closed_events(&harness).is_empty());
    assert_eq!(harness.owners.len(), 3);
}

#[test]
fn a_join_that_cannot_catch_up_in_time_is_closed_without_harming_others() {
    let seed = 61;
    let mut config = HarnessConfig::new(seed, 2);
    config.drifters = 900;
    config.world_capacity = 1_024;
    // Twenty kilobytes at two kilobytes a second cannot finish in three.
    config.budget.bulk_bytes_per_second = 2_000;
    config.budget.bulk_burst_bytes = 1_100;
    config.budget.sync_timeout = Duration::from_secs(3);
    // Per-tick deltas of 900 actors are far beyond the datagram budget, so
    // every peer depends on bulk transfers here.
    let mut harness = Harness::new(config);
    harness.connect(0);
    harness.run_ticks(60 * 5);
    let attempt = harness.net.host_attempt(0).unwrap();
    assert_eq!(
        closed_events(&harness),
        vec![(attempt, CloseReason::SyncTimeout)]
    );
    assert_eq!(harness.clients[0].session.phase(), Phase::Closed);
    assert!(harness.clients[0].activated.is_empty());
    assert!(harness.host.attempts().is_empty());
    // The session itself is intact: a later attempt is admitted.
    harness.connect(1);
    harness.run_ticks(30);
    assert_eq!(
        harness.host.phase(harness.net.host_attempt(1).unwrap()),
        Some(Phase::Synchronizing)
    );
}

#[test]
fn oversized_deltas_fall_back_to_repeated_full_transfers() {
    let seed = 62;
    let mut config = HarnessConfig::new(seed, 2);
    config.drifters = 100;
    config.budget.max_update_fragments = 1;
    let mut harness = Harness::new(config);
    harness.connect_all();
    assert!(harness.run_until(600, Harness::all_active));
    harness.run_ticks(600);
    harness.assert_consistent();
    let host = harness.host.diagnostics();
    assert!(
        host.full_transfers_started > 100,
        "{}",
        host.full_transfers_started
    );
    assert_eq!(host.state_updates_sent, 0, "no delta ever fit one datagram");
    for client in 0..2 {
        assert!(harness.clients[client].closed.is_empty());
        let lag = harness.lag_ticks(client).unwrap();
        assert!(lag <= 6, "seed {seed}: client {client} lags {lag} ticks");
    }
}

#[test]
fn control_messages_are_not_stuck_behind_a_bulk_transfer() {
    let seed = 63;
    let mut config = HarnessConfig::new(seed, 2);
    config.drifters = 900;
    config.world_capacity = 1_024;
    config.budget.bulk_bytes_per_second = 64 * 1024;
    // A narrow link: twenty kilobytes of snapshot take five seconds.
    config.profile = LinkProfile {
        latency_micros: 10_000,
        bandwidth_bytes_per_second: Some(4_000),
        queue_bytes: 3_000,
        ..LinkProfile::ideal()
    };
    let mut harness = Harness::new(config);
    harness.connect(0);
    harness.run_ticks(90);
    let attempt = harness.net.host_attempt(0).unwrap();
    assert_eq!(harness.host.phase(attempt), Some(Phase::Synchronizing));
    let progress = harness
        .host
        .peer_diagnostics(attempt)
        .unwrap()
        .sync_progress;
    assert!(progress.is_some_and(|p| p.transferred_bytes < p.total_bytes));

    // A control message sent now arrives long before the transfer could.
    harness.host.close_peer(attempt).unwrap();
    let before = harness.tick;
    assert!(harness.run_until(120, |h| !h.clients[0].closed.is_empty()));
    assert_eq!(
        harness.clients[0].closed,
        vec![CloseReason::RemoteClosed(1)]
    );
    assert!(
        harness.tick - before < 90,
        "close took {} ticks",
        harness.tick - before
    );
    assert_eq!(
        closed_events(&harness),
        vec![(attempt, CloseReason::Requested)]
    );
    assert_eq!(
        harness.host.close_peer(attempt),
        Err(ge_net::session::StateError::StaleAttempt)
    );
}

#[test]
fn a_partitioned_client_rejoins_as_a_new_attempt_and_converges() {
    let seed = 64;
    let mut config = HarnessConfig::new(seed, 2);
    config.profile = LinkProfile::with_rtt_and_loss(60, 20);
    config.command_lead_ticks = 8;
    let mut harness = Harness::new(config);
    harness.connect_all();
    assert!(harness.run_until(600, Harness::all_active));
    harness.run_ticks(120);
    let first = harness.net.host_attempt(0).unwrap();
    let (first_peer, _) = harness.clients[0].accepted.clone().unwrap();

    // A partition that outlasts every timer ends the first membership.
    harness
        .net
        .partition(0, harness.now, harness.now + 20_000_000);
    harness.run_ticks(60 * 12);
    assert_eq!(harness.clients[0].session.phase(), Phase::Closed);
    assert_eq!(closed_events(&harness).len(), 1);
    assert_eq!(harness.host.phase(first), None);

    // After the network heals, the client starts over: a new attempt, a new
    // epoch and peer, a fresh full snapshot. Nothing is resumed.
    harness.run_ticks(60 * 10);
    harness.connect(0);
    assert!(
        harness.run_until(600, Harness::all_active),
        "seed {seed}: rejoined"
    );
    harness.run_ticks(300);
    harness.assert_consistent();
    let second = harness.net.host_attempt(0).unwrap();
    let (second_peer, _) = harness.clients[0].accepted.clone().unwrap();
    assert_ne!(second, first);
    assert_ne!(second_peer, first_peer);
    assert_eq!(harness.clients[0].activated.len(), 2);
    assert_eq!(
        harness.clients[1].activated.len(),
        1,
        "the other client never left"
    );
    for client in 0..2 {
        assert!(harness.lag_ticks(client).unwrap() <= 6);
    }
    assert_eq!(harness.host.diagnostics().full_transfers_started, 3);
}

#[test]
fn a_blocked_datagram_lane_stays_bounded_and_recovers() {
    let mut budget = SessionBudget::small_session();
    budget.max_queued_bytes_per_peer = 8 * 1024;
    let mut pair = Pair::new(budget, 0, 65);
    pair.establish(1, filled(100, 0));
    // The transport refuses every datagram for five seconds.
    for step in 0..300u64 {
        pair.host
            .publish_snapshot(Tick::new(2 + step), filled(100, step as u8))
            .unwrap();
        pair.advance(16_666);
        pair.host
            .drain_outbound(DrainLimit::UNLIMITED, |outbound| match outbound {
                Outbound::Message {
                    lane: Lane::Datagram,
                    ..
                } => SendOutcome::Blocked,
                _ => SendOutcome::Sent,
            });
        let peer = pair.host.peer_diagnostics(pair.host_attempt).unwrap();
        let queued: usize = peer.queued_bytes.iter().sum();
        assert!(queued <= 8 * 1024, "step {step}: {queued} bytes queued");
        // Keep the peer alive: its own probes still arrive.
        pair.flush_client();
        pair.deliver_to_host();
    }
    let diagnostics = pair.host.diagnostics();
    assert!(
        diagnostics.drops(DropReason::Replaced) > 200,
        "stale updates were replaced"
    );
    assert!(diagnostics.queue_high_water_bytes <= 8 * 1024);
    assert_eq!(pair.host.phase(pair.host_attempt), Some(Phase::Active));
    // The lane opens again and the newest state gets through.
    pair.publish(400, filled(100, 77));
    pair.publish(401, filled(100, 78));
    let (_, tick, state) = pair.applied.clone().unwrap();
    assert_eq!((tick, &*state), (Tick::new(401), &filled(100, 78)));
}
