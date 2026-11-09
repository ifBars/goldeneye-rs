#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Record {
    pub key: u16,
    pub value: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    FreshConstructor,
    SuppliedSnapshot,
}

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    /// Host allocation/admission bound, including semantic source capacity.
    pub records: usize,
    /// Row comparisons/copies/writes allowed within one operation.
    pub work: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lookup {
    pub value: u32,
    pub matched: bool,
    pub scanned: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetOutcome {
    Invalid,
    Updated { index: usize },
    Inserted { index: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Admission {
    pub captured: bool,
    pub saved_generation: u32,
    pub current_generation: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreOutcome {
    Rejected,
    Restored { rows_applied: usize },
}

#[derive(Debug)]
pub struct Snapshot {
    pub(super) rows: Vec<Record>,
    pub(super) source_cap_reached: bool,
}

impl Snapshot {
    pub fn rows(&self) -> &[Record] {
        &self.rows
    }
    /// Source stops immediately after its500th nonzero row. This does not
    /// assert whether additional nonzero rows actually exist beyond that point.
    pub fn source_cap_reached(&self) -> bool {
        self.source_cap_reached
    }
}
