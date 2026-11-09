//! Lazy mode-transition selection and ordered command plans.
//!
//! Inputs describe an explicit caller/resource snapshot. A plan allocates no
//! animation child, clears no runtime layer and establishes no sampled pose or
//! camera readiness. Provider failures remain errors, never native misses.
mod plan;
mod selector;
mod types;

pub use plan::{
    ChildPlan, CleanupIntent, CommandPlan, GroupPlan, Insertion, LayerRequest, SlotInputs,
    StartSetPlan, WeightRequest,
};
pub use selector::select;
pub use types::{
    Attempt, AttemptKind, Budget, Child, Command, Group, Inputs, Mode, Provider, Selection,
    StartSet, Status, Work,
};

#[cfg(test)]
mod tests;
