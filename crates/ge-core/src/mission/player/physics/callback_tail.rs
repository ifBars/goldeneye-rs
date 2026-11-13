use super::*;

impl Owner {
    pub(crate) fn clear_callback_motion(&mut self, item: ItemId) -> Result<(u16, u16)> {
        ensure!(
            self.item_identity() == Some(item) && self.item() == Some(item.get()),
            "callback physics item backlink differs"
        );
        let before = self.half(8).context("callback physics flags unknown")?;
        let after = before & 0xfcfc;
        self.store_half(8, after);
        self.store_word(0x15c, 0);
        self.store_half(0xa, 0);
        Ok((before, after))
    }
}

#[cfg(test)]
mod tests;
