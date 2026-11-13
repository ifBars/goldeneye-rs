//! Authored test support: a seeded link simulator, a synthetic simulation
//! and a harness that drives a host and clients over the simulated link.
//!
//! Nothing here is game data. Each test binary uses a different subset.
#![allow(dead_code)]

pub mod harness;
pub mod link;
pub mod pipe;
pub mod rng;
pub mod simulation;
