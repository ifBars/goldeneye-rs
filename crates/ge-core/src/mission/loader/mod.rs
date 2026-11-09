//! Owned source loader operations and bounded host work. Phase transitions are
//! driven by actual operation returns/providers, never presentation receipts.
pub mod checkpoint;
pub mod outer;
pub mod phase;
pub mod work;
