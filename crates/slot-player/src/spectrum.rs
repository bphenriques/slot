//! What is heard, as the levels of a few bands from low to high, for a visualizer: each window of
//! what is written analysed as it goes to the device, and kept with where it ends, so the levels
//! shown are those of the sound at the speaker, not of what is still in the device.

use std::sync::Arc;

use rustfft::num_complex::Complex32;
use rustfft::{Fft, FftPlanner};

/// How many bands, from the bass up.
pub const BANDS: usize = 32;
/// Each band's level, 0 (quiet) to 255 (as loud as the samples go).
pub type Bands = [u8; BANDS];

/// Frames analysed at a time: about 21 ms at 48 kHz, a quarter of what is written at a time.
const WINDOW: usize = 1024;
/// How many windows are kept: more than the device ever holds, about 680 ms.
const KEPT: usize = 32;
/// The bands' span, in Hz: from the bass to past where most music has anything.
const LOWEST: f32 = 50.0;
const HIGHEST: f32 = 16_000.0;
/// The levels' span, in dB below the loudest a sample goes: quieter is nothing.
const FLOOR_DB: f32 = 60.0;
/// How far a level falls in a window, of its span: from the top to nothing in about 350 ms, as
/// a meter's needle falls; it rises at once.
const FALL: f32 = 0.06;

/// The levels of the last windows written, each with the frame of all written it ends at.
#[derive(Copy, Clone, PartialEq, Debug)]
pub(crate) struct Timeline {
    ends: [u64; KEPT],
    levels: [Bands; KEPT],
    next: usize,
    len: usize,
}

impl Default for Timeline {
    fn default() -> Self {
        Timeline {
            ends: [0; KEPT],
            levels: [[0; BANDS]; KEPT],
            next: 0,
            len: 0,
        }
    }
}

impl Timeline {
    fn push(&mut self, end: u64, levels: Bands) {
        (self.ends[self.next], self.levels[self.next]) = (end, levels);
        self.next = (self.next + 1) % KEPT;
        self.len = (self.len + 1).min(KEPT);
    }

    /// The levels heard once `heard` frames of all written have played: the last window ended
    /// by then; none before any has.
    pub(crate) fn at(&self, heard: u64) -> Option<Bands> {
        (0..self.len)
            .filter(|&i| self.ends[i] <= heard)
            .max_by_key(|&i| self.ends[i])
            .map(|i| self.levels[i])
    }
}

/// Analyses what is written into the bands' levels, window by window.
pub(crate) struct Analyser {
    fft: Arc<dyn Fft<f32>>,
    buffer: Vec<Complex32>,
    scratch: Vec<Complex32>,
    /// The Hann window, which keeps a tone from smearing into the bands beside it.
    hann: Vec<f32>,
    /// The rate the bands' bins were worked out for, and each band's first bin, the last's end
    /// after it.
    rate: u32,
    edges: [usize; BANDS + 1],
    /// Each band's level, 0 to 1, as it falls.
    levels: [f32; BANDS],
}

impl Analyser {
    pub(crate) fn new() -> Analyser {
        let fft = FftPlanner::new().plan_fft_forward(WINDOW);
        let scratch = vec![Complex32::default(); fft.get_inplace_scratch_len()];
        let hann = (0..WINDOW)
            .map(|i| {
                let x = std::f32::consts::TAU * i as f32 / WINDOW as f32;
                0.5 - 0.5 * x.cos()
            })
            .collect();
        Analyser {
            fft,
            buffer: vec![Complex32::default(); WINDOW],
            scratch,
            hann,
            rate: 0,
            edges: [0; BANDS + 1],
            levels: [0.0; BANDS],
        }
    }

    /// Takes `samples`, interleaved stereo just written at `rate`, the last of which is frame
    /// `end` of all written, into `timeline`, a window at a time.
    pub(crate) fn take(&mut self, samples: &[f32], rate: u32, end: u64, timeline: &mut Timeline) {
        if rate != self.rate {
            (self.rate, self.edges) = (rate, edges(rate));
        }
        let frames = samples.len() / 2;
        let begins = end - frames as u64;
        for (i, window) in samples.chunks_exact(WINDOW * 2).enumerate() {
            let levels = self.analyse(window);
            timeline.push(begins + ((i + 1) * WINDOW) as u64, levels);
        }
    }

    fn analyse(&mut self, window: &[f32]) -> Bands {
        for ((bin, frame), hann) in self
            .buffer
            .iter_mut()
            .zip(window.chunks_exact(2))
            .zip(&self.hann)
        {
            *bin = Complex32::new((frame[0] + frame[1]) * 0.5 * hann, 0.0);
        }
        self.fft
            .process_with_scratch(&mut self.buffer, &mut self.scratch);
        // A full-scale tone peaks at a quarter of the window, Hann's gain and the half of it in
        // the positive bins taken together: that is 0 dB.
        let full = (WINDOW / 4) as f32;
        std::array::from_fn(|band| {
            let bins = &self.buffer[self.edges[band]..self.edges[band + 1]];
            let peak = bins.iter().map(|b| b.norm()).fold(0.0, f32::max) / full;
            let db = 20.0 * peak.max(1e-9).log10();
            let level = (1.0 + db / FLOOR_DB).clamp(0.0, 1.0);
            let shown = &mut self.levels[band];
            *shown = level.max(*shown - FALL);
            (*shown * 255.0).round() as u8
        })
    }
}

/// Each band's first bin at `rate`, spaced evenly in pitch, the last's end after it; every band at
/// least a bin wide, so the lowest are not empty.
fn edges(rate: u32) -> [usize; BANDS + 1] {
    let nyquist = rate as f32 / 2.0;
    let top = HIGHEST.min(nyquist);
    let bin = |hz: f32| (hz * WINDOW as f32 / rate as f32).round() as usize;
    let mut edges = [0; BANDS + 1];
    for (i, edge) in edges.iter_mut().enumerate() {
        let hz = LOWEST * (top / LOWEST).powf(i as f32 / BANDS as f32);
        *edge = bin(hz).max(1);
    }
    for i in 1..=BANDS {
        edges[i] = edges[i].max(edges[i - 1] + 1);
    }
    edges[BANDS] = edges[BANDS].min(WINDOW / 2);
    edges
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `frames` of a tone at `hz` and `rate`, at `amplitude`, interleaved stereo.
    fn tone(hz: f32, rate: u32, amplitude: f32, frames: usize) -> Vec<f32> {
        (0..frames)
            .flat_map(|n| {
                let s = amplitude * (std::f32::consts::TAU * hz * n as f32 / rate as f32).sin();
                [s, s]
            })
            .collect()
    }

    fn loudest(levels: &Bands) -> usize {
        (0..BANDS).max_by_key(|&b| levels[b]).unwrap()
    }

    #[test]
    fn a_low_tone_lights_a_low_band_and_a_high_one_a_high_band() {
        let rate = 48_000;
        let mut timeline = Timeline::default();
        let mut low = Analyser::new();
        low.take(&tone(100.0, rate, 0.8, 4096), rate, 4096, &mut timeline);
        let low_band = loudest(&timeline.at(4096).unwrap());
        let mut high = Analyser::new();
        high.take(&tone(8_000.0, rate, 0.8, 4096), rate, 8192, &mut timeline);
        let high_band = loudest(&timeline.at(8192).unwrap());
        assert!(low_band < 6, "100 Hz in the bass: band {low_band}");
        assert!(
            high_band > BANDS - 8,
            "8 kHz in the treble: band {high_band}"
        );
        assert!(
            timeline.at(4096).unwrap()[low_band] > 200,
            "near the top, loud"
        );
    }

    #[test]
    fn silence_is_nothing_and_a_level_falls_rather_than_drops() {
        let rate = 48_000;
        let (mut a, mut timeline) = (Analyser::new(), Timeline::default());
        a.take(&tone(440.0, rate, 0.8, 1024), rate, 1024, &mut timeline);
        let band = loudest(&timeline.at(1024).unwrap());
        // A window of silence, two frames of zeros to each of its 1,024.
        a.take(&vec![0.0; 2048], rate, 2048, &mut timeline);
        let fallen = timeline.at(2048).unwrap()[band];
        assert!(
            fallen > 200 && fallen < timeline.at(1024).unwrap()[band],
            "falls: {fallen}"
        );
        for n in 3..24 {
            a.take(&vec![0.0; 2048], rate, n * 1024, &mut timeline);
        }
        assert_eq!(timeline.at(u64::MAX).unwrap(), [0; BANDS], "then nothing");
    }

    #[test]
    fn the_levels_shown_are_of_the_window_heard_not_the_last_written() {
        let rate = 48_000;
        let (mut a, mut timeline) = (Analyser::new(), Timeline::default());
        a.take(&tone(100.0, rate, 0.8, 1024), rate, 1024, &mut timeline);
        a.take(&vec![0.0; 2048 * 8], rate, 1024 + 8192, &mut timeline);
        assert_eq!(timeline.at(0), None, "nothing heard yet");
        let heard = timeline.at(1500).unwrap();
        assert!(
            heard[loudest(&heard)] > 200,
            "the tone, still at the speaker"
        );
    }

    #[test]
    fn every_band_has_bins_of_its_own_at_any_rate() {
        for rate in [22_050, 44_100, 48_000, 96_000] {
            let e = edges(rate);
            assert!(e.windows(2).all(|w| w[0] < w[1]), "{rate}: {e:?}");
            assert!(e[BANDS] <= WINDOW / 2, "{rate}");
        }
    }
}
