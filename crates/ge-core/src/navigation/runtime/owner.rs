use anyhow::{Context, Result, ensure};

use crate::navigation::{BoundaryCache, BoundaryMesh, build_boundary_cache};

use super::{Binding, Budget};

/// One fresh bound wrapper with its real initial categories and integer cache.
/// The host retains immutable source topology separately. No vertices are moved
/// into world space, and this object performs no navigation or physics query.
pub struct Owner {
    binding: Binding,
    categories: [Vec<u32>; 15],
    cache: BoundaryCache,
    face_count: usize,
}

impl Owner {
    /// Input topology uses the existing BoundaryMesh decoded low20 indices and
    /// optional raw flags. It is not the serialized contiguous partition-group
    /// table. Failures return no initialized owner; native allocation/publication
    /// side effects are not claimed by this checked host construction.
    pub fn build(binding: Binding, mesh: BoundaryMesh<'_>, budget: Budget) -> Result<Self> {
        let face_count = mesh.faces.len();
        ensure!(
            face_count <= budget.faces,
            "navigation owner face budget exhausted"
        );
        ensure!(
            mesh.neighbors.len() == face_count && mesh.flags.len() == face_count,
            "navigation owner face tables differ"
        );
        let reserved_edges = face_count
            .checked_mul(3)
            .context("navigation owner edge capacity overflow")?;
        ensure!(
            reserved_edges <= budget.cache_edges,
            "navigation owner cache allocation budget exhausted"
        );

        let mut lengths = [0usize; 15];
        let mut members = 0usize;
        for flags in mesh.flags {
            if let Some(category) = category(*flags) {
                members = members
                    .checked_add(1)
                    .context("navigation owner category count overflow")?;
                ensure!(
                    members <= budget.category_members,
                    "navigation owner category budget exhausted"
                );
                lengths[category] += 1;
            }
        }
        let mut categories: [Vec<u32>; 15] = std::array::from_fn(|_| Vec::new());
        for (items, length) in categories.iter_mut().zip(lengths) {
            items.try_reserve_exact(length)?;
        }
        for (index, flags) in mesh.flags.iter().enumerate() {
            if let Some(category) = category(*flags) {
                categories[category].push(u32::try_from(index)?);
            }
        }
        let cache = build_boundary_cache(mesh)?;
        Ok(Self {
            binding,
            categories,
            cache,
            face_count,
        })
    }

    pub fn binding(&self) -> &Binding {
        &self.binding
    }

    pub fn categories(&self) -> &[Vec<u32>; 15] {
        &self.categories
    }

    pub fn cache(&self) -> &BoundaryCache {
        &self.cache
    }

    pub fn face_count(&self) -> usize {
        self.face_count
    }
}

fn category(flags: Option<u32>) -> Option<usize> {
    let category = (flags.unwrap_or(0) & 0xf00) >> 8;
    (category != 0).then(|| category as usize - 1)
}
