//! The worker thread: one current-thread runtime that owns the endpoint.
//!
//! The application thread never blocks on it. Outbound messages go through
//! bounded per-lane queues, inbound events through one bounded queue, and
//! shutdown joins the thread so no socket task outlives its owner.

use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use bytes::Bytes;
use ge_net::{
    AttemptId, Lane, SendOutcome, TransportCapabilities, TransportEvent, TransportFailure,
};
use quinn::crypto::rustls::{QuicClientConfig, QuicServerConfig};
use quinn::{
    Connection, ConnectionError, Endpoint, IdleTimeout, RecvStream, SendDatagramError, SendStream,
    VarInt,
};
use rustls::crypto::CryptoProvider;
use tokio::sync::mpsc;
use tokio::time::timeout;

use crate::identity::{Fingerprint, HostIdentity, PinnedVerifier, SERVER_NAME};
use crate::{ALPN, QuicConfig, TransportError};

/// Close code for an orderly close.
const CLOSE_NORMAL: u32 = 0;
/// Close code when the peer broke this backend's framing or limits.
const CLOSE_PROTOCOL: u32 = 2;
/// The first byte on each stream names its lane.
const TAG_CONTROL: u8 = 0;
const TAG_BULK: u8 = 1;

pub(crate) enum Role {
    Host {
        address: SocketAddr,
        identity: HostIdentity,
    },
    Client {
        address: SocketAddr,
    },
}

enum Command {
    Connect {
        attempt: u64,
        address: SocketAddr,
        pinned: Fingerprint,
    },
    Shutdown,
}

/// What the application thread can reach of one connection.
struct Handle {
    control: mpsc::Sender<Vec<u8>>,
    bulk: mpsc::Sender<Vec<u8>>,
    connection: Connection,
}

enum Entry {
    /// A client attempt whose handshake has not finished.
    Pending,
    Open(Handle),
}

#[derive(Default)]
struct Shared {
    connections: Mutex<HashMap<u64, Entry>>,
    dropped_datagrams: AtomicU64,
}

impl Shared {
    fn remove(&self, attempt: u64) -> Option<Entry> {
        self.connections
            .lock()
            .expect("connection table lock")
            .remove(&attempt)
    }
}

/// State shared by every task on the worker.
struct Context {
    shared: Arc<Shared>,
    events: mpsc::Sender<TransportEvent>,
    config: QuicConfig,
    provider: Arc<CryptoProvider>,
    transport: Arc<quinn::TransportConfig>,
    /// Host attempts are numbered in the order their `Connected` is queued.
    next_host_attempt: tokio::sync::Mutex<u64>,
}

/// The application-side handle to a running worker.
pub(crate) struct Backend {
    shared: Arc<Shared>,
    events: mpsc::Receiver<TransportEvent>,
    /// Events produced on the application thread itself.
    local_events: VecDeque<TransportEvent>,
    commands: mpsc::UnboundedSender<Command>,
    thread: Option<JoinHandle<()>>,
    config: QuicConfig,
    local_addr: SocketAddr,
    next_client_attempt: u64,
}

impl Backend {
    pub(crate) fn start(role: Role, config: QuicConfig) -> Result<Self, TransportError> {
        validate(&config)?;
        let shared = Arc::new(Shared::default());
        let (events_tx, events) = mpsc::channel(config.event_queue_messages);
        let (commands, commands_rx) = mpsc::unbounded_channel();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let worker_shared = shared.clone();
        let worker_config = config.clone();
        let thread = std::thread::Builder::new()
            .name("ge-net-quic".into())
            .spawn(move || {
                run(
                    role,
                    worker_config,
                    worker_shared,
                    events_tx,
                    commands_rx,
                    ready_tx,
                );
            })
            .map_err(|error| TransportError::Worker(error.to_string()))?;
        let local_addr = match ready_rx.recv() {
            Ok(Ok(address)) => address,
            Ok(Err(error)) => {
                let _ = thread.join();
                return Err(error);
            }
            Err(_) => {
                let _ = thread.join();
                return Err(TransportError::Worker(
                    "worker exited during startup".into(),
                ));
            }
        };
        Ok(Self {
            shared,
            events,
            local_events: VecDeque::new(),
            commands,
            thread: Some(thread),
            config,
            local_addr,
            next_client_attempt: 1,
        })
    }

    pub(crate) fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub(crate) fn capabilities(&self) -> TransportCapabilities {
        TransportCapabilities {
            reliable_lanes: 2,
            max_reliable_message_bytes: self.config.max_reliable_message_bytes,
            max_datagram_bytes: Some(self.config.max_datagram_bytes),
        }
    }

    /// Datagrams discarded because the event queue was full or the path
    /// could not carry them.
    pub(crate) fn dropped_datagrams(&self) -> u64 {
        self.shared.dropped_datagrams.load(Ordering::Relaxed)
    }

    pub(crate) fn connect(&mut self, address: SocketAddr, pinned: Fingerprint) -> AttemptId {
        let attempt = self.next_client_attempt;
        self.next_client_attempt += 1;
        self.shared
            .connections
            .lock()
            .expect("connection table lock")
            .insert(attempt, Entry::Pending);
        let command = Command::Connect {
            attempt,
            address,
            pinned,
        };
        if self.commands.send(command).is_err() {
            self.shared.remove(attempt);
            self.local_events.push_back(TransportEvent::Closed {
                attempt: AttemptId::new(attempt),
                failure: TransportFailure::LocalShutdown,
            });
        }
        AttemptId::new(attempt)
    }

    pub(crate) fn poll(&mut self) -> Option<TransportEvent> {
        if let Some(event) = self.local_events.pop_front() {
            return Some(event);
        }
        self.events.try_recv().ok()
    }

    pub(crate) fn send(&mut self, attempt: AttemptId, lane: Lane, message: &[u8]) -> SendOutcome {
        let mut connections = self
            .shared
            .connections
            .lock()
            .expect("connection table lock");
        let Some(Entry::Open(handle)) = connections.get(&attempt.get()) else {
            return SendOutcome::Closed;
        };
        let queue = match lane {
            Lane::Control => &handle.control,
            Lane::Bulk => &handle.bulk,
            Lane::Datagram => {
                return match handle
                    .connection
                    .send_datagram(Bytes::copy_from_slice(message))
                {
                    Ok(()) => SendOutcome::Sent,
                    // The path shrank below the declared size. The datagram
                    // is lost, which the unreliable contract permits.
                    Err(SendDatagramError::TooLarge) => {
                        self.shared
                            .dropped_datagrams
                            .fetch_add(1, Ordering::Relaxed);
                        SendOutcome::Sent
                    }
                    Err(_) => SendOutcome::Closed,
                };
            }
        };
        if message.len() > self.config.max_reliable_message_bytes as usize {
            // A reliable message that cannot be carried breaks the lane's
            // contract; fail the connection explicitly instead of dropping it.
            connections.remove(&attempt.get());
            self.local_events.push_back(TransportEvent::Closed {
                attempt,
                failure: TransportFailure::ReliableDeliveryFailed,
            });
            return SendOutcome::Closed;
        }
        match queue.try_send(message.to_vec()) {
            Ok(()) => SendOutcome::Sent,
            Err(mpsc::error::TrySendError::Full(_)) => SendOutcome::Blocked,
            Err(mpsc::error::TrySendError::Closed(_)) => SendOutcome::Closed,
        }
    }

    /// Dropping the handle ends the lane queues; the connection task flushes
    /// what they already hold and then closes.
    pub(crate) fn close(&mut self, attempt: AttemptId) {
        self.shared.remove(attempt.get());
    }

    /// Flush, close every connection and join the worker thread.
    pub(crate) fn stop(&mut self) {
        let _ = self.commands.send(Command::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Backend {
    fn drop(&mut self) {
        self.stop();
    }
}

fn validate(config: &QuicConfig) -> Result<(), TransportError> {
    if !(256..=1_150).contains(&config.max_datagram_bytes) {
        return Err(TransportError::InvalidConfig("max_datagram_bytes"));
    }
    if config.max_reliable_message_bytes < 1_024 {
        return Err(TransportError::InvalidConfig("max_reliable_message_bytes"));
    }
    if config.send_queue_messages == 0 {
        return Err(TransportError::InvalidConfig("send_queue_messages"));
    }
    if config.event_queue_messages < 16 {
        return Err(TransportError::InvalidConfig("event_queue_messages"));
    }
    if config.keep_alive >= config.idle_timeout {
        return Err(TransportError::InvalidConfig("keep_alive"));
    }
    Ok(())
}

fn transport_config(config: &QuicConfig) -> Result<quinn::TransportConfig, TransportError> {
    let mut transport = quinn::TransportConfig::default();
    let idle = IdleTimeout::try_from(config.idle_timeout)
        .map_err(|_| TransportError::InvalidConfig("idle_timeout"))?;
    transport
        .max_idle_timeout(Some(idle))
        .keep_alive_interval(Some(config.keep_alive))
        // Exactly the two lanes, opened by the client.
        .max_concurrent_bidi_streams(VarInt::from_u32(2))
        .max_concurrent_uni_streams(VarInt::from_u32(0))
        .datagram_receive_buffer_size(Some(1 << 20))
        .datagram_send_buffer_size(1 << 20);
    Ok(transport)
}

fn make_endpoint(
    role: &Role,
    provider: &Arc<CryptoProvider>,
    transport: &Arc<quinn::TransportConfig>,
) -> Result<Endpoint, TransportError> {
    let tls_error = |error: &dyn std::fmt::Display| TransportError::Tls(error.to_string());
    match role {
        Role::Host { address, identity } => {
            let (certificate, key) = identity.tls_parts();
            let mut tls = rustls::ServerConfig::builder_with_provider(provider.clone())
                .with_protocol_versions(&[&rustls::version::TLS13])
                .map_err(|error| tls_error(&error))?
                .with_no_client_auth()
                .with_single_cert(vec![certificate], key)
                .map_err(|error| tls_error(&error))?;
            tls.alpn_protocols = vec![ALPN.to_vec()];
            let crypto = QuicServerConfig::try_from(tls).map_err(|error| tls_error(&error))?;
            let mut server = quinn::ServerConfig::with_crypto(Arc::new(crypto));
            server.transport_config(transport.clone());
            Endpoint::server(server, *address).map_err(TransportError::Io)
        }
        Role::Client { address } => Endpoint::client(*address).map_err(TransportError::Io),
    }
}

fn client_config(pinned: Fingerprint, context: &Context) -> Option<quinn::ClientConfig> {
    let verifier = PinnedVerifier::new(pinned, context.provider.clone());
    let mut tls = rustls::ClientConfig::builder_with_provider(context.provider.clone())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .ok()?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth();
    tls.alpn_protocols = vec![ALPN.to_vec()];
    let crypto = QuicClientConfig::try_from(tls).ok()?;
    let mut client = quinn::ClientConfig::new(Arc::new(crypto));
    client.transport_config(context.transport.clone());
    Some(client)
}

fn run(
    role: Role,
    config: QuicConfig,
    shared: Arc<Shared>,
    events: mpsc::Sender<TransportEvent>,
    mut commands: mpsc::UnboundedReceiver<Command>,
    ready: std::sync::mpsc::Sender<Result<SocketAddr, TransportError>>,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            let _ = ready.send(Err(TransportError::Worker(error.to_string())));
            return;
        }
    };
    runtime.block_on(async move {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let setup = transport_config(&config)
            .map(Arc::new)
            .and_then(|transport| {
                let endpoint = make_endpoint(&role, &provider, &transport)?;
                let address = endpoint.local_addr().map_err(TransportError::Io)?;
                Ok((transport, endpoint, address))
            });
        let (transport, endpoint, address) = match setup {
            Ok(parts) => parts,
            Err(error) => {
                let _ = ready.send(Err(error));
                return;
            }
        };
        let _ = ready.send(Ok(address));
        let flush = config.close_flush_timeout;
        let context = Arc::new(Context {
            shared,
            events,
            config,
            provider,
            transport,
            next_host_attempt: tokio::sync::Mutex::new(1),
        });
        if matches!(role, Role::Host { .. }) {
            tokio::spawn(accept_loop(endpoint.clone(), context.clone()));
        }
        while let Some(command) = commands.recv().await {
            match command {
                Command::Connect {
                    attempt,
                    address,
                    pinned,
                } => {
                    tokio::spawn(connect(
                        endpoint.clone(),
                        context.clone(),
                        attempt,
                        address,
                        pinned,
                    ));
                }
                Command::Shutdown => break,
            }
        }
        // Dropping every handle lets each connection flush and close itself.
        context
            .shared
            .connections
            .lock()
            .expect("connection table lock")
            .clear();
        let _ = timeout(flush, endpoint.wait_idle()).await;
        endpoint.close(VarInt::from_u32(CLOSE_NORMAL), b"shutdown");
        let _ = timeout(Duration::from_millis(250), endpoint.wait_idle()).await;
    });
}

struct Lanes {
    control: (SendStream, RecvStream),
    bulk: (SendStream, RecvStream),
}

/// Whether the path carries the datagram size this backend declared.
fn datagrams_fit(connection: &Connection, config: &QuicConfig) -> bool {
    connection
        .max_datagram_size()
        .is_some_and(|max| max >= usize::from(config.max_datagram_bytes))
}

async fn accept_loop(endpoint: Endpoint, context: Arc<Context>) {
    while let Some(incoming) = endpoint.accept().await {
        let context = context.clone();
        tokio::spawn(async move {
            let setup = async {
                let connection = incoming.await.ok()?;
                let lanes = accept_lanes(&connection).await?;
                Some((connection, lanes))
            };
            // A failed or slow handshake never becomes an attempt.
            let Ok(Some((connection, lanes))) =
                timeout(context.config.connect_timeout, setup).await
            else {
                return;
            };
            if !datagrams_fit(&connection, &context.config) {
                connection.close(VarInt::from_u32(CLOSE_PROTOCOL), b"datagram size");
                return;
            }
            let (handle, receivers) = handle_for(&connection, &context.config);
            let attempt = {
                let mut next = context.next_host_attempt.lock().await;
                let attempt = *next;
                *next += 1;
                context
                    .shared
                    .connections
                    .lock()
                    .expect("connection table lock")
                    .insert(attempt, Entry::Open(handle));
                // Queued under the lock, so attempts appear in increasing order.
                let connected = TransportEvent::Connected {
                    attempt: AttemptId::new(attempt),
                };
                if context.events.send(connected).await.is_err() {
                    return;
                }
                attempt
            };
            drive(attempt, connection, lanes, receivers, context).await;
        });
    }
}

async fn connect(
    endpoint: Endpoint,
    context: Arc<Context>,
    attempt: u64,
    address: SocketAddr,
    pinned: Fingerprint,
) {
    let setup = async {
        let config = client_config(pinned, &context).ok_or(TransportFailure::ConnectFailed)?;
        let connecting = endpoint
            .connect_with(config, address, SERVER_NAME)
            .map_err(|_| TransportFailure::ConnectFailed)?;
        let connection = connecting.await.map_err(|error| connect_failure(&error))?;
        let lanes = open_lanes(&connection)
            .await
            .ok_or(TransportFailure::ConnectFailed)?;
        if !datagrams_fit(&connection, &context.config) {
            connection.close(VarInt::from_u32(CLOSE_PROTOCOL), b"datagram size");
            return Err(TransportFailure::ConnectFailed);
        }
        Ok((connection, lanes))
    };
    let outcome = match timeout(context.config.connect_timeout, setup).await {
        Ok(outcome) => outcome,
        Err(_) => Err(TransportFailure::TimedOut),
    };
    let (connection, lanes) = match outcome {
        Ok(parts) => parts,
        Err(failure) => {
            // Silent if the application already cancelled this attempt.
            if context.shared.remove(attempt).is_some() {
                let closed = TransportEvent::Closed {
                    attempt: AttemptId::new(attempt),
                    failure,
                };
                let _ = context.events.send(closed).await;
            }
            return;
        }
    };
    let (handle, receivers) = handle_for(&connection, &context.config);
    {
        let mut connections = context
            .shared
            .connections
            .lock()
            .expect("connection table lock");
        match connections.get_mut(&attempt) {
            Some(entry) => *entry = Entry::Open(handle),
            None => {
                // Cancelled while connecting: close without a word.
                connection.close(VarInt::from_u32(CLOSE_NORMAL), b"");
                return;
            }
        }
    }
    let connected = TransportEvent::Connected {
        attempt: AttemptId::new(attempt),
    };
    if context.events.send(connected).await.is_err() {
        return;
    }
    drive(attempt, connection, lanes, receivers, context).await;
}

type LaneReceivers = (mpsc::Receiver<Vec<u8>>, mpsc::Receiver<Vec<u8>>);

fn handle_for(connection: &Connection, config: &QuicConfig) -> (Handle, LaneReceivers) {
    let (control, control_rx) = mpsc::channel(config.send_queue_messages);
    let (bulk, bulk_rx) = mpsc::channel(config.send_queue_messages);
    let handle = Handle {
        control,
        bulk,
        connection: connection.clone(),
    };
    (handle, (control_rx, bulk_rx))
}

/// The client opens both lanes and names each with its first byte.
async fn open_lanes(connection: &Connection) -> Option<Lanes> {
    let mut control = connection.open_bi().await.ok()?;
    control.0.write_all(&[TAG_CONTROL]).await.ok()?;
    let mut bulk = connection.open_bi().await.ok()?;
    bulk.0.write_all(&[TAG_BULK]).await.ok()?;
    Some(Lanes { control, bulk })
}

/// The host accepts exactly two streams and requires one of each lane.
async fn accept_lanes(connection: &Connection) -> Option<Lanes> {
    let mut control = None;
    let mut bulk = None;
    for _ in 0..2 {
        let (send, mut recv) = connection.accept_bi().await.ok()?;
        let mut tag = [0];
        recv.read_exact(&mut tag).await.ok()?;
        let slot = match tag[0] {
            TAG_CONTROL => &mut control,
            TAG_BULK => &mut bulk,
            _ => return None,
        };
        if slot.replace((send, recv)).is_some() {
            return None;
        }
    }
    Some(Lanes {
        control: control?,
        bulk: bulk?,
    })
}

/// Run one established connection until it ends.
async fn drive(
    attempt: u64,
    connection: Connection,
    lanes: Lanes,
    receivers: LaneReceivers,
    context: Arc<Context>,
) {
    let flush = context.config.close_flush_timeout;
    let (control_send, control_recv) = lanes.control;
    let (bulk_send, bulk_recv) = lanes.bulk;
    let writers = [
        tokio::spawn(write_lane(control_send, receivers.0, flush)),
        tokio::spawn(write_lane(bulk_send, receivers.1, flush)),
    ];
    let readers = [
        tokio::spawn(read_lane(
            Lane::Control,
            control_recv,
            attempt,
            connection.clone(),
            context.clone(),
        )),
        tokio::spawn(read_lane(
            Lane::Bulk,
            bulk_recv,
            attempt,
            connection.clone(),
            context.clone(),
        )),
        tokio::spawn(read_datagrams(attempt, connection.clone(), context.clone())),
    ];
    // Both writers finish once the application closed the attempt and the
    // queued reliable data was flushed, or once the connection died.
    let closer = {
        let connection = connection.clone();
        tokio::spawn(async move {
            for writer in writers {
                let _ = writer.await;
            }
            connection.close(VarInt::from_u32(CLOSE_NORMAL), b"");
        })
    };

    let reason = connection.closed().await;
    // Still registered means the application did not ask for this close.
    let unrequested = context.shared.remove(attempt).is_some();
    let _ = closer.await;
    // Readers drain what arrived before the close, so `Closed` comes last.
    for reader in readers {
        let _ = reader.await;
    }
    if unrequested {
        let closed = TransportEvent::Closed {
            attempt: AttemptId::new(attempt),
            failure: close_failure(&reason),
        };
        let _ = context.events.send(closed).await;
    }
}

async fn write_lane(mut send: SendStream, mut queue: mpsc::Receiver<Vec<u8>>, flush: Duration) {
    while let Some(message) = queue.recv().await {
        let length = (message.len() as u32).to_be_bytes();
        if send.write_all(&length).await.is_err() || send.write_all(&message).await.is_err() {
            return;
        }
    }
    // The queue ended: wait, within a bound, for the peer to receive it all.
    if send.finish().is_ok() {
        let _ = timeout(flush, send.stopped()).await;
    }
}

async fn read_lane(
    lane: Lane,
    mut recv: RecvStream,
    attempt: u64,
    connection: Connection,
    context: Arc<Context>,
) {
    loop {
        let mut length = [0; 4];
        if recv.read_exact(&mut length).await.is_err() {
            return;
        }
        let length = u32::from_be_bytes(length);
        // Checked before allocating.
        if length > context.config.max_reliable_message_bytes {
            connection.close(VarInt::from_u32(CLOSE_PROTOCOL), b"message too large");
            return;
        }
        let mut bytes = vec![0; length as usize];
        if recv.read_exact(&mut bytes).await.is_err() {
            return;
        }
        let message = TransportEvent::Message {
            attempt: AttemptId::new(attempt),
            lane,
            bytes,
        };
        // Waiting here is the backpressure: QUIC flow control stops the peer.
        if context.events.send(message).await.is_err() {
            return;
        }
    }
}

async fn read_datagrams(attempt: u64, connection: Connection, context: Arc<Context>) {
    while let Ok(bytes) = connection.read_datagram().await {
        let message = TransportEvent::Message {
            attempt: AttemptId::new(attempt),
            lane: Lane::Datagram,
            bytes: bytes.to_vec(),
        };
        // Never wait for an unreliable message.
        if context.events.try_send(message).is_err() {
            context
                .shared
                .dropped_datagrams
                .fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// QUIC reserves this transport error range for TLS alerts.
fn is_crypto_error(error: &ConnectionError) -> bool {
    match error {
        ConnectionError::TransportError(error) => {
            let code = u64::from(error.code);
            (0x100..0x200).contains(&code)
        }
        _ => false,
    }
}

fn connect_failure(error: &ConnectionError) -> TransportFailure {
    if is_crypto_error(error) {
        TransportFailure::AuthenticationFailed
    } else if matches!(error, ConnectionError::TimedOut) {
        TransportFailure::TimedOut
    } else {
        TransportFailure::ConnectFailed
    }
}

fn close_failure(error: &ConnectionError) -> TransportFailure {
    match error {
        ConnectionError::ApplicationClosed(_)
        | ConnectionError::ConnectionClosed(_)
        | ConnectionError::Reset => TransportFailure::RemoteClosed,
        ConnectionError::TimedOut => TransportFailure::TimedOut,
        _ => TransportFailure::Other,
    }
}
