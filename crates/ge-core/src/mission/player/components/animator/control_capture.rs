use super::Animator;
use crate::animation::{
    controls::Store,
    layers,
    scratch::{Arena, Context as Scratch, Refresh},
    skeleton::LocalPose,
};
use crate::mission::player::components::{Preparation, Stage};
use anyhow::{Context, Result, ensure};

impl Preparation {
    pub fn capture_third_person_control(
        &mut self,
        context: &mut Scratch,
        arena: &mut Arena,
    ) -> Result<()> {
        ensure!(
            self.stage() == Stage::AwaitingInitialModes && self.first_person().is_some(),
            "TP control capture precedes actual component returns"
        );
        self.animator_mut_for_capture()?
            .capture_fresh_control(context, arena)
    }
}
impl Animator {
    pub fn controls(&self) -> &Store {
        &self.controls
    }
    pub fn layers(&self) -> Option<&layers::State> {
        self.layers.as_ref()
    }
    pub fn capture_refresh(&self) -> Option<&Refresh> {
        self.capture_refresh.as_ref()
    }
    fn capture_fresh_control(&mut self, context: &mut Scratch, arena: &mut Arena) -> Result<()> {
        ensure!(
            self.layers.is_none()
                && self.controls.is_empty()
                && self.capture_refresh.is_none()
                && self.component()
                    == Some(crate::mission::player::components::skeleton::TP_COMPONENT),
            "TP capture cannot replay or replace existing controls"
        );
        let mut flags = [0; 4];
        let mut weights = [0; 4];
        for (index, start) in [0x138, 0x158, 0x178, 0x198].into_iter().enumerate() {
            ensure!(
                self.word(start) == Some(0) && self.word(start + 4) == Some(0),
                "TP capture requires empty actual lists"
            );
            flags[index] = u16::from_be_bytes([
                self.byte(start + 8).context("slot flags unknown")?,
                self.byte(start + 9).context("slot flags unknown")?,
            ]);
            weights[index] = self.word(start + 12).context("slot weight unknown")?;
            ensure!(
                flags[index] == 1
                    && weights[index] == 1f32.to_bits()
                    && self.byte(start + 10) == Some(3),
                "TP initial capture requires its actual configured fresh headers"
            );
        }
        ensure!(
            self.byte(0x130) == Some(1)
                && self.byte(0x131) == Some(0xff)
                && self.byte(0x136) == Some(0xff)
                && self.item().is_some()
                && self.callback_table().is_some(),
            "TP capture lacks its actual initial item/header bindings"
        );
        self.layers = Some(layers::State::from_empty_snapshot(
            layers::Snapshot {
                active_count: self.byte(0x130).context("TP active count unknown")?,
                selection_mask: self.byte(0x131).context("TP selection mask unknown")?,
                clear_mask: self.byte(0x136).context("TP clear mask unknown")?,
                slot_flags: flags,
                weight_bits: weights,
            },
            layers::Budget {
                nodes: 4096,
                commands: 4096,
                groups: 4096,
                children: 16384,
            },
        )?);
        let first = self.controls.allocate()?;
        // Actual ordinary refresh: no LOD table, children or linked base child.
        // The shared elapsed calculation has no consumer on this empty path.
        ensure!(
            self.word(0x11c) == Some(0)
                && self.word(0x10) == Some(0)
                && self.word(0x120) == Some(0),
            "TP control refresh requires actual fresh LOD/child/actor links"
        );
        self.capture_refresh = Some(self.refresh_for_capture(context, arena)?);
        let runtime = self.skeleton().context("TP capture skeleton is absent")?;
        let locals: Vec<_> = runtime
            .skeleton()
            .bones()
            .iter()
            .map(|bone| LocalPose {
                translation_bits: bone.local_translation_bits(),
                quaternion_bits: bone.local_quaternion_bits(),
            })
            .collect();
        let auxiliary = runtime.auxiliary().to_vec();
        self.controls.capture(first, locals, auxiliary)?;
        let result = self
            .layers
            .as_mut()
            .expect("retained TP layers")
            .capture_fresh_slots(&mut self.controls, first, 0x0f);
        // Reached host lists replace numeric source pointers, including partial
        // membership if a later clone/allocation fails.
        for (index, start) in [0x138, 0x158, 0x178, 0x198].into_iter().enumerate() {
            if !self.layers.as_ref().expect("retained layers").slots()[index]
                .nodes_head()
                .is_empty()
            {
                self.invalidate_control_header(start);
            }
        }
        result
    }
    fn refresh_for_capture(&mut self, context: &mut Scratch, arena: &mut Arena) -> Result<Refresh> {
        self.bytes[0x135] = Some(0);
        let flags = u16::from_be_bytes([
            self.byte(0xc).context("TP update flags unknown")?,
            self.byte(0xd).context("TP update flags unknown")?,
        ]);
        self.half_store(0xc, flags & !8);
        context.refresh_empty(arena)
    }
    fn invalidate_control_header(&mut self, start: usize) {
        self.bytes[start..start + 8].fill(None);
    }
}
