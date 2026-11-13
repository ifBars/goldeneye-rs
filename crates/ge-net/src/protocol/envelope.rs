//! The envelope that frames every message.
//!
//! ```text
//! offset size field
//!      0    1 wire version (1)
//!      1    1 message kind
//!      2    1 channel
//!      3    1 flags: bit 0 TICK, bit 1 FRAGMENT, bits 2-7 zero
//!      4    4 session epoch
//!      8    4 sequence
//!     12    2 payload length
//!     14    8 simulation tick            (only if TICK)
//!      +    1 fragment index             (only if FRAGMENT)
//!      +    1 fragment count             (only if FRAGMENT)
//!      +    n payload
//! ```
//!
//! The transport preserves message boundaries, so the envelope length must
//! equal the header plus the declared payload exactly.

use super::wire::{Reader, put_u8, put_u16, put_u32, put_u64};
use super::{DecodeError, EncodeError, WIRE_VERSION};
use crate::channel::ChannelId;
use crate::identity::{Sequence, SessionEpoch, Tick};

pub const FLAG_TICK: u8 = 0b0000_0001;
pub const FLAG_FRAGMENT: u8 = 0b0000_0010;
const FLAG_RESERVED: u8 = !(FLAG_TICK | FLAG_FRAGMENT);

/// Header bytes without the optional tick and fragment fields.
pub const BASE_HEADER_BYTES: usize = 14;
pub const TICK_BYTES: usize = 8;
pub const FRAGMENT_BYTES: usize = 2;
/// The largest header version one can produce.
pub const MAX_HEADER_BYTES: usize = BASE_HEADER_BYTES + TICK_BYTES + FRAGMENT_BYTES;

/// Position of one fragment within a logical message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fragment {
    pub index: u8,
    pub count: u8,
}

/// A decoded envelope header. `kind` stays raw so unknown optional kinds can
/// be skipped after their sequence is accounted for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    pub kind: u8,
    pub channel: ChannelId,
    pub epoch: SessionEpoch,
    pub sequence: Sequence,
    pub tick: Option<Tick>,
    pub fragment: Option<Fragment>,
}

impl Header {
    pub const fn encoded_len(&self) -> usize {
        BASE_HEADER_BYTES
            + if self.tick.is_some() { TICK_BYTES } else { 0 }
            + if self.fragment.is_some() {
                FRAGMENT_BYTES
            } else {
                0
            }
    }
}

/// A header and its borrowed payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Envelope<'a> {
    pub header: Header,
    pub payload: &'a [u8],
}

/// Append one encoded envelope to `out`.
pub fn encode(
    header: &Header,
    payload: &[u8],
    max_message_bytes: usize,
    out: &mut Vec<u8>,
) -> Result<(), EncodeError> {
    let total = header.encoded_len() + payload.len();
    let max_payload = max_message_bytes
        .saturating_sub(header.encoded_len())
        .min(usize::from(u16::MAX));
    if payload.len() > max_payload {
        return Err(EncodeError::PayloadTooLarge {
            len: payload.len(),
            max: max_payload,
        });
    }
    out.reserve(total);
    put_u8(out, WIRE_VERSION);
    put_u8(out, header.kind);
    put_u8(out, header.channel.wire());
    let mut flags = 0;
    if header.tick.is_some() {
        flags |= FLAG_TICK;
    }
    if header.fragment.is_some() {
        flags |= FLAG_FRAGMENT;
    }
    put_u8(out, flags);
    put_u32(out, header.epoch.get());
    put_u32(out, header.sequence.get());
    // Bounded above by `u16::MAX`.
    put_u16(out, payload.len() as u16);
    if let Some(tick) = header.tick {
        put_u64(out, tick.get());
    }
    if let Some(fragment) = header.fragment {
        put_u8(out, fragment.index);
        put_u8(out, fragment.count);
    }
    out.extend_from_slice(payload);
    Ok(())
}

/// Decode one envelope occupying exactly `bytes`.
pub fn decode(bytes: &[u8], max_message_bytes: usize) -> Result<Envelope<'_>, DecodeError> {
    if bytes.len() > max_message_bytes {
        return Err(DecodeError::MessageTooLarge {
            len: bytes.len(),
            max: max_message_bytes,
        });
    }
    let mut reader = Reader::new(bytes);
    let version = reader.u8()?;
    if version != WIRE_VERSION {
        return Err(DecodeError::UnsupportedVersion(version));
    }
    let kind = reader.u8()?;
    let channel = reader.u8()?;
    let channel = ChannelId::from_wire(channel).ok_or(DecodeError::UnknownChannel(channel))?;
    let flags = reader.u8()?;
    if flags & FLAG_RESERVED != 0 {
        return Err(DecodeError::ReservedFlags(flags & FLAG_RESERVED));
    }
    let epoch = SessionEpoch::new(reader.u32()?);
    let sequence = Sequence::new(reader.u32()?);
    let declared = usize::from(reader.u16()?);
    let tick = if flags & FLAG_TICK != 0 {
        Some(Tick::new(reader.u64()?))
    } else {
        None
    };
    let fragment = if flags & FLAG_FRAGMENT != 0 {
        let index = reader.u8()?;
        let count = reader.u8()?;
        if count < 2 || index >= count {
            return Err(DecodeError::InvalidFragment { index, count });
        }
        Some(Fragment { index, count })
    } else {
        None
    };
    if reader.remaining() != declared {
        return Err(DecodeError::LengthMismatch {
            declared,
            actual: reader.remaining(),
        });
    }
    Ok(Envelope {
        header: Header {
            kind,
            channel,
            epoch,
            sequence,
            tick,
            fragment,
        },
        payload: reader.rest(),
    })
}
