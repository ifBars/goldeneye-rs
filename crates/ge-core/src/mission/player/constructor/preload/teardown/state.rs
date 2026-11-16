use super::*;
fn receipts(
    children: &[(u8, ChildReceipt)],
    skeleton: Option<SkeletonReceipt>,
    emit: &mut impl FnMut(u32),
) {
    emit(children.len() as u32);
    for (slot, receipt) in children {
        emit(u32::from(*slot));
        receipt.visit_state_words(emit);
    }
    emit(u32::from(skeleton.is_some()));
    if let Some(skeleton) = skeleton {
        for count in [
            skeleton.bones,
            skeleton.auxiliary_words,
            skeleton.block_bytes,
        ] {
            emit(count as u32);
        }
    }
}
impl Teardown {
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        self.binding.visit_state_words(&mut emit);
        self.snapshot.visit_state_words(&mut emit);
        emit(self.stage as u32);
        emit(self.dependency.map_or(0, |d| d as u32 + 1));
        emit(u32::from(self.walk.slot));
        match self.walk.node {
            None => emit(0),
            Some(None) => emit(1),
            Some(Some(node)) => {
                emit(2);
                emit((node.get() >> 32) as u32);
                emit(node.get() as u32);
            }
        }
        emit(u32::from(self.walk.child.is_some()));
        if let Some(cursor) = &self.walk.child {
            cursor.visit_state_words(&mut emit);
        }
        receipts(&self.children, self.skeleton, &mut emit);
    }
}
impl Tombstone {
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        self.binding.visit_state_words(&mut emit);
        self.snapshot.visit_state_words(&mut emit);
        receipts(&self.children, self.skeleton, &mut emit);
    }
}
