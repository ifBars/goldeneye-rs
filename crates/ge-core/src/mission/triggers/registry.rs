use anyhow::{Context, Result, ensure};

use super::common_shape::{ShapeOutcome, ShapeRequest};
use super::types::*;
use crate::mission::spawn::{self, CatalogEntry, SourceSpawnPlan};

pub struct Registry {
    budget: Budget,
    maps: Vec<Map>,
    objects: Vec<Trigger>,
    scratch: Vec<TriggerId>,
    membership: Vec<TriggerId>,
    catalog: Vec<TriggerId>,
    catalog_ready: bool,
}

impl Registry {
    pub fn new(budget: Budget) -> Self {
        Self {
            budget,
            maps: Vec::new(),
            objects: Vec::new(),
            scratch: Vec::new(),
            membership: Vec::new(),
            catalog: Vec::new(),
            catalog_ready: false,
        }
    }

    pub fn begin_map(
        &mut self,
        context: MapContext,
        rows: usize,
        filter: Option<&[u32]>,
    ) -> Result<()> {
        ensure!(
            self.maps
                .last()
                .is_none_or(|map| map.phase == MapPhase::Initialized),
            "previous trigger map passes are unfinished"
        );
        ensure!(
            self.maps.len() < self.budget.maps,
            "trigger map budget exhausted"
        );
        ensure!(
            !self
                .maps
                .iter()
                .any(|map| map.context.instance == context.instance
                    || map.context.map_selector == context.map_selector),
            "trigger map identity or selector is repeated"
        );
        // The supported positive selector bound is a host scope, not an
        // assertion that native signed-byte consumers validate every slot.
        ensure!(
            context.map_selector > 0,
            "trigger map requires a supported positive source selector"
        );
        ensure!(rows <= 65_536, "trigger row ordinal exceeds u16");
        let total = self
            .maps
            .iter()
            .try_fold(rows, |sum, map| sum.checked_add(map.slots.len()))
            .context("trigger row count overflow")?;
        ensure!(total <= self.budget.rows, "trigger row budget exhausted");
        let words = self
            .maps
            .iter()
            .try_fold(filter.map_or(0, <[u32]>::len), |sum, map| {
                sum.checked_add(map.filter.as_ref().map_or(0, Vec::len))
            })
            .context("trigger filter count overflow")?;
        ensure!(
            words <= self.budget.filter_words,
            "trigger filter budget exhausted"
        );
        self.maps.push(Map {
            context,
            filter: filter.map(<[u32]>::to_vec),
            slots: vec![Slot::Unvisited; rows],
            next: 0,
            phase: MapPhase::Visiting,
        });
        self.catalog_ready = false;
        Ok(())
    }

    fn row_map(&self, ordinal: usize) -> Result<usize> {
        let index = self
            .maps
            .len()
            .checked_sub(1)
            .context("trigger map not started")?;
        let map = &self.maps[index];
        ensure!(
            map.phase == MapPhase::Visiting && ordinal == map.next && ordinal < map.slots.len(),
            "trigger callback out of storage order"
        );
        Ok(index)
    }

    fn store(&mut self, map: usize, slot: Slot) -> Slot {
        let state = &mut self.maps[map];
        state.slots[state.next] = slot;
        state.next += 1;
        slot
    }

    /// Explicit unclassified input; no unknown type is guessed filtered or
    /// disabled. The original slot still exists for the later reference pass.
    pub fn mark_unsupported(&mut self, ordinal: usize, reason: Unsupported) -> Result<Slot> {
        let map = self.row_map(ordinal)?;
        Ok(self.store(map, Slot::Unsupported(reason)))
    }

    pub fn visit(
        &mut self,
        row: RowInput,
        common: impl FnOnce() -> Result<CommonInput>,
        factory: &mut impl Factory,
    ) -> Result<Slot> {
        let map = self.row_map(row.ordinal)?;
        if row.descriptor.enable_byte_raw == 0 {
            return Ok(self.store(map, Slot::Disabled));
        }
        if self.maps[map]
            .filter
            .as_ref()
            .is_some_and(|filter| !filter.contains(&row.descriptor.subtype))
        {
            return Ok(self.store(map, Slot::Filtered));
        }
        if row.descriptor.is_source_null() {
            return Ok(self.store(map, Slot::SourceNull));
        }
        let Some(kind) = row.descriptor.kind() else {
            return Ok(self.store(
                map,
                Slot::Unsupported(Unsupported::Factory {
                    class: row.descriptor.class,
                    subtype: row.descriptor.subtype,
                }),
            ));
        };
        ensure!(
            self.objects.len() < self.budget.objects,
            "trigger object budget exhausted"
        );
        let mut common = common()?;
        ensure!(
            common.animator_present == (common.property_mask_raw & (1 << 26) != 0),
            "animator presence differs from source property mask"
        );
        ensure!(
            common.shape_index_raw.is_some() == (common.property_mask_raw & (1 << 27) != 0),
            "shape presence differs from source property mask"
        );
        let context = self.maps[map].context;
        let ordinal = u16::try_from(row.ordinal)?;
        let common_shape = if let Some(index) = common.shape_index_raw {
            // Native unchecked signed multiplication has no absent sentinel;
            // negative selectors are an explicit unsupported host scope.
            if (index as i32) < 0 {
                return Ok(self.store(map, Slot::Unsupported(Unsupported::Shape { index })));
            }
            match factory.initialize_common_shape(ShapeRequest {
                map: context,
                ordinal,
                selector: index,
                kind,
                zone: row.zone,
            })? {
                ShapeOutcome::Unsupported => {
                    return Ok(self.store(map, Slot::Unsupported(Unsupported::Shape { index })));
                }
                ShapeOutcome::Bound(binding) => {
                    ensure!(
                        binding.selector() == index,
                        "common shape binding selector differs from request"
                    );
                    if binding.type_raw() != 0 {
                        return Ok(self.store(map, Slot::Unsupported(Unsupported::Shape { index })));
                    }
                    Some(binding)
                }
            }
        } else {
            None
        };
        // Common TriggerInit supplies positive zero W even when a caller's raw
        // fourth lanes are different. It does not modify the copied XYZ words.
        common.position_bits[3] = 0;
        common.rotation_bits[3] = 0;
        let id = TriggerId::new(
            u64::try_from(self.objects.len())?
                .checked_add(1)
                .context("trigger identity overflow")?,
        )?;
        self.objects.push(Trigger {
            id,
            map: context,
            zone: row.zone,
            ordinal,
            descriptor: row.descriptor,
            original_position_bits: common.position_bits,
            original_rotation_bits: common.rotation_bits,
            kind,
            factory_status: kind.factory_status(),
            registration_status: None,
            registered: false,
            runtime_header_flags_raw: common.header_flags_raw,
            runtime_numeric: common.numeric,
            generic: None,
            resource08: None,
            range0c: None,
            radius06: None,
            actor12: None,
            range82: None,
            a3: None,
            range27: None,
            ad: None,
            radius03: None,
            common8a: None,
            radius1b: None,
            radius74: None,
            shape71: None,
            radius88: (kind == TriggerKind::Radius88).then(super::radius88::State::constructor),
            aux_ac: (kind == TriggerKind::AuxAc).then(super::aux_ac::State::constructor),
            common_shape,
            common,
            runtime_flags_raw: 0,
            references: [Reference::Unresolved; 8],
            parent: None,
            children: Vec::new(),
            animator: AnimatorState::NotRequired,
        });
        self.scratch.push(id);
        let derived = self
            .objects
            .last_mut()
            .expect("owned common trigger")
            .initialize_derived(factory);
        let reason = match derived {
            Ok(reason) => reason,
            Err(error) => {
                // Malformed/budget errors remain errors, while the reached
                // common scratch and constructor stage are still owned.
                if let Some(super::actor12::ConstructorStage::Provider(stage)) = self
                    .objects
                    .last()
                    .and_then(|object| object.actor12().map(|state| state.stage()))
                {
                    self.store(
                        map,
                        Slot::Unsupported(Unsupported::Actor12Provider { trigger: id, stage }),
                    );
                } else if let Some(resource) = self
                    .objects
                    .last()
                    .and_then(|object| object.radius03())
                    .and_then(|state| state.requested_resource())
                {
                    self.store(
                        map,
                        Slot::Unsupported(Unsupported::Radius03Provider {
                            trigger: id,
                            resource,
                        }),
                    );
                } else if self
                    .objects
                    .last()
                    .is_some_and(|object| object.shape71().is_some())
                {
                    self.store(
                        map,
                        Slot::Unsupported(Unsupported::Shape71Geometry { trigger: id }),
                    );
                }
                return Err(error);
            }
        };
        if let Some(reason) = reason {
            return Ok(self.store(map, Slot::Unsupported(reason)));
        }
        let object = self.objects.last().expect("owned trigger");
        let status = if object.runtime_flags_raw & 1 != 0 {
            FactoryStatus::Status2
        } else {
            object.factory_status
        };
        match status {
            FactoryStatus::Status1 => {
                self.objects
                    .last_mut()
                    .expect("owned trigger")
                    .runtime_flags_raw &= !1
            }
            FactoryStatus::Status2 => {
                self.objects
                    .last_mut()
                    .expect("owned trigger")
                    .runtime_flags_raw |= 1
            }
        }
        self.membership.push(id);
        let object = self.objects.last_mut().expect("owned trigger");
        object.registered = true;
        object.registration_status = Some(status);
        let slot = self.store(map, Slot::Owned(id));
        if status == FactoryStatus::Status1 {
            // Actual fresh shutdown after insertion observes common animator
            // NULL. No status2 construction occurs for its retained selector.
            self.objects.last_mut().expect("owned trigger").animator = AnimatorState::StoppedAbsent;
        } else {
            let object = self.objects.last_mut().expect("owned trigger");
            if object.common.animator_present && object.common.animator_index_raw >= 0 {
                object.animator = AnimatorState::Pending;
                let request = AnimatorRequest {
                    trigger: id,
                    kind,
                    map: context,
                    zone: row.zone,
                    ordinal,
                    index: object.common.animator_index_raw,
                    position_bits: object.common.position_bits,
                };
                let state = match factory.initialize_animator(request) {
                    Ok(animator) => AnimatorState::Initialized(animator),
                    Err(_) => AnimatorState::Failed,
                };
                self.objects.last_mut().expect("owned trigger").animator = state;
            }
        }
        Ok(slot)
    }

    pub fn fix_references(&mut self) -> Result<()> {
        let map = self.maps.last().context("trigger map not started")?;
        ensure!(
            map.phase == MapPhase::Visiting && map.next == map.slots.len(),
            "trigger rows are unfinished or references already fixed"
        );
        let indices: Vec<_> = self
            .objects
            .iter()
            .enumerate()
            .filter_map(|(index, object)| {
                (object.map == map.context && object.registered).then_some(index)
            })
            .collect();
        ensure!(
            indices
                .len()
                .checked_mul(8)
                .context("trigger fixup count overflow")?
                <= self.budget.fixups,
            "trigger reference work budget exhausted"
        );
        // Host validation precedes writes. Native out-of-range ordinal access
        // is unsupported, not a fabricated NULL or a compact-index lookup.
        for &index in &indices {
            for ordinal in self.objects[index].common.reference_ordinals {
                ensure!(
                    ordinal == u32::MAX || (ordinal as u64) < map.slots.len() as u64,
                    "trigger reference ordinal is out of range"
                );
            }
        }
        for index in indices {
            for (field, ordinal) in self.objects[index]
                .common
                .reference_ordinals
                .into_iter()
                .enumerate()
            {
                self.objects[index].references[field] = if ordinal == u32::MAX {
                    Reference::Null
                } else {
                    match map.slots[ordinal as usize] {
                        Slot::Owned(id) => Reference::Trigger(id),
                        Slot::Disabled | Slot::Filtered | Slot::SourceNull => Reference::Null,
                        Slot::Unsupported(_) => Reference::Blocked { ordinal },
                        Slot::Unvisited => unreachable!("all serialized rows visited"),
                    }
                };
            }
        }
        self.maps.last_mut().expect("checked map").phase = MapPhase::ReferencesFixed;
        Ok(())
    }

    pub fn link_parents(&mut self) -> Result<()> {
        let map = self.maps.last().context("trigger map not started")?;
        ensure!(
            map.phase == MapPhase::ReferencesFixed,
            "trigger parent pass out of sequence"
        );
        let context = map.context;
        for index in 0..self.objects.len() {
            let object = &self.objects[index];
            if object.map != context
                || !object.registered
                || object.runtime_header_flags_raw & 0x100 == 0
            {
                continue;
            }
            if let Reference::Trigger(parent) = object.references[7] {
                let child = object.id;
                self.objects[usize::try_from(parent.get() - 1)?]
                    .children
                    .push(child);
                self.objects[index].parent = Some(parent);
            }
        }
        self.maps.last_mut().expect("checked map").phase = MapPhase::ParentsLinked;
        Ok(())
    }

    /// Actor12 performs four source-ordered first-subtype scans; other currently
    /// supported kinds' callbacks are no-ops. Blocked references remain blockers.
    pub fn initialize_map(&mut self) -> Result<()> {
        let map = self.maps.last().context("trigger map not started")?;
        ensure!(
            map.phase == MapPhase::ParentsLinked,
            "trigger virtual init out of sequence"
        );
        let context = map.context;
        for index in 0..self.objects.len() {
            if self.objects[index].map != context
                || !self.objects[index].registered
                || self.objects[index].actor12.is_none()
            {
                continue;
            }
            let mut references = [super::actor12::ScanReference::Null; 7];
            for (field, reference) in self.objects[index].references[..7]
                .iter()
                .copied()
                .enumerate()
            {
                references[field] = super::actor12::scan_reference(reference, |id| {
                    Ok(self.trigger(id)?.descriptor.subtype)
                })?;
            }
            self.objects[index]
                .actor12
                .as_mut()
                .expect("checked actor state")
                .initialize_map(&references);
        }
        self.maps.last_mut().expect("checked map").phase = MapPhase::Initialized;
        Ok(())
    }

    /// Completeness of the submitted maps' supported constructor/fixup passes,
    /// not physics, later trigger callbacks, world/player or camera readiness.
    pub fn is_complete(&self) -> bool {
        !self.maps.is_empty()
            && self.maps.iter().all(|map| {
                map.phase == MapPhase::Initialized
                    && map
                        .slots
                        .iter()
                        .all(|slot| !matches!(slot, Slot::Unvisited | Slot::Unsupported(_)))
            })
            && self.objects.iter().all(|object| {
                object.shape71.as_ref().is_none_or(|state| {
                    matches!(
                        state.geometry_stage(),
                        super::shape71::Stage::Transformed
                            | super::shape71::Stage::FallbackComplete
                    )
                }) && object.actor12.as_ref().is_none_or(|state| {
                    state.map_initialization() == super::actor12::MapInitialization::Complete
                }) && !matches!(
                    object.animator,
                    AnimatorState::Failed | AnimatorState::Pending
                ) && object.references.iter().all(|reference| {
                    !matches!(reference, Reference::Blocked { .. } | Reference::Unresolved)
                })
            })
    }

    pub fn rebuild_catalog(&mut self) -> Result<()> {
        ensure!(
            self.is_complete(),
            "trigger world has unsupported or unfinished dependencies"
        );
        let candidates: Vec<_> = self
            .membership
            .iter()
            .copied()
            .filter(|id| {
                self.objects[(id.get() - 1) as usize]
                    .descriptor
                    .is_start_point()
            })
            .collect();
        ensure!(
            candidates.len() <= self.budget.catalog_entries,
            "StartPoint catalog budget exhausted"
        );
        self.catalog.clear();
        self.catalog.extend(candidates);
        self.catalog_ready = true;
        Ok(())
    }

    pub fn select_start_point(
        &self,
        requested_id: u32,
        budget: spawn::Budget,
    ) -> Result<SelectedStartPoint> {
        ensure!(
            self.catalog_ready && self.is_complete(),
            "live StartPoint catalog is not initialized"
        );
        ensure!(
            self.catalog.len() <= budget.entries,
            "spawn catalog entry budget exhausted"
        );
        let entries = self
            .catalog
            .iter()
            .map(|&id| {
                let object = self.trigger(id)?;
                Ok(CatalogEntry {
                    wrapper_flag_raw: 1,
                    id: object.runtime_numeric[0],
                    trigger_flags_raw: object.runtime_header_flags_raw,
                    position_bits: object.common.position_bits,
                    rotation_bits: object.common.rotation_bits,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let plan = SourceSpawnPlan::prepare(&entries, requested_id, budget)?;
        Ok(SelectedStartPoint {
            selection: plan.selection(),
            trigger: plan
                .selection()
                .selected_index()
                .map(|index| self.catalog[index]),
            pose: plan.pose(),
        })
    }

    pub fn trigger(&self, id: TriggerId) -> Result<&Trigger> {
        self.objects
            .get(usize::try_from(id.get() - 1)?)
            .context("trigger identity is not owned by this registry")
    }
    pub fn slots(&self, map_index: usize) -> Result<&[Slot]> {
        Ok(&self
            .maps
            .get(map_index)
            .context("trigger map index absent")?
            .slots)
    }
    pub fn phase(&self, map_index: usize) -> Result<MapPhase> {
        Ok(self
            .maps
            .get(map_index)
            .context("trigger map index absent")?
            .phase)
    }
    pub fn objects(&self) -> &[Trigger] {
        &self.objects
    }
    pub fn scratch(&self) -> &[TriggerId] {
        &self.scratch
    }
    pub fn membership(&self) -> &[TriggerId] {
        &self.membership
    }
    pub fn catalog(&self) -> &[TriggerId] {
        &self.catalog
    }
    /// Constructor/fixup completion does not itself rebuild the live catalog.
    pub fn catalog_ready(&self) -> bool {
        self.catalog_ready && self.is_complete()
    }
}
