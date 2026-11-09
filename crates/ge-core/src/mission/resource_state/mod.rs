//! Retained source resource-state records. Fresh constructor state and supplied
//! snapshots are explicit origins, never proof of original consumer history.
mod owner;
mod types;

pub use owner::Owner;
pub use types::{Admission, Budget, Lookup, Origin, Record, RestoreOutcome, SetOutcome, Snapshot};
