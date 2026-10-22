//! Dynamic Color: Now Playing's colour, taken from its album picture. A faint wash of it lies
//! behind the screen, and the progress bar takes it, lightened.

use super::chrome::{Canvas, Draw};
use super::theme::rgba;
use super::MusicTheme;

/// Now Playing's colour, and the last picture's, which it fades from.
#[derive(Copy, Clone, Debug)]
pub struct MusicTint {
    pub now: [u8; 3],
    pub was: Option<[u8; 3]>,
}

impl MusicTint {
    /// The colour `art_in` of the way from the last picture's to this one's.
    fn at(&self, art_in: f32) -> [f32; 3] {
        let now = self.now.map(|c| c as f32 / 255.0);
        match self.was {
            Some(was) => {
                let was = was.map(|c| c as f32 / 255.0);
                [0, 1, 2].map(|i| was[i] + (now[i] - was[i]) * art_in)
            }
            None => now,
        }
    }

    /// The wash across the screen; it comes up with the picture when there was none before.
    pub(super) fn draw_wash(
        &self,
        canvas: Canvas,
        art_in: f32,
        alpha: f32,
        theme: &MusicTheme,
        out: &mut Vec<Draw>,
    ) {
        let [r, g, b] = self.at(art_in);
        let up = if self.was.is_some() { 1.0 } else { art_in };
        out.push(Draw::Rect {
            x: 0.0,
            y: 0.0,
            w: canvas.w,
            h: canvas.h,
            colour: [r, g, b, theme.wash * up * alpha],
        });
    }

    /// The progress bar's colour, drawn towards the ink so it reads on the track; or the ink
    /// when there is no tint.
    pub(super) fn bar_ink(tint: Option<&MusicTint>, art_in: f32, theme: &MusicTheme) -> [f32; 4] {
        let ink = rgba(theme.lit, 1.0);
        match tint {
            Some(t) => {
                let c = t.at(art_in);
                [0, 1, 2, 3].map(|i| match i {
                    3 => 1.0,
                    i => c[i] + (ink[i] - c[i]) * theme.tint_to_lit,
                })
            }
            None => ink,
        }
    }
}

/// Dynamic Color's ink for a small mark: the picture's colour drawn towards the theme's ink,
/// as the progress bar's is.
pub fn music_tint_ink(tint: [u8; 3], theme: &MusicTheme) -> [u8; 3] {
    let to = |i: usize| tint[i] as f32 + (theme.lit[i] as f32 - tint[i] as f32) * theme.tint_to_lit;
    [0, 1, 2].map(|i| to(i).round() as u8)
}

/// A picture's colour at full brightness: its average, with vivid pixels counting most so
/// borders and shadows do not wash it out.
pub fn music_tint(rgba: &[u8]) -> [u8; 3] {
    let (mut sum, mut total) = ([0.0f32; 3], 0.0f32);
    // Every fourth pixel is plenty for an average.
    for p in rgba.chunks_exact(4).step_by(4) {
        let (hi, lo) = (p[0].max(p[1]).max(p[2]), p[0].min(p[1]).min(p[2]));
        let w = 1.0 + f32::from(hi - lo).powi(2) / 64.0;
        for (s, &c) in sum.iter_mut().zip(p) {
            *s += w * f32::from(c);
        }
        total += w;
    }
    let avg = sum.map(|s| s / total.max(1.0));
    let hi = avg.into_iter().fold(1.0, f32::max);
    avg.map(|c| (c * 255.0 / hi).round() as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_gives_its_vivid_colour_at_full_brightness() {
        // Mostly dark grey with a red patch: the red wins.
        let mut rgba = [[40, 40, 40, 255]; 64].concat();
        for px in rgba.chunks_exact_mut(4).take(16) {
            px.copy_from_slice(&[180, 20, 20, 255]);
        }
        let [r, g, b] = music_tint(&rgba);
        assert_eq!(r, 255);
        assert!(g < 120 && b < 120);
        assert_eq!(music_tint(&[]), [0, 0, 0]);
    }
}
