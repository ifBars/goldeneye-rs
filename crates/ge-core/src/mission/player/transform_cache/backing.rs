use anyhow::{Context, Result, ensure};
use std::mem::MaybeUninit;

/// Semantic owned allocation identity, never an original memory address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackingId(pub(super) u64);
impl BackingId {
    pub fn get(self) -> u64 {
        self.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackingKind {
    Matrix,
    Validity,
    Offsets,
    Temporary,
    Descendants,
}

#[derive(Debug)]
pub struct Backing {
    id: BackingId,
    kind: BackingKind,
    storage: Vec<MaybeUninit<u8>>,
    // Values are recorded only on actual writes. Reads/hashes use this proof,
    // never potentially unwritten allocation bytes. Matrix payload stays None.
    written: Vec<Option<u8>>,
}
impl Backing {
    pub(super) fn allocate(id: BackingId, kind: BackingKind, bytes: usize) -> Result<Self> {
        let mut storage = Vec::new();
        storage
            .try_reserve_exact(bytes)
            .context("allocating transform-cache backing")?;
        storage.resize_with(bytes, MaybeUninit::uninit);
        let mut written = Vec::new();
        written
            .try_reserve_exact(bytes)
            .context("allocating transform-cache write metadata")?;
        written.resize(bytes, None);
        Ok(Self {
            id,
            kind,
            storage,
            written,
        })
    }
    pub fn id(&self) -> BackingId {
        self.id
    }
    pub fn kind(&self) -> BackingKind {
        self.kind
    }
    pub fn native_bytes(&self) -> usize {
        self.storage.len()
    }
    pub fn host_capacity_bytes(&self) -> usize {
        self.storage.capacity() + self.written.capacity() * std::mem::size_of::<Option<u8>>()
    }
    /// None is unwritten, rather than a guessed zero or identity matrix lane.
    pub fn byte(&self, offset: usize) -> Result<Option<u8>> {
        self.written
            .get(offset)
            .copied()
            .context("transform-cache byte outside backing")
    }
    pub fn half(&self, offset: usize) -> Result<Option<u16>> {
        match (self.byte(offset)?, self.byte(offset + 1)?) {
            (Some(a), Some(b)) => Ok(Some(u16::from_be_bytes([a, b]))),
            _ => Ok(None),
        }
    }
    pub fn word(&self, offset: usize) -> Result<Option<u32>> {
        let bytes = [
            self.byte(offset)?,
            self.byte(offset + 1)?,
            self.byte(offset + 2)?,
            self.byte(offset + 3)?,
        ];
        Ok(match bytes {
            [Some(a), Some(b), Some(c), Some(d)] => Some(u32::from_be_bytes([a, b, c, d])),
            _ => None,
        })
    }
    pub(super) fn put(&mut self, offset: usize, value: u8) -> Result<()> {
        ensure!(
            offset < self.storage.len(),
            "transform-cache write outside backing"
        );
        self.storage[offset].write(value);
        self.written[offset] = Some(value);
        Ok(())
    }
    pub(super) fn bytes(&mut self, offset: usize, values: &[u8]) -> Result<()> {
        ensure!(
            offset
                .checked_add(values.len())
                .is_some_and(|end| end <= self.storage.len()),
            "transform-cache write range outside backing"
        );
        for (index, &value) in values.iter().enumerate() {
            self.put(offset + index, value)?;
        }
        Ok(())
    }
    pub(super) fn fill(&mut self, offset: usize, count: usize, value: u8) -> Result<()> {
        ensure!(
            offset
                .checked_add(count)
                .is_some_and(|end| end <= self.storage.len()),
            "transform-cache fill outside backing"
        );
        for index in offset..offset + count {
            self.put(index, value)?;
        }
        Ok(())
    }
    pub(super) fn visit(&self, emit: &mut impl FnMut(u32)) {
        super::state::u64_words(self.id.get(), emit);
        emit(self.kind as u32);
        super::state::usize_words(self.native_bytes(), emit);
        for value in &self.written {
            emit(value.map_or(0, |value| u32::from(value) + 1));
        }
    }
}
