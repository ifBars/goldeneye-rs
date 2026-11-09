//! Bounded asynchronous movie decoding. Native events never perform disc/JPEG work.
use anyhow::{Context, Result, ensure};
use ge_assets::{disc::GameDisc, resource::GameAssets, thp, tpl::RgbaImage};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::mpsc::{self, Receiver, SyncSender, TrySendError},
    thread::{self, JoinHandle},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Movie {
    Activision,
    Eurocom,
    Mgm,
    Frontend,
}
impl Movie {
    pub const ALL: [Self; 4] = [Self::Activision, Self::Eurocom, Self::Mgm, Self::Frontend];
    pub const fn path(self) -> &'static str {
        match self {
            Self::Activision => "Movies/US/ACTIVISION.THP",
            Self::Eurocom => "Movies/US/EUROCOM.thp",
            Self::Mgm => "Movies/US/MGM_Logo.THP",
            Self::Frontend => "Movies/US/frontend.THP",
        }
    }
}
#[derive(Clone)]
pub struct Clip {
    pub header: thp::Header,
    pub frames: Vec<thp::Frame>,
}
pub struct Catalog {
    pub clips: BTreeMap<Movie, Clip>,
}
impl Catalog {
    pub fn load(source: &mut GameAssets) -> Result<Self> {
        let mut clips = BTreeMap::new();
        for movie in Movie::ALL {
            let path = movie.path();
            let len = source
                .disc
                .files
                .iter()
                .find(|f| f.path.eq_ignore_ascii_case(path))
                .with_context(|| format!("movie missing: {path}"))?
                .bytes;
            let header = thp::read_header(len, |at, n| source.disc.read(path, at, n))?;
            let frames = thp::frame_index(&header, |at, n| source.disc.read(path, at, n))?;
            clips.insert(movie, Clip { header, frames });
        }
        Ok(Self { clips })
    }
}
pub struct Decoded {
    pub movie: Movie,
    pub frame: u32,
    pub image: RgbaImage,
    pub audio: Option<thp::PcmAudio>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct Request {
    movie: Movie,
    frame: u32,
}
pub struct Player {
    requests: Option<SyncSender<Request>>,
    responses: Option<Receiver<Result<Decoded>>>,
    worker: Option<JoinHandle<()>>,
    last: Option<Request>,
}
impl Player {
    pub fn new(disc: PathBuf, clips: BTreeMap<Movie, Clip>) -> Result<Self> {
        let (tx, rx) = mpsc::sync_channel::<Request>(1);
        let (output, input) = mpsc::sync_channel(2);
        let worker = thread::Builder::new()
            .name("frontend-media".into())
            .spawn(move || {
                let run = || -> Result<()> {
                    let mut disc = GameDisc::open(&disc)?;
                    let mut audio_decoder = thp::AudioDecoder::default();
                    let mut active = None;
                    let mut next_audio = 0;
                    while let Ok(request) = rx.recv() {
                        let clip = clips
                            .get(&request.movie)
                            .context("movie metadata unavailable")?;
                        let row = clip
                            .frames
                            .get(request.frame as usize)
                            .context("movie frame outside index")?;
                        if active != Some(request.movie)
                            || request.frame.saturating_add(4) < next_audio
                        {
                            audio_decoder.reset();
                            next_audio = 0;
                            active = Some(request.movie);
                        }
                        let video = disc.read(
                            request.movie.path(),
                            row.video_offset,
                            row.video_bytes as usize,
                        )?;
                        let image = thp::decode_frame(&clip.header, row, &video)?;
                        let mut pcm = None;
                        if clip.header.audio.is_some() {
                            let end = request.frame.saturating_add(4).min(clip.header.frame_count);
                            while next_audio < end {
                                let frame = &clip.frames[next_audio as usize];
                                let data = disc.read(
                                    request.movie.path(),
                                    frame.audio_offset.context("audio offset absent")?,
                                    frame.audio_bytes as usize,
                                )?;
                                let mut decoded =
                                    audio_decoder.decode(&clip.header, frame, &data)?;
                                if let Some(existing) = pcm.as_mut() {
                                    let existing: &mut thp::PcmAudio = existing;
                                    existing.samples_per_channel += decoded.samples_per_channel;
                                    existing.samples.append(&mut decoded.samples);
                                } else {
                                    pcm = Some(decoded);
                                }
                                next_audio += 1;
                            }
                        }
                        let decoded = Decoded {
                            movie: request.movie,
                            frame: request.frame,
                            image,
                            audio: pcm,
                        };
                        if output.send(Ok(decoded)).is_err() {
                            return Ok(());
                        }
                    }
                    Ok(())
                };
                if let Err(error) = run() {
                    let _ = output.send(Err(error));
                }
            })?;
        Ok(Self {
            requests: Some(tx),
            responses: Some(input),
            worker: Some(worker),
            last: None,
        })
    }
    pub fn request(&mut self, movie: Movie, frame: u32) -> Result<()> {
        let request = Request { movie, frame };
        if self.last == Some(request) {
            return Ok(());
        }
        match self
            .requests
            .as_ref()
            .context("movie worker stopped")?
            .try_send(request)
        {
            Ok(()) => self.last = Some(request),
            Err(TrySendError::Full(_)) => {}
            Err(TrySendError::Disconnected(_)) => anyhow::bail!("movie decoder stopped"),
        }
        Ok(())
    }
    pub fn poll(&mut self) -> Result<Vec<Decoded>> {
        let mut ready = Vec::new();
        if let Some(receiver) = &self.responses {
            for _ in 0..2 {
                match receiver.try_recv() {
                    Ok(frame) => ready.push(frame?),
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => anyhow::bail!("movie decoder stopped"),
                }
            }
        }
        ensure!(ready.len() <= 3, "movie response queue exceeded bound");
        Ok(ready)
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.requests.take();
        self.responses.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
#[path = "tests/frontend_media.rs"]
mod tests;
