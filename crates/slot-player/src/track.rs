//! A file as the writer reads it: decoded, brought to the device's stereo at the rate it plays
//! at (its own, unless the device will not take it), with a resampler as fine as a 64-tap sinc
//! filter where that is another, and played at its speed, its pitch kept.

use std::path::Path;

use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Indexing, Resampler};

use crate::decode::Decoder;
use crate::engine::CHANNELS;
use crate::level::{Gains, Leveling};
use crate::stretch::Stretch;

/// Frames decoded at a time.
const BLOCK: usize = 1024;
/// Below this a frame is silent, about -50 dB; past this much of it in a row, a pause in a
/// book (a quarter of a second), the rest is skipped when asked.
const SILENT: f32 = 0.003;
const STEREO: usize = CHANNELS as usize;

pub struct Track {
    decoder: Decoder,
    /// The rate it plays at, and the resampler for a file not at it.
    rate: u32,
    resampler: Option<Fft<f32>>,
    /// How fast it plays, and the stretch that does it for a speed other than 1.
    speed: f32,
    stretch: Option<Stretch>,
    /// Its ReplayGain, read once leveled, and what every sample is multiplied by for the
    /// leveling asked; and where it is, to read it from.
    gains: Option<Gains>,
    path: std::path::PathBuf,
    gain: f32,
    /// Decoded stereo at the file's rate, then ready to play, and how much of it is played.
    decoded: Vec<f32>,
    ready: Vec<f32>,
    at: usize,
    ended: bool,
    /// Where it was opened or moved to, how long it is, and the frames played since: given, or
    /// skipped as silence, which counts as played.
    from: f64,
    len: f64,
    played: u64,
    /// Whether long silences are skipped, how long the one going on has been, and the frames
    /// skipped not yet told.
    skip_silence: bool,
    quiet: usize,
    skipped: u64,
}

impl Track {
    /// `path` from `at` seconds in, at `speed`, leveled as asked, played at the rate `rate` gives
    /// for the file's own; and how long it is, 0 when that is not known.
    pub fn open(
        path: &Path,
        at: f64,
        speed: f32,
        leveling: Leveling,
        rate: impl FnOnce(u32) -> u32,
    ) -> Result<(Track, f64), String> {
        let mut decoder = Decoder::open(path)?;
        let len = decoder.len();
        if at > 0.0 {
            decoder.seek(at)?;
        }
        let own = decoder.rate();
        let rate = rate(own);
        let resampler = if own == rate {
            None
        } else {
            Some(resampler(own, rate)?)
        };
        // Its tags are read only when leveled: most play as recorded.
        let gains = (leveling != Leveling::Off).then(|| Gains::read(path));
        let mut track = Track {
            decoder,
            rate,
            resampler,
            speed,
            stretch: None,
            gains,
            gain: gains.map_or(1.0, |g| g.factor(leveling)),
            path: path.to_path_buf(),
            decoded: Vec::new(),
            ready: Vec::new(),
            at: 0,
            ended: false,
            from: at,
            len,
            played: 0,
            skip_silence: false,
            quiet: 0,
            skipped: 0,
        };
        track.restart();
        Ok((track, len))
    }

    /// Moves to `at` seconds in, dropping what was decoded ahead.
    pub fn seek(&mut self, at: f64) -> Result<(), String> {
        self.decoder.seek(at)?;
        (self.from, self.played) = (at, 0);
        self.restart();
        Ok(())
    }

    /// The rate it plays at, frames a second.
    pub fn rate(&self) -> u32 {
        self.rate
    }

    pub fn set_skip_silence(&mut self, on: bool) {
        self.skip_silence = on;
    }

    /// Frames skipped as silence since last asked: they count as heard.
    pub fn take_skipped(&mut self) -> u64 {
        std::mem::take(&mut self.skipped)
    }

    /// Seconds of the file left to play, as heard at its speed; none when its length is not
    /// known.
    pub fn left(&self) -> Option<f64> {
        let at = self.from + self.played as f64 * f64::from(self.speed) / f64::from(self.rate);
        (self.len > 0.0).then(|| (self.len - at).max(0.0) / f64::from(self.speed))
    }

    pub fn set_leveling(&mut self, leveling: Leveling) {
        if leveling != Leveling::Off && self.gains.is_none() {
            self.gains = Some(Gains::read(&self.path));
        }
        self.gain = self.gains.map_or(1.0, |g| g.factor(leveling));
    }

    /// Plays on at `speed`: what was ready at the last is dropped.
    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed;
        self.restart();
    }

    /// Starts the resampler and the stretch afresh, for sound that does not follow on.
    fn restart(&mut self) {
        if let Some(r) = &mut self.resampler {
            r.reset();
        }
        self.stretch = (self.speed != 1.0).then(|| Stretch::new(self.rate, CHANNELS, self.speed));
        self.ready.clear();
        self.at = 0;
        self.ended = false;
    }

    /// Fills `out` with stereo at the device's rate, and says how many samples of it are the
    /// file's: fewer than `out` holds once it has run out.
    pub fn read(&mut self, out: &mut [f32]) -> usize {
        let mut n = 0;
        while n < out.len() {
            if self.at == self.ready.len() && !self.refill() {
                break;
            }
            let take = (out.len() - n).min(self.ready.len() - self.at);
            out[n..n + take].copy_from_slice(&self.ready[self.at..self.at + take]);
            (n, self.at) = (n + take, self.at + take);
        }
        if self.gain != 1.0 {
            out[..n].iter_mut().for_each(|s| *s *= self.gain);
        }
        self.played += (n / STEREO) as u64;
        n
    }

    /// The next stretch of sound ready to play, or false at the end.
    fn refill(&mut self) -> bool {
        self.ready.clear();
        self.at = 0;
        while self.ready.is_empty() && !self.ended {
            let frames = self
                .resampler
                .as_ref()
                .map_or(BLOCK, |r| r.input_frames_next());
            let got = self.decode(frames);
            self.ended = got < frames;
            let mut ready = std::mem::take(&mut self.ready);
            match &mut self.resampler {
                Some(r) => resample(r, &self.decoded, got, frames, &mut ready),
                None => ready.extend_from_slice(&self.decoded),
            }
            if let Some(stretch) = &mut self.stretch {
                stretch.put(&ready);
                ready.clear();
                if self.ended {
                    stretch.flush();
                }
                stretch.take(&mut ready);
            }
            if self.skip_silence {
                self.skip_pauses(&mut ready);
            }
            self.ready = ready;
        }
        !self.ready.is_empty()
    }

    /// Drops from `ready` what goes on past a quarter of a second of silence, counting it as
    /// played.
    fn skip_pauses(&mut self, ready: &mut Vec<f32>) {
        let pause_kept = self.rate as usize / 4;
        let mut kept = 0;
        for i in (0..ready.len()).step_by(STEREO) {
            let silent = ready[i].abs() < SILENT && ready[i + 1].abs() < SILENT;
            self.quiet = if silent { self.quiet + 1 } else { 0 };
            if self.quiet > pause_kept {
                self.skipped += 1;
                self.played += 1;
                continue;
            }
            ready.copy_within(i..i + STEREO, kept);
            kept += STEREO;
        }
        ready.truncate(kept);
    }

    /// Decodes up to `frames` frames into `decoded`, as stereo, and says how many it got.
    fn decode(&mut self, frames: usize) -> usize {
        self.decoded.clear();
        self.decoder.read_stereo(frames, &mut self.decoded)
    }
}

/// A resampler from `rate` to `to`: FFT-based, as fine as a 64-tap sinc filter for
/// the one ratio a file has, at a fraction of the work. Its blocks are a whole number of the
/// ratio's input steps (147 frames from 44.1 kHz), or it converts at a ratio a little off.
fn resampler(rate: u32, to: u32) -> Result<Fft<f32>, String> {
    let (rate, to) = (rate as usize, to as usize);
    let step = rate / gcd(rate, to);
    let block = BLOCK.div_ceil(step) * step;
    Fft::new(rate, to, block, STEREO, FixedSync::Input).map_err(|e| e.to_string())
}

fn gcd(a: usize, b: usize) -> usize {
    match b {
        0 => a,
        b => gcd(b, a % b),
    }
}

/// `got` of the `frames` frames in `decoded`, resampled onto the end of `out`.
fn resample(r: &mut Fft<f32>, decoded: &[f32], got: usize, frames: usize, out: &mut Vec<f32>) {
    let start = out.len();
    out.resize(start + r.output_frames_max() * STEREO, 0.0);
    // The last block, short, is filled out with silence.
    let mut input = std::borrow::Cow::Borrowed(decoded);
    if got < frames {
        input.to_mut().resize(frames * STEREO, 0.0);
    }
    let indexing = Indexing {
        input_offset: 0,
        output_offset: 0,
        partial_len: (got < frames).then_some(got),
        active_channels_mask: None,
    };
    let done = InterleavedSlice::new(&input[..], STEREO, frames).and_then(|i| {
        let mut o = InterleavedSlice::new_mut(&mut out[start..], STEREO, r.output_frames_max())?;
        Ok(r.process_into_buffer(&i, &mut o, Some(&indexing)))
    });
    let written = match done {
        Ok(Ok((_, written))) => written,
        _ => 0,
    };
    out.truncate(start + written * STEREO);
}
