use super::*;
impl Header {
    pub(super) fn setup_clips(
        &mut self,
        source: &mut impl Source,
        trace: &mut Trace,
        offset: u32,
    ) -> Result<()> {
        let counts = source.counts(offset)?;
        trace.requested = Some(counts);
        let total = usize::from(counts[0]) + usize::from(counts[1]);
        self.clips = Some(if total <= 1 {
            self.bytes[0x76] = Some(0);
            Storage::Inline {
                file: None,
                cached: None,
            }
        } else {
            let mut records = Vec::new();
            if let Err(error) = records.try_reserve_exact(total) {
                self.w(0x5c, 0);
                return Err(error).context("clip-reference allocation failed");
            }
            records.resize_with(total, || Record {
                file: None,
                bytes: [None; 0x1c],
                cached: None,
            });
            self.bytes[0x76] = Some(1);
            Storage::Heap(records)
        });
        self.bytes[0x5c..0x60].fill(None);
        self.bytes[0x74] = Some(0);
        for secondary in [false, true] {
            let kind = usize::from(secondary);
            if secondary {
                self.bytes[0x75] = Some(0);
            }
            for row in 0..usize::from(counts[kind]) {
                let token = source.file_token(offset, secondary, row, counts[0])?;
                let file = if token == u32::MAX {
                    self.file
                } else {
                    match source.lookup_file(token)? {
                        FileLookup::Bound(file) => file,
                        FileLookup::Missing => {
                            trace.rows_visited[kind] += 1;
                            continue;
                        }
                        FileLookup::Pending => {
                            trace.pending_file = Some(token);
                            self.stage = Stage::AwaitingClipFile;
                            return Ok(());
                        }
                    }
                };
                let index = usize::from(trace.bound[0]) + usize::from(trace.bound[1]);
                let reference = source.reference(offset, secondary, row, counts[0])?;
                if secondary {
                    let start = source.start(offset, true, row, counts[0])?;
                    self.bind_clip_file(index, file)?;
                    let end = source.end(offset, true, row, counts[0])?;
                    self.clip_write(index, 4, &reference.to_be_bytes())?;
                    self.clip_write(index, 8, &u32::MAX.to_be_bytes())?;
                    self.clip_write(index, 0x10, &u32::MAX.to_be_bytes())?;
                    self.clip_write(index, 0x14, &0u32.to_be_bytes())?;
                    self.clip_write(index, 0x18, &start.to_be_bytes())?;
                    self.clip_write(index, 0x1a, &end.to_be_bytes())?;
                } else {
                    self.bind_clip_file(index, file)?;
                    let blend = source.blend(offset, row)?;
                    self.clip_write(index, 4, &reference.to_be_bytes())?;
                    let start = source.start(offset, false, row, counts[0])?;
                    self.clip_write(index, 8, &u32::MAX.to_be_bytes())?;
                    let end = source.end(offset, false, row, counts[0])?;
                    self.clip_write(index, 0x10, &u32::MAX.to_be_bytes())?;
                    self.clip_write(index, 0x14, &blend.to_be_bytes())?;
                    self.clip_write(index, 0x18, &start.to_be_bytes())?;
                    self.clip_write(index, 0x1a, &end.to_be_bytes())?;
                }
                trace.bound[kind] += 1;
                trace.rows_visited[kind] += 1;
                self.bytes[0x74 + kind] = Some(trace.bound[kind]);
            }
        }
        let raw = source.initial_index(offset)?;
        trace.initial_index_raw = Some(raw);
        let clamped = raw.min(trace.bound[0]);
        trace.initial_index_clamped = Some(clamped);
        self.h(0x6c, u16::from(clamped));
        let Some(range) = self.clip_word(usize::from(clamped), 0x18) else {
            self.stage = Stage::AwaitingClipRange;
            return Ok(());
        };
        let start = (range >> 16) as i16;
        let end = range as i16;
        let range = i32::from(end) - i32::from(start);
        trace.frame_range = Some(range);
        if range <= 0 {
            self.stage = Stage::AwaitingFrameCount;
            return Ok(());
        }
        self.h(0x70, range as u16);
        trace.setup_returned = true;
        // Native caller overwrites the helper's clamped index with the raw byte.
        let raw = source.initial_index(offset)?;
        self.h(0x6c, u16::from(raw));
        let flags = source.flags(offset)?;
        trace.flags_raw = Some(flags);
        let old = self.word(0x20).context("child flags unknown")?;
        self.w(
            0x20,
            if flags & 1 != 0 {
                old | 0x80
            } else {
                old & !0x80
            },
        );
        self.stage = Stage::AwaitingAnimation;
        Ok(())
    }
    fn bind_clip_file(&mut self, index: usize, file: u32) -> Result<()> {
        ensure!(file >> 24 == 1, "clip file identity is not a file");
        match self.clips.as_mut().context("clip storage absent")? {
            Storage::Inline { file: target, .. } => {
                ensure!(index == 0, "inline clip index exceeds storage");
                *target = Some(file);
            }
            Storage::Heap(records) => {
                records
                    .get_mut(index)
                    .context("clip allocation exhausted")?
                    .file = Some(file)
            }
        }
        self.clip_unknown_pointer(index)
    }
    fn clip_unknown_pointer(&mut self, index: usize) -> Result<()> {
        match self.clips.as_mut().context("clip storage absent")? {
            Storage::Inline { .. } => self.bytes[0x40..0x44].fill(None),
            Storage::Heap(records) => records
                .get_mut(index)
                .context("clip allocation exhausted")?
                .bytes[..4]
                .fill(None),
        }
        Ok(())
    }
    pub(in super::super) fn clip_write(
        &mut self,
        index: usize,
        at: usize,
        value: &[u8],
    ) -> Result<()> {
        let bytes = match self.clips.as_mut().context("clip storage absent")? {
            Storage::Inline { .. } => {
                ensure!(index == 0, "inline clip index exceeds storage");
                &mut self.bytes[0x40..0x5c]
            }
            Storage::Heap(records) => {
                &mut records
                    .get_mut(index)
                    .context("clip allocation exhausted")?
                    .bytes
            }
        };
        let target = bytes
            .get_mut(at..at + value.len())
            .context("clip write exceeds record")?;
        for (slot, byte) in target.iter_mut().zip(value) {
            *slot = Some(*byte);
        }
        Ok(())
    }
}
