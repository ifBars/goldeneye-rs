//! Actual ordered actor and per-bit membership owners. Tail actor registration
//! and head bit insertion are intentionally distinct operations.
use super::{
    identity::{ComponentId, ItemId, WorldId},
    item::{Item, Node, Payload, Registration},
};
use anyhow::{Context, Result, ensure};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub items: usize,
    pub memberships: usize,
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub first: Option<ItemId>,
    pub last: Option<ItemId>,
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Update {
    pub inserted: usize,
    pub removed: usize,
}
pub trait Components {
    fn activate(&mut self, item: ItemId, component: ComponentId, enabled: bool) -> Result<()>;
}

pub struct World {
    id: WorldId,
    budget: Budget,
    items: BTreeMap<ItemId, Item>,
    actors: Header,
    memberships: Vec<Header>,
    next_id: u64,
}
impl World {
    /// Actual world constructor's actor/header counts, before map operations.
    /// Calling this at player creation does not reconstruct previous history.
    pub fn from_constructor(
        id: WorldId,
        actor_headers: usize,
        membership_count: usize,
        budget: Budget,
    ) -> Result<Self> {
        ensure!(
            actor_headers == 1,
            "multiple actor-list headers are unsupported"
        );
        ensure!(
            (5..=32).contains(&membership_count) && membership_count <= budget.memberships,
            "world membership budget exceeded"
        );
        Ok(Self {
            id,
            budget,
            items: BTreeMap::new(),
            actors: Header::default(),
            memberships: vec![Header::default(); membership_count],
            next_id: 1,
        })
    }
    pub fn id(&self) -> WorldId {
        self.id
    }
    pub fn item_count(&self) -> usize {
        self.items.len()
    }
    pub fn membership_count(&self) -> usize {
        self.memberships.len()
    }
    pub fn actor_header(&self) -> Header {
        self.actors
    }
    pub fn membership_header(&self, index: usize) -> Option<Header> {
        self.memberships.get(index).copied()
    }
    pub fn item(&self, id: ItemId) -> Result<&Item> {
        self.items
            .get(&id)
            .context("world item identity is not owned")
    }
    pub fn item_mut(&mut self, id: ItemId) -> Result<&mut Item> {
        self.items
            .get_mut(&id)
            .context("world item identity is not owned")
    }
    pub fn construct_item(&mut self) -> Result<ItemId> {
        ensure!(
            self.items.len() < self.budget.items,
            "world item budget exhausted"
        );
        let id = ItemId::new(self.id, self.next_id)?;
        let next = self
            .next_id
            .checked_add(1)
            .context("world item identity exhausted")?;
        let item = Item::from_constructor(self.id, id, self.memberships.len())?;
        self.items.insert(id, item);
        self.next_id = next;
        Ok(id)
    }
    pub fn actors(&self) -> Result<Vec<ItemId>> {
        self.walk(self.actors, None)
    }
    pub fn members(&self, bit: usize) -> Result<Vec<ItemId>> {
        self.walk(
            *self
                .memberships
                .get(bit)
                .context("membership index outside world")?,
            Some(bit),
        )
    }
    fn walk(&self, header: Header, bit: Option<usize>) -> Result<Vec<ItemId>> {
        let mut ids = Vec::new();
        let mut current = header.first;
        while let Some(id) = current {
            ensure!(
                ids.len() < self.budget.items,
                "world actor list cycle or budget exhausted"
            );
            let node = self.node(id, bit)?;
            ensure!(
                node.previous == ids.last().copied(),
                "world actor previous link differs"
            );
            ids.push(id);
            current = node.next;
        }
        ensure!(
            ids.last().copied() == header.last,
            "world actor tail differs"
        );
        Ok(ids)
    }
    pub fn contains_actor(&self, id: ItemId) -> Result<bool> {
        Ok(self.actors()?.contains(&id))
    }
    pub fn register_if_absent(
        &mut self,
        id: ItemId,
        desired: u32,
        provider: &mut impl Components,
    ) -> Result<bool> {
        self.check_mask(desired)?;
        if self.contains_actor(id)? {
            ensure!(
                self.item(id)?.registration == Registration::Complete,
                "world retains an incomplete actor registration"
            );
            return Ok(false);
        }
        ensure!(
            self.item(id)?.registration == Registration::Constructed,
            "item registration is not fresh"
        );
        let tail = self.actors.last;
        self.item_mut(id)?.actor = Node {
            previous: tail,
            next: None,
            payload: Payload::SelfNode,
        };
        if let Some(tail) = tail {
            self.item_mut(tail)?.actor.next = Some(id);
        } else {
            self.actors.first = Some(id);
        }
        self.actors.last = Some(id);
        let result: Result<()> = (|| {
            let count = self.item(id)?.components.len();
            self.item_mut(id)?.registration = Registration::Activating { completed: 0 };
            for index in 0..count {
                let component = self.item(id)?.components[index];
                provider.activate(id, component, true)?;
                self.item_mut(id)?.registration = Registration::Activating {
                    completed: index + 1,
                };
            }
            self.update_membership(id, desired)?;
            self.item_mut(id)?.finish_registration();
            Ok(())
        })();
        if result.is_err() {
            self.item_mut(id)?.registration = Registration::Failed;
        }
        result?;
        Ok(true)
    }
    fn check_mask(&self, mask: u32) -> Result<()> {
        ensure!(
            self.memberships.len() == 32 || mask >> self.memberships.len() == 0,
            "membership mask reaches unavailable world list"
        );
        Ok(())
    }
    pub fn update_membership(&mut self, id: ItemId, desired: u32) -> Result<Update> {
        self.check_mask(desired)?;
        let changed = self.item(id)?.mask ^ desired;
        let mut receipt = Update::default();
        for bit in 0..self.memberships.len() {
            let mask = 1u32 << bit;
            if changed & mask == 0 {
                continue;
            }
            if desired & mask != 0 {
                self.insert_head(id, bit)?;
                receipt.inserted += 1;
            } else {
                self.remove_node(id, bit)?;
                receipt.removed += 1;
            }
        }
        // The source mask is committed after all individual list mutations.
        self.item_mut(id)?.mask = desired;
        Ok(receipt)
    }
    fn node(&self, id: ItemId, bit: Option<usize>) -> Result<Node> {
        let item = self.item(id)?;
        if let Some(bit) = bit {
            item.nodes
                .get(bit)
                .copied()
                .context("item membership node absent")
        } else {
            Ok(item.actor)
        }
    }
    fn node_mut(&mut self, id: ItemId, bit: usize) -> Result<&mut Node> {
        self.item_mut(id)?
            .nodes
            .get_mut(bit)
            .context("item membership node absent")
    }
    fn insert_head(&mut self, id: ItemId, bit: usize) -> Result<()> {
        let head = self.memberships[bit].first;
        *self.node_mut(id, bit)? = Node {
            previous: None,
            next: head,
            payload: Payload::Item(id),
        };
        if let Some(head) = head {
            self.node_mut(head, bit)?.previous = Some(id);
        }
        self.memberships[bit].first = Some(id);
        if self.memberships[bit].last.is_none() {
            self.memberships[bit].last = Some(id);
        }
        Ok(())
    }
    fn remove_node(&mut self, id: ItemId, bit: usize) -> Result<()> {
        let node = self.node(id, Some(bit))?;
        ensure!(
            node.payload == Payload::Item(id),
            "membership node has not been inserted"
        );
        if self.memberships[bit].first == Some(id) {
            self.memberships[bit].first = node.next;
        }
        if self.memberships[bit].last == Some(id) {
            self.memberships[bit].last = node.previous;
        }
        if let Some(previous) = node.previous {
            self.node_mut(previous, bit)?.next = node.next;
            self.node_mut(id, bit)?.previous = None;
        }
        if let Some(next) = node.next {
            self.node_mut(next, bit)?.previous = node.previous;
            self.node_mut(id, bit)?.next = None;
        }
        Ok(())
    }
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        fn wide(e: &mut impl FnMut(u32), n: u64) {
            e((n >> 32) as u32);
            e(n as u32);
        }
        fn header(e: &mut impl FnMut(u32), h: Header) {
            for id in [h.first, h.last] {
                e(u32::from(id.is_some()));
                if let Some(id) = id {
                    wide(e, id.get());
                }
            }
        }
        wide(&mut emit, self.id.get());
        wide(&mut emit, self.next_id);
        header(&mut emit, self.actors);
        wide(&mut emit, self.memberships.len() as u64);
        for h in &self.memberships {
            header(&mut emit, *h);
        }
        wide(&mut emit, self.items.len() as u64);
        for item in self.items.values() {
            item.visit_state_words(&mut emit);
        }
    }
}

#[cfg(test)]
#[path = "tests/world.rs"]
mod tests;
