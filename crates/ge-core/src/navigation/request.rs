//! Actor navigation dispatch phase; scheduling, actor policies and steering are separate.
use anyhow::Result;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct NavigationRequestState {
    pub flags_raw: u32,
    /// Explicit host candidate index, standing in for the original cached wrapper pointer.
    pub binding: Option<usize>,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum NavigationDispatch {
    Inactive,
    Acquire { binding: Option<usize> },
    Query { success: bool },
}
impl NavigationDispatch {
    /// Original return means a pending request was processed, not that a route exists.
    pub fn processed(&self) -> bool {
        !matches!(self, Self::Inactive)
    }
}
impl NavigationRequestState {
    /// Clear 0x800 on EVERY call; require both 0x100 and 0x200 to process.
    /// Acquiring a binding defers querying until a later dispatch. Preserve other bits.
    pub fn dispatch(
        &mut self,
        mut acquire: impl FnMut() -> Result<Option<usize>>,
        mut query: impl FnMut(usize) -> Result<bool>,
    ) -> Result<NavigationDispatch> {
        self.flags_raw &= !0x800;
        if self.flags_raw & 0x300 != 0x300 {
            return Ok(NavigationDispatch::Inactive);
        }
        if let Some(binding) = self.binding {
            self.flags_raw |= 0x400;
            let success = query(binding)?;
            if success {
                self.flags_raw |= 0x800;
            }
            Ok(NavigationDispatch::Query { success })
        } else {
            self.binding = acquire()?;
            if self.binding.is_none() {
                self.flags_raw |= 0x400;
            }
            Ok(NavigationDispatch::Acquire {
                binding: self.binding,
            })
        }
    }
}

#[cfg(test)]
#[path = "../tests/request.rs"]
mod tests;
