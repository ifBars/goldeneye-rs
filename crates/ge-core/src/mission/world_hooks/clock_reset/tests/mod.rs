use super::*;

#[path = "owner.rs"]
mod owner_tests;

struct Samples {
    words: Vec<u32>,
    calls: usize,
    fail_at: Option<usize>,
}

impl Samples {
    fn constant(bits: u32) -> Self {
        Self {
            words: vec![bits; 16],
            calls: 0,
            fail_at: None,
        }
    }

    fn failing(ordinal: usize) -> Self {
        Self {
            words: vec![0x4040_0000; 16],
            calls: 0,
            fail_at: Some(ordinal),
        }
    }
}

impl Clock for Samples {
    fn read(&mut self) -> Result<u32> {
        self.calls += 1;
        if self.fail_at == Some(self.calls) {
            anyhow::bail!("authored clock unavailable");
        }
        self.words
            .get(self.calls - 1)
            .copied()
            .context("authored clock exhausted")
    }
}

fn owner() -> Owner {
    Owner::from_constructor(&mut Samples::constant(0x3f80_0000)).unwrap()
}

fn supply(owner: &mut Owner, index: usize, capacity: u32, growth: u32, count: u32, bytes: Vec<u8>) {
    *owner.buffer_mut(index).unwrap() =
        Buffer::from_owned(capacity, growth, count, Some(bytes)).unwrap();
}
