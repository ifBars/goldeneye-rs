//! Compatibility descriptor and its comparison.
//!
//! The application computes every value locally. This module only compares
//! them; it opens no game image. A matching fingerprint is a compatibility
//! check, not proof of ownership or of a peer's behaviour.

use std::fmt;

use super::wire::{Reader, put_u8, put_u32, put_u64};
use super::{DecodeError, EncodeError};

/// Most extension identifiers a descriptor may list.
pub const MAX_EXTENSIONS: usize = 32;

/// Simulation ticks per second as a rational, so no float crosses the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TickRate {
    pub numerator: u32,
    pub denominator: u32,
}

impl TickRate {
    pub const fn per_second(ticks: u32) -> Self {
        Self {
            numerator: ticks,
            denominator: 1,
        }
    }

    /// Microseconds per tick, rounded down. Zero if the rate is invalid.
    pub const fn tick_micros(self) -> u64 {
        if self.numerator == 0 {
            return 0;
        }
        self.denominator as u64 * 1_000_000 / self.numerator as u64
    }
}

/// What both sides must agree on before a session forms.
///
/// The initial policy is exact equality for every identity field. Features
/// are bit sets: each side must support everything the other requires.
/// Extensions are optional; the session uses the intersection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompatibilityDescriptor {
    /// Version of the application's opaque payload encoding.
    pub app_protocol: u32,
    /// Version of the simulation state schema.
    pub simulation_schema: u32,
    /// Build identity. Exact-build compatibility is the initial policy.
    pub build: [u8; 32],
    /// Supported content edition, as opaque bytes.
    pub edition: [u8; 8],
    /// Supported content revision.
    pub revision: u32,
    /// Fingerprint of the canonical logical content, not of a container file.
    pub content: [u8; 32],
    /// Map and rules identity for this session.
    pub ruleset: [u8; 32],
    pub tick_rate: TickRate,
    pub supported_features: u64,
    pub required_features: u64,
    /// Strictly ascending optional extension identifiers.
    pub extensions: Vec<u32>,
}

/// A descriptor that cannot be sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DescriptorError {
    TooManyExtensions(usize),
    ExtensionsNotAscending,
    ZeroTickRate,
    /// A required feature is absent from the supported set.
    RequiredFeatureUnsupported(u64),
}

impl fmt::Display for DescriptorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyExtensions(count) => write!(f, "{count} extensions exceed the limit"),
            Self::ExtensionsNotAscending => write!(f, "extensions must be strictly ascending"),
            Self::ZeroTickRate => write!(f, "tick rate must be nonzero"),
            Self::RequiredFeatureUnsupported(bits) => {
                write!(
                    f,
                    "required features {bits:#x} are not in the supported set"
                )
            }
        }
    }
}

impl std::error::Error for DescriptorError {}

/// The first field on which two descriptors disagree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mismatch {
    AppProtocol,
    SimulationSchema,
    Build,
    Edition,
    Revision,
    Content,
    Ruleset,
    TickRate,
    /// Feature bits the host requires and the client does not support.
    HostRequiresFeatures(u64),
    /// Feature bits the client requires and the host does not support.
    ClientRequiresFeatures(u64),
}

impl Mismatch {
    /// Wire form: a field code and a detail word.
    pub const fn to_wire(self) -> (u8, u64) {
        match self {
            Self::AppProtocol => (1, 0),
            Self::SimulationSchema => (2, 0),
            Self::Build => (3, 0),
            Self::Edition => (4, 0),
            Self::Revision => (5, 0),
            Self::Content => (6, 0),
            Self::Ruleset => (7, 0),
            Self::TickRate => (8, 0),
            Self::HostRequiresFeatures(bits) => (9, bits),
            Self::ClientRequiresFeatures(bits) => (10, bits),
        }
    }

    pub const fn from_wire(field: u8, detail: u64) -> Option<Self> {
        Some(match field {
            1 => Self::AppProtocol,
            2 => Self::SimulationSchema,
            3 => Self::Build,
            4 => Self::Edition,
            5 => Self::Revision,
            6 => Self::Content,
            7 => Self::Ruleset,
            8 => Self::TickRate,
            9 => Self::HostRequiresFeatures(detail),
            10 => Self::ClientRequiresFeatures(detail),
            _ => return None,
        })
    }
}

impl fmt::Display for Mismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AppProtocol => write!(f, "application protocol version differs"),
            Self::SimulationSchema => write!(f, "simulation schema version differs"),
            Self::Build => write!(f, "build identity differs"),
            Self::Edition => write!(f, "content edition differs"),
            Self::Revision => write!(f, "content revision differs"),
            Self::Content => write!(f, "content fingerprint differs"),
            Self::Ruleset => write!(f, "map or rules identity differs"),
            Self::TickRate => write!(f, "tick configuration differs"),
            Self::HostRequiresFeatures(bits) => {
                write!(f, "host requires unsupported features {bits:#x}")
            }
            Self::ClientRequiresFeatures(bits) => {
                write!(f, "client requires unsupported features {bits:#x}")
            }
        }
    }
}

impl CompatibilityDescriptor {
    pub fn validate(&self) -> Result<(), DescriptorError> {
        if self.extensions.len() > MAX_EXTENSIONS {
            return Err(DescriptorError::TooManyExtensions(self.extensions.len()));
        }
        if !self.extensions.windows(2).all(|pair| pair[0] < pair[1]) {
            return Err(DescriptorError::ExtensionsNotAscending);
        }
        if self.tick_rate.numerator == 0 || self.tick_rate.denominator == 0 {
            return Err(DescriptorError::ZeroTickRate);
        }
        let unsupported = self.required_features & !self.supported_features;
        if unsupported != 0 {
            return Err(DescriptorError::RequiredFeatureUnsupported(unsupported));
        }
        Ok(())
    }

    /// Compare the host's descriptor (`self`) with a client's.
    ///
    /// Returns the negotiated extensions, or the first mismatching field in
    /// declaration order.
    pub fn negotiate(&self, client: &Self) -> Result<Vec<u32>, Mismatch> {
        let checks = [
            (
                self.app_protocol == client.app_protocol,
                Mismatch::AppProtocol,
            ),
            (
                self.simulation_schema == client.simulation_schema,
                Mismatch::SimulationSchema,
            ),
            (self.build == client.build, Mismatch::Build),
            (self.edition == client.edition, Mismatch::Edition),
            (self.revision == client.revision, Mismatch::Revision),
            (self.content == client.content, Mismatch::Content),
            (self.ruleset == client.ruleset, Mismatch::Ruleset),
            (self.tick_rate == client.tick_rate, Mismatch::TickRate),
        ];
        if let Some((_, mismatch)) = checks.iter().find(|(equal, _)| !equal) {
            return Err(*mismatch);
        }
        let host_unmet = self.required_features & !client.supported_features;
        if host_unmet != 0 {
            return Err(Mismatch::HostRequiresFeatures(host_unmet));
        }
        let client_unmet = client.required_features & !self.supported_features;
        if client_unmet != 0 {
            return Err(Mismatch::ClientRequiresFeatures(client_unmet));
        }
        Ok(self
            .extensions
            .iter()
            .copied()
            .filter(|extension| client.extensions.binary_search(extension).is_ok())
            .collect())
    }

    pub(crate) fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        if self.extensions.len() > MAX_EXTENSIONS {
            return Err(EncodeError::CountTooLarge("extensions"));
        }
        put_u32(out, self.app_protocol);
        put_u32(out, self.simulation_schema);
        out.extend_from_slice(&self.build);
        out.extend_from_slice(&self.edition);
        put_u32(out, self.revision);
        out.extend_from_slice(&self.content);
        out.extend_from_slice(&self.ruleset);
        put_u32(out, self.tick_rate.numerator);
        put_u32(out, self.tick_rate.denominator);
        put_u64(out, self.supported_features);
        put_u64(out, self.required_features);
        encode_extensions(&self.extensions, out);
        Ok(())
    }

    pub(crate) fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let descriptor = Self {
            app_protocol: reader.u32()?,
            simulation_schema: reader.u32()?,
            build: reader.array()?,
            edition: reader.array()?,
            revision: reader.u32()?,
            content: reader.array()?,
            ruleset: reader.array()?,
            tick_rate: TickRate {
                numerator: reader.u32()?,
                denominator: reader.u32()?,
            },
            supported_features: reader.u64()?,
            required_features: reader.u64()?,
            extensions: decode_extensions(reader)?,
        };
        descriptor
            .validate()
            .map_err(|_| DecodeError::InvalidField("descriptor"))?;
        Ok(descriptor)
    }
}

/// Callers bound `extensions` to [`MAX_EXTENSIONS`] first.
pub(crate) fn encode_extensions(extensions: &[u32], out: &mut Vec<u8>) {
    put_u8(out, extensions.len() as u8);
    for extension in extensions {
        put_u32(out, *extension);
    }
}

pub(crate) fn decode_extensions(reader: &mut Reader<'_>) -> Result<Vec<u32>, DecodeError> {
    let count = usize::from(reader.u8()?);
    if count > MAX_EXTENSIONS {
        return Err(DecodeError::LimitExceeded("extensions"));
    }
    let mut extensions = Vec::with_capacity(count);
    for _ in 0..count {
        let extension = reader.u32()?;
        if extensions.last().is_some_and(|last| *last >= extension) {
            return Err(DecodeError::InvalidField("extensions"));
        }
        extensions.push(extension);
    }
    Ok(extensions)
}
