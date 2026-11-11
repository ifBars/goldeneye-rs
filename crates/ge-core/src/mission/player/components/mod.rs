//! Actual fresh TP/FP entry; allocation, residency, binding and attachment are
//! separate source operations. An owned constructor is not a bound component.
mod animator;
mod first_person;
pub use first_person::{FirstPerson, FirstPersonStage};
pub mod callbacks;
mod preparation;
mod registry;
mod skeleton;
mod visual;
pub use animator::Animator;
pub use preparation::{Attachment, BindReturn, ThirdPersonReturn, VisualPostbind};
pub use preparation::{
    Bindings, Preparation, Provider, Stage, TextureEnumeration, TextureOutcome,
    TextureTargetRequest,
};
pub use registry::{Identity, Registry};
pub use skeleton::{PoseCallback, ResourceBinding, SkeletonBudget, SkeletonInput, SkeletonRuntime};
pub use visual::{TextureRequest, Visual};
