//! Initial post-common mutation only; later relay/event callbacks are unsupported.
pub fn initialize_runtime_flags(header_flags_raw: u32, runtime_flags_raw: u32) -> u32 {
    if header_flags_raw & 1 == 0 {
        runtime_flags_raw | 1
    } else {
        runtime_flags_raw
    }
}
