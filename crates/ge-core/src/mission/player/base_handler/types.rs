use super::super::identity::PlayerId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Fresh,
    GenericPrefix,
    GenericComplete,
    BaseComplete,
    PlayerBasicPrefix,
    ResetPrefix,
    ResetComplete,
    BasicComplete,
    RuntimeResetPrefix,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    RandomSeed,
    GlobalReset,
    ResourceReset,
}

/// Semantic bindings replace source vtable/string addresses. No source pointer
/// or executable image is retained by this pure owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binding {
    Generic,
    Basic,
    PlayerBasic,
    Container,
    EmbeddedInterface,
    NestedContainer,
    EmptyString,
}
impl Binding {
    pub(super) fn raw(self) -> u32 {
        match self {
            Self::Generic => 1,
            Self::Basic => 2,
            Self::PlayerBasic => 3,
            Self::Container => 4,
            Self::EmbeddedInterface => 5,
            Self::NestedContainer => 6,
            Self::EmptyString => 7,
        }
    }
}

/// Proof of the complete supported 801001f0 prefix, including its first reset.
/// This is not a completed PlayerNew or a provider-history claim.
#[derive(Debug)]
pub struct BasicReceipt {
    pub(super) player: PlayerId,
}
impl BasicReceipt {
    pub const fn player(&self) -> PlayerId {
        self.player
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResetReceipt {
    pub player: PlayerId,
    pub invocation: usize,
}

pub trait Provider {
    /// Consume the SAME source RNG owner used by reached actor definitions.
    fn draw_handler_seed(&mut self) -> anyhow::Result<u16>;
    /// Apply actual global byte[r13-0x5504]=0 before the resource setter.
    fn reset_base_global_byte(&mut self) -> anyhow::Result<()>;
    /// Apply the actual persistent resource-state setter, not a lookup/default.
    fn reset_resource_state(&mut self, token: u32, value: u32) -> anyhow::Result<()>;
}
