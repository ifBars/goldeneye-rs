//! Controller-lived wrappers. A new attempt retains publication history and
//! section ownership instead of constructing another fresh origin for a file.
use super::{File, Returned};
use anyhow::Result;
use ge_core::mission::post_trigger::Request;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub(crate) struct Registry(Arc<Mutex<BTreeMap<u32, File>>>);

impl Registry {
    pub(crate) fn from_constructor() -> Self {
        Self(Arc::new(Mutex::new(BTreeMap::new())))
    }

    pub(super) fn with_files<T>(
        &self,
        operation: impl FnOnce(&mut BTreeMap<u32, File>) -> Result<T>,
    ) -> Result<T> {
        let mut files = self
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("resource wrapper registry lock poisoned"))?;
        operation(&mut files)
    }

    /// The completed hook describes only files and sections reached by its own
    /// ordered requests. Later player requests cannot rewrite that snapshot.
    pub(super) fn operation_snapshot(
        &self,
        requests: &[Request],
        returned: &[Returned],
    ) -> Result<BTreeMap<u32, File>> {
        self.with_files(|files| {
            let requested: BTreeSet<_> = requests.iter().map(|request| request.file).collect();
            let mut snapshot = BTreeMap::new();
            for key in requested {
                if let Some(file) = files.get(&key) {
                    let sections = file
                        .sections
                        .iter()
                        .filter(|section| {
                            returned.iter().any(|response| {
                                response.request.file == key
                                    && response.resolution.as_ref().is_some_and(|target| {
                                        !target.base && target.section_index == section.index
                                    })
                            })
                        })
                        .cloned()
                        .collect();
                    snapshot.insert(
                        key,
                        File {
                            edb: Arc::clone(&file.edb),
                            sections,
                        },
                    );
                }
            }
            Ok(snapshot)
        })
    }

    #[cfg(test)]
    pub(crate) fn same_owner(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

#[cfg(test)]
mod tests;
