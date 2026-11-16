//! A small authored simulation: numbered actors with integer positions.
//!
//! It exists to test networking contracts. It is not a game, and its tick
//! rate is a test profile rather than evidence about any original title.

use std::collections::{BTreeMap, BTreeSet};

use ge_net::{EntityAllocator, EntityId, EntityRecord, PlayerSlot, ReceivedCommand, Snapshot};

/// Owner value of actors that no player controls.
pub const UNOWNED: u8 = u8::MAX;
pub const ACTOR_BYTES: usize = 13;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Actor {
    pub owner: u8,
    pub x: i32,
    pub y: i32,
    pub moves: u32,
}

impl Actor {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(ACTOR_BYTES);
        out.push(self.owner);
        out.extend_from_slice(&self.x.to_be_bytes());
        out.extend_from_slice(&self.y.to_be_bytes());
        out.extend_from_slice(&self.moves.to_be_bytes());
        out
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != ACTOR_BYTES {
            return None;
        }
        Some(Self {
            owner: bytes[0],
            x: i32::from_be_bytes(bytes[1..5].try_into().ok()?),
            y: i32::from_be_bytes(bytes[5..9].try_into().ok()?),
            moves: u32::from_be_bytes(bytes[9..13].try_into().ok()?),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    /// Move the slot's avatar.
    Move { dx: i8, dy: i8 },
    /// Spawn an extra actor owned by the slot.
    Spawn,
    /// Despawn the slot's oldest extra actor.
    Despawn,
}

impl Command {
    pub fn encode(&self) -> Vec<u8> {
        match self {
            Self::Move { dx, dy } => vec![0, *dx as u8, *dy as u8],
            Self::Spawn => vec![1],
            Self::Despawn => vec![2],
        }
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        match bytes {
            [0, dx, dy] => Some(Self::Move {
                dx: *dx as i8,
                dy: *dy as i8,
            }),
            [1] => Some(Self::Spawn),
            [2] => Some(Self::Despawn),
            _ => None,
        }
    }
}

/// The authoritative synthetic world.
#[derive(Debug)]
pub struct World {
    allocator: EntityAllocator,
    actors: BTreeMap<EntityId, Actor>,
    avatars: BTreeMap<PlayerSlot, EntityId>,
    extras: BTreeMap<PlayerSlot, Vec<EntityId>>,
    drifters: Vec<EntityId>,
    /// Every executed `(slot, sequence)`, to prove at-most-once execution.
    executed: BTreeSet<(u8, u32)>,
    pub duplicate_executions: u64,
    pub rejected_commands: u64,
    pub steps: u64,
}

impl World {
    pub fn new(capacity: u16, drifters: usize) -> Self {
        let mut world = Self {
            allocator: EntityAllocator::new(capacity),
            actors: BTreeMap::new(),
            avatars: BTreeMap::new(),
            extras: BTreeMap::new(),
            drifters: Vec::new(),
            executed: BTreeSet::new(),
            duplicate_executions: 0,
            rejected_commands: 0,
            steps: 0,
        };
        for index in 0..drifters {
            let id = world.spawn(Actor {
                owner: UNOWNED,
                x: index as i32,
                y: 0,
                moves: 0,
            });
            world.drifters.extend(id);
        }
        world
    }

    fn spawn(&mut self, actor: Actor) -> Option<EntityId> {
        let id = self.allocator.allocate().ok()?;
        self.actors.insert(id, actor);
        Some(id)
    }

    fn despawn(&mut self, id: EntityId) {
        if self.actors.remove(&id).is_some() {
            self.allocator.free(id).expect("live actor");
        }
    }

    /// Give `slot` an avatar. Joining twice is a no-op.
    pub fn join(&mut self, slot: PlayerSlot) {
        if self.avatars.contains_key(&slot) {
            return;
        }
        let avatar = self.spawn(Actor {
            owner: slot.get(),
            x: 0,
            y: i32::from(slot.get()) * 10,
            moves: 0,
        });
        if let Some(avatar) = avatar {
            self.avatars.insert(slot, avatar);
        }
    }

    /// Remove everything `slot` owns.
    pub fn leave(&mut self, slot: PlayerSlot) {
        if let Some(avatar) = self.avatars.remove(&slot) {
            self.despawn(avatar);
        }
        for extra in self.extras.remove(&slot).unwrap_or_default() {
            self.despawn(extra);
        }
    }

    /// Validate and execute one command. Transport does not make it valid.
    pub fn apply(&mut self, command: &ReceivedCommand) {
        if !self
            .executed
            .insert((command.slot.get(), command.sequence.get()))
        {
            self.duplicate_executions += 1;
            return;
        }
        let Some(decoded) = Command::decode(&command.payload) else {
            self.rejected_commands += 1;
            return;
        };
        let Some(avatar) = self.avatars.get(&command.slot).copied() else {
            self.rejected_commands += 1;
            return;
        };
        match decoded {
            Command::Move { dx, dy } => {
                // A rule the host enforces: one step per axis per command.
                // Range checks, not `abs()`: `i8::MIN.abs()` overflows.
                if !(-1..=1).contains(&dx) || !(-1..=1).contains(&dy) {
                    self.rejected_commands += 1;
                    return;
                }
                let actor = self.actors.get_mut(&avatar).expect("avatar exists");
                actor.x += i32::from(dx);
                actor.y += i32::from(dy);
                actor.moves += 1;
            }
            Command::Spawn => {
                let origin = self.actors[&avatar];
                let extra = self.spawn(Actor {
                    owner: command.slot.get(),
                    x: origin.x,
                    y: origin.y,
                    moves: 0,
                });
                match extra {
                    Some(extra) => self.extras.entry(command.slot).or_default().push(extra),
                    None => self.rejected_commands += 1,
                }
            }
            Command::Despawn => {
                let extras = self.extras.entry(command.slot).or_default();
                if extras.is_empty() {
                    self.rejected_commands += 1;
                } else {
                    let extra = extras.remove(0);
                    self.despawn(extra);
                }
            }
        }
    }

    /// Advance one tick: unowned actors drift so state changes every tick.
    pub fn step(&mut self) {
        self.steps += 1;
        for id in &self.drifters {
            let actor = self.actors.get_mut(id).expect("drifter exists");
            actor.x = actor.x.wrapping_add(1);
            actor.moves = actor.moves.wrapping_add(1);
        }
    }

    pub fn executed(&self) -> &BTreeSet<(u8, u32)> {
        &self.executed
    }

    pub fn actor(&self, id: EntityId) -> Option<Actor> {
        self.actors.get(&id).copied()
    }

    pub fn avatar(&self, slot: PlayerSlot) -> Option<EntityId> {
        self.avatars.get(&slot).copied()
    }

    pub fn actor_count(&self) -> usize {
        self.actors.len()
    }

    pub fn snapshot(&self) -> Snapshot {
        let records = self
            .actors
            .iter()
            .map(|(id, actor)| EntityRecord {
                id: *id,
                state: actor.encode(),
            })
            .collect();
        Snapshot::from_records(records).expect("allocator indices are unique")
    }
}

/// FNV-1a over a snapshot's canonical content.
pub fn digest(snapshot: &Snapshot) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |bytes: &[u8]| {
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    for record in snapshot.records() {
        feed(&record.id.index.to_be_bytes());
        feed(&record.id.generation.to_be_bytes());
        feed(&(record.state.len() as u32).to_be_bytes());
        feed(&record.state);
    }
    hash
}
