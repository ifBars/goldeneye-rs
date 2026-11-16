//! Test-only sparse header stores used to reach destructor dependencies.
use super::*;

/// Authored word store; None makes the word unknown.
pub(crate) fn store_header_word(store: &mut Store, id: NodeId, at: usize, bits: Option<u32>) {
    let header = store.headers.get_mut(&id).expect("authored header present");
    match bits {
        Some(bits) => header.w(at, bits),
        None => header.bytes[at..at + 4].fill(None),
    }
}
