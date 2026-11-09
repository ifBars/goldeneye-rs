//! Initial base-bound diagnostics, separate from full live component assembly.
use anyhow::{Context, Result, ensure};
use ge_assets::{edb::Edb, scene::Placement};
use ge_core::collision::bounds::{Bounds, EntityBounds, prepare_instance_bounds};
use serde::Serialize;

#[derive(Default, Serialize)]
pub struct Audit {
    pub maps: usize,
    pub placements: usize,
    pub unresolved_entity_references: usize,
    pub unsupported_entity_types: usize,
    pub base_bounds_probed: usize,
    pub component_headers: usize,
    pub component_free_headers: usize,
    pub ordinary_components_decoded: usize,
    pub shape_components_decoded: usize,
    pub shape_point_updates: usize,
    pub empty_shape_aggregates: usize,
    pub unsupported_components: usize,
    pub assembled_bounds_probed: usize,
    pub assembled_bounds_pending: usize,
    pub strict_assembled_bounds: usize,
    pub rounding_assembled_bounds: usize,
    pub inverted_base_bounds: usize,
    pub strict_bounds: usize,
    pub rounding_bounds: usize,
    pub maximum_bound_error_f64: f64,
    pub maximum_assembled_error_f64: f64,
}
impl Audit {
    pub fn include(&mut self, edb: &Edb, placements: &[Placement]) -> Result<()> {
        self.maps += 1;
        for p in placements {
            self.placements += 1;
            let Some(reference) = edb.entity_reference(p.object_hash)? else {
                self.unresolved_entity_references += 1;
                continue;
            };
            if !matches!(reference.entity.entity_type, 0x601 | 0x603 | 0x607 | 0x608) {
                self.unsupported_entity_types += 1;
                continue;
            }
            let header = edb.entity_spatial_at(reference.directory_index)?;
            self.component_headers += usize::from(header.components_raw & 0x5f != 0);
            self.component_free_headers += usize::from(header.components_raw & 0x5f == 0);
            self.inverted_base_bounds +=
                usize::from((0..3).any(|i| header.bounds[0][i] > header.bounds[1][i]));
            let bounds = Bounds {
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
            };
            self.probe(bounds, &p.model)?;
            let mut ordinary = std::collections::BTreeMap::new();
            let mut pending = false;
            let shape = if header.components_raw & 0x40 != 0 {
                let shape = edb
                    .entity_shape_bounds_at(reference.directory_index)?
                    .context("required shape aggregate absent")?;
                let updates = shape.first_point_updates + shape.second_point_updates;
                self.shape_components_decoded += 1;
                self.shape_point_updates += updates;
                self.empty_shape_aggregates += usize::from(updates == 0);
                Some(Bounds {
                    minimum: shape.minimum,
                    maximum: shape.maximum,
                })
            } else {
                None
            };
            for slot in 0..5 {
                if header.components_raw & (1 << slot) == 0 {
                    continue;
                }
                let component = edb
                    .entity_component_at(reference.directory_index, slot)?
                    .context("required ordinary component absent")?;
                if !matches!(component.entity_type, 0x601 | 0x603 | 0x607 | 0x608) {
                    self.unsupported_components += 1;
                    pending = true;
                    continue;
                }
                let component = edb
                    .component_spatial_at(reference.directory_index, slot)?
                    .context("ordinary component header absent")?;
                ordinary.insert(
                    slot,
                    Bounds {
                        minimum: [
                            component.bounds[0][0],
                            component.bounds[0][1],
                            component.bounds[0][2],
                            f32::from_bits(component.packed_w_raw[0]),
                        ],
                        maximum: [
                            component.bounds[1][0],
                            component.bounds[1][1],
                            component.bounds[1][2],
                            f32::from_bits(component.packed_w_raw[1]),
                        ],
                    },
                );
                self.ordinary_components_decoded += 1;
            }
            if pending {
                self.assembled_bounds_pending += 1;
                continue;
            }
            let actual = prepare_instance_bounds(
                &p.model,
                Some(EntityBounds {
                    base: bounds,
                    component_mask: header.components_raw as u8,
                }),
                |slot| {
                    ordinary
                        .get(&slot)
                        .copied()
                        .context("component not decoded")
                },
                || Ok(shape),
            )?;
            let sources = std::iter::once(bounds)
                .chain(ordinary.values().copied())
                .chain(shape)
                .collect::<Vec<_>>();
            let error = compare_geometry(&sources, &p.model, actual)?;
            self.assembled_bounds_probed += 1;
            self.maximum_assembled_error_f64 = self.maximum_assembled_error_f64.max(error);
            if error <= 0.00001 {
                self.strict_assembled_bounds += 1;
            } else {
                self.rounding_assembled_bounds += 1;
            }
        }
        Ok(())
    }
    fn probe(&mut self, bounds: Bounds, matrix: &[f32; 16]) -> Result<()> {
        let actual = bounds.transformed(matrix)?;
        let error = compare_geometry(&[bounds], matrix, actual)?;
        self.maximum_bound_error_f64 = self.maximum_bound_error_f64.max(error);
        self.base_bounds_probed += 1;
        if error <= 0.00001 {
            self.strict_bounds += 1;
        } else {
            self.rounding_bounds += 1;
        }
        Ok(())
    }
}
fn compare_geometry(sources: &[Bounds], matrix: &[f32; 16], actual: Bounds) -> Result<f64> {
    // Independent geometry: transform all eight endpoints in f64, then
    // reduce their extrema. Do not reuse endpoint-product selection.
    let mut minimum = [f64::INFINITY; 3];
    let mut maximum = [f64::NEG_INFINITY; 3];
    for bounds in sources {
        for corner in 0..8 {
            let point = std::array::from_fn::<_, 3, _>(|axis| {
                f64::from(if corner & (1 << axis) == 0 {
                    bounds.minimum[axis]
                } else {
                    bounds.maximum[axis]
                })
            });
            for row in 0..3 {
                let value = f64::from(matrix[12 + row])
                    + (0..3)
                        .map(|col| f64::from(matrix[col * 4 + row]) * point[col])
                        .sum::<f64>();
                minimum[row] = minimum[row].min(value);
                maximum[row] = maximum[row].max(value);
            }
        }
    }
    let mut error = 0f64;
    for row in 0..3 {
        let magnitude = sources
            .iter()
            .map(|bounds| {
                f64::from(matrix[12 + row]).abs()
                    + (0..3)
                        .map(|col| {
                            f64::from(matrix[col * 4 + row]).abs()
                                * f64::from(bounds.minimum[col])
                                    .abs()
                                    .max(f64::from(bounds.maximum[col]).abs())
                        })
                        .sum::<f64>()
            })
            .fold(0f64, f64::max);
        let bound = 0.00001 + 16. * f64::from(f32::EPSILON) * magnitude;
        for (actual, reference) in [
            (actual.minimum[row], minimum[row]),
            (actual.maximum[row], maximum[row]),
        ] {
            let difference = (f64::from(actual) - reference).abs();
            ensure!(
                difference <= bound,
                "base bounds error {difference} exceeds {bound}"
            );
            error = error.max(difference);
        }
    }
    Ok(error)
}

#[cfg(test)]
#[path = "tests/bounds_audit.rs"]
mod tests;
