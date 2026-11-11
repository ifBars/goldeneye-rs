use super::super::super::Owned;
use anyhow::{Context, Result, ensure};
use ge_core::mission::player::world::World;
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VisualPostbind {
    pub wrapper_reused: bool,
    pub secondary_skin_bound: bool,
    pub skin_requests: usize,
    pub texture_requests: usize,
    pub texture_targets_resident: usize,
    pub override_generation: u32,
    pub override_capacity: u32,
    pub override_growth: u32,
    pub override_count: u32,
    pub item_collection_bound: bool,
    pub configured_headers: usize,
    pub callback_bindings_complete: bool,
    pub state_sha256: String,
}
pub(super) fn build(player: &Owned, world: &World) -> Result<VisualPostbind> {
    let owner = &player.components;
    let returned = owner
        .preparation
        .visual_postbind()
        .context("visual postbind return is absent")?;
    let source = owner
        .postbind
        .source
        .as_ref()
        .context("visual postbind wrapper is absent")?;
    let body = owner
        .postbind
        .body
        .as_ref()
        .context("visual postbind body is absent")?;
    let target = owner
        .postbind
        .target
        .as_ref()
        .context("visual postbind skin target is absent")?;
    let original = owner
        .target
        .as_ref()
        .context("initial visual target is absent")?;
    let enumeration = owner
        .postbind
        .enumeration
        .as_ref()
        .context("postbind traversal is absent")?;
    let animator = owner
        .preparation
        .animator()
        .context("postbind animator is absent")?;
    let item = world.item(returned.item())?;
    ensure!(
        source.same_target(original)
            && source.same_target(target)
            && body.file() == returned.secondary().file
            && body.reference() == returned.secondary().reference
            && returned.component() == animator.identity()
            && player.base.visual_skeleton() == Some(animator.identity())
            && item.visual_overrides()? == player.base.player()
            && item.flags() & 0x80 != 0
            && returned.overrides() == player.base.visual_overrides()?
            && returned.overrides().count == 0
            && owner.postbind.textures.len() == enumeration.hashes.len()
            && returned.texture_requests() == enumeration.hashes.len()
            && animator.byte(0x8c) == Some(0x11)
            && animator.word(0x118) == Some(0),
        "visual postbind source or owner state differs"
    );
    for start in [0x138, 0x158, 0x178, 0x198] {
        ensure!(
            animator.byte(start + 8) == Some(0)
                && animator.byte(start + 9) == Some(1)
                && animator.byte(start + 10) == Some(3),
            "TP header configuration differs"
        );
    }
    let mut hash = Sha256::new();
    hash.update(b"ge_player_tp_visual_postbind_v1\0");
    returned.visit_state_words(|word| hash.update(word.to_be_bytes()));
    for raw in [body.file(), body.reference(), body.stored_hash()] {
        hash.update(raw.to_be_bytes());
    }
    for at in [body.directory_index(), body.address()] {
        hash.update((at as u64).to_be_bytes());
    }
    hash.update(target.fingerprint().as_bytes());
    for (texture, &expected) in owner.postbind.textures.iter().zip(&enumeration.hashes) {
        ensure!(
            target.same_source(texture) && texture.target() == expected,
            "postbind texture prefix differs"
        );
        hash.update(texture.fingerprint().as_bytes());
    }
    let overrides = returned.overrides();
    Ok(VisualPostbind {
        wrapper_reused: true,
        secondary_skin_bound: true,
        skin_requests: 1,
        texture_requests: returned.texture_requests(),
        texture_targets_resident: owner.postbind.textures.len(),
        override_generation: overrides.generation,
        override_capacity: overrides.capacity,
        override_growth: overrides.growth,
        override_count: overrides.count,
        item_collection_bound: true,
        configured_headers: 4,
        callback_bindings_complete: false,
        state_sha256: format!("{:x}", hash.finalize()),
    })
}
