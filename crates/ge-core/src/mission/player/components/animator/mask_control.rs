//! Live opcode-five capture, separate from the earlier four-physical-slot TP
//! capture. The command owns allocation/refresh/capture/list return evidence.
use super::*;
use crate::animation::{
    layers::command,
    scratch::{Arena, Context as Scratch},
    skeleton::LocalPose,
};
use anyhow::Context;
impl super::super::Preparation {
    pub fn finish_mode_mask_command(
        &mut self,
        receiver: Identity,
        command: &mut command::Pending,
        context: &mut Scratch,
        arena: &mut Arena,
    ) -> Result<()> {
        let owner = self.mode_animator_mut(receiver)?;
        let result = (|| {
            let first = command.begin_mask_effects(
                owner
                    .layers
                    .as_mut()
                    .context("mask command layers absent")?,
                &mut owner.controls,
                &mut owner.node_ids,
            )?;
            if let Some(first) = first {
                // Reached ordinary refresh dependencies. Nonempty lists need
                // their real animation evaluation; they are not a no-op pose.
                let layers = owner.layers.as_ref().expect("retained layers");
                if layers
                    .slots()
                    .iter()
                    .take(usize::from(layers.active_count()))
                    .any(|s| !s.nodes_head().is_empty())
                {
                    return Ok(());
                }
                let lod = owner
                    .word(0x11c)
                    .context("mask refresh LOD owner unknown")?;
                let child = owner
                    .word(0x10)
                    .context("mask refresh child owner unknown")?;
                if lod != 0 || child != 0 {
                    return Ok(());
                }
                let refresh = owner.refresh_for_capture(context, arena)?;
                command.record_mask_refresh(refresh)?;
                // The extra-record provider belongs to backing construction,
                // after the ordinary refresh has returned and been retained.
                let third = owner
                    .word(0x120)
                    .context("mask capture third-record owner unknown")?;
                if third != 0 {
                    return Ok(());
                }
                let runtime = owner.skeleton().context("mask capture skeleton absent")?;
                let mut locals = Vec::new();
                locals
                    .try_reserve_exact(runtime.skeleton().bones().len())
                    .context("mask local backing allocation")?;
                for bone in runtime.skeleton().bones() {
                    locals.push(LocalPose {
                        translation_bits: bone.local_translation_bits(),
                        quaternion_bits: bone.local_quaternion_bits(),
                    });
                }
                let mut auxiliary = Vec::new();
                auxiliary
                    .try_reserve_exact(runtime.auxiliary().len())
                    .context("mask auxiliary backing allocation")?;
                auxiliary.extend_from_slice(runtime.auxiliary());
                owner.controls.capture(first, locals, auxiliary)?;
                command.mask_capture_returned()?;
            }
            command.finish_mask_effects(
                owner.layers.as_mut().expect("retained mask layers"),
                &mut owner.controls,
                &mut owner.node_ids,
            )
        })();
        owner.mirror_mode_layers();
        if result.is_err() {
            command.fail_factory(owner.layers.as_mut().context("failed mask layers absent")?);
        }
        result
    }
}
