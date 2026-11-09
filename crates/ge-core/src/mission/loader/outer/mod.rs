//! Case7 outer resets and ordered bank-source requests over retained owners.
//! No resource I/O, checkpoint restoration, task completion or readiness is
//! inferred by these operations; actual providers and the loader own those.
mod banks;
mod owner;

pub use banks::{
    BankFields, BankOwner, BankProvider, BankReceipt, BankRow, BankStage, BankStep, Call,
    DispatchOutcome, JobOutcome, bank_class_valid,
};
pub use owner::{AnimatorFields, ContextSlot, Owner, PreResetReceipt};

#[cfg(test)]
mod tests;
