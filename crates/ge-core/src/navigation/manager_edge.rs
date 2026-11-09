//! Manager-installed surface traversal policy, distinct from default and actor target edges.
use super::TraversalEdge;
use anyhow::{Context, Result, ensure};

pub fn manager_navigation_edge(
    neighbors: &[[Option<usize>; 3]],
    flags: &[Option<u32>],
    edge: TraversalEdge,
) -> Result<bool> {
    let actual = neighbors
        .get(edge.from_triangle)
        .and_then(|row| row.get(edge.slot))
        .context("manager edge source/slot out of bounds")?;
    ensure!(
        *actual == Some(edge.to_triangle),
        "manager edge destination disagrees with neighbor slot"
    );
    if Some(edge.to_triangle) == edge.previous_triangle {
        return Ok(false);
    }
    let destination = flags
        .get(edge.to_triangle)
        .context("manager edge destination flags out of bounds")?
        .unwrap_or(0);
    let source = flags
        .get(edge.from_triangle)
        .context("manager edge source flags out of bounds")?
        .unwrap_or(0);
    Ok(!(destination & 2 != 0 && source & 2 == 0 || source & 0x10 != 0 && destination & 8 != 0))
}
#[cfg(test)]
#[path = "tests/manager_edge.rs"]
mod tests;
