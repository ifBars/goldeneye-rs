# ge-net-transport

The QUIC backend for [`ge-net`](../ge-net/README.md). It implements
`ge_net::Transport` and owns what the core deliberately does not: the UDP
socket, the async runtime, the worker thread and the host's TLS identity.

This is direct connection only. It does not do NAT traversal, relaying,
matchmaking or account services, and it has been exercised on the loopback
interface only.

## Selected backend

| | |
| --- | --- |
| Transport | QUIC, through `quinn` 0.11.12 (MIT OR Apache-2.0) |
| TLS | `rustls` 0.23 with the `ring` provider, TLS 1.3 only |
| Identity | `rcgen` 0.14 self-signed certificate, pinned by SHA-256 fingerprint |
| Runtime | `tokio`, current-thread, on one worker thread owned by the endpoint |

QUIC was chosen because one maintained implementation supplies everything the
transport contract asks for: encryption and peer authentication, congestion
control, independent reliable ordered streams, and unreliable datagrams
(RFC 9221). This crate adds no reliability, ordering or cryptography of its
own. Dependency licenses are collected by `cargo xtask notices`.

## How the contract is met

| Contract | Mechanism |
| --- | --- |
| Control lane, reliable ordered | A bidirectional stream; messages prefixed by a 32-bit length |
| Bulk lane, reliable ordered | A second stream, with its own flow control |
| Datagram lane, unreliable | QUIC datagrams; never carried on a stream |
| Usable datagram size | Declared as 1100 bytes by default. A connection whose path cannot carry it is refused at setup |
| Congestion | A full lane queue makes `send` return `Blocked`; nothing is buffered without bound |
| Inbound pressure | Reliable lanes stop reading when the event queue is full, which stalls the sender through QUIC flow control. Datagrams are dropped and counted |
| Closing | Reliable messages already accepted are flushed, within a bound, before the connection closes |
| Shutdown | `shutdown` or drop flushes, closes every connection and joins the worker thread |

## Identity

A host has a `HostIdentity`: a self-signed certificate and its private key.
`HostIdentity::generate()` makes one from the operating system's secure random
source; `from_der` restores one the application stored. A client connects with
the certificate's `Fingerprint`, which it must receive out of band.

The client accepts exactly one certificate, the pinned one, and the handshake
still verifies the host's signature with that certificate's key. A host that
cannot prove the pinned identity is reported as
`TransportFailure::AuthenticationFailed`. There is no mode that accepts any
certificate.

Limits of this policy, stated plainly:

- The fingerprint is public. It identifies a host; it does not admit a client.
  Clients are not authenticated individually. The host knows each one only as
  its connection.
- Whoever controls the channel that delivers the fingerprint controls which
  host the client trusts.
- Keep the private key out of logs, version control and reports. `Debug` on
  `HostIdentity` prints the fingerprint only.

## Use

```rust,no_run
use std::net::{Ipv4Addr, SocketAddr};
use ge_net::Transport;
use ge_net_transport::{HostIdentity, QuicClient, QuicConfig, QuicHost};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let identity = HostIdentity::generate()?;
let host = QuicHost::bind(
    SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
    identity,
    QuicConfig::default(),
)?;

let mut client = QuicClient::new(QuicConfig::default())?;
let attempt = client.connect(host.local_addr(), host.fingerprint());
// Poll `client` and `host` like any `ge_net::Transport`, for example with
// `ge_net::transport::pump_client` and `pump_host`.
# let _ = (attempt, client.capabilities());
# Ok(())
# }
```

## Evidence

```text
cargo test --locked -p ge-net-transport
```

- `tests/loopback.rs` checks the contract in one process over real loopback
  sockets: the declared datagram and reliable sizes really cross the encrypted
  path, a reliable lane blocks under backpressure without dropping or
  reordering, a backed-up bulk lane does not block the control lane, a wrong
  pin fails authentication, closing flushes first, and shutdown joins.
- `tests/processes.rs` runs one host and two clients as separate
  operating-system processes (`ge-net-loopback`), on a port the operating
  system assigns. Every state a client applied must equal what the host
  published for that tick.

These are loopback results. They show the adapter honours the contract and
that separate processes can hold a session. They say nothing about loss,
latency or reachability on a real network.
