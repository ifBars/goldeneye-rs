//! Required session budgets.
//!
//! Every queue, history and unit of per-update work is bounded by a value
//! here. Both ends of a session should use the same profile: a peer that
//! exceeds the other side's limit is closed with a typed reason.
//!
//! | Limit exceeded | Who closes | What survives |
//! | --- | --- | --- |
//! | Peer or handshake count | Host refuses the attempt | Existing members |
//! | Reliable outbound queue bytes | The queueing side closes that connection | Other peers and the session |
//! | Unreliable outbound queue bytes | Nobody; oldest unreliable messages are dropped and counted | The connection |
//! | Inbound messages per update, pending events | Nobody; `ingest` refuses without consuming | The message, for a retry after `advance` or `poll_event` |
//! | Message, snapshot, entity, fragment or command size received | The receiver closes that connection as a violation | Other peers and the session |
//! | Snapshot or entity size published locally | Nobody; `publish_snapshot` returns an error | The session and prior snapshots |
//! | Command outbox | Nobody; `submit_command` returns an error | The connection and queued commands |
//! | Handshake, synchronization or idle time | The side whose timer expires closes that connection | Other peers and the session |
//! | Skipped optional messages | The receiver closes that connection | Other peers and the session |

use std::fmt;
use std::time::Duration;

use crate::protocol::envelope::{BASE_HEADER_BYTES, MAX_HEADER_BYTES, TICK_BYTES};
use crate::protocol::message::CodecLimits;
use crate::replication::ReplicationLimits;
use crate::transport::TransportCapabilities;

/// Limits for one session object. There is no implicit default: construct a
/// profile deliberately, starting from [`SessionBudget::small_session`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionBudget {
    /// Accepted members, excluding the host.
    pub max_peers: u16,
    /// Admitted attempts that have not finished negotiating.
    pub max_handshakes: u16,
    /// Player slots in the session.
    pub max_player_slots: u8,
    /// Player slots one peer may own.
    pub max_slots_per_peer: u8,
    /// Largest envelope on a reliable lane.
    pub max_message_bytes: u16,
    /// Largest datagram envelope; the transport's limit may be lower.
    pub max_datagram_bytes: u16,
    /// Outbound bytes queued for one connection across all channels. It
    /// must hold the larger of one reliable message and four snapshot
    /// chunks, plus one whole fragmented state update.
    pub max_queued_bytes_per_peer: u32,
    /// Messages `ingest` accepts between two `advance` calls.
    pub max_inbound_messages_per_update: u32,
    /// Events held before `ingest` refuses further input.
    pub max_pending_events: u32,
    /// Unknown optional messages skipped per connection before it is closed.
    pub max_skipped_optional_messages: u32,
    /// Entities in one snapshot; also the exclusive bound on entity indices.
    pub max_entities: u16,
    /// Opaque state bytes of one entity.
    pub max_entity_bytes: u16,
    /// Encoded bytes of one full snapshot.
    pub max_snapshot_bytes: u32,
    /// Snapshot bytes per bulk chunk.
    pub snapshot_chunk_bytes: u16,
    /// Datagrams one state update may span.
    pub max_update_fragments: u8,
    /// Snapshots retained for baseline lookup; at least two. Size it above
    /// the number of snapshots a client receives during one acknowledgement
    /// round trip, or the client will keep asking for full-state recovery.
    pub baseline_history: u8,
    /// Sustained bulk transfer rate for one connection.
    pub bulk_bytes_per_second: u32,
    /// Bulk bytes one connection may send in a burst.
    pub bulk_burst_bytes: u32,
    /// Opaque bytes of one command.
    pub max_command_bytes: u16,
    /// Unacknowledged commands per slot on the client, and commands buffered
    /// per slot on the host.
    pub max_pending_commands: u16,
    /// How many ticks ahead of the host's consumed tick a command may be.
    pub command_future_ticks: u16,
    /// From admission to a completed negotiation.
    pub handshake_timeout: Duration,
    /// From acceptance to an active member, including catch-up.
    pub sync_timeout: Duration,
    /// Silence after which a connection has failed.
    pub idle_timeout: Duration,
    /// How long a closing connection waits to hand over its last message.
    pub close_timeout: Duration,
    /// Interval between liveness probes.
    pub ping_interval: Duration,
    /// Interval between client input and acknowledgement datagrams.
    pub input_interval: Duration,
    /// Minimum spacing of a client's full-state recovery requests.
    pub recovery_retry: Duration,
    /// Round-trip samples retained per connection.
    pub max_rtt_samples: u8,
}

impl SessionBudget {
    /// A conservative profile for a small session. It is a starting point
    /// for tests and examples, not a measured recommendation for any game.
    pub const fn small_session() -> Self {
        Self {
            max_peers: 8,
            max_handshakes: 4,
            max_player_slots: 8,
            max_slots_per_peer: 4,
            max_message_bytes: 4_096,
            max_datagram_bytes: 1_200,
            max_queued_bytes_per_peer: 256 * 1024,
            max_inbound_messages_per_update: 1_024,
            max_pending_events: 1_024,
            max_skipped_optional_messages: 64,
            max_entities: 1_024,
            max_entity_bytes: 256,
            max_snapshot_bytes: 256 * 1024,
            snapshot_chunk_bytes: 1_024,
            max_update_fragments: 8,
            baseline_history: 64,
            bulk_bytes_per_second: 128 * 1024,
            bulk_burst_bytes: 16 * 1024,
            max_command_bytes: 64,
            max_pending_commands: 128,
            command_future_ticks: 120,
            handshake_timeout: Duration::from_secs(5),
            sync_timeout: Duration::from_secs(20),
            idle_timeout: Duration::from_secs(10),
            close_timeout: Duration::from_secs(2),
            ping_interval: Duration::from_millis(500),
            input_interval: Duration::from_micros(16_666),
            recovery_retry: Duration::from_secs(1),
            max_rtt_samples: 32,
        }
    }
}

/// A budget or transport that cannot carry a session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigError {
    /// The backend lacks two independent reliable ordered lanes.
    MissingReliableLanes { available: u8 },
    /// The backend has no unreliable datagram lane.
    MissingDatagrams,
    /// The usable datagram cannot hold a required message.
    DatagramTooSmall { usable: usize, required: usize },
    /// A budget value is zero, inconsistent or out of range.
    InvalidBudget(&'static str),
    /// The compatibility descriptor cannot be sent.
    InvalidDescriptor,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingReliableLanes { available } => write!(
                f,
                "transport offers {available} reliable ordered lanes; two are required"
            ),
            Self::MissingDatagrams => write!(f, "transport offers no unreliable datagrams"),
            Self::DatagramTooSmall { usable, required } => {
                write!(
                    f,
                    "usable datagram is {usable} bytes; {required} are required"
                )
            }
            Self::InvalidBudget(name) => write!(f, "invalid budget: {name}"),
            Self::InvalidDescriptor => write!(f, "invalid compatibility descriptor"),
        }
    }
}

impl std::error::Error for ConfigError {}

/// Sizes derived from a budget and the backend's declared capabilities.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Limits {
    /// Largest datagram envelope, after transport framing and encryption.
    pub datagram_bytes: usize,
    /// Largest envelope on a reliable lane.
    pub reliable_bytes: usize,
    /// Payload bytes of one state-update fragment.
    pub fragment_payload: usize,
    /// Largest state-update body, possibly fragmented.
    pub max_update_bytes: usize,
    /// Most chunks a full snapshot can need.
    pub max_chunks: usize,
    pub codec: CodecLimits,
    pub replication: ReplicationLimits,
}

impl Limits {
    pub(crate) fn derive(
        budget: &SessionBudget,
        capabilities: &TransportCapabilities,
    ) -> Result<Self, ConfigError> {
        if capabilities.reliable_lanes < 2 {
            return Err(ConfigError::MissingReliableLanes {
                available: capabilities.reliable_lanes,
            });
        }
        let transport_datagram = capabilities
            .max_datagram_bytes
            .ok_or(ConfigError::MissingDatagrams)?;
        let nonzero: [(&'static str, u64); 17] = [
            ("max_peers", budget.max_peers.into()),
            ("max_handshakes", budget.max_handshakes.into()),
            ("max_player_slots", budget.max_player_slots.into()),
            ("max_slots_per_peer", budget.max_slots_per_peer.into()),
            (
                "max_inbound_messages_per_update",
                budget.max_inbound_messages_per_update.into(),
            ),
            ("max_pending_events", budget.max_pending_events.into()),
            ("max_entities", budget.max_entities.into()),
            ("max_snapshot_bytes", budget.max_snapshot_bytes.into()),
            ("snapshot_chunk_bytes", budget.snapshot_chunk_bytes.into()),
            ("max_update_fragments", budget.max_update_fragments.into()),
            ("baseline_history", budget.baseline_history.into()),
            ("bulk_bytes_per_second", budget.bulk_bytes_per_second.into()),
            ("max_pending_commands", budget.max_pending_commands.into()),
            ("handshake_timeout", micros(budget.handshake_timeout)),
            ("sync_timeout", micros(budget.sync_timeout)),
            ("idle_timeout", micros(budget.idle_timeout)),
            ("ping_interval", micros(budget.ping_interval)),
        ];
        if let Some((name, _)) = nonzero.iter().find(|(_, value)| *value == 0) {
            return Err(ConfigError::InvalidBudget(name));
        }
        if budget.max_slots_per_peer > budget.max_player_slots {
            return Err(ConfigError::InvalidBudget("max_slots_per_peer"));
        }
        // A client acknowledges its newest snapshot and must still hold it
        // when the host starts using it as a baseline one update later.
        if budget.baseline_history < 2 {
            return Err(ConfigError::InvalidBudget("baseline_history"));
        }
        if budget.ping_interval >= budget.idle_timeout {
            return Err(ConfigError::InvalidBudget("ping_interval"));
        }

        let datagram_bytes = usize::from(budget.max_datagram_bytes.min(transport_datagram));
        let reliable_bytes = usize::from(budget.max_message_bytes)
            .min(capabilities.max_reliable_message_bytes as usize);
        // One input batch holding one largest command, with its group header.
        let required_datagram =
            BASE_HEADER_BYTES + TICK_BYTES + 6 + 6 + 4 + usize::from(budget.max_command_bytes);
        if datagram_bytes < required_datagram.max(MAX_HEADER_BYTES + 32) {
            return Err(ConfigError::DatagramTooSmall {
                usable: datagram_bytes,
                required: required_datagram.max(MAX_HEADER_BYTES + 32),
            });
        }
        let fragment_payload = datagram_bytes - MAX_HEADER_BYTES;
        let max_update_bytes = if budget.max_update_fragments >= 2 {
            fragment_payload * usize::from(budget.max_update_fragments)
        } else {
            datagram_bytes - BASE_HEADER_BYTES - TICK_BYTES
        };

        let chunk = usize::from(budget.snapshot_chunk_bytes);
        // A chunk payload carries a snapshot id and a chunk index before the data.
        if BASE_HEADER_BYTES + 6 + chunk > reliable_bytes {
            return Err(ConfigError::InvalidBudget("snapshot_chunk_bytes"));
        }
        // Hello is the largest fixed control message.
        if reliable_bytes < 512 {
            return Err(ConfigError::InvalidBudget("max_message_bytes"));
        }
        let max_chunks = (budget.max_snapshot_bytes as usize).div_ceil(chunk);
        if max_chunks > usize::from(u16::MAX) {
            return Err(ConfigError::InvalidBudget("max_snapshot_bytes"));
        }
        let chunk_envelope = BASE_HEADER_BYTES + 6 + chunk;
        if (budget.bulk_burst_bytes as usize) < chunk_envelope {
            return Err(ConfigError::InvalidBudget("bulk_burst_bytes"));
        }
        // The largest state update, with every fragment's envelope.
        let update_envelopes = if budget.max_update_fragments >= 2 {
            datagram_bytes * usize::from(budget.max_update_fragments)
        } else {
            datagram_bytes
        };
        // The queue must hold a whole update beside a reliable message, or a
        // fragmented update could never be queued while control traffic waits.
        let queue_floor = reliable_bytes.max(4 * chunk_envelope) + update_envelopes;
        if (budget.max_queued_bytes_per_peer as usize) < queue_floor {
            return Err(ConfigError::InvalidBudget("max_queued_bytes_per_peer"));
        }

        Ok(Self {
            datagram_bytes,
            reliable_bytes,
            fragment_payload,
            max_update_bytes,
            max_chunks,
            codec: CodecLimits {
                max_command_bytes: usize::from(budget.max_command_bytes),
                max_slots_per_peer: usize::from(budget.max_slots_per_peer),
            },
            replication: ReplicationLimits {
                max_entities: usize::from(budget.max_entities),
                max_entity_bytes: usize::from(budget.max_entity_bytes),
                max_snapshot_bytes: budget.max_snapshot_bytes as usize,
            },
        })
    }
}

fn micros(duration: Duration) -> u64 {
    crate::time::duration_micros(duration)
}
