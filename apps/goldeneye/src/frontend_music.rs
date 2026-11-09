//! Resident title sound selection and bounded asynchronous NGCA music decoding.
use anyhow::{Context, Result, ensure};
use ge_assets::{
    frontend_stream::{self, NgcaInfo, PcmChunk},
    resource::GameAssets,
};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, SyncSender},
    thread::{self, JoinHandle},
};

const BANK: u32 = 0x2d500017;
const SOUND: u32 = 0x2d700109;

struct Source {
    assets: GameAssets,
    archive: String,
    offset: u64,
    bytes: u64,
}
impl Source {
    fn open(disc: &Path) -> Result<Self> {
        let assets = GameAssets::open(disc, "File_COM.bin")?;
        let entry = assets
            .index
            .entries
            .iter()
            .find(|e| e.hash == BANK)
            .context("title sound bank absent")?;
        let location = entry
            .locations
            .first()
            .context("title bank location absent")?;
        let archive = format!("File_COM.{:03}", location.archive);
        let offset = u64::from(location.offset);
        let bytes = u64::from(entry.base_bytes);
        Ok(Self {
            assets,
            archive,
            offset,
            bytes,
        })
    }
    fn read(&mut self, at: u64, count: usize) -> Result<Vec<u8>> {
        ensure!(
            at.checked_add(count as u64)
                .is_some_and(|end| end <= self.bytes),
            "title bank read outside resource"
        );
        self.assets
            .disc
            .read(&self.archive, self.offset + at, count)
    }
    fn info(&mut self) -> Result<NgcaInfo> {
        let info = frontend_stream::read_bank_sound(self.bytes, SOUND, |at, n| self.read(at, n))?;
        ensure!(
            info.resource_hash == BANK && info.sound_hash == SOUND,
            "title sound identity differs"
        );
        ensure!(
            info.channels == 2 && info.loop_start_frame == Some(0),
            "unsupported title music channel/loop configuration"
        );
        Ok(info)
    }
}
#[derive(Serialize)]
pub struct Evidence {
    pub info: NgcaInfo,
    pub decoded_frames: u64,
    pub decoded_chunks: u64,
    pub decoded_loops: u64,
    pub scope: &'static str,
}
pub struct Music {
    info: NgcaInfo,
    request: Option<SyncSender<()>>,
    response: Option<Receiver<Result<(PcmChunk, u64)>>>,
    worker: Option<JoinHandle<()>>,
    pending: bool,
    frames: u64,
    chunks: u64,
    loops: u64,
}
impl Music {
    pub fn new(disc: PathBuf) -> Result<Self> {
        let info = Source::open(&disc)?.info()?;
        let worker_info = info.clone();
        let (tx, rx) = mpsc::sync_channel::<()>(1);
        let (output, input) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("frontend-music".into())
            .spawn(move || {
                let run = || -> Result<()> {
                    let mut source = Source::open(&disc)?;
                    let mut decoder = worker_info.decoder();
                    let mut loops = 0;
                    while rx.recv().is_ok() {
                        if decoder.remaining_blocks(&worker_info) == 0 {
                            decoder = worker_info.decoder();
                            loops += 1;
                        }
                        let count = decoder.remaining_blocks(&worker_info).min(1024) as usize;
                        let chunk = decoder
                            .decode_blocks(&worker_info, count, |at, n| source.read(at, n))?;
                        if output.send(Ok((chunk, loops))).is_err() {
                            break;
                        }
                    }
                    Ok(())
                };
                if let Err(error) = run() {
                    let _ = output.send(Err(error));
                }
            })?;
        Ok(Self {
            info,
            request: Some(tx),
            response: Some(input),
            worker: Some(worker),
            pending: false,
            frames: 0,
            chunks: 0,
            loops: 0,
        })
    }
    pub fn request(&mut self) -> Result<()> {
        if !self.pending {
            self.request
                .as_ref()
                .context("music decoder stopped")?
                .send(())
                .context("music decoder stopped")?;
            self.pending = true;
        }
        Ok(())
    }
    pub fn poll(&mut self) -> Result<Option<PcmChunk>> {
        match self
            .response
            .as_ref()
            .context("music response stopped")?
            .try_recv()
        {
            Ok(value) => {
                let (chunk, loops) = value?;
                self.pending = false;
                self.frames += chunk.frame_count as u64;
                self.chunks += 1;
                self.loops = loops;
                Ok(Some(chunk))
            }
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => anyhow::bail!("music decoder stopped"),
        }
    }
    pub fn evidence(&self) -> Evidence {
        Evidence {
            info: self.info.clone(),
            decoded_frames: self.frames,
            decoded_chunks: self.chunks,
            decoded_loops: self.loops,
            scope: "Native sound 2d700109 resolves resident NGCA wave 2da021bf in bank 2d500017. Sequential planar DSP decode; host whole-track restart at source loop start zero. Original hardware loop-history restoration and mixer equivalence remain unverified.",
        }
    }
}
impl Drop for Music {
    fn drop(&mut self) {
        self.request.take();
        self.response.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// Full resident-wave decode diagnostics; keeps all original PCM in memory only.
pub fn audit(disc: &Path) -> Result<serde_json::Value> {
    use sha2::{Digest, Sha256};
    let mut source = Source::open(disc)?;
    let info = source.info()?;
    let mut decoder = info.decoder();
    let mut hash = Sha256::new();
    let mut frames = 0u64;
    let mut values = 0u64;
    let mut nonzero = 0u64;
    let mut peak = 0u32;
    while decoder.remaining_blocks(&info) > 0 {
        let count = decoder.remaining_blocks(&info).min(1024) as usize;
        let pcm = decoder.decode_blocks(&info, count, |at, n| source.read(at, n))?;
        frames += pcm.frame_count as u64;
        for sample in pcm.samples {
            hash.update(sample.to_le_bytes());
            values += 1;
            nonzero += u64::from(sample != 0);
            peak = peak.max(i32::from(sample).unsigned_abs());
        }
    }
    ensure!(
        frames == info.sample_frames && values == frames * u64::from(info.channels),
        "title PCM count differs from header"
    );
    Ok(
        serde_json::json!({"info":info,"decoded_sample_frames":frames,"interleaved_samples":values,"nonzero_samples":nonzero,"peak_absolute_sample":peak,"pcm_s16le_sha256":format!("{:x}",hash.finalize()),"reference_compared":false,"scope":"Resident sound/wave binding and independent sequential NGCA decoding; no original mixer/loop synchronization or speaker comparison."}),
    )
}
