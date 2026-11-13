//! Version-one wire format: envelope, message payloads and compatibility.
//!
//! Every integer is big-endian with a fixed width. Nothing here depends on
//! Rust layout, pointer width or enum representation. The normative
//! description is `docs/networking-wire-format.md`; byte fixtures in
//! `tests/protocol.rs` pin it.

pub mod compat;
pub mod envelope;
pub mod message;
pub mod wire;

use std::fmt;

/// The only wire version this build speaks.
pub const WIRE_VERSION: u8 = 1;

/// Why bytes could not be decoded. Decoding never allocates more than the
/// input length and never trusts a length field before checking it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// The input ended before a fixed-width field or declared length.
    Truncated,
    /// Bytes remain after the last field. Version one has no trailing data.
    TrailingBytes,
    UnsupportedVersion(u8),
    UnknownChannel(u8),
    /// Flag bits that version one reserves as zero.
    ReservedFlags(u8),
    /// The envelope exceeds the configured message limit.
    MessageTooLarge {
        len: usize,
        max: usize,
    },
    /// The declared payload length disagrees with the message boundary.
    LengthMismatch {
        declared: usize,
        actual: usize,
    },
    /// A fragment index outside its count, or a count below two.
    InvalidFragment {
        index: u8,
        count: u8,
    },
    /// A kind without the optional bit that this version does not define.
    UnknownMandatoryKind(u8),
    /// A known kind on a channel it is not defined for.
    WrongChannel {
        kind: u8,
        channel: u8,
    },
    /// A known kind with or without a tick, contrary to its definition.
    TickPresence {
        kind: u8,
    },
    /// A known kind that may not be fragmented.
    FragmentNotAllowed {
        kind: u8,
    },
    /// A field value outside its defined range.
    InvalidField(&'static str),
    /// A count or length above a configured limit.
    LimitExceeded(&'static str),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => write!(f, "message truncated"),
            Self::TrailingBytes => write!(f, "trailing bytes after message"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported wire version {v}"),
            Self::UnknownChannel(c) => write!(f, "unknown channel {c}"),
            Self::ReservedFlags(bits) => write!(f, "reserved flag bits {bits:#04x}"),
            Self::MessageTooLarge { len, max } => {
                write!(f, "message of {len} bytes exceeds limit {max}")
            }
            Self::LengthMismatch { declared, actual } => {
                write!(
                    f,
                    "declared payload {declared} bytes, boundary holds {actual}"
                )
            }
            Self::InvalidFragment { index, count } => {
                write!(f, "invalid fragment {index} of {count}")
            }
            Self::UnknownMandatoryKind(kind) => write!(f, "unknown mandatory kind {kind:#04x}"),
            Self::WrongChannel { kind, channel } => {
                write!(f, "kind {kind:#04x} is not defined on channel {channel}")
            }
            Self::TickPresence { kind } => write!(f, "kind {kind:#04x} has wrong tick presence"),
            Self::FragmentNotAllowed { kind } => {
                write!(f, "kind {kind:#04x} may not be fragmented")
            }
            Self::InvalidField(name) => write!(f, "invalid field: {name}"),
            Self::LimitExceeded(name) => write!(f, "limit exceeded: {name}"),
        }
    }
}

impl std::error::Error for DecodeError {}

/// Why a message could not be encoded. These are caller or budget errors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncodeError {
    /// The payload does not fit the 16-bit length field or the message limit.
    PayloadTooLarge { len: usize, max: usize },
    /// A count does not fit its wire field.
    CountTooLarge(&'static str),
    /// The header alone exceeds the message limit.
    HeaderTooLarge { len: usize, max: usize },
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PayloadTooLarge { len, max } => {
                write!(f, "payload of {len} bytes exceeds limit {max}")
            }
            Self::CountTooLarge(name) => write!(f, "count too large: {name}"),
            Self::HeaderTooLarge { len, max } => {
                write!(f, "header of {len} bytes exceeds limit {max}")
            }
        }
    }
}

impl std::error::Error for EncodeError {}
