//! A file as the writer reads it: decoded, brought to the device's stereo at 48 kHz the way Muse
//! brings it, with a 64-tap sinc filter, and played at its speed, its pitch kept.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::time::Duration;

use rodio::{Decoder, Source};
use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Indexing, Resampler};

use crate::engine::{CHANNELS, RATE};
use crate::level::{Gains, Leveling};
use crate::stretch::Stretch;

/// Frames decoded at a time.
const BLOCK: usize = 1024;
/// Below this a frame is silent, about -50 dB; past this much of it in a row, a pause in a
/// book, the rest is skipped when asked.
const SILENT: f32 = 0.003;
const PAUSE_KEPT: usize = RATE as usize / 4;
const STEREO: usize = CHANNELS as usize;

pub struct Track {
    decoder: Decoder<BufReader<File>>,
    /// The file's channels, of which the first two are played; a mono file on both sides.
    channels: usize,
    /// For a file not at the device's rate.
    resampler: Option<Fft<f32>>,
    /// How fast it plays, and the stretch that does it for a speed other than 1.
    speed: f32,
    stretch: Option<Stretch>,
    /// Its ReplayGain, and what every sample is multiplied by for the leveling asked.
    gains: Gains,
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
    /// `path` from `at` seconds in, at `speed`, leveled as asked; and how long it is, 0 when that
    /// is not known.
    pub fn open(
        path: &Path,
        at: f64,
        speed: f32,
        leveling: Leveling,
    ) -> Result<(Track, f64), String> {
        let file = File::open(path).map_err(|e| e.to_string())?;
        let mut decoder = Decoder::try_from(file).map_err(|e| e.to_string())?;
        let len = decoder.total_duration().map_or(0.0, |d| d.as_secs_f64());
        if at > 0.0 {
            decoder.try_seek(secs(at)).map_err(|e| e.to_string())?;
        }
        let rate = decoder.sample_rate().get();
        let resampler = match rate == RATE {
            true => None,
            false => Some(resampler(rate)?),
        };
        let gains = Gains::read(path);
        let mut track = Track {
            channels: usize::from(decoder.channels().get()),
            decoder,
            resampler,
            speed,
            stretch: None,
            gains,
            gain: gains.factor(leveling),
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
        self.decoder.try_seek(secs(at)).map_err(|e| e.to_string())?;
        (self.from, self.played) = (at, 0);
        self.restart();
        Ok(())
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
        let at = self.from + self.played as f64 * f64::from(self.speed) / f64::from(RATE);
        (self.len > 0.0).then(|| (self.len - at).max(0.0) / f64::from(self.speed))
    }

    pub fn set_leveling(&mut self, leveling: Leveling) {
        self.gain = self.gains.factor(leveling);
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
        self.stretch = (self.speed != 1.0).then(|| Stretch::new(RATE, CHANNELS, self.speed));
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

    /// Drops from `ready` what goes on past [`PAUSE_KEPT`] of silence, counting it as played.
    fn skip_pauses(&mut self, ready: &mut Vec<f32>) {
        let mut kept = 0;
        for i in (0..ready.len()).step_by(STEREO) {
            let silent = ready[i].abs() < SILENT && ready[i + 1].abs() < SILENT;
            self.quiet = if silent { self.quiet + 1 } else { 0 };
            if self.quiet > PAUSE_KEPT {
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
        let mut frame = [0.0; 8];
        let channels = self.channels.clamp(1, frame.len());
        for got in 0..frames {
            for sample in &mut frame[..channels] {
                match self.decoder.next() {
                    Some(s) => *sample = s,
                    None => return got,
                }
            }
            let right = if channels == 1 { frame[0] } else { frame[1] };
            self.decoded.extend([frame[0], right]);
        }
        frames
    }
}

/// A resampler from `rate` to the device's: FFT-based, as fine as Muse's 64-tap sinc filter for
/// the one ratio a file has, at a fraction of the work. Its blocks are a whole number of the
/// ratio's input steps (147 frames from 44.1 kHz), or it converts at a ratio a little off.
fn resampler(rate: u32) -> Result<Fft<f32>, String> {
    let (rate, to) = (rate as usize, RATE as usize);
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

/// `secs` as a duration: none below 0, and past any end above every length.
pub fn secs(secs: f64) -> Duration {
    Duration::try_from_secs_f64(secs.max(0.0)).unwrap_or(Duration::MAX)
}
