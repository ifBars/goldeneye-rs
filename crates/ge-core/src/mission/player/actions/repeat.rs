//! The repeated secondary action retains three independent setter calls.
use super::mode_call::{Call, Inputs};
use super::secondary_parameters::{self, Globals, LowerKind};
use super::*;
use crate::animation::layers::NodeId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    FirstThirdPerson,
    ThirdPersonCoordinate,
    ThirdPersonSeek,
    EnablingOverride,
    SecondThirdPerson,
    DisablingOverride,
    FirstPerson,
    FirstPersonCoordinate,
    Returned,
}
pub struct Dispatch {
    player: PlayerId,
    generic: u32,
    parameter_bits: u32,
    resolved: [Resolved; 3],
    stage: Stage,
    calls: Vec<Call>,
    captured: super::super::components::AnimationPosition,
    tp_coordinate_written: Option<bool>,
    seek_bits: Option<u32>,
    seek_node: Option<NodeId>,
    seek_returned: bool,
    override_writes: usize,
    fp_coordinate_bits: Option<u32>,
    fp_coordinate_written: Option<bool>,
    entry_fallback: (u32, u64),
}
impl Dispatch {
    pub fn begin(
        pending: &mut Pending,
        preparation: &Preparation,
        fallback: (u32, u64),
    ) -> Result<Self> {
        ensure!(
            fallback.0 == 0x0900_000b,
            "repeat TP entry fallback differs"
        );
        pending.begin_repeat_dispatch()?;
        let resolved: [Resolved; 3] = pending
            .resolved()
            .try_into()
            .context("repeat mappings incomplete")?;
        let captured = preparation.mode_animation_position(resolved[1].request.component, 3)?;
        Ok(Self {
            player: pending.player(),
            generic: pending.generic(),
            parameter_bits: pending.parameter_bits(),
            resolved,
            stage: Stage::FirstThirdPerson,
            calls: Vec::new(),
            captured,
            tp_coordinate_written: None,
            seek_bits: None,
            seek_node: None,
            seek_returned: false,
            override_writes: 0,
            fp_coordinate_bits: None,
            fp_coordinate_written: None,
            entry_fallback: fallback,
        })
    }
    pub fn stage(&self) -> Stage {
        self.stage
    }
    pub fn player(&self) -> PlayerId {
        self.player
    }
    pub fn calls(&self) -> &[Call] {
        &self.calls
    }
    pub fn captured(&self) -> super::super::components::AnimationPosition {
        self.captured
    }
    pub fn tp_coordinate_written(&self) -> Option<bool> {
        self.tp_coordinate_written
    }
    pub fn seek_bits(&self) -> Option<u32> {
        self.seek_bits
    }
    pub fn seek_node(&self) -> Option<NodeId> {
        self.seek_node
    }
    pub fn seek_returned(&self) -> bool {
        self.seek_returned
    }
    pub fn override_writes(&self) -> usize {
        self.override_writes
    }
    pub fn fp_coordinate_bits(&self) -> Option<u32> {
        self.fp_coordinate_bits
    }
    pub fn fp_coordinate_written(&self) -> Option<bool> {
        self.fp_coordinate_written
    }
    pub fn begin_call(
        &mut self,
        tracks: bool,
        base: &mut base_handler::Owner,
        reset: &PendingReset,
        preparation: &Preparation,
        fallback: (u32, u64),
    ) -> Result<()> {
        let index = match self.stage {
            Stage::FirstThirdPerson => 1,
            Stage::SecondThirdPerson => 2,
            Stage::FirstPerson => 0,
            _ => anyhow::bail!("repeat setter call out of order"),
        };
        let ordinal = match self.stage {
            Stage::FirstThirdPerson => 0,
            Stage::SecondThirdPerson => 1,
            _ => 2,
        };
        ensure!(self.calls.len() == ordinal, "repeat setter call replays");
        let resolved = self.resolved[index];
        self.calls.try_reserve(1)?;
        self.calls.push(Call::begin(
            Inputs {
                target: resolved.request.target,
                receiver: resolved.request.component,
                generic: self.generic,
                requested: resolved.output,
                parameter_bits: self.parameter_bits,
                tracks,
                fallback,
            },
            base,
            reset,
            preparation,
        )?);
        Ok(())
    }
    pub fn current_call_mut(&mut self) -> Result<&mut Call> {
        ensure!(
            matches!(
                self.stage,
                Stage::FirstThirdPerson | Stage::SecondThirdPerson | Stage::FirstPerson
            ),
            "repeat call is not current"
        );
        self.calls
            .last_mut()
            .context("repeat current setter absent")
    }
    pub fn call_returned(&mut self) -> Result<()> {
        ensure!(
            self.calls
                .last()
                .is_some_and(|call| call.stage() == mode_call::Stage::Returned),
            "repeat setter has not returned"
        );
        self.stage = match self.stage {
            Stage::FirstThirdPerson => Stage::ThirdPersonCoordinate,
            Stage::SecondThirdPerson => Stage::DisablingOverride,
            Stage::FirstPerson => Stage::FirstPersonCoordinate,
            _ => anyhow::bail!("repeat setter return replays"),
        };
        Ok(())
    }
    pub fn begin_seek(
        &mut self,
        reset: &PendingReset,
        preparation: &mut Preparation,
    ) -> Result<()> {
        ensure!(
            self.stage == Stage::ThirdPersonCoordinate && reset.player() == self.player,
            "repeat coordinate/seek out of order"
        );
        let receiver = self.resolved[1].request.component;
        self.tp_coordinate_written = Some(preparation.write_mode_dispatch_scalar(
            receiver,
            0,
            reset.word(0x9a0).context("repeat TP coordinate unknown")?,
        )?);
        let bits = (0.25f32 * f32::from_bits(self.captured.bits)).to_bits();
        self.seek_bits = Some(bits);
        self.seek_node = preparation.begin_mode_seek(receiver, 0, bits)?;
        self.stage = Stage::ThirdPersonSeek;
        Ok(())
    }
    pub fn finish_seek(&mut self, preparation: &Preparation) -> Result<()> {
        ensure!(
            self.stage == Stage::ThirdPersonSeek,
            "repeat seek return out of order"
        );
        if let Some(node) = self.seek_node {
            preparation.ensure_third_person_dispatch_update_returned(self.player, node)?;
        }
        self.seek_returned = true;
        self.stage = Stage::EnablingOverride;
        Ok(())
    }
    pub fn override_stored(&mut self, enabled: bool) -> Result<()> {
        ensure!(
            (enabled && self.stage == Stage::EnablingOverride)
                || (!enabled && self.stage == Stage::DisablingOverride),
            "repeat slot override out of order"
        );
        self.override_writes += 1;
        self.stage = if enabled {
            Stage::SecondThirdPerson
        } else {
            Stage::FirstPerson
        };
        Ok(())
    }
    pub fn finish_fp_coordinate(
        &mut self,
        reset: &PendingReset,
        preparation: &mut Preparation,
        globals: &Globals,
    ) -> Result<()> {
        ensure!(
            self.stage == Stage::FirstPersonCoordinate && reset.player() == self.player,
            "repeat FP coordinate out of order"
        );
        let bits = secondary_parameters::coordinate_bits(
            reset.word(0xec4).context("repeat FP scalar unknown")?,
            reset.word(0x9a0).context("repeat FP coordinate unknown")?,
            *globals,
            LowerKind::Dispatch,
        );
        self.fp_coordinate_bits = Some(bits);
        self.fp_coordinate_written = Some(preparation.write_mode_dispatch_scalar(
            self.resolved[0].request.component,
            0,
            bits,
        )?);
        self.stage = Stage::Returned;
        Ok(())
    }
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        emit(self.stage as u32);
        emit((self.player.get() >> 32) as u32);
        emit(self.player.get() as u32);
        emit(self.generic);
        emit(self.parameter_bits);
        emit(self.entry_fallback.0);
        emit((self.entry_fallback.1 >> 32) as u32);
        emit(self.entry_fallback.1 as u32);
        emit(self.captured.bits);
        emit(u32::from(self.captured.matched));
        emit(u32::from(self.captured.head.is_some()));
        if let Some(id) = self.captured.head {
            emit((id.get() >> 32) as u32);
            emit(id.get() as u32);
        }
        emit(self.calls.len() as u32);
        for call in &self.calls {
            call.visit_state_words(&mut emit);
        }
        for value in [
            self.tp_coordinate_written.map(u32::from),
            self.seek_bits,
            self.fp_coordinate_bits,
            self.fp_coordinate_written.map(u32::from),
        ] {
            emit(u32::from(value.is_some()));
            if let Some(value) = value {
                emit(value);
            }
        }
        emit(u32::from(self.seek_node.is_some()));
        if let Some(id) = self.seek_node {
            emit((id.get() >> 32) as u32);
            emit(id.get() as u32);
        }
        emit(u32::from(self.seek_returned));
        emit(self.override_writes as u32);
    }
}
