use super::{Budget, Child, Group, Set, View, add};
use crate::binary::Bytes;
use anyhow::{Context, Result, ensure};

pub(super) fn read(view: &View<'_>, reference: u32, budget: Budget) -> Result<Option<Set>> {
    let Some(index) = crate::reference::resolve(view.data, view.sets, 20, reference)? else {
        return Ok(None);
    };
    let descriptor = view.descriptor(view.sets, index)?;
    let count = usize::try_from(descriptor.count)?;
    ensure!(
        count <= budget.groups,
        "animation set group budget exhausted"
    );
    let mut groups = Vec::new();
    groups
        .try_reserve_exact(count)
        .context("allocating bounded animation groups")?;
    if count == 0 {
        return Ok(Some(Set { descriptor, groups }));
    }
    let b = Bytes(view.data);
    let mut at = view.body_address(descriptor)?;
    let mut total_children = 0usize;
    for _ in 0..count {
        let source_address = at;
        let slot = b.u16(at)?;
        let child_count = b.u16(add(at, 2)?)? as i16;
        ensure!(
            child_count >= 0,
            "unsupported negative animation child count at {at:#x}"
        );
        let child_count = child_count as usize;
        let weight_bits = b.u32(add(at, 4)?)?;
        total_children = total_children
            .checked_add(child_count)
            .context("animation child count overflow")?;
        ensure!(
            total_children <= budget.children,
            "animation set child budget exhausted"
        );
        let mut children = Vec::new();
        children
            .try_reserve_exact(child_count)
            .context("allocating bounded animation children")?;
        at = add(at, 8)?;
        for _ in 0..child_count {
            children.push(Child {
                source_address: at,
                reference: b.u32(at)?,
                flags_raw: b.u16(add(at, 4)?)?,
                divisor_raw: b.u16(add(at, 6)?)?,
                word8_raw: b.u32(add(at, 8)?)?,
            });
            at = add(at, 12)?;
        }
        groups.push(Group {
            source_address,
            slot,
            weight_bits,
            children,
        });
    }
    Ok(Some(Set { descriptor, groups }))
}
