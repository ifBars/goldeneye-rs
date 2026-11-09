//! Explicit clip selection, frame requests and source-skin context requests.
//!
//! These pure prerequisites read no assets, own no frame cache, sample no
//! animation, advance no clock and commit no bone pose.
mod blend;
mod context;
mod request;

pub use blend::{BlendConstants, BlendInputs, Selection, select};
pub use context::{SkinPair, SkinRequest, skin_request};
pub use request::{FrameConstants, FrameInputs, FrameRequest, frame_request};

#[cfg(test)]
mod tests;
