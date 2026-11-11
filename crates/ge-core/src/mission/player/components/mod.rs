//! Actual fresh TP/FP entry; allocation, residency, binding and attachment are
//! separate source operations. An owned constructor is not a bound component.
mod animator;
mod preparation;
mod registry;
mod skeleton;
mod visual;
pub use animator::Animator;
pub use preparation::{Attachment, BindReturn, VisualPostbind};
pub use preparation::{
    Bindings, Preparation, Provider, Stage, TextureEnumeration, TextureOutcome,
    TextureTargetRequest,
};
pub use registry::{Identity, Registry};
pub use skeleton::{ResourceBinding, SkeletonBudget, SkeletonInput, SkeletonRuntime};
pub use visual::{TextureRequest, Visual};
