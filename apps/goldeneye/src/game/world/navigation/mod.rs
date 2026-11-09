//! Fresh navigation-manager ownership from retained level/map inputs. Completing
//! this component does not execute later world hooks or publish user readiness.
mod evidence;

use anyhow::{Context, Result, ensure};
use ge_assets::graph::TriangleGraph;
use ge_core::navigation::{BoundaryMesh, runtime};
use std::collections::BTreeMap;

use crate::{mission::MissionContext, mission::world::TriggerMap};
pub use evidence::Evidence;

struct Entry {
    owner: runtime::Owner,
    graph: usize,
}

pub(super) struct Owned {
    entries: Vec<Entry>,
    graphs: Vec<TriangleGraph>,
    evidence: Evidence,
}

impl Owned {
    pub fn build(context: &MissionContext, maps: &[TriggerMap<'_>]) -> Result<Self> {
        let first = maps.first().context("navigation initial map is absent")?;
        let level = context
            .retained_resources()
            .first()
            .context("navigation level resource is absent")?;
        ensure!(
            level.header.hash == context.registry().current_hash(),
            "navigation selected level owner differs"
        );
        let level_map = maps
            .iter()
            .find(|map| std::ptr::eq(map.source, level))
            .context("navigation selected level has no retained wrapper")?;
        ensure!(maps.len() <= 128, "navigation map budget exceeded");
        let placement_count = maps.iter().try_fold(0usize, |count, map| {
            count
                .checked_add(map.data.placements.len())
                .context("navigation placement count overflow")
        })?;
        ensure!(
            placement_count <= 65_536,
            "navigation placement budget exceeded"
        );
        let placements = maps
            .iter()
            .map(|map| {
                map.data
                    .placements
                    .iter()
                    .map(|placement| runtime::Placement {
                        key: runtime::SourceInstanceKey::new(placement.source.hash),
                        entity_reference_raw: placement.source.object_hash,
                        position_bits: placement.source.position.map(f32::to_bits),
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let views = maps
            .iter()
            .zip(&placements)
            .map(|(map, placements)| runtime::MapView {
                instance: map.loaded.instance,
                wrapper: map.loaded.key.wrapper,
                placements,
            })
            .collect::<Vec<_>>();
        // The source scans only the first registration map, then uses all loaded
        // maps for each lazy key lookup. Both entity checks use the LEVEL owner.
        let entities = level.entities()?;
        let mut resolve = |wrapper, reference| {
            ensure!(
                wrapper == level_map.loaded.key.wrapper,
                "navigation entity resolver changed its level owner"
            );
            level
                .entity_reference_index(reference)?
                .map(|index| {
                    let entity = entities
                        .get(index)
                        .context("navigation entity index is outside the level directory")?;
                    Ok(runtime::Entity {
                        directory_index: index,
                        entity_type: entity.entity_type,
                    })
                })
                .transpose()
        };
        let budget = runtime::Budget {
            maps: 128,
            placements: 65_536,
            faces: 1_000_000,
            cache_edges: 3_000_000,
            category_members: 1_000_000,
        };
        let mut entries = Vec::new();
        let mut graphs = Vec::new();
        let mut graph_indices = BTreeMap::new();
        let mut total_faces = 0usize;
        let mut unique_vertices = 0usize;
        for index in 0..first.data.placements.len() {
            let Some(binding) = runtime::resolve_binding(
                level_map.loaded.key.wrapper,
                runtime::PlacementLocation {
                    instance: first.loaded.instance,
                    index,
                },
                &views,
                &mut resolve,
                budget,
            )?
            else {
                continue;
            };
            ensure!(entries.len() < 65_536, "navigation owner budget exceeded");
            let graph_index =
                if let Some(&index) = graph_indices.get(&binding.entity_directory_index) {
                    index
                } else {
                    let graph = level.entity_graph_at(binding.entity_directory_index)?;
                    unique_vertices = unique_vertices
                        .checked_add(graph.vertices.len())
                        .context("navigation retained vertex count overflow")?;
                    ensure!(
                        unique_vertices <= 1_000_000,
                        "navigation retained vertex budget exceeded"
                    );
                    let index = graphs.len();
                    graph_indices.insert(binding.entity_directory_index, index);
                    graphs.push(graph);
                    index
                };
            let graph = &graphs[graph_index];
            total_faces = total_faces
                .checked_add(graph.triangles.len())
                .context("navigation total face count overflow")?;
            ensure!(
                total_faces <= budget.faces,
                "navigation total face budget exceeded"
            );
            let faces = graph
                .triangles
                .iter()
                .map(|face| face.vertices.map(|v| v as usize))
                .collect::<Vec<_>>();
            let neighbors = graph
                .triangles
                .iter()
                .map(|face| face.neighbors.map(|v| v.map(|v| v as usize)))
                .collect::<Vec<_>>();
            let flags = graph
                .triangles
                .iter()
                .map(|face| face.flags_raw)
                .collect::<Vec<_>>();
            let owner = runtime::Owner::build(
                binding,
                BoundaryMesh {
                    vertex_count: graph.vertices.len(),
                    faces: &faces,
                    neighbors: &neighbors,
                    flags: &flags,
                },
                budget,
            )?;
            entries.push(Entry {
                owner,
                graph: graph_index,
            });
        }
        let evidence = evidence::build(level, maps, &entries, &graphs)?;
        Ok(Self {
            entries,
            graphs,
            evidence,
        })
    }

    pub fn evidence(&self) -> &Evidence {
        debug_assert_eq!(self.entries.len(), self.evidence.owned_wrappers);
        debug_assert!(
            self.entries
                .iter()
                .all(|entry| entry.graph < self.graphs.len())
        );
        &self.evidence
    }
}
