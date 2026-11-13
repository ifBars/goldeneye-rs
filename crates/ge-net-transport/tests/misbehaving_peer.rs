//! The QUIC backend against peers that break the lane contract or hold
//! resources open, and against a host whose application stops polling.
//!
//! A raw `quinn` client stands in for a peer that `QuicClient` would never
//! be: it finishes or resets one lane, or stops reading while keeping the
//! connection alive. Everything runs on loopback with bounded waits.

use std::future::Future;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use ge_net::{AttemptId, Lane, SendOutcome, Transport, TransportEvent, TransportFailure};
use ge_net_transport::{ALPN, HostIdentity, QuicClient, QuicConfig, QuicHost};
use quinn::crypto::rustls::QuicClientConfig;
use quinn::{Connection, ConnectionError, Endpoint, RecvStream, SendStream, VarInt};
use rustls::DigitallySignedStruct;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, verify_tls12_signature, verify_tls13_signature};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};

const DEADLINE: Duration = Duration::from_secs(20);
/// Short enough that tests finish quickly, far below the idle timeout.
const FLUSH: Duration = Duration::from_millis(300);

fn loopback() -> SocketAddr {
    SocketAddr::from((Ipv4Addr::LOCALHOST, 0))
}

fn config() -> QuicConfig {
    QuicConfig {
        close_flush_timeout: FLUSH,
        ..QuicConfig::default()
    }
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime")
}

/// Accepts exactly the host certificate it was given. Test-only.
#[derive(Debug)]
struct ExactCertificate {
    certificate: Vec<u8>,
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for ExactCertificate {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        if end_entity.as_ref() == self.certificate.as_slice() {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General("unexpected certificate".into()))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// A hand-driven peer with both lanes open, as the host expects.
struct RawPeer {
    _endpoint: Endpoint,
    connection: Connection,
    control: (SendStream, RecvStream),
    _bulk: (SendStream, RecvStream),
}

/// Connect the way `QuicClient` does: same ALPN, both lanes tagged.
async fn raw_peer(host: SocketAddr, identity: &HostIdentity) -> RawPeer {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let verifier = ExactCertificate {
        certificate: identity.certificate_der().to_vec(),
        provider: provider.clone(),
    };
    let mut tls = rustls::ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])
        .unwrap()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth();
    tls.alpn_protocols = vec![ALPN.to_vec()];
    let mut transport = quinn::TransportConfig::default();
    // Keep the connection alive however long the test waits, and keep the
    // stream window small so a peer that stops reading exhausts it quickly.
    transport
        .keep_alive_interval(Some(Duration::from_millis(200)))
        .stream_receive_window(VarInt::from_u32(64 * 1024));
    let mut client = quinn::ClientConfig::new(Arc::new(QuicClientConfig::try_from(tls).unwrap()));
    client.transport_config(Arc::new(transport));
    let endpoint = Endpoint::client(loopback()).unwrap();
    let connection = endpoint
        .connect_with(client, host, "ge-net-host")
        .unwrap()
        .await
        .expect("raw peer connects");
    let mut control = connection.open_bi().await.unwrap();
    control.0.write_all(&[0]).await.unwrap();
    let mut bulk = connection.open_bi().await.unwrap();
    bulk.0.write_all(&[1]).await.unwrap();
    RawPeer {
        _endpoint: endpoint,
        connection,
        control,
        _bulk: bulk,
    }
}

/// Poll a transport from async code, letting the raw peer run meanwhile.
async fn next_event<T: Transport>(transport: &mut T, what: &str) -> TransportEvent {
    let deadline = Instant::now() + DEADLINE;
    loop {
        if let Some(event) = transport.poll() {
            return event;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
}

async fn host_connected(host: &mut QuicHost) -> AttemptId {
    match next_event(host, "a connection").await {
        TransportEvent::Connected { attempt } => attempt,
        other => panic!("expected a connection, got {other:?}"),
    }
}

/// Wait for `Closed`, skipping other events, and time how long it took.
/// The deadline covers the whole wait, however many events arrive first.
async fn host_closed(
    host: &mut QuicHost,
    since: Instant,
) -> (AttemptId, TransportFailure, Duration) {
    let deadline = Instant::now() + DEADLINE;
    loop {
        match host.poll() {
            Some(TransportEvent::Closed { attempt, failure }) => {
                return (attempt, failure, since.elapsed());
            }
            Some(_) => {}
            None => tokio::time::sleep(Duration::from_millis(1)).await,
        }
        assert!(Instant::now() < deadline, "timed out waiting for a close");
    }
}

/// Keep unreliable traffic flowing while `future` runs, so the connection
/// is plainly alive apart from the broken lane.
async fn with_datagrams<F: Future>(connection: &Connection, future: F) -> F::Output {
    let ticker = {
        let connection = connection.clone();
        tokio::spawn(async move {
            loop {
                let _ = connection.send_datagram(Bytes::from_static(b"alive"));
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
    };
    let output = future.await;
    ticker.abort();
    output
}

fn application_code(error: &ConnectionError) -> Option<u64> {
    match error {
        ConnectionError::ApplicationClosed(close) => Some(close.error_code.into_inner()),
        _ => None,
    }
}

#[test]
fn a_peer_that_finishes_one_lane_but_stays_connected_is_closed_promptly() {
    runtime().block_on(async {
        let identity = HostIdentity::generate().unwrap();
        let mut host = QuicHost::bind(loopback(), identity.clone(), config()).unwrap();
        let mut peer = raw_peer(host.local_addr(), &identity).await;
        let attempt = host_connected(&mut host).await;

        // Finish only the control lane; datagrams and keep-alives continue.
        peer.control.0.finish().unwrap();
        let started = Instant::now();
        let (closed, failure, elapsed) =
            with_datagrams(&peer.connection, host_closed(&mut host, started)).await;
        assert_eq!(closed, attempt);
        assert_eq!(failure, TransportFailure::ReliableDeliveryFailed);
        // A normal closing peer gets twice the flush time; this one gets no
        // more, and nowhere near the ten-second idle timeout.
        assert!(
            elapsed < FLUSH * 2 + Duration::from_secs(1),
            "took {elapsed:?}"
        );
        let reason = tokio::time::timeout(DEADLINE, peer.connection.closed())
            .await
            .expect("the host closed the connection");
        assert_eq!(application_code(&reason), Some(3), "{reason:?}");
    });
}

#[test]
fn a_peer_that_resets_one_lane_is_closed_at_once() {
    runtime().block_on(async {
        let identity = HostIdentity::generate().unwrap();
        let mut host = QuicHost::bind(loopback(), identity.clone(), config()).unwrap();
        let mut peer = raw_peer(host.local_addr(), &identity).await;
        let attempt = host_connected(&mut host).await;

        // A frame header that promises more bytes than ever arrive, then a
        // reset: either alone must end the attempt.
        peer.control
            .0
            .write_all(&100u32.to_be_bytes())
            .await
            .unwrap();
        peer.control.0.write_all(&[1, 2, 3]).await.unwrap();
        peer.control.0.reset(VarInt::from_u32(9)).unwrap();
        let started = Instant::now();
        let (closed, failure, elapsed) =
            with_datagrams(&peer.connection, host_closed(&mut host, started)).await;
        assert_eq!(closed, attempt);
        assert_eq!(failure, TransportFailure::ReliableDeliveryFailed);
        assert!(elapsed < Duration::from_secs(2), "took {elapsed:?}");
    });
}

#[test]
fn a_peer_that_stops_one_lane_ends_the_attempt_for_the_sender() {
    runtime().block_on(async {
        let identity = HostIdentity::generate().unwrap();
        let mut host = QuicHost::bind(loopback(), identity.clone(), config()).unwrap();
        let mut peer = raw_peer(host.local_addr(), &identity).await;
        let attempt = host_connected(&mut host).await;

        // The peer refuses everything the host sends on the control lane.
        peer.control.1.stop(VarInt::from_u32(5)).unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        let started = Instant::now();
        let deadline = Instant::now() + DEADLINE;
        let (closed, failure, elapsed) = loop {
            match host.send(attempt, Lane::Control, b"undeliverable") {
                SendOutcome::Sent | SendOutcome::Blocked => {}
                SendOutcome::Closed => {}
            }
            if let Some(TransportEvent::Closed { attempt, failure }) = host.poll() {
                break (attempt, failure, started.elapsed());
            }
            assert!(Instant::now() < deadline, "the host never noticed");
            tokio::time::sleep(Duration::from_millis(5)).await;
        };
        assert_eq!(closed, attempt);
        assert_eq!(failure, TransportFailure::ReliableDeliveryFailed);
        assert!(elapsed < Duration::from_secs(2), "took {elapsed:?}");
    });
}

#[test]
fn closing_is_bounded_when_the_peer_stops_reading_but_stays_connected() {
    runtime().block_on(async {
        let identity = HostIdentity::generate().unwrap();
        let mut host = QuicHost::bind(loopback(), identity.clone(), config()).unwrap();
        let peer = raw_peer(host.local_addr(), &identity).await;
        let attempt = host_connected(&mut host).await;

        // The peer never reads its control lane. Fill the host's queue past
        // the peer's flow-control window until the lane pushes back.
        let message = vec![0x42; 64 * 1024];
        let mut blocked = 0;
        let deadline = Instant::now() + DEADLINE;
        while blocked < 50 {
            match host.send(attempt, Lane::Control, &message) {
                SendOutcome::Sent => {}
                SendOutcome::Blocked => blocked += 1,
                SendOutcome::Closed => panic!("closed before the test closed it"),
            }
            assert!(Instant::now() < deadline, "the lane never pushed back");
            tokio::time::sleep(Duration::from_millis(1)).await;
        }

        // The writer is stuck on flow control; closing must still finish
        // within the flush deadline, not wait for the peer forever.
        host.close(attempt);
        let started = Instant::now();
        let reason = tokio::time::timeout(DEADLINE, peer.connection.closed())
            .await
            .expect("the host closed the connection");
        let elapsed = started.elapsed();
        assert_eq!(application_code(&reason), Some(3), "{reason:?}");
        assert!(elapsed < FLUSH + Duration::from_secs(1), "took {elapsed:?}");
        // A close the application asked for produces no event.
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(host.poll(), None);
    });
}

#[test]
fn a_host_refuses_connections_beyond_its_admission_limit() {
    let config = QuicConfig {
        max_connections: 1,
        ..config()
    };
    let identity = HostIdentity::generate().unwrap();
    let mut host = QuicHost::bind(loopback(), identity, config.clone()).unwrap();
    let wait = |transport: &mut dyn Transport, what: &str| {
        let deadline = Instant::now() + DEADLINE;
        loop {
            if let Some(event) = transport.poll() {
                return event;
            }
            assert!(Instant::now() < deadline, "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(1));
        }
    };

    let mut first = QuicClient::bind(loopback(), config.clone()).unwrap();
    let first_attempt = first.connect(host.local_addr(), host.fingerprint());
    assert!(matches!(
        wait(&mut first, "first"),
        TransportEvent::Connected { .. }
    ));
    let TransportEvent::Connected { attempt: admitted } = wait(&mut host, "admission") else {
        panic!("expected the first connection");
    };

    // The only slot is taken: the next connection is refused outright.
    let mut second = QuicClient::bind(loopback(), config.clone()).unwrap();
    let second_attempt = second.connect(host.local_addr(), host.fingerprint());
    assert_eq!(
        wait(&mut second, "refusal"),
        TransportEvent::Closed {
            attempt: second_attempt,
            failure: TransportFailure::ConnectFailed,
        }
    );
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(host.poll(), None, "a refused connection is never reported");

    // When the admitted connection ends, its slot is released.
    first.close(first_attempt);
    assert!(matches!(
        wait(&mut host, "the first close"),
        TransportEvent::Closed { attempt, .. } if attempt == admitted
    ));
    std::thread::sleep(Duration::from_millis(100));
    let mut third = QuicClient::bind(loopback(), config).unwrap();
    third.connect(host.local_addr(), host.fingerprint());
    assert!(matches!(
        wait(&mut third, "third"),
        TransportEvent::Connected { .. }
    ));
    let TransportEvent::Connected { attempt } = wait(&mut host, "readmission") else {
        panic!("expected the third connection");
    };
    assert!(attempt > admitted);
}

#[test]
fn a_host_whose_application_stops_polling_turns_new_connections_away() {
    let config = QuicConfig {
        event_queue_messages: 16,
        connect_timeout: Duration::from_millis(500),
        ..config()
    };
    let identity = HostIdentity::generate().unwrap();
    let mut host = QuicHost::bind(loopback(), identity, config.clone()).unwrap();
    let poll_until = |transport: &mut dyn Transport, what: &str| {
        let deadline = Instant::now() + DEADLINE;
        loop {
            if let Some(event) = transport.poll() {
                return event;
            }
            assert!(Instant::now() < deadline, "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(1));
        }
    };

    let mut first = QuicClient::bind(loopback(), config.clone()).unwrap();
    let first_attempt = first.connect(host.local_addr(), host.fingerprint());
    assert!(matches!(
        poll_until(&mut first, "first"),
        TransportEvent::Connected { .. }
    ));
    let TransportEvent::Connected { attempt: admitted } = poll_until(&mut host, "first") else {
        panic!("expected the first connection");
    };

    // The host application stops polling while datagrams fill its queue.
    for _ in 0..64 {
        first.send(first_attempt, Lane::Datagram, b"filler");
    }
    std::thread::sleep(Duration::from_millis(200));

    // A new connection cannot be reported, so after the connect timeout it
    // is closed rather than waiting on the queue with the attempt lock held.
    let mut second = QuicClient::bind(loopback(), config.clone()).unwrap();
    let second_attempt = second.connect(host.local_addr(), host.fingerprint());
    let deadline = Instant::now() + DEADLINE;
    let failure = loop {
        match poll_until(&mut second, "the busy close") {
            TransportEvent::Closed { attempt, failure } => {
                assert_eq!(attempt, second_attempt);
                break failure;
            }
            TransportEvent::Connected { .. } => {}
            other => panic!("unexpected {other:?}"),
        }
        assert!(Instant::now() < deadline);
    };
    assert!(
        matches!(
            failure,
            TransportFailure::Other | TransportFailure::ConnectFailed
        ),
        "{failure:?}"
    );

    // Once the application drains its queue, connections are reported again.
    let mut drained = 0;
    while let Some(event) = host.poll() {
        assert!(
            matches!(
                event,
                TransportEvent::Message {
                    lane: Lane::Datagram,
                    ..
                }
            ),
            "only the queued datagrams: {event:?}"
        );
        drained += 1;
    }
    assert_eq!(drained, 16);
    assert!(host.dropped_datagrams() > 0);
    let mut third = QuicClient::bind(loopback(), config).unwrap();
    third.connect(host.local_addr(), host.fingerprint());
    let deadline = Instant::now() + DEADLINE;
    let attempt = loop {
        match poll_until(&mut host, "the third connection") {
            TransportEvent::Connected { attempt } => break attempt,
            TransportEvent::Message { .. } => {}
            other => panic!("unexpected {other:?}"),
        }
        assert!(Instant::now() < deadline);
    };
    assert!(attempt > admitted);
}
