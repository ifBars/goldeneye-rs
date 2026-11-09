//! Explicit received-event arithmetic. The separate identity-bound loader
//! bridge applies actual phase19 operations after host providers finish;
//! map metadata alone never authorizes readiness.
use super::UserId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlagsDelta {
    pub set: u32,
    pub clear: u32,
}

impl FlagsDelta {
    pub const fn apply(self, previous: u32) -> u32 {
        (previous | self.set) & !self.clear
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeerSnapshot {
    pub identity: UserId,
    pub flags_raw: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserSnapshot {
    pub identity: UserId,
    pub flags_raw: u32,
    pub created_byte_raw: u8,
}

/// Apply an explicitly received source delta to an existing matching peer,
/// then mirror it only to the matching admitted user. Unknown peers are not
/// constructed. This helper does not emit a loader event or mutate Session.
pub fn receive_delta(
    source: UserId,
    peer: Option<&mut PeerSnapshot>,
    user: &mut UserSnapshot,
    delta: FlagsDelta,
) -> bool {
    let Some(peer) = peer.filter(|peer| peer.identity == source) else {
        return false;
    };
    peer.flags_raw = delta.apply(peer.flags_raw);
    if user.identity != source {
        return false;
    }
    user.flags_raw = peer.flags_raw;
    true
}
