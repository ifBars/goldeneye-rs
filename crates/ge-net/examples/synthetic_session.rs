//! A host and several clients running the synthetic simulation over the
//! seeded link simulator, in one process, with measured costs.
//!
//! ```text
//! cargo run --release -p ge-net --example synthetic_session -- \
//!     --seed 7 --clients 2 --ticks 1800 --loss-per-mille 50 --rtt-ms 150
//! ```
//!
//! It needs no game image, renderer, socket or async runtime. The numbers it
//! prints describe this scenario, seed and build only. They are not a claim
//! about real-network performance or about any game.

#[path = "../tests/support/mod.rs"]
mod support;

use std::alloc::{GlobalAlloc, Layout, System};
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use ge_net::protocol::envelope::{self, Header};
use ge_net::protocol::message::{CodecLimits, Message, MessageKind, StateUpdate, decode_payload};
use ge_net::replication::{ReplicationLimits, apply_delta, encode_delta};
use ge_net::{ChannelId, DropReason, Phase, SessionEpoch, SnapshotId, Tick};
use support::harness::{Harness, HarnessConfig};
use support::link::LinkProfile;
use support::simulation::World;

/// Counts allocations so the report can state them per simulated tick.
struct CountingAllocator;

static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static ALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);

// SAFETY: every call is forwarded unchanged to the system allocator; the
// counters are only observed, never used to decide an allocation.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        ALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        // SAFETY: the caller upholds `GlobalAlloc::alloc`'s contract.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` came from `System.alloc` with this layout.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

struct Options {
    seed: u64,
    clients: usize,
    ticks: u64,
    loss_per_mille: u32,
    rtt_millis: u64,
    drifters: usize,
}

fn parse() -> Result<Options, String> {
    let mut options = Options {
        seed: 7,
        clients: 2,
        ticks: 1_800,
        loss_per_mille: 50,
        rtt_millis: 150,
        drifters: 16,
    };
    let mut arguments = std::env::args().skip(1);
    while let Some(flag) = arguments.next() {
        let value = arguments
            .next()
            .ok_or_else(|| format!("{flag} needs a value"))?;
        let number: u64 = value
            .parse()
            .map_err(|_| format!("{flag}: `{value}` is not a number"))?;
        match flag.as_str() {
            "--seed" => options.seed = number,
            "--clients" => options.clients = number as usize,
            "--ticks" => options.ticks = number,
            "--loss-per-mille" => options.loss_per_mille = number as u32,
            "--rtt-ms" => options.rtt_millis = number,
            "--drifters" => options.drifters = number as usize,
            other => return Err(format!("unknown option {other}")),
        }
    }
    if !(1..=8).contains(&options.clients) {
        return Err("--clients must be between 1 and 8".into());
    }
    Ok(options)
}

fn main() -> ExitCode {
    let options = match parse() {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::FAILURE;
        }
    };

    let mut config = HarnessConfig::new(options.seed, options.clients);
    config.profile = LinkProfile::with_rtt_and_loss(options.rtt_millis, options.loss_per_mille);
    config.drifters = options.drifters;
    config.command_lead_ticks = 4 + options.rtt_millis * 1_000 / config.tick_micros;
    let tick_micros = config.tick_micros;
    let mut harness = Harness::new(config);

    println!("ge-net synthetic session");
    println!(
        "  scenario: seed {}, {} clients, {} ticks at {} µs per tick (a 60 Hz test profile)",
        options.seed, options.clients, options.ticks, tick_micros
    );
    println!(
        "  link: {} ms RTT, {}‰ datagram loss, {} unowned actors changing every tick",
        options.rtt_millis, options.loss_per_mille, options.drifters
    );
    println!(
        "  build: {} profile",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    );

    harness.connect_all();
    if !harness.run_until(1_200, Harness::all_active) {
        eprintln!("clients did not become active within 1200 ticks");
        return ExitCode::FAILURE;
    }
    let joined_tick = harness.tick;
    // Bandwidth covers only the measured interval, not the handshake and
    // initial synchronization that happened before it.
    let channels_before = ChannelId::ALL.map(|channel| harness.host.diagnostics().channel(channel));
    let allocations_before = ALLOCATIONS.load(Ordering::Relaxed);
    let bytes_before = ALLOCATED_BYTES.load(Ordering::Relaxed);
    let started = Instant::now();
    harness.run_ticks(options.ticks);
    let elapsed = started.elapsed();
    let allocations = ALLOCATIONS.load(Ordering::Relaxed) - allocations_before;
    let allocated = ALLOCATED_BYTES.load(Ordering::Relaxed) - bytes_before;
    harness.assert_consistent();

    println!("\nsession");
    println!("  every client active by tick {joined_tick}");
    for (index, node) in harness.clients.iter().enumerate() {
        let diagnostics = node.session.peer_diagnostics();
        println!(
            "  client {index}: phase {:?}, {} snapshots applied, state {} ticks behind the host, \
             RTT {} µs, {} commands submitted",
            node.session.phase(),
            node.snapshots_seen,
            harness.lag_ticks(index).unwrap_or(u64::MAX),
            diagnostics
                .and_then(|peer| peer.rtt_micros)
                .map_or("unmeasured".to_string(), |rtt| rtt.to_string()),
            node.submitted.len(),
        );
        if node.session.phase() != Phase::Active {
            eprintln!("client {index} is not active");
            return ExitCode::FAILURE;
        }
    }
    println!(
        "  host executed {} commands exactly once; every snapshot a client applied matched the host",
        harness.world.executed().len()
    );

    let seconds = options.ticks as f64 * tick_micros as f64 / 1_000_000.0;
    let host = harness.host.diagnostics();
    println!("\nbandwidth over {seconds:.1} simulated seconds (host side, all clients)");
    for (channel, before) in ChannelId::ALL.into_iter().zip(channels_before) {
        let now = host.channel(channel);
        println!(
            "  {channel:?}: sent {:.0} B/s in {} messages, received {:.0} B/s in {} messages",
            (now.bytes_sent - before.bytes_sent) as f64 / seconds,
            now.messages_sent - before.messages_sent,
            (now.bytes_received - before.bytes_received) as f64 / seconds,
            now.messages_received - before.messages_received,
        );
    }
    println!(
        "  outbound queue high water on any one connection: {} bytes",
        host.queue_high_water_bytes
    );
    let client = harness.clients[0].session.diagnostics();
    for (name, diagnostics) in [("host", host), ("client 0", client)] {
        let reasons: Vec<String> = DropReason::ALL
            .into_iter()
            .filter(|reason| diagnostics.drops(*reason) > 0)
            .map(|reason| format!("{reason:?} {}", diagnostics.drops(reason)))
            .collect();
        println!("  {name} discards: {}", reasons.join(", "));
    }
    println!(
        "  full transfers {}, recovery requests {}",
        host.full_transfers_started, host.recovery_requests
    );

    println!("\nprocessing cost (host, link simulator and all clients together)");
    println!(
        "  {:.1} µs wall time per simulated tick",
        elapsed.as_micros() as f64 / options.ticks as f64
    );
    println!(
        "  {:.1} allocations and {:.0} bytes allocated per simulated tick",
        allocations as f64 / options.ticks as f64,
        allocated as f64 / options.ticks as f64
    );

    codec_cost(options.drifters);
    ExitCode::SUCCESS
}

/// Time the codec on a representative state update, in isolation.
fn codec_cost(drifters: usize) {
    let limits = ReplicationLimits {
        max_entities: 1_024,
        max_entity_bytes: 256,
        max_snapshot_bytes: 256 * 1024,
    };
    let codec = CodecLimits {
        max_command_bytes: 64,
        max_slots_per_peer: 4,
    };
    let mut world = World::new(1_024, drifters.max(1));
    let base = world.snapshot();
    world.step();
    let target = world.snapshot();

    const ROUNDS: u32 = 20_000;
    let mut delta = Vec::new();
    let mut payload = Vec::new();
    let mut wire = Vec::new();
    let header = Header {
        kind: MessageKind::StateUpdate.wire(),
        channel: ChannelId::State,
        epoch: SessionEpoch::new(1),
        sequence: ge_net::identity::Sequence::new(1),
        tick: Some(Tick::new(1)),
        fragment: None,
    };

    let allocations_before = ALLOCATIONS.load(Ordering::Relaxed);
    let started = Instant::now();
    for _ in 0..ROUNDS {
        delta.clear();
        encode_delta(Some(&base), &target, &mut delta);
        let update = Message::StateUpdate(StateUpdate {
            snapshot: SnapshotId::new(2),
            baseline: Some(SnapshotId::new(1)),
            acks: Vec::new(),
            delta: &delta,
        });
        payload.clear();
        update.encode_payload(&mut payload).expect("encodable");
        wire.clear();
        envelope::encode(&header, &payload, usize::MAX, &mut wire).expect("encodable");
    }
    let encode = started.elapsed();
    let encode_allocations = ALLOCATIONS.load(Ordering::Relaxed) - allocations_before;

    let allocations_before = ALLOCATIONS.load(Ordering::Relaxed);
    let started = Instant::now();
    let mut entities = 0;
    for _ in 0..ROUNDS {
        let envelope = envelope::decode(&wire, usize::MAX).expect("decodable");
        let Ok(Message::StateUpdate(update)) =
            decode_payload(MessageKind::StateUpdate, envelope.payload, &codec)
        else {
            unreachable!("the encoder produced a state update");
        };
        let snapshot = apply_delta(Some(&base), update.delta, &limits).expect("applicable");
        entities = snapshot.len();
    }
    let decode = started.elapsed();
    let decode_allocations = ALLOCATIONS.load(Ordering::Relaxed) - allocations_before;

    println!(
        "\ncodec on one state update ({entities} entities, {} bytes on the wire, unfragmented here)",
        wire.len()
    );
    println!(
        "  delta, payload and envelope encode: {:.2} µs and {:.1} allocations each",
        encode.as_nanos() as f64 / f64::from(ROUNDS) / 1_000.0,
        encode_allocations as f64 / f64::from(ROUNDS)
    );
    println!(
        "  envelope and payload decode, delta apply: {:.2} µs and {:.1} allocations each",
        decode.as_nanos() as f64 / f64::from(ROUNDS) / 1_000.0,
        decode_allocations as f64 / f64::from(ROUNDS)
    );
}
