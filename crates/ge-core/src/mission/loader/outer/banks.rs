use anyhow::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BankRow {
    pub primary: u32,
    pub optional: [u32; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BankFields {
    pub word_4c: u32,
    pub word_50: u32,
    pub word_54: u32,
    pub byte_58: u8,
}

/// Source work ownership, not completion or bank residency. Unavailable means
/// an actual source resource descriptor/allocation/submission could not produce work;
/// the bank helper still reaches its later token/backend branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobOutcome {
    Existing,
    Submitted,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchOutcome {
    BackendAbsent,
    Submitted,
}

pub trait BankProvider {
    fn selected_level(&mut self) -> Result<Option<BankRow>>;
    fn multiplayer_extra_required(&mut self) -> Result<bool>;
    fn ensure_shared_type1_job(&mut self) -> Result<JobOutcome>;
    fn dispatch_bank(&mut self, token: u32, flags: u32) -> Result<DispatchOutcome>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Call {
    pub ordinal: usize,
    pub token: u32,
    pub flags: u32,
    pub job: Option<JobOutcome>,
    pub valid_class: Option<bool>,
    pub dispatch: Option<DispatchOutcome>,
    pub returned: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BankStep {
    ReadLevel,
    EnsureJob { ordinal: usize },
    Dispatch { ordinal: usize },
    ReadSession,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BankStage {
    Constructor,
    Disabled,
    Running(BankStep),
    Failed(BankStep),
    Complete,
}

/// A completed source caller is not an audio/task completion receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BankReceipt {
    pub disabled: bool,
    pub source_calls: usize,
    pub valid_class_calls: usize,
    pub job_submissions: usize,
    pub jobs_existing: usize,
    pub jobs_unavailable: usize,
    pub backend_submissions: usize,
    pub backends_absent: usize,
}

pub struct BankOwner {
    fields: BankFields,
    calls: Vec<Call>,
    stage: BankStage,
    last_rejected: Option<u32>,
}

impl BankOwner {
    pub fn from_constructor() -> Self {
        Self::from_snapshot(BankFields {
            word_4c: 0,
            word_50: 0,
            word_54: 0,
            byte_58: 0,
        })
    }

    /// Explicit retained logical slot state; does not claim native history.
    pub fn from_snapshot(fields: BankFields) -> Self {
        Self {
            fields,
            calls: Vec::new(),
            stage: BankStage::Constructor,
            last_rejected: None,
        }
    }

    pub fn run(
        &mut self,
        enable_byte: u8,
        provider: &mut impl BankProvider,
    ) -> Result<BankReceipt> {
        self.calls.clear();
        // Signed nonzero and unsigned nonzero are identical for raw byte tests.
        if enable_byte == 0 {
            self.stage = BankStage::Disabled;
            return Ok(self.receipt(true));
        }
        self.stage = BankStage::Running(BankStep::ReadLevel);
        let row = match provider.selected_level() {
            Ok(row) => row,
            Err(error) => {
                self.stage = BankStage::Failed(BankStep::ReadLevel);
                return Err(error);
            }
        };
        self.fields.word_4c = 0;
        self.fields.byte_58 = 1;
        if self.fields.word_54 != 0x2d50_0001 {
            self.issue(0x2d50_0001, provider)?;
            self.fields.word_54 = 0x2d50_0001;
        }
        if self.fields.word_50 != 0x2d50_0014 {
            if self.fields.word_4c != 0 {
                self.fields.word_4c = 0;
            }
            self.issue(0x2d50_0014, provider)?;
            self.fields.word_50 = 0x2d50_0014;
        }
        let primary = row.map_or(0, |row| row.primary);
        if self.fields.word_4c != primary {
            self.issue(primary, provider)?;
            self.fields.word_4c = primary;
        }
        for token in row.map_or([u32::MAX; 2], |row| row.optional) {
            if token != u32::MAX {
                self.issue(token, provider)?;
            }
        }
        self.stage = BankStage::Running(BankStep::ReadSession);
        let multiplayer = match provider.multiplayer_extra_required() {
            Ok(value) => value,
            Err(error) => {
                self.stage = BankStage::Failed(BankStep::ReadSession);
                return Err(error);
            }
        };
        if multiplayer {
            self.issue(0x2d50_0019, provider)?;
        }
        self.stage = BankStage::Complete;
        Ok(self.receipt(false))
    }

    fn issue(&mut self, token: u32, provider: &mut impl BankProvider) -> Result<()> {
        // Source caller emits at most 2 common +1 primary +2 optional +1 MP.
        let ordinal = self.calls.len();
        self.calls.push(Call {
            ordinal,
            token,
            flags: 0,
            job: None,
            valid_class: None,
            dispatch: None,
            returned: false,
        });
        let step = BankStep::EnsureJob { ordinal };
        self.stage = BankStage::Running(step);
        match provider.ensure_shared_type1_job() {
            Ok(outcome) => self.calls[ordinal].job = Some(outcome),
            Err(error) => {
                self.stage = BankStage::Failed(step);
                return Err(error);
            }
        }
        let valid = bank_class_valid(token);
        self.calls[ordinal].valid_class = Some(valid);
        if valid {
            let step = BankStep::Dispatch { ordinal };
            self.stage = BankStage::Running(step);
            match provider.dispatch_bank(token, 0) {
                Ok(outcome) => self.calls[ordinal].dispatch = Some(outcome),
                Err(error) => {
                    self.stage = BankStage::Failed(step);
                    return Err(error);
                }
            }
        } else {
            // Source stores its diagnostic word even when the token repeated.
            // No invented initial value or log-count claim is necessary here.
            self.last_rejected = Some(token);
        }
        self.calls[ordinal].returned = true;
        Ok(())
    }

    fn receipt(&self, disabled: bool) -> BankReceipt {
        BankReceipt {
            disabled,
            source_calls: self.calls.len(),
            valid_class_calls: self
                .calls
                .iter()
                .filter(|call| call.valid_class == Some(true))
                .count(),
            job_submissions: self
                .calls
                .iter()
                .filter(|call| call.job == Some(JobOutcome::Submitted))
                .count(),
            jobs_existing: self
                .calls
                .iter()
                .filter(|call| call.job == Some(JobOutcome::Existing))
                .count(),
            jobs_unavailable: self
                .calls
                .iter()
                .filter(|call| call.job == Some(JobOutcome::Unavailable))
                .count(),
            backend_submissions: self
                .calls
                .iter()
                .filter(|call| call.dispatch == Some(DispatchOutcome::Submitted))
                .count(),
            backends_absent: self
                .calls
                .iter()
                .filter(|call| call.dispatch == Some(DispatchOutcome::BackendAbsent))
                .count(),
        }
    }

    pub fn fields(&self) -> BankFields {
        self.fields
    }
    pub fn calls(&self) -> &[Call] {
        &self.calls
    }
    pub fn stage(&self) -> BankStage {
        self.stage
    }
    pub fn last_rejected(&self) -> Option<u32> {
        self.last_rejected
    }
}

pub fn bank_class_valid(token: u32) -> bool {
    let nibble = token & 0x00f0_0000;
    let class = if nibble == 0x00e0_0000 {
        token & 0x00ff_0000
    } else {
        nibble
    };
    class == 0x0050_0000
}
