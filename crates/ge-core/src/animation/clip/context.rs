use anyhow::{Result, ensure};

/// Semantic host owner identity and native skin reference, never a host pointer.
/// Reference zero remains a valid coded index; the asset resolver checks it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkinPair {
    pub owner: u64,
    pub reference: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkinRequest {
    BoundSecondary(SkinPair),
    DescriptorReference(SkinPair),
}

/// Selects only a source skin request. The primary target skin, table-four
/// resolution and actual retained owners remain caller responsibilities. The
/// animation header's authored model pair is deliberately not an input here.
pub fn skin_request(
    animation_owner: u64,
    descriptor_skin_reference_raw: u32,
    bound_secondary: Option<SkinPair>,
) -> Result<SkinRequest> {
    if descriptor_skin_reference_raw == u32::MAX {
        let pair = bound_secondary
            .ok_or_else(|| anyhow::anyhow!("bound secondary skin pair is unavailable"))?;
        ensure!(pair.owner != 0, "secondary skin owner is unavailable");
        Ok(SkinRequest::BoundSecondary(pair))
    } else {
        ensure!(
            animation_owner != 0,
            "animation owner for descriptor skin reference is unavailable"
        );
        Ok(SkinRequest::DescriptorReference(SkinPair {
            owner: animation_owner,
            reference: descriptor_skin_reference_raw,
        }))
    }
}
