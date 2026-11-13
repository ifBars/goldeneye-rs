use super::*;
use crate::animation::skeleton::{self, Skeleton};
use anyhow::Context;

impl Owner {
    /// Actual caller sequence: both receiver gates, selected reuse gate, FP
    /// build, TP build, then select FP. No preload or validity clear is implied.
    /// Unknown skeleton returns an error with resumable reached trace; genuine
    /// allocation/topology errors latch failure and cannot replay this call.
    pub fn initialize_if_unselected(
        &mut self,
        fp: Option<Input<'_>>,
        tp: Option<Input<'_>>,
        budget: Budget,
    ) -> Result<&Trace> {
        if let (Some(fp), Some(tp)) = (fp, tp) {
            ensure!(
                fp.receiver.player() == tp.receiver.player() && fp.receiver != tp.receiver,
                "transform-cache receivers must be distinct components of the same actual player"
            );
        }
        let receivers = [
            fp.map(|input| input.receiver),
            tp.map(|input| input.receiver),
        ];
        if let Some(previous) = &self.trace {
            ensure!(
                !previous.failed,
                "transform-cache invocation previously failed; replay denied"
            );
            if !previous.returned {
                ensure!(
                    previous.receivers == receivers,
                    "pending transform-cache receiver changed"
                );
            }
        }
        let mut trace = if self.trace.as_ref().is_some_and(|trace| !trace.returned) {
            self.trace.take().expect("checked pending trace")
        } else {
            let ordinal = self
                .invocation_count
                .checked_add(1)
                .context("transform-cache invocation count overflow")?;
            self.invocation_count = ordinal;
            Trace {
                ordinal,
                stage: Stage::Entry,
                receivers,
                selected_before: self.selected,
                builds: [None, None],
                missing_receiver_skip: false,
                reused_selected: false,
                selection_returned: false,
                selected_after: None,
                failed: false,
                returned: false,
            }
        };
        let result = self.initialize_inner(fp, tp, budget, &mut trace);
        if result.is_err() {
            let waiting = trace
                .builds
                .iter()
                .flatten()
                .any(|build| build.waiting_skeleton);
            if !waiting {
                trace.failed = true;
                if let Some(build) = match trace.stage {
                    Stage::FirstPerson => trace.builds[0].as_mut(),
                    Stage::ThirdPerson => trace.builds[1].as_mut(),
                    _ => None,
                } {
                    build.failed = true;
                }
            }
        }
        self.trace = Some(trace);
        result?;
        Ok(self.trace.as_ref().expect("retained transform-cache trace"))
    }

    fn initialize_inner(
        &mut self,
        fp: Option<Input<'_>>,
        tp: Option<Input<'_>>,
        budget: Budget,
        trace: &mut Trace,
    ) -> Result<()> {
        if trace.stage == Stage::Entry {
            if fp.is_none() || tp.is_none() {
                trace.missing_receiver_skip = true;
                trace.selected_after = self.selected;
                trace.stage = Stage::Returned;
                trace.returned = true;
                return Ok(());
            }
            if self.selected.is_some() {
                trace.reused_selected = true;
                trace.selected_after = self.selected;
                trace.stage = Stage::Returned;
                trace.returned = true;
                return Ok(());
            }
            trace.stage = Stage::FirstPerson;
        }
        for (slot, input, stage, next) in [
            (
                Slot::FirstPerson,
                fp,
                Stage::FirstPerson,
                Stage::ThirdPerson,
            ),
            (Slot::ThirdPerson, tp, Stage::ThirdPerson, Stage::Selecting),
        ] {
            if trace.stage != stage {
                continue;
            }
            let input = input.context("reached transform-cache receiver absent")?;
            let build = trace.builds[slot.index()]
                .get_or_insert_with(|| BuildTrace::new(slot, input.receiver));
            self.build_slot(slot, input, budget, build)?;
            trace.stage = next;
        }
        if trace.stage == Stage::Selecting {
            trace.selected_after = self.select(0);
            trace.selection_returned = true;
            trace.stage = Stage::Returned;
            trace.returned = true;
        }
        Ok(())
    }

    fn allocate(&mut self, kind: BackingKind, bytes: usize, budget: Budget) -> Result<Backing> {
        ensure!(
            self.allocations() < budget.allocations,
            "transform-cache allocation count budget exhausted"
        );
        let total = self
            .native_bytes()
            .checked_add(bytes)
            .context("transform-cache byte count overflow")?;
        ensure!(
            total <= budget.native_bytes,
            "transform-cache native byte budget exhausted"
        );
        let next = self
            .next_backing
            .checked_add(1)
            .context("transform-cache backing identity overflow")?;
        let backing = Backing::allocate(BackingId(next), kind, bytes)?;
        self.next_backing = next;
        Ok(backing)
    }

    fn build_slot(
        &mut self,
        slot: Slot,
        input: Input<'_>,
        budget: Budget,
        trace: &mut BuildTrace,
    ) -> Result<()> {
        let index = slot.index();
        if trace.stage == BuildStage::CleanupGate {
            trace.releases.extend(self.headers[index].release(true));
            trace.stage = BuildStage::Skeleton;
        }
        // None is a reached unknown provider, not a known native NULL. This is
        // before any new allocation. A resume never repeats old release work.
        trace.waiting_skeleton = input.skeleton.is_none();
        let skeleton = input
            .skeleton
            .context("transform-cache actual skeleton provider unresolved")?;
        trace.waiting_skeleton = false;
        let count = skeleton.bones().len();
        if trace.stage == BuildStage::Skeleton {
            ensure!(
                count <= skeleton::SUPPORTED_BONES,
                "transform-cache exceeds supported skeleton bone bound"
            );
            trace.bone_count = Some(u32::try_from(count)?);
            if count == 0 {
                self.headers[index].count = 0;
                trace.stage = BuildStage::Returned;
                trace.returned = true;
                return Ok(());
            }
            trace.stage = BuildStage::MatrixAllocation;
        }
        if trace.stage == BuildStage::MatrixAllocation {
            let bytes = count
                .checked_mul(64)
                .and_then(|bytes| bytes.checked_add(16))
                .context("transform matrix allocation overflow")?;
            let mut backing = self.allocate(BackingKind::Matrix, bytes, budget)?;
            // NULL native constructor writes only allocation prefix stride/count.
            backing.bytes(0, &64u32.to_be_bytes())?;
            backing.bytes(4, &u32::try_from(count)?.to_be_bytes())?;
            trace.allocations.push(backing.id());
            self.headers[index].matrix = Some(backing);
            trace.stage = BuildStage::ValidityAllocation;
        }
        if trace.stage == BuildStage::ValidityAllocation {
            let backing = self.allocate(BackingKind::Validity, count, budget)?;
            trace.allocations.push(backing.id());
            self.headers[index].validity = Some(backing);
            self.headers[index].count = u32::try_from(count)?;
            trace.stage = BuildStage::ValidityClear;
        }
        if trace.stage == BuildStage::ValidityClear {
            self.headers[index]
                .validity
                .as_mut()
                .context("transform validity backing absent")?
                .fill(0, count, 0)?;
            trace.stage = BuildStage::OffsetsAllocation;
        }
        if trace.stage == BuildStage::OffsetsAllocation {
            let backing = self.allocate(
                BackingKind::Offsets,
                count
                    .checked_mul(2)
                    .context("transform offsets allocation overflow")?,
                budget,
            )?;
            trace.allocations.push(backing.id());
            self.headers[index].offsets = Some(backing);
            trace.stage = BuildStage::TemporaryAllocation;
        }
        if trace.stage == BuildStage::TemporaryAllocation {
            let backing = self.allocate(
                BackingKind::Temporary,
                count
                    .checked_mul(count)
                    .context("transform temporary allocation overflow")?,
                budget,
            )?;
            trace.allocations.push(backing.id());
            self.headers[index].temporary = Some(backing);
            trace.stage = BuildStage::Descendants;
        }
        if trace.stage == BuildStage::Descendants {
            let header = &mut self.headers[index];
            for bone in skeleton.bones() {
                let offset = u16::try_from(trace.descendant_bytes)
                    .context("transform descendant offset exceeds U16")?;
                header
                    .offsets
                    .as_mut()
                    .context("transform offsets absent")?
                    .bytes(trace.offsets_written * 2, &offset.to_be_bytes())?;
                trace.offsets_written += 1;
                for &child in bone.children() {
                    walk_descendant(header, skeleton, child, trace, budget.descendant_visits)?;
                }
                append(header, trace, 0xff)?;
                trace.roots_completed += 1;
            }
            trace.stage = BuildStage::CompactAllocation;
        }
        if trace.stage == BuildStage::CompactAllocation {
            let backing =
                self.allocate(BackingKind::Descendants, trace.descendant_bytes, budget)?;
            trace.allocations.push(backing.id());
            self.headers[index].descendants = Some(backing);
            trace.stage = BuildStage::CompactCopy;
        }
        if trace.stage == BuildStage::CompactCopy {
            let header = &mut self.headers[index];
            let source = header
                .temporary
                .as_ref()
                .context("transform temporary absent")?;
            let target = header
                .descendants
                .as_mut()
                .context("transform descendants absent")?;
            for offset in 0..trace.descendant_bytes {
                target.put(
                    offset,
                    source
                        .byte(offset)?
                        .context("transform temporary copy source unwritten")?,
                )?;
            }
            trace.stage = BuildStage::TemporaryRelease;
        }
        if trace.stage == BuildStage::TemporaryRelease {
            let temporary = self.headers[index]
                .temporary
                .take()
                .context("transform temporary release absent")?;
            trace.releases.push(temporary.id());
            trace.stage = BuildStage::Returned;
            trace.returned = true;
        }
        Ok(())
    }
}

fn append(header: &mut Header, trace: &mut BuildTrace, value: u8) -> Result<()> {
    header
        .temporary
        .as_mut()
        .context("transform temporary absent")?
        .put(trace.descendant_bytes, value)?;
    trace.descendant_bytes += 1;
    Ok(())
}

fn walk_descendant(
    header: &mut Header,
    skeleton: &Skeleton,
    node: u16,
    trace: &mut BuildTrace,
    limit: usize,
) -> Result<()> {
    ensure!(
        trace.descendant_visits < limit,
        "transform descendant visit budget exhausted"
    );
    trace.descendant_visits += 1;
    let bone = skeleton
        .bones()
        .get(usize::from(node))
        .context("transform descendant outside retained skeleton")?;
    if u32::from(bone.index()) < header.count {
        let index = u8::try_from(bone.index()).context("transform descendant index exceeds U8")?;
        ensure!(
            index != 0xff,
            "transform descendant index collides with sentinel"
        );
        append(header, trace, index)?;
    }
    for &child in bone.children() {
        walk_descendant(header, skeleton, child, trace, limit)?;
    }
    Ok(())
}
