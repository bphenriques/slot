//! The equalizer: a peaking filter a band, as the Audio EQ Cookbook (R. Bristow-Johnson) sets
//! them, over stereo as it is written. The bands are whatever slot asks; none is no filter at
//! all, and no work.

/// One band: its centre in Hz, how far up or down in dB, and how wide (Q: 1 is about an octave
/// and a third).
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Band {
    pub hz: f32,
    pub db: f32,
    pub q: f32,
}

/// A band's filter: its coefficients, and each side's last two in and out.
#[derive(Copy, Clone, Default)]
struct Biquad {
    b: [f32; 3],
    a: [f32; 2],
    state: [[f32; 4]; 2],
}

impl Biquad {
    fn peaking(band: Band, rate: u32) -> Biquad {
        let a = 10f32.powf(band.db / 40.0);
        let w0 = std::f32::consts::TAU * band.hz / rate as f32;
        let alpha = w0.sin() / (2.0 * band.q.max(0.1));
        let cos = w0.cos();
        let a0 = 1.0 + alpha / a;
        Biquad {
            b: [
                (1.0 + alpha * a) / a0,
                -2.0 * cos / a0,
                (1.0 - alpha * a) / a0,
            ],
            a: [-2.0 * cos / a0, (1.0 - alpha / a) / a0],
            state: [[0.0; 4]; 2],
        }
    }

    /// `x` through the filter, on side `c`: its last in, the one before, its last out, the one
    /// before.
    fn run(&mut self, c: usize, x: f32) -> f32 {
        let [x1, x2, y1, y2] = self.state[c];
        let y = self.b[0] * x + self.b[1] * x1 + self.b[2] * x2 - self.a[0] * y1 - self.a[1] * y2;
        self.state[c] = [x, x1, y, y1];
        y
    }
}

/// The bands asked for, at the rate playing, and how far down all of it is set first so the
/// most a band lifts never clips.
#[derive(Default)]
pub struct Equalizer {
    bands: Vec<Band>,
    rate: u32,
    filters: Vec<Biquad>,
    pre: f32,
}

impl Equalizer {
    /// Plays through `bands` from now; none, as recorded.
    pub fn set(&mut self, bands: Vec<Band>) {
        self.bands = bands;
        self.build();
    }

    /// At `rate`, the filters set again for it.
    pub fn rate(&mut self, rate: u32) {
        if rate != self.rate {
            self.rate = rate;
            self.build();
        }
    }

    fn build(&mut self) {
        let rate = self.rate.max(1);
        let bands = self.bands.iter().filter(|b| b.db != 0.0);
        self.filters = bands.map(|&b| Biquad::peaking(b, rate)).collect();
        let lift = self.bands.iter().map(|b| b.db).fold(0.0, f32::max);
        self.pre = 10f32.powf(-lift / 20.0);
    }

    /// Interleaved stereo `samples`, through the bands.
    pub fn run(&mut self, samples: &mut [f32]) {
        if self.filters.is_empty() {
            return;
        }
        for frame in samples.chunks_exact_mut(2) {
            for (c, sample) in frame.iter_mut().enumerate() {
                let mut x = *sample * self.pre;
                for filter in &mut self.filters {
                    x = filter.run(c, x);
                }
                *sample = x;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// How loud a sine at `hz` comes out of `eq` at 48 kHz, as a factor of how it went in.
    fn gain_at(eq: &mut Equalizer, hz: f32) -> f32 {
        let rate = 48_000.0;
        let mut samples: Vec<f32> = (0..48_000)
            .flat_map(|i| {
                let s = (std::f32::consts::TAU * hz * i as f32 / rate).sin() * 0.25;
                [s, s]
            })
            .collect();
        eq.run(&mut samples);
        // Past the first half second, the filters settled.
        let peak = samples[48_000..].iter().fold(0.0f32, |m, s| m.max(s.abs()));
        peak / 0.25
    }

    fn db(factor: f32) -> f32 {
        20.0 * factor.log10()
    }

    #[test]
    fn a_band_lifts_its_own_and_leaves_far_ones_with_the_whole_set_down_by_the_most_lifted() {
        let mut eq = Equalizer::default();
        eq.rate(48_000);
        eq.set(vec![Band {
            hz: 1000.0,
            db: 6.0,
            q: 1.0,
        }]);
        // Set down 6 dB first, so the band at its centre is as recorded and nothing clips.
        assert!(db(gain_at(&mut eq, 1000.0)).abs() < 0.3);
        assert!(
            (db(gain_at(&mut eq, 50.0)) + 6.0).abs() < 0.5,
            "far below: down"
        );
        let mut cut = Equalizer::default();
        cut.rate(48_000);
        cut.set(vec![Band {
            hz: 1000.0,
            db: -6.0,
            q: 1.0,
        }]);
        assert!((db(gain_at(&mut cut, 1000.0)) + 6.0).abs() < 0.3, "a cut");
        assert!(db(gain_at(&mut cut, 50.0)).abs() < 0.3, "and no more");
    }

    #[test]
    fn with_no_bands_or_all_flat_it_does_nothing() {
        for bands in [
            vec![],
            vec![Band {
                hz: 100.0,
                db: 0.0,
                q: 1.0,
            }],
        ] {
            let mut eq = Equalizer::default();
            eq.rate(48_000);
            eq.set(bands);
            let mut samples = vec![0.5, -0.5, 0.25, 0.1];
            eq.run(&mut samples);
            assert_eq!(samples, [0.5, -0.5, 0.25, 0.1]);
        }
    }
}
