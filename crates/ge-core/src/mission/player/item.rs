//! Source-written EXItem fields needed by the initial player owner operation.
//! This is sparse typed state, not a zeroed native object image.
use super::identity::{ComponentId, ItemId, PhysicsId, PlayerId, WorldId};
use anyhow::{Result, ensure};

/// Reached wrapperless derived-factory fields, separate from actor membership.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WrapperlessFactory {
    numeric_id: u32,
    word_174: u32,
    word_178: u32,
    word_17c: u32,
    word_184: u32,
    cache_index: u32,
    wrapper_bound: bool,
    key: u32,
    resource_cache_index: u32,
    attachment_bound: bool,
    byte_8e: u8,
}
impl WrapperlessFactory {
    pub fn numeric_id(&self) -> u32 {
        self.numeric_id
    }
    pub fn wrapper_bound(&self) -> bool {
        self.wrapper_bound
    }
    pub fn key(&self) -> u32 {
        self.key
    }
    pub fn cache_index(&self) -> u32 {
        self.cache_index
    }
    pub fn resource_cache_index(&self) -> u32 {
        self.resource_cache_index
    }
    pub fn attachment_bound(&self) -> bool {
        self.attachment_bound
    }
    pub fn byte_8e(&self) -> u8 {
        self.byte_8e
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Payload {
    SelfNode,
    Item(ItemId),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Node {
    pub previous: Option<ItemId>,
    pub next: Option<ItemId>,
    pub payload: Payload,
}
impl Node {
    pub(super) const fn from_constructor() -> Self {
        Self {
            previous: None,
            next: None,
            payload: Payload::SelfNode,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Registration {
    Constructed,
    Activating { completed: usize },
    Complete,
    Failed,
}

pub struct Item {
    pub(super) world: WorldId,
    pub(super) id: ItemId,
    pub(super) actor: Node,
    pub(super) nodes: Vec<Node>,
    pub(super) mask: u32,
    pub(super) registration: Registration,
    pub(super) components: Vec<ComponentId>,
    flags: u16,
    handler: Option<PlayerId>,
    physics: Option<PhysicsId>,
    position: [u32; 4],
    rotation: [u32; 4],
    scale: [u32; 4],
    zone: u16,
    priority: u8,
    word_180: Option<u32>,
    factory_prefix: Option<[u32; 3]>,
    factory: Option<WrapperlessFactory>,
}
impl Item {
    pub(super) fn from_constructor(world: WorldId, id: ItemId, memberships: usize) -> Result<Self> {
        ensure!(
            (5..=32).contains(&memberships),
            "EXItem membership count outside checked node budget"
        );
        let mut nodes = Vec::new();
        nodes.try_reserve_exact(memberships)?;
        nodes.resize(memberships, Node::from_constructor());
        Ok(Self {
            world,
            id,
            actor: Node::from_constructor(),
            nodes,
            mask: 0,
            registration: Registration::Constructed,
            components: Vec::new(),
            flags: 0xb,
            handler: None,
            physics: None,
            position: [0; 4],
            rotation: [0; 4],
            scale: [0x3f80_0000; 4],
            zone: u16::MAX,
            priority: 0,
            word_180: None,
            factory_prefix: None,
            factory: None,
        })
    }
    pub fn id(&self) -> ItemId {
        self.id
    }
    pub fn world(&self) -> WorldId {
        self.world
    }
    pub fn flags(&self) -> u16 {
        self.flags
    }
    pub fn handler(&self) -> Option<PlayerId> {
        self.handler
    }
    pub fn physics(&self) -> Option<PhysicsId> {
        self.physics
    }
    pub fn position_bits(&self) -> [u32; 4] {
        self.position
    }
    pub fn rotation_bits(&self) -> [u32; 4] {
        self.rotation
    }
    pub fn scale_bits(&self) -> [u32; 4] {
        self.scale
    }
    pub fn zone(&self) -> u16 {
        self.zone
    }
    pub fn priority(&self) -> u8 {
        self.priority
    }
    pub fn membership_mask(&self) -> u32 {
        self.mask
    }
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }
    pub fn actor_node(&self) -> Node {
        self.actor
    }
    pub fn registration(&self) -> Registration {
        self.registration
    }
    pub fn components(&self) -> &[ComponentId] {
        &self.components
    }
    pub fn word_180(&self) -> Option<u32> {
        self.word_180
    }
    pub fn factory(&self) -> Option<&WrapperlessFactory> {
        self.factory.as_ref()
    }
    pub fn factory_prefix_initialized(&self) -> bool {
        self.factory_prefix.is_some()
    }
    /// Source derived-type installation and +178/+17c/+184 stores happen
    /// before the registry consumes its numeric ID and may fail allocation.
    pub fn begin_wrapperless_factory(&mut self) -> Result<()> {
        ensure!(
            self.factory_prefix.is_none() && self.factory.is_none(),
            "wrapperless item factory already started"
        );
        ensure!(
            self.registration == Registration::Constructed,
            "wrapperless factory must precede actor registration"
        );
        self.factory_prefix = Some([0; 3]);
        Ok(())
    }
    /// Complete only the supported wrapperless factory after its actual
    /// persistent numeric-registry append. Actor/component registration stays
    /// separate; no PlayerNew/EXItem readiness is inferred from this receipt.
    pub fn finish_wrapperless_factory(
        &mut self,
        receipt: &super::item_registry::RegistrationReceipt,
    ) -> Result<()> {
        ensure!(
            receipt.item() == self.id,
            "item factory registration receipt belongs to another item"
        );
        ensure!(
            self.factory.is_none(),
            "wrapperless item factory already completed"
        );
        ensure!(
            self.factory_prefix == Some([0; 3]),
            "wrapperless item factory prefix has not run"
        );
        ensure!(
            self.registration == Registration::Constructed,
            "wrapperless factory must precede actor registration"
        );
        self.factory = Some(WrapperlessFactory {
            numeric_id: receipt.numeric_id(),
            word_174: 0,
            word_178: 0,
            word_17c: 0,
            word_184: 0,
            cache_index: u32::MAX,
            wrapper_bound: false,
            key: u32::MAX,
            resource_cache_index: u32::MAX,
            attachment_bound: false,
            byte_8e: 0,
        });
        self.word_180 = Some(0);
        self.position = [0; 4];
        Ok(())
    }
    /// Actual caller writes before binding the player handler.
    pub fn prepare_player_owner(&mut self) {
        self.flags |= 0x20;
        self.word_180 = Some(0);
    }
    /// Binding returns the real backlink association for the caller's handler.
    /// Clearing this link never implies destruction of the separate owner.
    pub fn bind_handler(&mut self, handler: Option<PlayerId>) -> Option<(PlayerId, ItemId)> {
        self.handler = handler;
        handler.map(|handler| (handler, self.id))
    }
    pub fn bind_physics(&mut self, physics: Option<PhysicsId>) -> Option<(PhysicsId, ItemId)> {
        self.physics = physics;
        physics.map(|physics| (physics, self.id))
    }
    pub fn set_zone(&mut self, zone: u16) {
        self.zone = zone;
    }
    /// The later source pose write is separate from the constructor-origin query.
    pub fn set_pose(&mut self, position: [u32; 4], rotation: [u32; 4]) {
        self.position = position;
        self.rotation = rotation;
    }
    pub fn attach_component(&mut self, component: ComponentId, limit: usize) -> Result<()> {
        ensure!(
            self.components.len() < limit,
            "item component budget exhausted"
        );
        ensure!(
            !self.components.contains(&component),
            "component is already linked to this item"
        );
        self.components.try_reserve(1)?;
        self.components.push(component);
        Ok(())
    }
    pub(super) fn finish_registration(&mut self) {
        self.priority = 0xff;
        self.registration = Registration::Complete;
    }
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        fn wide(emit: &mut impl FnMut(u32), n: u64) {
            emit((n >> 32) as u32);
            emit(n as u32);
        }
        fn option(emit: &mut impl FnMut(u32), n: Option<u64>) {
            emit(u32::from(n.is_some()));
            if let Some(n) = n {
                wide(emit, n);
            }
        }
        fn node(emit: &mut impl FnMut(u32), n: Node) {
            option(emit, n.previous.map(ItemId::get));
            option(emit, n.next.map(ItemId::get));
            match n.payload {
                Payload::SelfNode => emit(0),
                Payload::Item(id) => {
                    emit(1);
                    wide(emit, id.get());
                }
            }
        }
        wide(&mut emit, self.world.get());
        wide(&mut emit, self.id.get());
        emit(u32::from(self.flags));
        option(&mut emit, self.handler.map(PlayerId::get));
        option(&mut emit, self.physics.map(PhysicsId::get));
        for word in self
            .position
            .into_iter()
            .chain(self.rotation)
            .chain(self.scale)
        {
            emit(word);
        }
        emit(u32::from(self.zone));
        emit(u32::from(self.priority));
        emit(self.mask);
        emit(u32::from(self.word_180.is_some()));
        if let Some(word) = self.word_180 {
            emit(word);
        }
        node(&mut emit, self.actor);
        wide(&mut emit, self.nodes.len() as u64);
        for n in &self.nodes {
            node(&mut emit, *n);
        }
        wide(&mut emit, self.components.len() as u64);
        for id in &self.components {
            wide(&mut emit, id.get());
        }
        match self.registration {
            Registration::Constructed => emit(0),
            Registration::Activating { completed } => {
                emit(1);
                wide(&mut emit, completed as u64);
            }
            Registration::Complete => emit(2),
            Registration::Failed => emit(3),
        }
        emit(u32::from(self.factory_prefix.is_some()));
        if let Some(prefix) = self.factory_prefix {
            for value in prefix {
                emit(value);
            }
        }
        emit(u32::from(self.factory.is_some()));
        if let Some(factory) = self.factory {
            for value in [
                factory.numeric_id,
                factory.word_174,
                factory.word_178,
                factory.word_17c,
                factory.word_184,
                factory.cache_index,
                u32::from(factory.wrapper_bound),
                factory.key,
                factory.resource_cache_index,
                u32::from(factory.attachment_bound),
                u32::from(factory.byte_8e),
            ] {
                emit(value);
            }
        }
    }
}
