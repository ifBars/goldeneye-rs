//! A host or client process for the loopback acceptance test.
//!
//! ```text
//! ge-net-loopback host --clients 2 --ticks 180
//! ge-net-loopback client --port <port> --fingerprint <hex>
//! ```
//!
//! The host prints `LISTEN <port> <fingerprint>` once it is bound. Both run
//! the authored synthetic simulation over real QUIC on the loopback
//! interface. Neither needs a game image, and neither contacts any other
//! host. The fingerprint is a public identifier, not a secret.

#[allow(dead_code)]
#[path = "../../../ge-net/tests/support/simulation.rs"]
mod simulation;

use std::collections::BTreeMap;
use std::io::Write;
use std::net::{Ipv4Addr, SocketAddr};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use ge_net::session::host::HostEvent;
use ge_net::transport::{pump_client, pump_host};
use ge_net::{
    AttemptId, ClientConfig, ClientEvent, ClientSession, CloseReason, CompatibilityDescriptor,
    DrainLimit, HostConfig, HostSession, NetInstant, Phase, PlayerSlot, SessionBudget, Tick,
    TickRate, Transport,
};
use ge_net_transport::{Fingerprint, HostIdentity, QuicClient, QuicConfig, QuicHost, secure_seed};
use simulation::{Command, World, digest};

/// A 60 Hz test profile, not a claim about any game.
const TICK_MICROS: u64 = 16_666;
const DEADLINE: Duration = Duration::from_secs(60);
const EXIT_USAGE: u8 = 2;
const EXIT_TIMEOUT: u8 = 3;
const EXIT_FAILED: u8 = 4;

/// Authored, synthetic compatibility values shared by both processes.
fn descriptor() -> CompatibilityDescriptor {
    CompatibilityDescriptor {
        app_protocol: 1,
        simulation_schema: 1,
        build: [0x4c; 32],
        edition: *b"LOOPBACK",
        revision: 0,
        content: [0x4f; 32],
        ruleset: [0x50; 32],
        tick_rate: TickRate::per_second(60),
        supported_features: 1,
        required_features: 1,
        extensions: Vec::new(),
    }
}

fn option(arguments: &[String], name: &str) -> Option<String> {
    let at = arguments.iter().position(|argument| argument == name)?;
    arguments.get(at + 1).cloned()
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let result = match arguments.first().map(String::as_str) {
        Some("host") => {
            let clients = option(&arguments, "--clients").and_then(|v| v.parse().ok());
            let ticks = option(&arguments, "--ticks").and_then(|v| v.parse().ok());
            match (clients, ticks) {
                (Some(clients), Some(ticks)) => host(clients, ticks),
                _ => Err(EXIT_USAGE),
            }
        }
        Some("client") => {
            let port = option(&arguments, "--port").and_then(|v| v.parse().ok());
            let pinned = option(&arguments, "--fingerprint").and_then(|v| v.parse().ok());
            match (port, pinned) {
                (Some(port), Some(pinned)) => client(port, pinned),
                _ => Err(EXIT_USAGE),
            }
        }
        _ => Err(EXIT_USAGE),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(EXIT_USAGE) => {
            eprintln!("usage: ge-net-loopback host --clients N --ticks N");
            eprintln!("       ge-net-loopback client --port N --fingerprint HEX");
            ExitCode::from(EXIT_USAGE)
        }
        Err(code) => ExitCode::from(code),
    }
}

fn host(clients: usize, ticks: u64) -> Result<(), u8> {
    let fail = |message: &dyn std::fmt::Display| {
        eprintln!("host: {message}");
        EXIT_FAILED
    };
    let identity = HostIdentity::generate().map_err(|error| fail(&error))?;
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let mut transport =
        QuicHost::bind(address, identity, QuicConfig::default()).map_err(|error| fail(&error))?;
    println!(
        "LISTEN {} {}",
        transport.local_addr().port(),
        transport.fingerprint()
    );
    std::io::stdout().flush().map_err(|error| fail(&error))?;

    let seed = secure_seed().map_err(|error| fail(&error))?;
    let mut entropy = move || seed;
    let mut session = HostSession::new(
        HostConfig {
            budget: SessionBudget::small_session(),
            descriptor: descriptor(),
            sequence_origin: 0,
        },
        transport.capabilities(),
        &mut entropy,
        NetInstant::ZERO,
    )
    .map_err(|error| fail(&error))?;

    let mut world = World::new(256, 6);
    let mut owners: BTreeMap<AttemptId, Vec<PlayerSlot>> = BTreeMap::new();
    let mut digests: Vec<(u64, u64)> = Vec::new();
    let mut active = 0usize;
    let mut tick = 0u64;
    let mut remaining = ticks;
    let mut shutting_down = false;
    let started = Instant::now();
    let mut next_tick = TICK_MICROS;

    loop {
        if started.elapsed() > DEADLINE {
            eprintln!("host: deadline passed with {active} of {clients} clients active");
            return Err(EXIT_TIMEOUT);
        }
        let now = started.elapsed().as_micros() as u64;
        session
            .advance(NetInstant::from_micros(now))
            .map_err(|error| fail(&error))?;
        pump_host(&mut session, &mut transport, DrainLimit::UNLIMITED);
        while let Some(event) = session.poll_event() {
            match event {
                HostEvent::PeerAccepted { attempt, slots, .. } => {
                    owners.insert(attempt, slots);
                }
                HostEvent::PeerActive { attempt, .. } => {
                    active += 1;
                    for slot in owners.get(&attempt).into_iter().flatten() {
                        world.join(*slot);
                    }
                }
                HostEvent::PeerClosed {
                    attempt, reason, ..
                } => {
                    for slot in owners.remove(&attempt).unwrap_or_default() {
                        world.leave(slot);
                    }
                    if !shutting_down {
                        eprintln!("host: {attempt} closed early: {reason}");
                    }
                }
            }
        }
        if shutting_down {
            if session.is_shut_down() {
                break;
            }
        } else if now >= next_tick {
            // Simulation ticks follow the clock, not the pump rate.
            next_tick += TICK_MICROS;
            tick += 1;
            let commands = session
                .take_commands(Tick::new(tick))
                .map_err(|error| fail(&error))?;
            for command in &commands {
                world.apply(command);
            }
            world.step();
            let snapshot = world.snapshot();
            digests.push((tick, digest(&snapshot)));
            session
                .publish_snapshot(Tick::new(tick), snapshot)
                .map_err(|error| fail(&error))?;
            if active >= clients {
                remaining -= 1;
                if remaining == 0 {
                    session.shutdown();
                    shutting_down = true;
                }
            }
        }
        std::thread::sleep(Duration::from_micros(500));
    }
    // Joins the worker after its connections have flushed and closed.
    transport.shutdown();

    let mut out = std::io::stdout().lock();
    for (tick, digest) in &digests {
        writeln!(out, "DIGEST {tick} {digest:016x}").map_err(|error| fail(&error))?;
    }
    let diagnostics = session.diagnostics();
    writeln!(
        out,
        "HOST_DONE ticks={tick} executed={} duplicates={} rejected={} shutdown_closes={}",
        world.executed().len(),
        world.duplicate_executions,
        world.rejected_commands,
        diagnostics.disconnects.shutdown,
    )
    .map_err(|error| fail(&error))?;
    Ok(())
}

fn client(port: u16, pinned: Fingerprint) -> Result<(), u8> {
    let fail = |message: &dyn std::fmt::Display| {
        eprintln!("client: {message}");
        EXIT_FAILED
    };
    // Loopback only: this process never listens on an external interface.
    let local = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let mut transport =
        QuicClient::bind(local, QuicConfig::default()).map_err(|error| fail(&error))?;
    let mut session = ClientSession::new(
        ClientConfig {
            budget: SessionBudget::small_session(),
            descriptor: descriptor(),
            requested_slots: 1,
            sequence_origin: 0,
        },
        transport.capabilities(),
        NetInstant::ZERO,
    )
    .map_err(|error| fail(&error))?;
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let attempt = transport.connect(address, pinned);
    session.begin(attempt).map_err(|error| fail(&error))?;

    let mut applied: Vec<(u64, u64)> = Vec::new();
    let mut commands = 0u64;
    let mut last_command_tick = 0u64;
    let started = Instant::now();
    let mut next_command = 0u64;
    let reason = loop {
        if started.elapsed() > DEADLINE {
            eprintln!("client: deadline passed in phase {:?}", session.phase());
            return Err(EXIT_TIMEOUT);
        }
        let now = started.elapsed().as_micros() as u64;
        session
            .advance(NetInstant::from_micros(now))
            .map_err(|error| fail(&error))?;
        pump_client(&mut session, &mut transport, DrainLimit::UNLIMITED);
        let mut closed = None;
        while let Some(event) = session.poll_event() {
            match event {
                ClientEvent::SyncSnapshot { id, tick, state }
                | ClientEvent::Snapshot {
                    id, tick, state, ..
                } => {
                    // "Applying" is recording the state; then say so.
                    applied.push((tick.get(), digest(&state)));
                    if session.phase() == Phase::Synchronizing {
                        session
                            .acknowledge_applied(id)
                            .map_err(|error| fail(&error))?;
                    }
                }
                ClientEvent::Closed { reason } => closed = Some(reason),
                ClientEvent::Accepted { .. } | ClientEvent::Activated { .. } => {}
            }
        }
        if let Some(reason) = closed {
            break reason;
        }
        if session.phase() == Phase::Active && now >= next_command {
            next_command = now + TICK_MICROS;
            if let Some((_, newest)) = session.latest_snapshot() {
                let tick = (newest.get() + 6).max(last_command_tick + 1);
                let step = if commands.is_multiple_of(2) { 1 } else { -1 };
                let command = Command::Move { dx: step, dy: 0 }.encode();
                for slot in session.slots() {
                    if session
                        .submit_command(slot, Tick::new(tick), &command)
                        .is_ok()
                    {
                        commands += 1;
                        last_command_tick = tick;
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_micros(500));
    };
    // Let the transport act on the final disconnect, then join its worker.
    pump_client(&mut session, &mut transport, DrainLimit::UNLIMITED);
    transport.close(attempt);
    transport.shutdown();

    let mut out = std::io::stdout().lock();
    for (tick, digest) in &applied {
        writeln!(out, "APPLIED {tick} {digest:016x}").map_err(|error| fail(&error))?;
    }
    writeln!(
        out,
        "CLIENT_DONE reason={reason:?} snapshots={} commands={commands}",
        applied.len()
    )
    .map_err(|error| fail(&error))?;
    // Only the host's orderly shutdown counts as success.
    if reason == CloseReason::RemoteClosed(CloseReason::Shutdown.wire_code()) {
        Ok(())
    } else {
        Err(EXIT_FAILED)
    }
}
