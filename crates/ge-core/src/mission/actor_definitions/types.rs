use std::num::NonZeroU64;

#[derive(Debug, Clone, Copy)]
pub struct Primary {
    pub source_index: usize,
    pub words: [u32; 17],
}

/// Host allocation bounds, not native table limits. Rows include duplicates;
/// members count references across all groups, not the largest single group.
#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub rows: usize,
    pub groups: usize,
    pub members: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct LookupBudget {
    pub comparisons: usize,
}

/// Semantic owned ID, equal to final unsigned-sorted record index plus one.
/// This never represents an original pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RecordId(pub(super) NonZeroU64);

impl RecordId {
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

#[derive(Debug)]
pub struct Record {
    pub(super) id: RecordId,
    pub(super) source_index: usize,
    pub(super) words: [u32; 17],
}

impl Record {
    pub fn id(&self) -> RecordId {
        self.id
    }

    pub fn key(&self) -> u32 {
        self.words[0]
    }

    pub fn source_index(&self) -> usize {
        self.source_index
    }

    pub fn words(&self) -> &[u32; 17] {
        &self.words
    }

    pub fn group_word(&self) -> u32 {
        self.words[1]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Duplicate {
    pub key: u32,
    pub retained_source_index: usize,
    pub rejected_source_index: usize,
    pub(super) arrival: usize,
}

#[derive(Debug)]
pub struct Group {
    pub(super) key: u16,
    pub(super) count: usize,
    pub(super) members: Vec<RecordId>,
    pub(super) cursor: usize,
}

impl Group {
    pub fn key(&self) -> u16 {
        self.key
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn members(&self) -> &[RecordId] {
        &self.members
    }

    /// May equal count after selection; wrapping happens on the next call.
    pub fn cursor(&self) -> usize {
        self.cursor
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultReason {
    InvalidReference { reference: u32 },
    MissingGroup { key: u16 },
    MissingRecord { reference: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialKind {
    /// Source manager's embedded record at +0x58 (reference 0x4d000102).
    Embedded102,
    /// Source static record (reference 0x4d0001ac); data is not embedded here.
    Static1ac,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selection {
    Selected {
        record: RecordId,
        group: Option<u16>,
        member_index: Option<usize>,
    },
    RequiresDefault {
        reason: DefaultReason,
    },
    RequiresSpecial {
        kind: SpecialKind,
    },
}
