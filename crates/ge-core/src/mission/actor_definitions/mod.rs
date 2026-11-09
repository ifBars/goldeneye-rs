//! Owned primary M4d records and a fresh grouped-selection cursor snapshot.
//!
//! This is the primary-record slice of source registration/lookup, not a whole
//! actor manager, RNG owner, or proof of original-runtime cursor history.
mod registry;
mod types;

pub use registry::Registry;
pub use types::{
    Budget, DefaultReason, Duplicate, Group, LookupBudget, Primary, Record, RecordId, Selection,
    SpecialKind,
};
