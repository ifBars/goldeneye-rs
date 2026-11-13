use super::*;
use anyhow::Context;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnableWrite {
    pub receiver: Identity,
    pub reference: u32,
    pub enabled: bool,
    pub reference_matched: bool,
    pub flags_before: Option<u16>,
    pub flags_after: Option<u16>,
    pub invalidated_8e: bool,
}

impl super::super::Preparation {
    pub fn apply_pose_enable(
        &mut self,
        receiver: Identity,
        reference: u32,
        enabled: bool,
    ) -> Result<EnableWrite> {
        self.mode_animator_mut(receiver)?
            .apply_pose_enable(reference, enabled)
    }
}
impl Animator {
    fn apply_pose_enable(&mut self, reference: u32, enabled: bool) -> Result<EnableWrite> {
        ensure!(
            !matches!((reference >> 24) & 0x7f, 2 | 0x0e),
            "pose enable needs reached skeleton reference provider"
        );
        let matched = self
            .word(0x20)
            .context("pose enable current reference unknown")?
            == reference;
        let mut write = EnableWrite {
            receiver: self.identity,
            reference,
            enabled,
            reference_matched: matched,
            flags_before: None,
            flags_after: None,
            invalidated_8e: false,
        };
        if !matched {
            return Ok(write);
        }
        let before = u16::from_be_bytes([
            self.byte(0xc).context("pose enable flags unknown")?,
            self.byte(0xd).context("pose enable flags unknown")?,
        ]);
        let after = if enabled { before | 1 } else { before & 0xfffe };
        if enabled {
            self.bytes[0x8e] = Some(0);
            write.invalidated_8e = true;
        }
        self.half_store(0xc, after);
        write.flags_before = Some(before);
        write.flags_after = Some(after);
        Ok(write)
    }
}

#[cfg(test)]
mod tests;
