//! Typed source bindings for common shapes and required fresh animators.
use anyhow::{Context, Result, ensure};
use ge_assets::edb::Edb;
use ge_core::mission::{
    animator,
    triggers::{self, AnimatorId, AnimatorRequest},
};

pub(super) struct Factory<'a> {
    pub source: &'a Edb,
    pub reference: u32,
    pub owned: &'a mut super::animators::Owned,
    pub map: triggers::MapContext,
    pub map_index: usize,
    pub shapes: &'a mut Vec<ShapeSource>,
    pub definitions: &'a mut crate::game::actors::Definitions,
    pub resource_states: &'a mut crate::game::resource_states::Attempt,
}

pub(super) struct ShapeSource {
    pub map: triggers::MapContext,
    pub map_index: usize,
    pub ordinal: u16,
    pub binding: ge_assets::mission::common_shapes::Binding,
}

impl triggers::Factory for Factory<'_> {
    fn shape71_geometry(
        &mut self,
        request: triggers::shape71::Request,
    ) -> Result<triggers::actor12::ProviderOutcome<triggers::shape71::GeometryInput>> {
        ensure!(
            request.map == self.map,
            "shape geometry source map binding differs"
        );
        // Derived initialization follows this object's common binding. Its
        // retained tail gives constant-time validation without selecting the
        // zone world's potentially different EDB as the geometry owner.
        let retained = self
            .shapes
            .last()
            .context("shape geometry requires retained common binding")?;
        ensure!(
            retained.map == request.map
                && retained.ordinal == request.ordinal
                && retained.binding.raw_selector == request.selector,
            "shape geometry differs from the current common binding"
        );
        let geometry = match self.source.map_common_shape_geometry(
            self.reference,
            request.selector,
            ge_assets::mission::common_shapes::Budget {
                maps: 128,
                selectors: 65_536,
                addressed_bytes: 48,
            },
        ) {
            Ok(Some(geometry)) => geometry,
            Ok(None) => anyhow::bail!("required shape geometry map is absent"),
            Err(error)
                if error
                    .downcast_ref::<ge_assets::mission::common_shapes::Unsupported>()
                    .is_some() =>
            {
                return Ok(triggers::actor12::ProviderOutcome::Unavailable);
            }
            Err(error) => return Err(error),
        };
        ensure!(
            *geometry.binding() == retained.binding,
            "shape geometry locator differs from retained source"
        );
        Ok(triggers::actor12::ProviderOutcome::Ready(
            triggers::shape71::GeometryInput {
                type_raw: geometry.type_raw(),
                word_04_raw: geometry.word_04_raw(),
                center_bits: geometry.center_bits(),
                orientation_bits: geometry.orientation_bits(),
                dimensions_bits: geometry.dimensions_bits(),
            },
        ))
    }

    fn radius03_resource(
        &mut self,
        request: triggers::radius03::Request,
    ) -> Result<triggers::actor12::ProviderOutcome<u32>> {
        ensure!(
            request.map == self.map,
            "resource-state source map binding differs"
        );
        Ok(triggers::actor12::ProviderOutcome::Ready(
            self.resource_states
                .lookup(request.resource, request.fallback)?,
        ))
    }
    fn actor12_descriptor_fallback(
        &mut self,
        request: triggers::actor12::Request,
    ) -> Result<triggers::actor12::ProviderOutcome<u32>> {
        ensure!(request.map == self.map, "actor source map binding differs");
        Ok(triggers::actor12::ProviderOutcome::Ready(
            self.definitions.descriptor_fallback(),
        ))
    }
    fn actor12_level_selector(
        &mut self,
        request: triggers::actor12::Request,
        token: u32,
    ) -> Result<triggers::actor12::ProviderOutcome<triggers::actor12::LevelSelection>> {
        ensure!(
            request.map == self.map,
            "actor selector map binding differs"
        );
        self.definitions.level_selector(token)
    }
    fn actor12_definition(
        &mut self,
        request: triggers::actor12::Request,
        reference: u32,
    ) -> Result<triggers::actor12::ProviderOutcome<ge_core::mission::actor_definitions::RecordId>>
    {
        ensure!(
            request.map == self.map,
            "actor definition map binding differs"
        );
        self.definitions.definition(reference)
    }

    fn initialize_common_shape(
        &mut self,
        request: triggers::ShapeRequest,
    ) -> Result<triggers::ShapeOutcome> {
        ensure!(request.map == self.map, "shape source map binding differs");
        let binding = match self.source.map_common_shape(
            self.reference,
            request.selector,
            ge_assets::mission::common_shapes::Budget {
                maps: 128,
                selectors: 65_536,
                addressed_bytes: 7,
            },
        ) {
            Ok(Some(binding)) => binding,
            Ok(None) => anyhow::bail!("required common shape map is absent"),
            Err(error)
                if error
                    .downcast_ref::<ge_assets::mission::common_shapes::Unsupported>()
                    .is_some() =>
            {
                return Ok(triggers::ShapeOutcome::Unsupported);
            }
            Err(error) => return Err(error),
        };
        ensure!(
            self.shapes.len() < 65_536,
            "common shape binding budget exhausted"
        );
        self.shapes
            .try_reserve(1)
            .context("owning checked shape sources")?;
        self.shapes.push(ShapeSource {
            map: request.map,
            map_index: self.map_index,
            ordinal: request.ordinal,
            binding,
        });
        Ok(triggers::ShapeOutcome::Bound(
            triggers::ShapeBinding::SelectorRetained {
                selector: binding.raw_selector,
                type_raw: binding.type_raw,
            },
        ))
    }

    fn initialize_animator(&mut self, request: AnimatorRequest) -> Result<AnimatorId> {
        ensure!(
            request.kind == ge_core::mission::triggers::TriggerKind::StartPoint,
            "fresh animator callback for this trigger kind is unsupported"
        );
        ensure!(
            self.owned.values.len() < 65_536,
            "owned animator budget exhausted"
        );
        let source = self
            .source
            .map_animator(
                self.reference,
                usize::try_from(request.index)?,
                ge_assets::map_animator::Budget {
                    maps: 128,
                    animator_indices: 65_536,
                    states: 256,
                    instructions: 65_536,
                    bytes: 8 * 1024 * 1024,
                },
            )?
            .context("required map animator is absent")?;
        let header = source.header();
        // Fresh owner NULL denies create before program reads. Only the header
        // and reached event mappings are required; no dummy VM is installed.
        let runtime = animator::Runtime::fresh_startpoint(
            animator::Header {
                statement_count_raw: header.statement_count_raw,
                variables: header.variables,
                shared_variables: header.shared_variables,
                states: header.states,
                format: header.format,
                event_ff: source.event_mapping(0xff)?,
                event_create: source.event_mapping(0x100)?,
                event_105: source.event_mapping(0x105)?,
                event_10e: source.event_mapping(0x10e)?,
            },
            animator::FreshContext {
                trigger_handle: request.trigger.get(),
                map_selector: request.map.map_selector,
                position_bits: request.position_bits,
                manager_present: true,
                camera: animator::CameraContext::KnownMissing,
            },
            animator::Budget {
                variables: 256,
                states: 256,
            },
        )?;
        let id = AnimatorId::new(u64::try_from(self.owned.values.len())? + 1)?;
        self.owned.values.push(runtime);
        Ok(id)
    }
}
