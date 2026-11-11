//! Reached visual writes use the base constructor's actual slots and +474
//! collection header. Unknown/nonempty backing requires its real cleanup.
use super::{Binding, Owner, Stage};
use crate::mission::player::components::Identity;
use anyhow::{Context, Result, ensure};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisualOverrides {
    pub generation: u32,
    pub capacity: u32,
    pub growth: u32,
    pub count: u32,
}

impl Owner {
    pub(crate) fn clear_visual_slots(&mut self) -> Result<()> {
        ensure!(
            self.stage == Stage::VirtualBasicComplete
                && self.visual_skeleton.is_none()
                && [0x464, 0x468, 0x46c]
                    .into_iter()
                    .all(|at| self.word(at) == Some(0)),
            "visual slot clear requires actual fresh player fields"
        );
        self.zero_words(0x464, 0x46c);
        Ok(())
    }
    pub(crate) fn bind_visual_skeleton(&mut self, identity: Identity) -> Result<()> {
        ensure!(
            identity.player() == self.player && self.stage == Stage::VirtualBasicComplete,
            "visual skeleton player differs"
        );
        self.visual_skeleton = Some(identity);
        self.words.remove(&0x464);
        self.bindings.insert(0x464, Binding::VisualSkeleton);
        Ok(())
    }
    pub fn visual_skeleton(&self) -> Option<Identity> {
        self.visual_skeleton
    }
    pub fn visual_overrides(&self) -> Result<VisualOverrides> {
        ensure!(
            self.bindings.get(&0x488) == Some(&Binding::Container),
            "visual collection constructor is absent"
        );
        Ok(VisualOverrides {
            generation: self.word(0x474).context("visual generation is unknown")?,
            capacity: self.word(0x478).context("visual capacity is unknown")?,
            growth: self.word(0x47c).context("visual growth is unknown")?,
            count: self.word(0x480).context("visual count is unknown")?,
        })
    }
    pub(crate) fn reset_empty_visual_overrides(&mut self) -> Result<VisualOverrides> {
        let before = self.visual_overrides()?;
        ensure!(
            before.count == 0 && self.word(0x484) == Some(0),
            "nonempty visual overrides require actual cleanup/backing ownership"
        );
        for at in [0x478, 0x480, 0x484] {
            self.w(at, 0);
        }
        self.w(0x474, before.generation.wrapping_add(1));
        self.visual_overrides()
    }
}

#[cfg(test)]
#[path = "tests/visual.rs"]
mod tests;
