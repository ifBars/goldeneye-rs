//! Shared source scalar/token rules, not a native floating-point parity claim.
pub(super) fn signed_tenths(raw: u32) -> f32 {
    let converted = (raw as i32) as f32;
    converted * f32::from_bits(0x3dcc_cccd)
}

pub(super) fn reference(raw: u32, prefix: u16) -> u32 {
    if raw >> 16 == u32::from(prefix) && raw & 0xffff != 0 {
        raw
    } else {
        0
    }
}
