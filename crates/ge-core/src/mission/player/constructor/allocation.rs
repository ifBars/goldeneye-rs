use anyhow::{Context, Result, ensure};
use std::{fmt, mem::MaybeUninit};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Raw,
    Array34,
    Array64,
}
impl Kind {
    pub const fn index(self) -> usize {
        match self {
            Self::Raw => 0,
            Self::Array34 => 1,
            Self::Array64 => 2,
        }
    }
    pub const fn bytes(self) -> usize {
        match self {
            Self::Raw => 512,
            Self::Array34 => 7088,
            Self::Array64 => 56080,
        }
    }
    pub const fn stride(self) -> Option<u32> {
        match self {
            Self::Raw => None,
            Self::Array34 => Some(208),
            Self::Array64 => Some(876),
        }
    }
    pub const fn entries(self) -> u32 {
        match self {
            Self::Raw => 0,
            Self::Array34 => 34,
            Self::Array64 => 64,
        }
    }
}

pub struct Allocation {
    kind: Kind,
    storage: Vec<MaybeUninit<u8>>,
    noop_entries: u32,
}
impl fmt::Debug for Allocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PlayerConstructorAllocation")
            .field("kind", &self.kind)
            .field("bytes", &self.storage.len())
            .field("noop_entries", &self.noop_entries)
            .finish()
    }
}
impl Allocation {
    pub(super) fn allocate(kind: Kind, remaining: usize) -> Result<Self> {
        ensure!(
            kind.bytes() <= remaining,
            "player constructor allocation budget exhausted"
        );
        let mut storage = Vec::new();
        storage
            .try_reserve_exact(kind.bytes())
            .context("player constructor allocation failed")?;
        storage.resize_with(kind.bytes(), MaybeUninit::uninit);
        let mut allocation = Self {
            kind,
            storage,
            noop_entries: 0,
        };
        if let Some(stride) = kind.stride() {
            for (i, byte) in stride
                .to_be_bytes()
                .into_iter()
                .chain(kind.entries().to_be_bytes())
                .enumerate()
            {
                allocation.storage[i].write(byte);
            }
            // Source entry constructor 80006800 is literally return. Reaching
            // it never initializes an entry or the other two header words.
            for _ in 0..kind.entries() {
                allocation.noop_entries += 1;
            }
        }
        Ok(allocation)
    }
    pub fn kind(&self) -> Kind {
        self.kind
    }
    pub fn native_bytes(&self) -> usize {
        self.storage.len()
    }
    pub fn host_capacity_bytes(&self) -> usize {
        self.storage.capacity()
    }
    pub fn payload_offset(&self) -> usize {
        if self.kind.stride().is_some() { 16 } else { 0 }
    }
    pub fn payload_bytes(&self) -> usize {
        self.native_bytes() - self.payload_offset()
    }
    pub fn entry_noops(&self) -> u32 {
        self.noop_entries
    }
    /// Metadata of the only written header words, with no uninitialized read.
    pub fn header_word(&self, offset: usize) -> Option<u32> {
        match offset {
            0 => self.kind.stride(),
            4 => self.kind.stride().map(|_| self.kind.entries()),
            _ => None,
        }
    }
    pub(super) fn visit(&self, visit: &mut impl FnMut(u32)) {
        visit(self.kind.index() as u32);
        visit(self.native_bytes() as u32);
        visit(self.payload_offset() as u32);
        visit(self.payload_bytes() as u32);
        visit((self.host_capacity_bytes() as u64 >> 32) as u32);
        visit(self.host_capacity_bytes() as u32);
        for offset in [0, 4, 8, 12] {
            let value = self.header_word(offset);
            visit(u32::from(value.is_some()));
            if let Some(value) = value {
                visit(value);
            }
        }
        visit(self.noop_entries);
        // All payload bytes and header8/c remain unknown. No byte accessor or
        // serialization can accidentally read the actual MaybeUninit storage.
    }
}
