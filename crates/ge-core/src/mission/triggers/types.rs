use std::num::NonZeroU64;

use anyhow::Result;

use super::common_shape::{ShapeBinding, ShapeOutcome, ShapeRequest};
use crate::mission::{spawn, world::InstanceId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TriggerId(NonZeroU64);

impl TriggerId {
    pub fn new(value: u64) -> Result<Self> {
        Ok(Self(NonZeroU64::new(value).ok_or_else(|| {
            anyhow::anyhow!("trigger identity must be nonzero")
        })?))
    }

    pub fn get(self) -> u64 {
        self.0.get()
    }
}

/// Identity of an actual owned animator, not its source index (zero is valid).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnimatorId(NonZeroU64);

impl AnimatorId {
    pub fn new(value: u64) -> Result<Self> {
        Ok(Self(NonZeroU64::new(value).ok_or_else(|| {
            anyhow::anyhow!("animator identity must be nonzero")
        })?))
    }

    pub fn get(self) -> u64 {
        self.0.get()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    pub maps: usize,
    pub rows: usize,
    pub objects: usize,
    pub filter_words: usize,
    pub fixups: usize,
    pub catalog_entries: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapContext {
    pub instance: InstanceId,
    /// Source sign-extended preregistration slot, not the loaded ordinal.
    pub map_selector: i8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Descriptor {
    pub native_type_index: u32,
    pub class: u32,
    pub subtype: u32,
    /// Source sign-extends then tests nonzero; high-bit bytes are enabled too.
    pub enable_byte_raw: u8,
}

impl Descriptor {
    pub fn is_start_point(self) -> bool {
        self.class == 0x1e00_0003 && self.subtype == 0x1f00_0081
    }

    pub fn is_common_be(self) -> bool {
        self.class == 0x1e00_0004 && self.subtype == 0x1f00_00be
    }

    pub fn is_resource08(self) -> bool {
        self.class == 0x1e00_0004 && self.subtype == 0x1f00_0008
    }
    pub fn is_active91(self) -> bool {
        self.class == 0x1e00_0004 && self.subtype == 0x1f00_0091
    }
    pub fn is_range0c(self) -> bool {
        self.class == 0x1e00_0005 && self.subtype == 0x1f00_000c
    }
    pub fn is_radius06(self) -> bool {
        self.class == 0x1e00_0004 && self.subtype == 0x1f00_0006
    }
    pub fn is_actor12(self) -> bool {
        self.class == 0x1e00_0002 && self.subtype == 0x1f00_0012
    }
    pub fn is_range82(self) -> bool {
        self.class == 0x1e00_0005 && self.subtype == 0x1f00_0082
    }
    pub fn is_a3(self) -> bool {
        self.class == 0x1e00_0004 && self.subtype == 0x1f00_00a3
    }
    pub fn is_range27(self) -> bool {
        self.class == 0x1e00_0005 && self.subtype == 0x1f00_0027
    }
    pub fn is_common09(self) -> bool {
        self.class == 0x1e00_0004 && self.subtype == 0x1f00_0009
    }
    pub fn is_ad(self) -> bool {
        self.class == 0x1e00_0009 && self.subtype == 0x1f00_00ad
    }
    pub fn is_radius03(self) -> bool {
        self.class == 0x1e00_0001 && self.subtype == 0x1f00_0003
    }
    pub fn is_common8a(self) -> bool {
        self.class == 0x1e00_000b && self.subtype == 0x1f00_008a
    }
    pub fn is_radius1b(self) -> bool {
        self.class == 0x1e00_0004 && self.subtype == 0x1f00_001b
    }
    pub fn is_radius74(self) -> bool {
        self.class == 0x1e00_000a && self.subtype == 0x1f00_0074
    }
    pub fn is_shape71(self) -> bool {
        self.class == 0x1e00_0009 && self.subtype == 0x1f00_0071
    }
    pub fn is_radius88(self) -> bool {
        self.class == 0x1e00_0004 && self.subtype == 0x1f00_0088
    }
    pub fn is_aux_ac(self) -> bool {
        self.class == 0x1e00_0007 && self.subtype == 0x1f00_00ac
    }
    /// Exact factory path returns no object, not an unsupported host kind.
    pub fn is_source_null(self) -> bool {
        self.class == 0x1e00_0004 && self.subtype == 0x1f00_00a1
    }

    pub fn kind(self) -> Option<TriggerKind> {
        if self.is_start_point() {
            Some(TriggerKind::StartPoint)
        } else if self.class == 0x1e00_0001 && self.subtype == 0x1f00_0001 {
            Some(TriggerKind::Generic)
        } else if self.is_common_be() {
            Some(TriggerKind::CommonBe)
        } else if self.is_resource08() {
            Some(TriggerKind::Resource08)
        } else if self.is_active91() {
            Some(TriggerKind::Active91)
        } else if self.is_range0c() {
            Some(TriggerKind::Range0c)
        } else if self.is_radius06() {
            Some(TriggerKind::Radius06)
        } else if self.is_actor12() {
            Some(TriggerKind::Actor12)
        } else if self.is_range82() {
            Some(TriggerKind::Range82)
        } else if self.is_a3() {
            Some(TriggerKind::A3)
        } else if self.is_range27() {
            Some(TriggerKind::Range27)
        } else if self.is_common09() {
            Some(TriggerKind::Common09)
        } else if self.is_ad() {
            Some(TriggerKind::Ad)
        } else if self.is_radius03() {
            Some(TriggerKind::Radius03)
        } else if self.is_common8a() {
            Some(TriggerKind::Common8A)
        } else if self.is_radius1b() {
            Some(TriggerKind::Radius1b)
        } else if self.is_radius74() {
            Some(TriggerKind::Radius74)
        } else if self.is_shape71() {
            Some(TriggerKind::Shape71)
        } else if self.is_radius88() {
            Some(TriggerKind::Radius88)
        } else if self.is_aux_ac() {
            Some(TriggerKind::AuxAc)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerKind {
    StartPoint,
    Generic,
    /// Common-only initial constructor/map-init. Later external callbacks are
    /// deliberately absent from this supported initialization interface.
    CommonBe,
    Resource08,
    Active91,
    Range0c,
    Radius06,
    Actor12,
    Range82,
    A3,
    Range27,
    Common09,
    Ad,
    Radius03,
    Common8A,
    Radius1b,
    Radius74,
    Shape71,
    Radius88,
    AuxAc,
}

impl TriggerKind {
    pub fn factory_status(self) -> FactoryStatus {
        match self {
            Self::StartPoint => FactoryStatus::Status2,
            Self::Generic
            | Self::CommonBe
            | Self::Resource08
            | Self::Active91
            | Self::Range0c
            | Self::Radius06
            | Self::Actor12
            | Self::Range82
            | Self::A3
            | Self::Range27
            | Self::Common09
            | Self::Ad
            | Self::Radius03
            | Self::Common8A
            | Self::Radius1b
            | Self::Radius74
            | Self::Shape71
            | Self::Radius88
            | Self::AuxAc => FactoryStatus::Status1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FactoryStatus {
    Status1,
    Status2,
}

impl FactoryStatus {
    pub fn raw(self) -> u8 {
        match self {
            Self::Status1 => 1,
            Self::Status2 => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowInput {
    pub ordinal: usize,
    /// Supplied by the separate actual loaded-world zone query, before callback.
    pub zone: u16,
    pub descriptor: Descriptor,
}

/// Checked common-reader output. Arrays retain source raw words; no subtype
/// interpretation, quaternion conversion or finite-value repair occurs here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonInput {
    pub header_flags_raw: u32,
    pub property_mask_raw: u32,
    pub position_bits: [u32; 4],
    pub rotation_bits: [u32; 4],
    pub numeric: [u32; 16],
    pub reference_ordinals: [u32; 8],
    pub word_74_raw: u32,
    pub word_78_raw: u32,
    pub animator_present: bool,
    pub animator_index_raw: i32,
    /// Parsed selector only. A present selector requires a checked common-init
    /// binding; later shape geometry is separate. Preserve signed-like raw bits.
    pub shape_index_raw: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimatorRequest {
    pub trigger: TriggerId,
    /// Source kind is required: callers must not silently apply a supported
    /// StartPoint-specific animator admission policy to another trigger kind.
    pub kind: TriggerKind,
    pub map: MapContext,
    pub zone: u16,
    pub ordinal: u16,
    pub index: i32,
    pub position_bits: [u32; 4],
}

pub trait Factory {
    /// Reached after common selector binding and scratch retention. A checked
    /// type0 descriptor must provide its actual geometry from the owning map.
    fn shape71_geometry(
        &mut self,
        _request: super::shape71::Request,
    ) -> Result<super::actor12::ProviderOutcome<super::shape71::GeometryInput>> {
        Ok(super::actor12::ProviderOutcome::Unavailable)
    }
    /// A missing owner is distinct from an authoritative lookup miss in an
    /// existing retained owner. The source caller requests fallback0.
    fn radius03_resource(
        &mut self,
        _request: super::radius03::Request,
    ) -> Result<super::actor12::ProviderOutcome<u32>> {
        Ok(super::actor12::ProviderOutcome::Unavailable)
    }
    /// Reached only for an invalid serialized M4d reference. A selected source
    /// row's fallback word is returned verbatim, without revalidation.
    fn actor12_descriptor_fallback(
        &mut self,
        _request: super::actor12::Request,
    ) -> Result<super::actor12::ProviderOutcome<u32>> {
        Ok(super::actor12::ProviderOutcome::Unavailable)
    }

    /// Receives the validated 4b30 token. Actual selected-level/RNG ownership
    /// must decide the lazy branch; zero is not permission to invent selector1.
    fn actor12_level_selector(
        &mut self,
        _request: super::actor12::Request,
        _validated_token: u32,
    ) -> Result<super::actor12::ProviderOutcome<super::actor12::LevelSelection>> {
        Ok(super::actor12::ProviderOutcome::Unavailable)
    }

    fn actor12_definition(
        &mut self,
        _request: super::actor12::Request,
        _reference: u32,
    ) -> Result<super::actor12::ProviderOutcome<crate::mission::actor_definitions::RecordId>> {
        Ok(super::actor12::ProviderOutcome::Unavailable)
    }

    /// Addressed common-shape binding before object/scratch retention. The
    /// default preserves unsupported scope for hosts without a checked reader.
    /// Malformed/budget errors must propagate as Err, not Unsupported.
    fn initialize_common_shape(&mut self, _request: ShapeRequest) -> Result<ShapeOutcome> {
        Ok(ShapeOutcome::Unsupported)
    }

    /// Called after common scratch append, status2 activation and tail insertion.
    /// Failure retains those source side effects and blocks completed catalog.
    fn initialize_animator(&mut self, request: AnimatorRequest) -> Result<AnimatorId>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unsupported {
    Unclassified {
        native_type_index: u32,
    },
    Factory {
        class: u32,
        subtype: u32,
    },
    Shape {
        index: u32,
    },
    /// Common state and scratch ownership exist, but subtype initialization
    /// cannot finish before the actual resource-state provider returns.
    GenericResource {
        trigger: TriggerId,
        resource: u32,
    },
    Resource08Provider {
        trigger: TriggerId,
        resource: u32,
    },
    Range0cProvider {
        trigger: TriggerId,
        resource: u32,
    },
    Radius06Provider {
        trigger: TriggerId,
        resource: u32,
    },
    Actor12Provider {
        trigger: TriggerId,
        stage: super::actor12::ProviderStage,
    },
    Range82Provider {
        trigger: TriggerId,
        resource: u32,
    },
    Range27Provider {
        trigger: TriggerId,
        resource: u32,
    },
    Radius03Provider {
        trigger: TriggerId,
        resource: u32,
    },
    Common8AHook {
        trigger: TriggerId,
    },
    Shape71Geometry {
        trigger: TriggerId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    Unvisited,
    Disabled,
    Filtered,
    /// Proven factory/callback NULL; no common object or status was created.
    SourceNull,
    Unsupported(Unsupported),
    Owned(TriggerId),
}

impl Slot {
    /// A staged constructor can retain a real object/common binding without
    /// membership or completed geometry. Metadata-only unsupported slots cannot.
    pub fn retained_trigger(self) -> Option<TriggerId> {
        match self {
            Self::Owned(id) => Some(id),
            Self::Unsupported(
                Unsupported::GenericResource { trigger, .. }
                | Unsupported::Resource08Provider { trigger, .. }
                | Unsupported::Range0cProvider { trigger, .. }
                | Unsupported::Radius06Provider { trigger, .. }
                | Unsupported::Actor12Provider { trigger, .. }
                | Unsupported::Range82Provider { trigger, .. }
                | Unsupported::Range27Provider { trigger, .. }
                | Unsupported::Radius03Provider { trigger, .. }
                | Unsupported::Common8AHook { trigger }
                | Unsupported::Shape71Geometry { trigger },
            ) => Some(trigger),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reference {
    Unresolved,
    Null,
    Trigger(TriggerId),
    /// An unsupported target is not a legitimate source filtered NULL.
    Blocked {
        ordinal: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimatorState {
    NotRequired,
    Pending,
    Initialized(AnimatorId),
    Failed,
    /// Actual fresh status1 shutdown observed an absent animator. No status2
    /// initializer was called, even if a serialized selector was present.
    StoppedAbsent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapPhase {
    Visiting,
    ReferencesFixed,
    ParentsLinked,
    Initialized,
}

#[derive(Debug, Clone)]
pub struct Trigger {
    pub(super) id: TriggerId,
    pub(super) map: MapContext,
    pub(super) zone: u16,
    pub(super) ordinal: u16,
    pub(super) descriptor: Descriptor,
    pub(super) common: CommonInput,
    pub(super) common_shape: Option<ShapeBinding>,
    pub(super) kind: TriggerKind,
    pub(super) factory_status: FactoryStatus,
    pub(super) registration_status: Option<FactoryStatus>,
    pub(super) registered: bool,
    pub(super) runtime_header_flags_raw: u32,
    pub(super) runtime_numeric: [u32; 16],
    pub(super) generic: Option<super::generic::State>,
    pub(super) resource08: Option<super::resource08::State>,
    pub(super) range0c: Option<super::range0c::State>,
    pub(super) radius06: Option<super::radius06::State>,
    pub(super) actor12: Option<super::actor12::State>,
    pub(super) range82: Option<super::range82::State>,
    pub(super) a3: Option<super::a3::State>,
    pub(super) range27: Option<super::range27::State>,
    pub(super) ad: Option<super::ad::State>,
    pub(super) radius03: Option<super::radius03::State>,
    pub(super) common8a: Option<super::common8a::State>,
    pub(super) radius1b: Option<super::radius1b::State>,
    pub(super) radius74: Option<super::radius74::State>,
    pub(super) shape71: Option<super::shape71::State>,
    pub(super) radius88: Option<super::radius88::State>,
    pub(super) aux_ac: Option<super::aux_ac::State>,
    pub(super) original_position_bits: [u32; 4],
    pub(super) original_rotation_bits: [u32; 4],
    pub(super) runtime_flags_raw: u32,
    pub(super) references: [Reference; 8],
    pub(super) parent: Option<TriggerId>,
    pub(super) children: Vec<TriggerId>,
    pub(super) animator: AnimatorState,
}

impl Trigger {
    pub fn kind(&self) -> TriggerKind {
        self.kind
    }
    pub fn factory_status(&self) -> FactoryStatus {
        self.factory_status
    }
    pub fn registration_status(&self) -> Option<FactoryStatus> {
        self.registration_status
    }
    pub fn is_registered(&self) -> bool {
        self.registered
    }
    pub fn runtime_header_flags_raw(&self) -> u32 {
        self.runtime_header_flags_raw
    }
    pub fn runtime_numeric(&self) -> &[u32; 16] {
        &self.runtime_numeric
    }
    pub fn generic(&self) -> Option<&super::generic::State> {
        self.generic.as_ref()
    }
    pub fn resource08(&self) -> Option<&super::resource08::State> {
        self.resource08.as_ref()
    }
    pub fn range0c(&self) -> Option<&super::range0c::State> {
        self.range0c.as_ref()
    }
    pub fn radius06(&self) -> Option<&super::radius06::State> {
        self.radius06.as_ref()
    }
    pub fn actor12(&self) -> Option<&super::actor12::State> {
        self.actor12.as_ref()
    }
    pub fn range82(&self) -> Option<&super::range82::State> {
        self.range82.as_ref()
    }
    pub fn a3(&self) -> Option<&super::a3::State> {
        self.a3.as_ref()
    }
    pub fn range27(&self) -> Option<&super::range27::State> {
        self.range27.as_ref()
    }
    pub fn ad(&self) -> Option<&super::ad::State> {
        self.ad.as_ref()
    }
    pub fn radius03(&self) -> Option<&super::radius03::State> {
        self.radius03.as_ref()
    }
    pub fn common8a(&self) -> Option<&super::common8a::State> {
        self.common8a.as_ref()
    }
    pub fn radius1b(&self) -> Option<&super::radius1b::State> {
        self.radius1b.as_ref()
    }
    pub fn radius74(&self) -> Option<&super::radius74::State> {
        self.radius74.as_ref()
    }
    pub fn shape71(&self) -> Option<&super::shape71::State> {
        self.shape71.as_ref()
    }
    pub fn radius88(&self) -> Option<&super::radius88::State> {
        self.radius88.as_ref()
    }
    pub fn aux_ac(&self) -> Option<&super::aux_ac::State> {
        self.aux_ac.as_ref()
    }
    pub fn id(&self) -> TriggerId {
        self.id
    }
    pub fn map(&self) -> MapContext {
        self.map
    }
    pub fn zone(&self) -> u16 {
        self.zone
    }
    pub fn ordinal(&self) -> u16 {
        self.ordinal
    }
    pub fn descriptor(&self) -> Descriptor {
        self.descriptor
    }
    pub fn common(&self) -> &CommonInput {
        &self.common
    }
    pub fn common_shape(&self) -> Option<&ShapeBinding> {
        self.common_shape.as_ref()
    }
    pub fn original_position_bits(&self) -> [u32; 4] {
        self.original_position_bits
    }
    pub fn original_rotation_bits(&self) -> [u32; 4] {
        self.original_rotation_bits
    }
    pub fn runtime_flags_raw(&self) -> u32 {
        self.runtime_flags_raw
    }
    pub fn references(&self) -> &[Reference; 8] {
        &self.references
    }
    pub fn parent(&self) -> Option<TriggerId> {
        self.parent
    }
    pub fn children(&self) -> &[TriggerId] {
        &self.children
    }
    pub fn animator(&self) -> AnimatorState {
        self.animator
    }
}

#[derive(Debug, Clone)]
pub(super) struct Map {
    pub context: MapContext,
    pub filter: Option<Vec<u32>>,
    pub slots: Vec<Slot>,
    pub next: usize,
    pub phase: MapPhase,
}

#[derive(Debug, Clone, Copy)]
pub struct SelectedStartPoint {
    pub selection: spawn::Selection,
    pub trigger: Option<TriggerId>,
    pub pose: Option<spawn::Pose>,
}
