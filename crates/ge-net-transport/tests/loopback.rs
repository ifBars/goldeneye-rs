//! The QUIC backend against the transport contract, over real loopback
//! sockets in one process. Ports are assigned by the operating system and
//! every wait has a deadline.

use std::net::{Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};

use ge_net::{AttemptId, Lane, SendOutcome, Transport, TransportEvent, TransportFailure};
use ge_net_transport::{
    Fingerprint, FingerprintParseError, HostIdentity, QuicClient, QuicConfig, QuicHost,
    TransportError,
};

const DEADLINE: Duration = Duration::from_secs(20);

fn loopback() -> SocketAddr {
    SocketAddr::from((Ipv4Addr::LOCALHOST, 0))
}

/// Poll `transport` until `accept` returns a value or the deadline passes.
fn wait_for<T: Transport, R>(
    transport: &mut T,
    what: &str,
    mut accept: impl FnMut(TransportEvent) -> Option<R>,
) -> R {
    let deadline = Instant::now() + DEADLINE;
    loop {
        while let Some(event) = transport.poll() {
            if let Some(result) = accept(event) {
                return result;
            }
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn connected<T: Transport>(transport: &mut T) -> AttemptId {
    wait_for(transport, "a connection", |event| match event {
        TransportEvent::Connected { attempt } => Some(attempt),
        other => panic!("expected a connection, got {other:?}"),
    })
}

fn message<T: Transport>(transport: &mut T, from: Lane) -> Vec<u8> {
    wait_for(transport, "a message", |event| match event {
        TransportEvent::Message { lane, bytes, .. } if lane == from => Some(bytes),
        TransportEvent::Message { .. } => None,
        other => panic!("expected a message, got {other:?}"),
    })
}

fn closed<T: Transport>(transport: &mut T) -> (AttemptId, TransportFailure) {
    wait_for(transport, "a close", |event| match event {
        TransportEvent::Closed { attempt, failure } => Some((attempt, failure)),
        _ => None,
    })
}

/// Send, retrying while the lane reports congestion.
fn send_all<T: Transport>(transport: &mut T, attempt: AttemptId, lane: Lane, bytes: &[u8]) {
    let deadline = Instant::now() + DEADLINE;
    loop {
        match transport.send(attempt, lane, bytes) {
            SendOutcome::Sent => return,
            SendOutcome::Blocked => {
                assert!(Instant::now() < deadline, "lane stayed blocked");
                std::thread::sleep(Duration::from_millis(1));
            }
            SendOutcome::Closed => panic!("connection closed during send"),
        }
    }
}

struct Link {
    host: QuicHost,
    client: QuicClient,
    host_attempt: AttemptId,
    client_attempt: AttemptId,
}

fn link(config: QuicConfig) -> Link {
    let identity = HostIdentity::generate().expect("identity");
    let mut host = QuicHost::bind(loopback(), identity, config.clone()).expect("host binds");
    let mut client = QuicClient::bind(loopback(), config).expect("client binds");
    let client_attempt = client.connect(host.local_addr(), host.fingerprint());
    let reported = connected(&mut client);
    assert_eq!(reported, client_attempt);
    let host_attempt = connected(&mut host);
    Link {
        host,
        client,
        host_attempt,
        client_attempt,
    }
}

#[test]
fn both_delivery_contracts_carry_what_the_capabilities_declare() {
    let mut link = link(QuicConfig::default());
    let capabilities = link.host.capabilities();
    assert_eq!(capabilities, link.client.capabilities());
    assert_eq!(capabilities.reliable_lanes, 2);
    let datagram = usize::from(capabilities.max_datagram_bytes.expect("datagrams declared"));
    let reliable = capabilities.max_reliable_message_bytes as usize;

    // The largest declared datagram really crosses the encrypted path, in
    // both directions. Loopback loses nothing in practice; a real path may.
    let payload: Vec<u8> = (0..datagram).map(|i| i as u8).collect();
    assert_eq!(
        link.client
            .send(link.client_attempt, Lane::Datagram, &payload),
        SendOutcome::Sent
    );
    assert_eq!(message(&mut link.host, Lane::Datagram), payload);
    assert_eq!(
        link.host.send(link.host_attempt, Lane::Datagram, &payload),
        SendOutcome::Sent
    );
    assert_eq!(message(&mut link.client, Lane::Datagram), payload);

    // The largest declared reliable message, and an empty one, on each lane.
    let large: Vec<u8> = (0..reliable).map(|i| (i * 7) as u8).collect();
    for lane in [Lane::Control, Lane::Bulk] {
        send_all(&mut link.host, link.host_attempt, lane, &large);
        send_all(&mut link.host, link.host_attempt, lane, &[]);
        assert_eq!(message(&mut link.client, lane), large);
        assert_eq!(message(&mut link.client, lane), Vec::<u8>::new());
    }
    assert_eq!(link.host.dropped_datagrams(), 0);
}

/// Reliable frames carry their index first; it must count up from zero.
fn check_order(event: TransportEvent, lane: Lane, expected: &mut u32) {
    let TransportEvent::Message {
        lane: got, bytes, ..
    } = event
    else {
        panic!("unexpected {event:?}");
    };
    assert_eq!(got, lane);
    let index = u32::from_be_bytes(bytes[..4].try_into().unwrap());
    assert_eq!(
        index, *expected,
        "reliable lane reordered or lost a message"
    );
    *expected += 1;
}

#[test]
fn a_reliable_lane_blocks_under_backpressure_and_never_drops_or_reorders() {
    let mut link = link(QuicConfig {
        send_queue_messages: 8,
        ..QuicConfig::default()
    });
    // The host reads only when the client is blocked, so the client pushes
    // far more than the queues and flow-control windows hold.
    const FRAMES: u32 = 400;
    let body = vec![0xab; 16 * 1024];
    let mut blocked = 0u32;
    let mut sent = 0u32;
    let mut received = 0u32;
    let deadline = Instant::now() + DEADLINE;
    while sent < FRAMES {
        let mut frame = sent.to_be_bytes().to_vec();
        frame.extend_from_slice(&body);
        match link.client.send(link.client_attempt, Lane::Bulk, &frame) {
            SendOutcome::Sent => sent += 1,
            SendOutcome::Blocked => {
                blocked += 1;
                // Read a little, so the transfer can continue.
                for _ in 0..4 {
                    if let Some(event) = link.host.poll() {
                        check_order(event, Lane::Bulk, &mut received);
                    }
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            SendOutcome::Closed => panic!("closed"),
        }
        assert!(Instant::now() < deadline, "transfer stalled at {sent}");
    }
    assert!(
        blocked > 0,
        "the lane reported congestion instead of buffering"
    );
    while received < FRAMES {
        while let Some(event) = link.host.poll() {
            check_order(event, Lane::Bulk, &mut received);
        }
        assert!(
            Instant::now() < deadline,
            "only {received} of {FRAMES} arrived"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn a_backed_up_bulk_lane_does_not_block_the_control_lane() {
    let mut link = link(QuicConfig {
        send_queue_messages: 4,
        ..QuicConfig::default()
    });
    // Fill the bulk lane until it pushes back. The host reads nothing.
    let frame = vec![0x5a; 32 * 1024];
    let deadline = Instant::now() + DEADLINE;
    let mut stalled = 0;
    while stalled < 200 {
        match link.client.send(link.client_attempt, Lane::Bulk, &frame) {
            SendOutcome::Sent => stalled = 0,
            SendOutcome::Blocked => {
                stalled += 1;
                std::thread::sleep(Duration::from_millis(1));
            }
            SendOutcome::Closed => panic!("closed"),
        }
        assert!(Instant::now() < deadline);
    }
    // The control lane is a separate stream: it is not blocked, and its
    // message arrives while bulk data is still waiting to be read.
    assert_eq!(
        link.client
            .send(link.client_attempt, Lane::Control, b"urgent"),
        SendOutcome::Sent
    );
    assert_eq!(message(&mut link.host, Lane::Control), b"urgent");
    // Bulk data is still in flight behind it.
    assert_eq!(message(&mut link.host, Lane::Bulk), frame);
}

#[test]
fn a_client_refuses_a_host_that_is_not_the_pinned_identity() {
    let identity = HostIdentity::generate().unwrap();
    let mut host = QuicHost::bind(loopback(), identity, QuicConfig::default()).unwrap();
    let mut client = QuicClient::bind(loopback(), QuicConfig::default()).unwrap();

    // A different, perfectly valid identity is pinned instead.
    let other = HostIdentity::generate().unwrap().fingerprint();
    assert_ne!(other, host.fingerprint());
    let attempt = client.connect(host.local_addr(), other);
    assert_eq!(
        closed(&mut client),
        (attempt, TransportFailure::AuthenticationFailed)
    );
    assert_eq!(
        client.send(attempt, Lane::Control, b"x"),
        SendOutcome::Closed
    );
    // The host never reports a connection that failed its handshake.
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(host.poll(), None);

    // The same client then reaches the host with the right pin, as a new
    // attempt with a larger identifier.
    let second = client.connect(host.local_addr(), host.fingerprint());
    assert!(second > attempt);
    assert_eq!(connected(&mut client), second);
    connected(&mut host);
}

#[test]
fn a_restored_identity_keeps_its_fingerprint_and_a_bad_one_is_rejected() {
    let identity = HostIdentity::generate().unwrap();
    let restored = HostIdentity::from_der(
        identity.certificate_der().to_vec(),
        identity.private_key_der().to_vec(),
    );
    assert_eq!(restored.fingerprint(), identity.fingerprint());
    // Debug output names the public fingerprint and never the key.
    let debug = format!("{restored:?}");
    assert!(debug.contains(&identity.fingerprint().to_string()));
    assert!(!debug.contains("private"));

    let text = identity.fingerprint().to_string();
    assert_eq!(text.len(), 64);
    assert_eq!(text.parse::<Fingerprint>(), Ok(identity.fingerprint()));
    assert_eq!(
        Fingerprint::from_bytes(*identity.fingerprint().as_bytes()),
        identity.fingerprint()
    );
    for bad in ["", "abc", &text[..63], &format!("{}zz", &text[..62])] {
        assert_eq!(bad.parse::<Fingerprint>(), Err(FingerprintParseError));
    }

    // A host works from the restored identity.
    let mut host = QuicHost::bind(loopback(), restored, QuicConfig::default()).unwrap();
    let mut client = QuicClient::bind(loopback(), QuicConfig::default()).unwrap();
    client.connect(host.local_addr(), identity.fingerprint());
    connected(&mut client);
    connected(&mut host);

    // Garbage is refused when the endpoint is created, not later.
    let garbage = HostIdentity::from_der(vec![1, 2, 3], vec![4, 5, 6]);
    assert!(matches!(
        QuicHost::bind(loopback(), garbage, QuicConfig::default()),
        Err(TransportError::Tls(_))
    ));
}

#[test]
fn closing_flushes_reliable_messages_before_the_peer_sees_the_close() {
    let mut link = link(QuicConfig::default());
    for index in 0..300u32 {
        let mut frame = index.to_be_bytes().to_vec();
        frame.extend_from_slice(&[7; 900]);
        send_all(&mut link.host, link.host_attempt, Lane::Control, &frame);
    }
    link.host.close(link.host_attempt);
    // Nothing more can be sent on a closed attempt.
    assert_eq!(
        link.host.send(link.host_attempt, Lane::Control, b"late"),
        SendOutcome::Closed
    );
    assert_eq!(
        link.host.send(link.host_attempt, Lane::Datagram, b"late"),
        SendOutcome::Closed
    );

    let mut received = 0u32;
    let reason = wait_for(&mut link.client, "the close", |event| match event {
        TransportEvent::Message { lane, bytes, .. } => {
            assert_eq!(lane, Lane::Control);
            assert_eq!(u32::from_be_bytes(bytes[..4].try_into().unwrap()), received);
            received += 1;
            None
        }
        TransportEvent::Closed { attempt, failure } => Some((attempt, failure)),
        other => panic!("unexpected {other:?}"),
    });
    assert_eq!(
        received, 300,
        "every accepted message arrived before the close"
    );
    assert_eq!(
        reason,
        (link.client_attempt, TransportFailure::RemoteClosed)
    );
    // The side that closed gets no event for its own decision.
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(link.host.poll(), None);
    // Closing again, or closing something unknown, is a no-op.
    link.host.close(link.host_attempt);
    link.host.close(AttemptId::new(9_999));
}

#[test]
fn shutdown_joins_the_worker_and_the_peer_is_told() {
    let mut link = link(QuicConfig::default());
    send_all(
        &mut link.client,
        link.client_attempt,
        Lane::Control,
        b"last words",
    );
    let started = Instant::now();
    // Returns only after the worker thread has been joined.
    link.client.shutdown();
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "shutdown is bounded"
    );
    assert_eq!(message(&mut link.host, Lane::Control), b"last words");
    assert_eq!(
        closed(&mut link.host),
        (link.host_attempt, TransportFailure::RemoteClosed)
    );
    // Dropping joins as well; a second host can bind right after.
    drop(link.host);
    let identity = HostIdentity::generate().unwrap();
    QuicHost::bind(loopback(), identity, QuicConfig::default())
        .unwrap()
        .shutdown();
}

#[test]
fn a_host_that_is_not_there_is_reported_within_the_connect_timeout() {
    let config = QuicConfig {
        connect_timeout: Duration::from_millis(600),
        ..QuicConfig::default()
    };
    // Reserve a port, then free it: nothing listens there.
    let unused = std::net::UdpSocket::bind(loopback())
        .unwrap()
        .local_addr()
        .unwrap();
    let mut client = QuicClient::bind(loopback(), config).unwrap();
    let started = Instant::now();
    let attempt = client.connect(unused, HostIdentity::generate().unwrap().fingerprint());
    let (reported, failure) = closed(&mut client);
    assert_eq!(reported, attempt);
    assert!(
        matches!(
            failure,
            TransportFailure::TimedOut | TransportFailure::ConnectFailed
        ),
        "{failure:?}"
    );
    assert!(started.elapsed() < Duration::from_secs(5));

    // An attempt cancelled while connecting reports nothing afterwards.
    let cancelled = client.connect(unused, HostIdentity::generate().unwrap().fingerprint());
    client.close(cancelled);
    std::thread::sleep(Duration::from_millis(900));
    assert_eq!(client.poll(), None);
    assert_eq!(client.local_addr().ip(), Ipv4Addr::LOCALHOST);
    // The default constructor binds an unspecified ephemeral address.
    let any = QuicClient::new(QuicConfig::default()).unwrap();
    assert_eq!(any.local_addr().ip(), Ipv4Addr::UNSPECIFIED);
    assert_ne!(any.local_addr().port(), 0);
}

#[test]
fn configuration_limits_are_checked_before_a_socket_is_opened() {
    let identity = HostIdentity::generate().unwrap();
    let cases: [(QuicConfig, &str); 6] = [
        (
            QuicConfig {
                max_connections: 0,
                ..QuicConfig::default()
            },
            "max_connections",
        ),
        (
            QuicConfig {
                max_datagram_bytes: 1_400,
                ..QuicConfig::default()
            },
            "max_datagram_bytes",
        ),
        (
            QuicConfig {
                max_reliable_message_bytes: 10,
                ..QuicConfig::default()
            },
            "max_reliable_message_bytes",
        ),
        (
            QuicConfig {
                send_queue_messages: 0,
                ..QuicConfig::default()
            },
            "send_queue_messages",
        ),
        (
            QuicConfig {
                event_queue_messages: 1,
                ..QuicConfig::default()
            },
            "event_queue_messages",
        ),
        (
            QuicConfig {
                keep_alive: Duration::from_secs(60),
                ..QuicConfig::default()
            },
            "keep_alive",
        ),
    ];
    for (config, field) in cases {
        let error = QuicHost::bind(loopback(), identity.clone(), config).unwrap_err();
        assert!(matches!(error, TransportError::InvalidConfig(name) if name == field));
        assert!(error.to_string().contains(field));
    }
    // An address that cannot be bound is an I/O error, reported at once.
    let taken = QuicHost::bind(loopback(), identity.clone(), QuicConfig::default()).unwrap();
    let error = QuicHost::bind(taken.local_addr(), identity, QuicConfig::default()).unwrap_err();
    assert!(matches!(error, TransportError::Io(_)), "{error}");
    assert!(std::error::Error::source(&error).is_some());
}

#[test]
fn an_oversized_reliable_message_fails_the_connection_explicitly() {
    let mut link = link(QuicConfig::default());
    let limit = link.host.capabilities().max_reliable_message_bytes as usize;
    let outcome = link
        .host
        .send(link.host_attempt, Lane::Control, &vec![0; limit + 1]);
    assert_eq!(outcome, SendOutcome::Closed);
    assert_eq!(
        closed(&mut link.host),
        (link.host_attempt, TransportFailure::ReliableDeliveryFailed)
    );
    assert_eq!(
        closed(&mut link.client),
        (link.client_attempt, TransportFailure::RemoteClosed)
    );
}
