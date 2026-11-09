//! Initial placement bounds with native coded references and lazy components.
use anyhow::{Context, Result};
use ge_assets::{edb::Edb, entity_spatial::SpatialHeader, scene::Placement};
use ge_core::collision::bounds::{Bounds, EntityBounds, prepare_instance_bounds};

fn spatial_bounds(header: &SpatialHeader) -> Bounds {
    Bounds {
        minimum: [
            header.bounds[0][0],
            header.bounds[0][1],
            header.bounds[0][2],
            f32::from_bits(header.packed_w_raw[0]),
        ],
        maximum: [
            header.bounds[1][0],
            header.bounds[1][1],
            header.bounds[1][2],
            f32::from_bits(header.packed_w_raw[1]),
        ],
    }
}

/// Prepare the serialized placement's initial bound. Failed entity resolution
/// produces the native finite inverted sentinel without inspecting its matrix.
/// Addressed unknown/corrupt components are errors; an empty shape aggregate is
/// still transformed and unioned in its native slot-six position.
pub fn placement(edb: &Edb, p: &Placement) -> Result<Bounds> {
    let Some(reference) = edb.entity_reference(p.object_hash)? else {
        return Ok(Bounds::MISSING_ENTITY);
    };
    let index = reference.directory_index;
    let header = edb
        .entity_spatial_at(index)
        .context("initial placement spatial header")?;
    prepare_instance_bounds(
        &p.model,
        Some(EntityBounds {
            base: spatial_bounds(&header),
            component_mask: header.components_raw as u8,
        }),
        |slot| {
            let component = edb
                .component_spatial_at(index, slot)?
                .with_context(|| format!("required ordinary component slot {slot} absent"))?;
            Ok(spatial_bounds(&component))
        },
        || {
            let shape = edb
                .entity_shape_bounds_at(index)?
                .context("required slot-six aggregate absent")?;
            Ok(Some(Bounds {
                minimum: shape.minimum,
                maximum: shape.maximum,
            }))
        },
    )
}

#[cfg(test)]
#[path = "tests/initial_bounds.rs"]
mod tests;
