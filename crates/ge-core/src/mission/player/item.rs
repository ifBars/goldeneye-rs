//! Source-written EXItem fields needed by the initial player owner operation.
//! This is sparse typed state, not a zeroed native object image.
use super::identity::{ComponentId, ItemId, PhysicsId, PlayerId, WorldId};
use anyhow::{Result, ensure};

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
    }
}
