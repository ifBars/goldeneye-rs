//! Version-one message kinds and payload codecs.
//!
//! Payloads have exact lengths. Unknown trailing bytes are an error, so a
//! new field needs a new wire version or a negotiated optional kind.

use super::compat::{
    CompatibilityDescriptor, MAX_EXTENSIONS, Mismatch, decode_extensions, encode_extensions,
};
use super::envelope::Header;
use super::wire::{Reader, put_u8, put_u16, put_u32, put_u64};
use super::{DecodeError, EncodeError};
use crate::channel::ChannelId;
use crate::identity::{CommandSeq, PeerId, PlayerSlot, SnapshotId};
use crate::replication::CommandAck;

/// Kinds with this bit are optional extensions. A receiver that does not
/// know one skips it, within a bounded count, after accounting for its
/// sequence. Every other unknown kind is a terminal error.
pub const OPTIONAL_KIND_BIT: u8 = 0x80;

/// The mandatory message kinds of wire version one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MessageKind {
    Hello = 0x01,
    Accept = 0x02,
    Reject = 0x03,
    SnapshotRequest = 0x04,
    SyncBegin = 0x05,
    SyncChunk = 0x06,
    SyncApplied = 0x07,
    Activate = 0x08,
    Ready = 0x09,
    Close = 0x0a,
    InputBatch = 0x10,
    StateAck = 0x11,
    StateUpdate = 0x12,
    Ping = 0x20,
    Pong = 0x21,
}

impl MessageKind {
    pub const ALL: [Self; 15] = [
        Self::Hello,
        Self::Accept,
        Self::Reject,
        Self::SnapshotRequest,
        Self::SyncBegin,
        Self::SyncChunk,
        Self::SyncApplied,
        Self::Activate,
        Self::Ready,
        Self::Close,
        Self::InputBatch,
        Self::StateAck,
        Self::StateUpdate,
        Self::Ping,
        Self::Pong,
    ];

    pub fn from_wire(value: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.wire() == value)
    }

    pub const fn wire(self) -> u8 {
        self as u8
    }

    /// The only channel this kind is defined on.
    pub const fn channel(self) -> ChannelId {
        match self {
            Self::Hello
            | Self::Accept
            | Self::Reject
            | Self::SnapshotRequest
            | Self::SyncApplied
            | Self::Activate
            | Self::Ready
            | Self::Close => ChannelId::Control,
            Self::SyncBegin | Self::SyncChunk => ChannelId::Bulk,
            Self::InputBatch | Self::StateAck => ChannelId::Input,
            Self::StateUpdate => ChannelId::State,
            Self::Ping | Self::Pong => ChannelId::Probe,
        }
    }

    /// Whether the envelope carries a simulation tick.
    pub const fn has_tick(self) -> bool {
        matches!(
            self,
            Self::SyncBegin | Self::Activate | Self::Ready | Self::InputBatch | Self::StateUpdate
        )
    }

    /// Whether the logical message may span several datagrams.
    pub const fn fragmentable(self) -> bool {
        matches!(self, Self::StateUpdate)
    }
}

/// How a raw kind byte is treated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KindClass {
    Known(MessageKind),
    /// Not defined here, but marked skippable.
    UnknownOptional,
}

/// Classify a header's kind and check the envelope rules for known kinds.
pub fn classify(header: &Header) -> Result<KindClass, DecodeError> {
    let Some(kind) = MessageKind::from_wire(header.kind) else {
        return if header.kind & OPTIONAL_KIND_BIT != 0 {
            // Only `StateUpdate` may be fragmented. An unknown kind must not
            // be, or partial assemblies would bypass the skip budget.
            if header.fragment.is_some() {
                return Err(DecodeError::FragmentNotAllowed { kind: header.kind });
            }
            Ok(KindClass::UnknownOptional)
        } else {
            Err(DecodeError::UnknownMandatoryKind(header.kind))
        };
    };
    if kind.channel() != header.channel {
        return Err(DecodeError::WrongChannel {
            kind: header.kind,
            channel: header.channel.wire(),
        });
    }
    if kind.has_tick() != header.tick.is_some() {
        return Err(DecodeError::TickPresence { kind: header.kind });
    }
    if header.fragment.is_some() && !kind.fragmentable() {
        return Err(DecodeError::FragmentNotAllowed { kind: header.kind });
    }
    Ok(KindClass::Known(kind))
}

/// Limits a decoder enforces before it allocates.
#[derive(Clone, Copy, Debug)]
pub struct CodecLimits {
    pub max_command_bytes: usize,
    pub max_slots_per_peer: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hello {
    pub requested_slots: u8,
    pub descriptor: CompatibilityDescriptor,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Accept {
    pub peer: PeerId,
    pub slots: Vec<PlayerSlot>,
    pub extensions: Vec<u32>,
    /// First command sequence of every slot in this membership.
    pub command_origin: CommandSeq,
}

/// Why the host refused a `Hello`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RejectReason {
    Incompatible(Mismatch),
    /// The peer limit is reached.
    SessionFull,
    /// The requested player slots are not available.
    SlotsUnavailable,
    /// The request itself was out of range.
    InvalidRequest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SnapshotRequest {
    /// The newest snapshot the client still holds, if any.
    pub held: Option<SnapshotId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncBegin {
    pub snapshot: SnapshotId,
    pub total_bytes: u32,
    pub chunk_count: u16,
    /// What the host had done with this client's commands when the transfer
    /// started. Strictly ascending by slot.
    pub acks: Vec<CommandAck>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SyncChunk<'a> {
    pub snapshot: SnapshotId,
    pub index: u16,
    pub data: &'a [u8],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WireCommand<'a> {
    /// Ticks after the envelope's base tick.
    pub tick_offset: u16,
    pub payload: &'a [u8],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandGroup<'a> {
    pub slot: PlayerSlot,
    /// Sequence of the first command; the rest follow consecutively.
    pub first: CommandSeq,
    pub commands: Vec<WireCommand<'a>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputBatch<'a> {
    /// The newest snapshot the client holds as a baseline.
    pub ack: Option<SnapshotId>,
    /// Strictly ascending by slot; each holds at least one command.
    pub groups: Vec<CommandGroup<'a>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateUpdate<'a> {
    pub snapshot: SnapshotId,
    /// The acknowledged snapshot this delta applies to; `None` means empty.
    pub baseline: Option<SnapshotId>,
    /// Strictly ascending by slot.
    pub acks: Vec<CommandAck>,
    pub delta: &'a [u8],
}

/// A decoded version-one message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message<'a> {
    Hello(Hello),
    Accept(Accept),
    Reject(RejectReason),
    SnapshotRequest(SnapshotRequest),
    SyncBegin(SyncBegin),
    SyncChunk(SyncChunk<'a>),
    SyncApplied(SnapshotId),
    Activate,
    Ready,
    Close(u8),
    InputBatch(InputBatch<'a>),
    StateAck(SnapshotId),
    StateUpdate(StateUpdate<'a>),
    Ping(u64),
    Pong(u64),
}

const FLAG_PRESENT: u8 = 1;

fn put_optional_snapshot(out: &mut Vec<u8>, value: Option<SnapshotId>) {
    put_u8(out, if value.is_some() { FLAG_PRESENT } else { 0 });
    put_u32(out, value.map_or(0, SnapshotId::get));
}

fn optional_snapshot(reader: &mut Reader<'_>) -> Result<Option<SnapshotId>, DecodeError> {
    let flags = reader.u8()?;
    let value = reader.u32()?;
    match (flags, value) {
        (FLAG_PRESENT, value) => Ok(Some(SnapshotId::new(value))),
        (0, 0) => Ok(None),
        _ => Err(DecodeError::InvalidField("optional snapshot")),
    }
}

impl Message<'_> {
    pub const fn kind(&self) -> MessageKind {
        match self {
            Self::Hello(_) => MessageKind::Hello,
            Self::Accept(_) => MessageKind::Accept,
            Self::Reject(_) => MessageKind::Reject,
            Self::SnapshotRequest(_) => MessageKind::SnapshotRequest,
            Self::SyncBegin(_) => MessageKind::SyncBegin,
            Self::SyncChunk(_) => MessageKind::SyncChunk,
            Self::SyncApplied(_) => MessageKind::SyncApplied,
            Self::Activate => MessageKind::Activate,
            Self::Ready => MessageKind::Ready,
            Self::Close(_) => MessageKind::Close,
            Self::InputBatch(_) => MessageKind::InputBatch,
            Self::StateAck(_) => MessageKind::StateAck,
            Self::StateUpdate(_) => MessageKind::StateUpdate,
            Self::Ping(_) => MessageKind::Ping,
            Self::Pong(_) => MessageKind::Pong,
        }
    }

    /// Append this message's payload to `out`.
    pub fn encode_payload(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        match self {
            Self::Hello(hello) => {
                put_u8(out, hello.requested_slots);
                hello.descriptor.encode(out)?;
            }
            Self::Accept(accept) => {
                put_u16(out, accept.peer.get());
                let slots = u8::try_from(accept.slots.len())
                    .map_err(|_| EncodeError::CountTooLarge("slots"))?;
                put_u8(out, slots);
                for slot in &accept.slots {
                    put_u8(out, slot.get());
                }
                if accept.extensions.len() > MAX_EXTENSIONS {
                    return Err(EncodeError::CountTooLarge("extensions"));
                }
                encode_extensions(&accept.extensions, out);
                put_u32(out, accept.command_origin.get());
            }
            Self::Reject(reason) => {
                let (code, field, detail) = match reason {
                    RejectReason::Incompatible(mismatch) => {
                        let (field, detail) = mismatch.to_wire();
                        (1, field, detail)
                    }
                    RejectReason::SessionFull => (2, 0, 0),
                    RejectReason::SlotsUnavailable => (3, 0, 0),
                    RejectReason::InvalidRequest => (4, 0, 0),
                };
                put_u8(out, code);
                put_u8(out, field);
                put_u64(out, detail);
            }
            Self::SnapshotRequest(request) => put_optional_snapshot(out, request.held),
            Self::SyncBegin(begin) => {
                put_u32(out, begin.snapshot.get());
                put_u32(out, begin.total_bytes);
                put_u16(out, begin.chunk_count);
                put_acks(out, &begin.acks)?;
            }
            Self::SyncChunk(chunk) => {
                put_u32(out, chunk.snapshot.get());
                put_u16(out, chunk.index);
                out.extend_from_slice(chunk.data);
            }
            Self::SyncApplied(snapshot) | Self::StateAck(snapshot) => {
                put_u32(out, snapshot.get());
            }
            Self::Activate | Self::Ready => {}
            Self::Close(code) => put_u8(out, *code),
            Self::InputBatch(batch) => {
                put_optional_snapshot(out, batch.ack);
                let groups = u8::try_from(batch.groups.len())
                    .map_err(|_| EncodeError::CountTooLarge("command groups"))?;
                put_u8(out, groups);
                for group in &batch.groups {
                    put_u8(out, group.slot.get());
                    put_u32(out, group.first.get());
                    let count = u8::try_from(group.commands.len())
                        .map_err(|_| EncodeError::CountTooLarge("commands"))?;
                    put_u8(out, count);
                    for command in &group.commands {
                        put_u16(out, command.tick_offset);
                        let len = u16::try_from(command.payload.len())
                            .map_err(|_| EncodeError::CountTooLarge("command bytes"))?;
                        put_u16(out, len);
                        out.extend_from_slice(command.payload);
                    }
                }
            }
            Self::StateUpdate(update) => {
                put_u32(out, update.snapshot.get());
                put_optional_snapshot(out, update.baseline);
                put_acks(out, &update.acks)?;
                out.extend_from_slice(update.delta);
            }
            Self::Ping(stamp) | Self::Pong(stamp) => put_u64(out, *stamp),
        }
        Ok(())
    }
}

/// Decode the payload of a known kind.
pub fn decode_payload<'a>(
    kind: MessageKind,
    payload: &'a [u8],
    limits: &CodecLimits,
) -> Result<Message<'a>, DecodeError> {
    let mut reader = Reader::new(payload);
    let message = match kind {
        MessageKind::Hello => Message::Hello(Hello {
            requested_slots: reader.u8()?,
            descriptor: CompatibilityDescriptor::decode(&mut reader)?,
        }),
        MessageKind::Accept => {
            let peer = PeerId::new(reader.u16()?);
            let count = usize::from(reader.u8()?);
            if count == 0 || count > limits.max_slots_per_peer {
                return Err(DecodeError::LimitExceeded("slots"));
            }
            let mut slots: Vec<PlayerSlot> = Vec::with_capacity(count);
            for _ in 0..count {
                let slot = PlayerSlot::new(reader.u8()?);
                if slots.last().is_some_and(|last| *last >= slot) {
                    return Err(DecodeError::InvalidField("slots"));
                }
                slots.push(slot);
            }
            Message::Accept(Accept {
                peer,
                slots,
                extensions: decode_extensions(&mut reader)?,
                command_origin: CommandSeq::new(reader.u32()?),
            })
        }
        MessageKind::Reject => {
            let code = reader.u8()?;
            let field = reader.u8()?;
            let detail = reader.u64()?;
            Message::Reject(match (code, field, detail) {
                (1, field, detail) => RejectReason::Incompatible(
                    Mismatch::from_wire(field, detail)
                        .ok_or(DecodeError::InvalidField("mismatch field"))?,
                ),
                (2, 0, 0) => RejectReason::SessionFull,
                (3, 0, 0) => RejectReason::SlotsUnavailable,
                (4, 0, 0) => RejectReason::InvalidRequest,
                _ => return Err(DecodeError::InvalidField("reject reason")),
            })
        }
        MessageKind::SnapshotRequest => Message::SnapshotRequest(SnapshotRequest {
            held: optional_snapshot(&mut reader)?,
        }),
        MessageKind::SyncBegin => Message::SyncBegin(SyncBegin {
            snapshot: SnapshotId::new(reader.u32()?),
            total_bytes: reader.u32()?,
            chunk_count: reader.u16()?,
            acks: decode_acks(&mut reader)?,
        }),
        MessageKind::SyncChunk => Message::SyncChunk(SyncChunk {
            snapshot: SnapshotId::new(reader.u32()?),
            index: reader.u16()?,
            data: reader.rest(),
        }),
        MessageKind::SyncApplied => Message::SyncApplied(SnapshotId::new(reader.u32()?)),
        MessageKind::Activate => Message::Activate,
        MessageKind::Ready => Message::Ready,
        MessageKind::Close => Message::Close(reader.u8()?),
        MessageKind::InputBatch => Message::InputBatch(decode_input_batch(&mut reader, limits)?),
        MessageKind::StateAck => Message::StateAck(SnapshotId::new(reader.u32()?)),
        MessageKind::StateUpdate => Message::StateUpdate(decode_state_update(&mut reader)?),
        MessageKind::Ping => Message::Ping(reader.u64()?),
        MessageKind::Pong => Message::Pong(reader.u64()?),
    };
    reader.finish()?;
    Ok(message)
}

fn decode_input_batch<'a>(
    reader: &mut Reader<'a>,
    limits: &CodecLimits,
) -> Result<InputBatch<'a>, DecodeError> {
    let ack = optional_snapshot(reader)?;
    let group_count = usize::from(reader.u8()?);
    if group_count == 0 || group_count > limits.max_slots_per_peer {
        return Err(DecodeError::LimitExceeded("command groups"));
    }
    let mut groups: Vec<CommandGroup<'a>> = Vec::with_capacity(group_count);
    for _ in 0..group_count {
        let slot = PlayerSlot::new(reader.u8()?);
        if groups.last().is_some_and(|last| last.slot >= slot) {
            return Err(DecodeError::InvalidField("command group order"));
        }
        let first = CommandSeq::new(reader.u32()?);
        let count = usize::from(reader.u8()?);
        if count == 0 {
            return Err(DecodeError::InvalidField("empty command group"));
        }
        // Each command needs at least four bytes, which bounds this capacity.
        if count * 4 > reader.remaining() {
            return Err(DecodeError::Truncated);
        }
        let mut commands = Vec::with_capacity(count);
        for _ in 0..count {
            let tick_offset = reader.u16()?;
            let len = usize::from(reader.u16()?);
            if len > limits.max_command_bytes {
                return Err(DecodeError::LimitExceeded("command bytes"));
            }
            commands.push(WireCommand {
                tick_offset,
                payload: reader.bytes(len)?,
            });
        }
        groups.push(CommandGroup {
            slot,
            first,
            commands,
        });
    }
    Ok(InputBatch { ack, groups })
}

fn decode_state_update<'a>(reader: &mut Reader<'a>) -> Result<StateUpdate<'a>, DecodeError> {
    let snapshot = SnapshotId::new(reader.u32()?);
    let baseline = optional_snapshot(reader)?;
    let acks = decode_acks(reader)?;
    Ok(StateUpdate {
        snapshot,
        baseline,
        acks,
        delta: reader.rest(),
    })
}

/// `u8` count, then slot, received-next and processed-next per entry.
fn put_acks(out: &mut Vec<u8>, acks: &[CommandAck]) -> Result<(), EncodeError> {
    let count = u8::try_from(acks.len()).map_err(|_| EncodeError::CountTooLarge("command acks"))?;
    put_u8(out, count);
    for ack in acks {
        put_u8(out, ack.slot.get());
        put_u32(out, ack.received_next.get());
        put_u32(out, ack.processed_next.get());
    }
    Ok(())
}

fn decode_acks(reader: &mut Reader<'_>) -> Result<Vec<CommandAck>, DecodeError> {
    let count = usize::from(reader.u8()?);
    // Nine bytes each, checked before allocating.
    if count * 9 > reader.remaining() {
        return Err(DecodeError::Truncated);
    }
    let mut acks: Vec<CommandAck> = Vec::with_capacity(count);
    for _ in 0..count {
        let slot = PlayerSlot::new(reader.u8()?);
        if acks.last().is_some_and(|last| last.slot >= slot) {
            return Err(DecodeError::InvalidField("command ack order"));
        }
        acks.push(CommandAck {
            slot,
            received_next: CommandSeq::new(reader.u32()?),
            processed_next: CommandSeq::new(reader.u32()?),
        });
    }
    Ok(acks)
}
